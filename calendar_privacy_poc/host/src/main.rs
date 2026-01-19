use anyhow::{Context, Result};
use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store, StoreContextMut};
use wasmtime_wasi::preview2::{WasiCtx, WasiCtxBuilder, WasiView};
use clap::Parser;
use pypes_analyser::{verify, Blueprint, SafetyViolation};
use std::fs;
use std::collections::HashMap;

// Import from the local lib
use host::{HostState, local};
use host::local::calendar_privacy::calendar_api::Host as CalendarHost;
use host::local::calendar_privacy::search_api::Host as SearchHost;
use host::local::calendar_privacy::llm_api::Host as LlmHost;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long, default_value = "secure")]
    mode: String, // "secure" or "leak"
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    println!("Running in mode: {}", args.mode);

    // 1. SELECT BLUEPRINT
    let config_path = if args.mode == "leak" {
        "leaky.toml"
    } else {
        "secure.toml"
    };
    
    // 2. LOAD & PARSE
    println!("Loading blueprint from '{}'...", config_path);
    let content = fs::read_to_string(config_path)
        .with_context(|| format!("Failed to read config file: {}", config_path))?;
    let blueprint: Blueprint = toml::from_str(&content)
        .context("Failed to parse TOML configuration")?;

    // 3. STATIC ANALYSIS (PYPES)
    println!("🛡️  Running Pypes Static Analysis...");
    match verify(&blueprint) {
        Ok(_) => {
            println!("✅ Architecture VERIFIED SAFE.");
        },
        Err(violations) => {
             println!("❌ SAFETY VIOLATION DETECTED!");
             for v in violations {
                 println!("   ⚠️  [{:?}] in component '{}': {}", v.violation, v.component, v.details);
             }
             println!("⛔ EXECUTION BLOCKED due to safety policy.");
             return Ok(());
        }
    }
    
    // 4. RUNTIME EXECUTION
    println!("Authorized. Initializing Pypes Runtime...");

    let mut config = Config::new();
    config.wasm_component_model(true);
    config.async_support(true);
    let engine = Engine::new(&config)?;
    let mut linker = Linker::new(&engine);

    // Link HOST capabilities to the runtime (Filesystem, HTTP, etc.)
    wasmtime_wasi::preview2::command::add_to_linker(&mut linker)?;
    
    // Register LLM API (Common to both, provided by Host for this POC)
    local::calendar_privacy::llm_api::add_to_linker(&mut linker, |s: &mut HostState| s)?;

    let mut store = Store::new(&engine, HostState::new());

    if args.mode == "leak" {
        println!("Loading Leaky Agent...");
        // For leaky mode, we link the Host APIs directly because the leaky agent imports them from Host
        local::calendar_privacy::calendar_api::add_to_linker(&mut linker, |s: &mut HostState| s)?;
        local::calendar_privacy::search_api::add_to_linker(&mut linker, |s: &mut HostState| s)?;

         // Simulation of leaky agent logic (Verification failed anyway)
         println!("(Leaky agent logic here - but blocked by verification)");
    } else {
        println!("Loading Secure Components...");
        
        // Step A: Load & Instantiate Providers
        
        // 1. Calendar Reader
        println!(" [Pypes] Instantiating Calendar Reader...");
        let calendar_reader_comp = Component::from_file(&engine, "components/calendar_reader.wasm")?;
        let calendar_reader_instance = linker.instantiate_async(&mut store, &calendar_reader_comp).await?;
        
        // Glue: Extract functions manually
        let mut calendar_funcs = HashMap::new();
        if let Some(wasmtime::component::Extern::Instance(inst)) = calendar_reader_instance.get_export(&mut store, None, "local:calendar-privacy/calendar-api") {
             if let Some(wasmtime::component::Extern::Func(func)) = inst.get_export(&mut store, "get-free-slots") {
                 calendar_funcs.insert("get-free-slots", func);
             } else {
                 return Err(anyhow::anyhow!("Missing get-free-slots func"));
             }
        } else {
             panic!("calendar-api is not an instance");
        }
        
        let get_free_slots = *calendar_funcs.get("get-free-slots").unwrap();
        
        // Wire to Linker (Trampoline)
        use local::calendar_privacy::calendar_api::TimeWindow;
        linker.instance("local:calendar-privacy/calendar-api")?
              .func_wrap("get-free-slots", move |mut ctx: StoreContextMut<HostState>, ()| {
                  let res = get_free_slots.typed::<(), (Vec<TimeWindow>,)>(&ctx).unwrap().call_async(ctx, ()).await?;
                  Ok(res.0)
              })?
              .func_wrap("get-events-sensitive", |_, ()| -> Result<Vec<local::calendar_privacy::calendar_api::CalendarEvent>> { Ok(vec![]) })?;


        // 2. Web Searcher
         println!(" [Pypes] Instantiating Web Searcher...");
        let web_searcher_comp = Component::from_file(&engine, "components/web_searcher.wasm")?;
        let web_searcher_instance = linker.instantiate_async(&mut store, &web_searcher_comp).await?;
        
        // Glue: Extract search
        let mut search_funcs = HashMap::new();
        if let Some(wasmtime::component::Extern::Instance(inst)) = web_searcher_instance.get_export(&mut store, None, "local:calendar-privacy/search-api") {
             if let Some(wasmtime::component::Extern::Func(func)) = inst.get_export(&mut store, "search") {
                 search_funcs.insert("search", func);
             } else {
                 return Err(anyhow::anyhow!("Missing search func"));
             }
        }
        let search_func = *search_funcs.get("search").unwrap();
        
        use local::calendar_privacy::search_api::SearchResult;
        linker.instance("local:calendar-privacy/search-api")?
              .func_wrap("search", move |mut ctx: StoreContextMut<HostState>, (query,): (String,)| {
                  let res = search_func.typed::<(String,), (Vec<SearchResult>,)>(&ctx).unwrap().call_async(ctx, (query,)).await?;
                  Ok(res.0)
              })?;

        // 3. Orchestrator
        println!(" [Pypes] Instantiating Orchestrator...");
        let orchestrator_comp = Component::from_file(&engine, "components/orchestrator.wasm")?;
        let orchestrator_instance = linker.instantiate_async(&mut store, &orchestrator_comp).await?;
        
        let run_func = orchestrator_instance.get_func(&mut store, "run")
            .ok_or(anyhow::anyhow!("Orchestrator missing 'run' export"))?;
            
        println!("🚀 Pypes: Executing Orchestrator...");
        let results = run_func.typed::<(), (String,)>(&store)?.call_async(&mut store, ()).await?;
        
        println!("✅ Orchestrator Result: {}", results.0);
    }

    Ok(())
}
