use anyhow::{Context, Result, anyhow};
use clap::Parser;
use pypes_analyser::{verify, Blueprint};
use std::fs;
use std::path::PathBuf;
use std::collections::{HashMap, HashSet};
use wasmtime::{Config, Engine, Store, component::{Component, Linker, ResourceTable}};
use wasmtime_wasi::preview2::{WasiCtx, WasiCtxBuilder, WasiView};

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

// Generate types from WIT
wasmtime::component::bindgen!({
    path: "../../calendar_privacy_poc/wit/calendar.wit",
    world: "capabilities",
    async: true
});

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
    
    for (name, rel_path) in &blueprint.components {
        let path = base_dir.join(rel_path);
        println!(" - Loading component '{}' from {:?}", name, path);
        let component = Component::from_file(&engine, &path)
            .with_context(|| format!("Failed to load component {}", name))?;
        components.insert(name.clone(), component);
    }
    
    let mut instances = HashMap::new();
    let component_names: Vec<String> = components.keys().cloned().collect();
    let mut pending = component_names.clone();
    
    let mut wiring_map: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for (consumer_key, provider_key) in &blueprint.wiring {
        let p_parts: Vec<&str> = provider_key.splitn(2, '.').collect();
        if p_parts.len() < 2 { continue; }
        if p_parts[0] == "host" { continue; }
        let provider = p_parts[0].to_string();
        let export = p_parts[1].to_string();

        let c_parts: Vec<&str> = consumer_key.splitn(2, '.').collect();
        let import = if c_parts.len() == 2 { c_parts[1].to_string() } else { consumer_key.clone() };
        
        wiring_map.entry(provider).or_default().push((export, import));
    }
    
    for list in wiring_map.values_mut() {
        list.sort();
        list.dedup();
    }

    let mut made_progress = true;
    while !pending.is_empty() && made_progress {
        made_progress = false;
        let mut next_pending = Vec::new();
        
        for name in &pending {
            let comp = components.get(name).unwrap();
            
            println!("   Trying to instantiate '{}'...", name);
            match linker.instantiate_async(&mut store, comp).await {
                Ok(instance) => {
                    println!("   ✅ Instantiated '{}'", name);
                    instances.insert(name.clone(), instance);
                    made_progress = true;
                    
                    if let Some(wires) = wiring_map.get(name) {
                        for (export_name, linker_name) in wires {
                            
                            // Use Generated Types (at root level)
                            use local::calendar_privacy::calendar_api::{TimeWindow, CalendarEvent};
                            use local::calendar_privacy::search_api::SearchResult;
                            // UserState from LLM API (remapped from Calendar API)
                            use local::calendar_privacy::llm_api::UserState;
                            
                            if export_name.contains("calendar-api") {
                                let get_free_slots = {
                                    let mut exports = instance.exports(&mut store);
                                    let mut api = exports.instance(export_name); 
                                    if let Some(mut a) = api { a.func("get-free-slots") } else { None }
                                };
                                
                                let mut instance_linker = linker.instance(linker_name)?;

                                if let Some(func) = get_free_slots {
                                     println!("      -> Wiring 'get-free-slots' to '{}'", linker_name);
                                     instance_linker.func_wrap_async("get-free-slots", move |mut ctx, ()| {
                                         let func = func;
                                         Box::new(async move {
                                             let res = func.typed::<(), (Vec<TimeWindow>,)>(&ctx).unwrap().call_async(ctx, ()).await?;
                                             Ok((res.0,))
                                         })
                                     })?;
                                }
                                
                                instance_linker.func_wrap("get-events-sensitive", move |_ctx, ()| -> Result<(Vec<CalendarEvent>,)> {
                                     Ok((Vec::new(),))
                                })?;

                            } else if export_name.contains("search-api") {
                                let search_func = {
                                    let mut exports = instance.exports(&mut store);
                                    let mut api = exports.instance(export_name);
                                    if let Some(mut a) = api { a.func("search") } else { None }
                                };

                                let mut instance_linker = linker.instance(linker_name)?;

                                if let Some(func) = search_func {
                                     println!("      -> Wiring 'search' to '{}'", linker_name);
                                     instance_linker.func_wrap_async("search", move |mut ctx, (q,): (String,)| {
                                         let func = func;
                                         Box::new(async move {
                                             let res = func.typed::<(String,), (Vec<SearchResult>,)>(&ctx).unwrap().call_async(ctx, (q,)).await?;
                                             Ok((res.0,))
                                         })
                                     })?;
                                }

                            } else if export_name.contains("llm-api") {
                                let (predict, complete) = {
                                    let mut exports = instance.exports(&mut store);
                                    let mut api = exports.instance(export_name);
                                    if let Some(mut a) = api { (a.func("predict-state"), a.func("completion")) } else { (None, None) }
                                };

                                let mut instance_linker = linker.instance(linker_name)?;

                                if let Some(func) = predict {
                                     println!("      -> Wiring 'predict-state' to '{}'", linker_name);
                                     instance_linker.func_wrap_async("predict-state", move |mut ctx, (c,): (String,)| {
                                         let func = func;
                                         Box::new(async move {
                                             let res = func.typed::<(String,), (UserState,)>(&ctx).unwrap().call_async(ctx, (c,)).await?;
                                             Ok((res.0,))
                                         })
                                     })?;
                                }
                                if let Some(func) = complete {
                                     println!("      -> Wiring 'completion' to '{}'", linker_name);
                                     instance_linker.func_wrap_async("completion", move |mut ctx, (p,): (String,)| {
                                         let func = func;
                                         Box::new(async move {
                                             let res = func.typed::<(String,), (String,)>(&ctx).unwrap().call_async(ctx, (p,)).await?;
                                             Ok((res.0,))
                                         })
                                     })?;
                                }
                            }
                        }
                    }
                },
                Err(e) => {
                    if !made_progress {
                         println!("   ⚠️  Failed to instantiate '{}': {:?}", name, e);
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
