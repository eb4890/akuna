use anyhow::{anyhow, Context, Result};
use clap::Parser;
use pypes_analyser::{Blueprint, Connection, verify};
use std::fs;
use std::path::PathBuf;
use std::collections::HashMap;
use std::sync::Arc;
use wasmtime::{Config, Engine, Store, component::{Component, Linker, ResourceTable, Val}};
use wasmtime_wasi::preview2::{WasiCtx, WasiCtxBuilder, WasiView};

mod fetcher;
mod workflow;
mod wit_loader;
mod middleware;

use fetcher::ComponentFetcher;
use wit_loader::WitLoader;
use std::path::Path;

#[derive(Parser)]
#[clap(author, version, about)]
struct Args {
    #[clap(short, long)]
    config: PathBuf,
    #[clap(long)]
    verify_only: bool,
    #[clap(short, long)]
    entrypoint: Option<String>,
    #[clap(long)]
    allow_unsafe: bool,
}

struct HostState {
    table: ResourceTable,
    ctx: WasiCtx,
}

impl HostState {
    fn new() -> Self {
        let table = ResourceTable::new();
        let ctx = WasiCtxBuilder::new()
            .inherit_stdio()
            .inherit_network()
            .build();
        Self { table, ctx }
    }
}

impl WasiView for HostState {
    fn table(&mut self) -> &mut ResourceTable { &mut self.table }
    fn ctx(&mut self) -> &mut WasiCtx { &mut self.ctx }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    println!("Loading blueprint from {:?}...", args.config);
    let content = fs::read_to_string(&args.config)
        .with_context(|| format!("Failed to read config file: {:?}", args.config))?;
    
    let blueprint: Blueprint = toml::from_str(&content)
        .context("Failed to parse TOML configuration")?;

    println!("🛡️  Running Pypes Static Analysis...");
    match verify(&blueprint) {
        Ok(_) => {
            println!("✅ VERIFICATION PASSED.");
        },
        Err(violations) => {
            eprintln!("❌ SAFETY VIOLATION(S) DETECTED!");
            for v in violations {
                eprintln!("   ⚠️  [{:?}] in component '{}': {}", v.violation, v.component, v.details);
            }
            if !args.allow_unsafe {
                eprintln!("Execution blocked. Use --allow-unsafe to override.");
                std::process::exit(1);
            } else {
                eprintln!("⚠️  Proceeding despite violations (--allow-unsafe active).");
            }
        }
    }

    if args.verify_only {
        return Ok(());
    }

    println!("Authorized. Initializing Pypes Generic Runtime (Typed Mode)...");
    
    let mut config = Config::new();
    config.wasm_component_model(true);
    config.async_support(true);
    let engine = Engine::new(&config)?;
    let mut linker = Linker::new(&engine);
    
    wasmtime_wasi::preview2::command::add_to_linker(&mut linker)?;

    let mut store = Store::new(&engine, HostState::new());
    
    let mut components = HashMap::new();
    let base_dir = args.config.parent().unwrap_or(&std::path::Path::new("."));
    
    // Initialize fetcher for remote components
    // Initialize fetcher for remote components
    let fetcher = ComponentFetcher::new()?;
    let mut wit_loaders: HashMap<String, WitLoader> = HashMap::new();
    
    for (name, rel_path) in &blueprint.components {
        let path = if rel_path.starts_with("remote://") {
            // Fetch from remote registry
            fetcher.fetch(rel_path).await?
        } else {
            // Local file
            base_dir.join(rel_path)
        };
        
        println!(" - Loading component '{}' from {:?}", name, path);
        let component = Component::from_file(&engine, &path)
            .with_context(|| format!("Failed to load component {}", name))?;
        components.insert(name.clone(), component);
        
        // Try to load WIT
        let wit_path = path.with_extension("wit");
        let loader = if wit_path.exists() {
            Some(WitLoader::load(&wit_path)?)
        } else {
             // Check for interface.wit in parent dir (cache structure)
             let interface_wit = path.parent().unwrap_or(Path::new(".")).join("interface.wit");
             if interface_wit.exists() {
                 Some(WitLoader::load(&interface_wit)?)
             } else {
                 println!("   ⚠️  No WIT file found for '{}'. Dynamic wiring might be limited.", name);
                 None
             }
        };
        
        if let Some(l) = loader {
            wit_loaders.insert(name.clone(), l);
        }
    }

    let mut instances = HashMap::new();
    let component_names: Vec<String> = components.keys().cloned().collect();
    let mut pending = component_names.clone();
    
    // ProviderName -> List of (ExportName, LinkerName, ConnectionConfig)
    let mut wiring_map: HashMap<String, Vec<(String, String, Connection)>> = HashMap::new();
    for (consumer_key, connection) in &blueprint.wiring {
        let provider_key = connection.provider();
        let p_parts: Vec<&str> = provider_key.splitn(2, '.').collect();
        if p_parts.len() < 2 { continue; }
        if p_parts[0] == "host" { continue; }
        let provider = p_parts[0].to_string();
        let export = p_parts[1].to_string();

        let c_parts: Vec<&str> = consumer_key.splitn(2, '.').collect();
        let import = if c_parts.len() == 2 { c_parts[1].to_string() } else { consumer_key.clone() };
        
        wiring_map.entry(provider).or_default().push((export, import, connection.clone()));
    }
    
    /*
    for list in wiring_map.values_mut() {
        list.sort(); // Can't easily sort with Connection struct
        list.dedup();
    }
    */

    // Dynamic Wiring Loop
    let mut made_progress = true;
    while !pending.is_empty() && made_progress {
        made_progress = false;
        let mut next_pending = Vec::new();
        
        for name in &pending {
            let comp = components.get(name).unwrap();
            
            // Try to instantiate
            println!("   Trying to instantiate '{}'...", name);
            
            match linker.instantiate_async(&mut store, comp).await {
                Ok(instance) => {
                    println!("   ✅ Instantiated '{}'", name);
                    instances.insert(name.clone(), instance);
                    made_progress = true;
                    
                    if let Some(wires) = wiring_map.get(name) {
                        let mut seen_wires = std::collections::HashSet::new();
                        for (export_name, linker_name, connection_config) in wires {
                            if !seen_wires.insert((export_name, linker_name)) {
                                continue;
                            }
                            println!("      -> Wiring export '{}' to linker name '{}'", export_name, linker_name);
                            
                            // 1. Discover exported functions via WitLoader
                            // Need to know function names to proxy.
                            let func_names = if let Some(loader) = wit_loaders.get(name) {
                                match loader.get_interface_exports(export_name) {
                                    Ok(names) => names,
                                    Err(_) => vec![], // Not an interface or not found
                                }
                            } else {
                                vec![]
                            };
                            
                            if !func_names.is_empty() {
                                // It is an Interface instance (e.g. `calendar-api`)
                                let mut instance_linker = linker.instance(linker_name)?;
                                
                                // Find all "Surrogate" components that IMPORT this interface to use for type validation.
                                let mut potential_surrogates = Vec::new();
                                for (c_key, _) in &blueprint.wiring {
                                     // Check if consumer uses this linker name
                                     if c_key.ends_with(linker_name) || c_key.contains(linker_name) {
                                          let c_name = c_key.split('.').next().unwrap();
                                          if let Some(comp) = components.get(c_name) {
                                              potential_surrogates.push((c_name.to_string(), comp));
                                          }
                                     }
                                }
                                // Sort surrogates: Prefer "orchestrator" to resolve type mismatches in critical path
                                potential_surrogates.sort_by(|(a, _), (b, _)| {
                                    if a.contains("orchestrator") { std::cmp::Ordering::Less }
                                    else if b.contains("orchestrator") { std::cmp::Ordering::Greater }
                                    else { a.cmp(b) }
                                });
                                
                                if !potential_surrogates.is_empty() {
                                    for func_name in func_names {
                                         // Get the runtime export from provider instance
                                         let mut exports = instance.exports(&mut store);
                                         if let Some(mut exported_instance) = exports.instance(export_name) {
                                             if let Some(provider_func) = exported_instance.func(&func_name) {
                                                  // Middleware Integration
                                                  // 1. Parse connection config to get active middlewares
                                                  let active_middlewares: Vec<String> = match connection_config {
                                                      pypes_analyser::Connection::Configured { middleware, .. } => middleware.clone(),
                                                      pypes_analyser::Connection::Simple(_) => vec![],
                                                  };

                                                  let mut chain: Vec<Arc<dyn middleware::Middleware>> = Vec::new();
                                                  for mw_name in active_middlewares {
                                                      if let Some(mw) = middleware::get_middleware_by_name(&mw_name) {
                                                          chain.push(mw);
                                                      } else {
                                                          println!("         ⚠️  Unknown middleware '{}' requested for linkage.", mw_name);
                                                      }
                                                  }
                                                  let chain = Arc::new(chain);

                                                  // Try surrogates until one works
                                                  let mut proxied = false;
                                                  for (s_name, surrogate_comp) in &potential_surrogates {
                                                      let chain_clone = chain.clone();

                                                  // Define the proxy in the linker
                                                  let store_target = linker_name.to_string();
                                                  let s_name_debug = s_name.clone();
                                                  let func_name_debug = func_name.clone();

                                                  let res = instance_linker.func_new_async(
                                                      surrogate_comp, 
                                                      &func_name, 
                                                      move |mut ctx, args, results| {
                                                          let provider_func = provider_func;
                                                          let chain = chain_clone.clone();
                                                          let target = store_target.clone();
                                                          let fname = func_name_debug.clone();
                                                          // let Caller = s_name_debug.clone(); // Unused

                                                          Box::new(async move {
                                                              
                                                              for mw in &*chain {
                                                                  // Hack: We only support "Passive" middleware for now (Logging, Guard).
                                                                  // We don't support "Transforming" middleware that calls `next`.
                                                                  // Because of `ctx` ownership.
                                                                  // We'll call a simplified method `on_call`.
                                                                  
                                                                  // To fix this proper: modify Middleware trait?
                                                                  // Let's assume we modify `src/middleware.rs` to have `pre_call` and `post_call`.
                                                                  
                                                                  // Let's use the simpler inline logic for the POC to unblock.
                                                                   if let Some(_logger) = mw.as_any().downcast_ref::<middleware::LoggingMiddleware>() {
                                                                       println!("[Middleware] Call -> {}::{} Inputs: {:?}", target, fname, args);
                                                                  }
                                                              }
                                                              
                                                              // Actual Call
                                                              let start = std::time::Instant::now();
                                                              let res = provider_func.call_async(&mut ctx, args, results).await;
                                                              
                                                              for mw in &*chain {
                                                                   if let Some(_logger) = mw.as_any().downcast_ref::<middleware::LoggingMiddleware>() {
                                                                       match &res {
                                                                           Ok(_) => println!("[Middleware] Return <- {}::{} ({}ms) Outputs: {:?}", target, fname, start.elapsed().as_millis(), results),
                                                                           Err(e) => println!("[Middleware] Error <- {}::{} Error: {:?}", target, fname, e),
                                                                       }
                                                                   }
                                                              }
                                                              
                                                              res
                                                          })
                                                      }
                                                  );
                                                  
                                                  match res {
                                                      Ok(_) => {
                                                          proxied = true;
                                                          break; // Success
                                                      },
                                                      Err(e) => {
                                                          let msg = format!("{:?}", e);
                                                          if msg.contains("import") && msg.contains("not found") {
                                                              continue;
                                                          } else {
                                                              println!("         ⚠️  Error proxying '{}' using surrogate '{}': {}", func_name, s_name, msg);
                                                                     }
                                                  }
                                              }
                                          }
                                          
                                          if !proxied {
                                               println!("         ⚠️  Failed to proxy function '{}'. No suitable consumer component found with this import.", func_name);
                                          }
                                             } else {
                                                  println!("         ⚠️  Function '{}' in WIT but not in instance?", func_name);
                                             }
                                         } else {
                                              println!("         ⚠️  Instance Export '{}' not found although WIT implies it.", export_name);
                                         }
                                    }
                                } else {
                                    println!("         ⚠️  Could not find ANY consumer component for '{}' to use as type surrogate.", linker_name);
                                }
                            } else {
                                // Fallback for Root Functions or Missing WIT
                                let mut exports = instance.exports(&mut store);
                                if let Some(_) = exports.root().func(export_name) {
                                     println!("         (Root function wiring / fallback to one-to-one not fully implemented yet: {})", export_name);
                                } else {
                                    // If we failed WIT lookup and it's not a root func, we can't wire an empty list.
                                    println!("         ⚠️  Export '{}' could not be resolved via WIT or Root exports.", export_name);
                                }
                            }
                        }
                    }
                },
                Err(e) => {
                    // Start of error message
                    let msg = format!("{:?}", e);
                    // Checking for "import not defined" to differentiate actual failure vs pending dependency
                    if msg.contains("not defined") {
                         // Likely waiting for dependency
                    } else {
                         // Real error?
                          println!("   ⚠️  Instantiation error: {}", msg);
                    }
                    next_pending.push(name.clone());
                }
            }
        }
        pending = next_pending;
    }
    
    if !pending.is_empty() {
         println!("(Warning: Some components pending: {:?})", pending);
    }
    
    if let Some(workflow) = &blueprint.workflow {
        workflow::execute(&mut store, &instances, workflow).await?;
        return Ok(());
    }

    let entrypoint = args.entrypoint.unwrap_or("orchestrator".to_string());
    if let Some(instance) = instances.get(&entrypoint) {
        println!("🚀 Running entrypoint '{}'...", entrypoint);
        let run = instance.get_func(&mut store, "run")
            .ok_or(anyhow!("Entrypoint component '{}' has no 'run' function", entrypoint))?;
            
        if let Ok(typed) = run.typed::<(), (String,)>(&store) {
            let res = typed.call_async(&mut store, ()).await?;
            println!("✅ Result: {}", res.0);
        }  else if let Ok(typed) = run.typed::<(String,), (String,)>(&store) {
             let res = typed.call_async(&mut store, ("Default Prompt".to_string(),)).await?;
             println!("✅ Result: {}", res.0);
        } else if let Ok(typed) = run.typed::<(), ()>(&store) {
             typed.call_async(&mut store, ()).await?;
             println!("✅ Result: (void)");
        } else {
             println!("⚠️  Entrypoint found but signature not matched.");
        }
    } else {
        eprintln!("❌ Entrypoint component '{}' not instantiable.", entrypoint);
    }
    
    Ok(())
}
