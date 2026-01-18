#[cfg(test)]
mod tests {
    use host::{HostState, local};
    use wasmtime::component::{Component, Linker};
    use wasmtime::{Config, Engine, Store};
    use wasmtime_wasi::preview2::command::add_to_linker;

    // Helper to setup engine and linker
    fn setup() -> (Engine, Linker<HostState>, Store<HostState>) {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.async_support(true);
        let engine = Engine::new(&config).unwrap();
        let mut linker = Linker::new(&engine);
        
        // Link Host capabilities
        local::calendar_privacy::calendar_api::add_to_linker(&mut linker, |s: &mut HostState| s).unwrap();
        local::calendar_privacy::search_api::add_to_linker(&mut linker, |s: &mut HostState| s).unwrap();
        local::calendar_privacy::llm_api::add_to_linker(&mut linker, |s: &mut HostState| s).unwrap();
        
        add_to_linker(&mut linker).unwrap();

        let store = Store::new(&engine, HostState::new());
        (engine, linker, store)
    }

    #[tokio::test]
    async fn test_web_searcher_isolation() {
        // Test Strategy:
        // Create a Linker that specifically DOES NOT have Calendar API.
        // If Web Searcher asks for it, instantiation will fail.
        // If it obeys the diode pattern, it will only ask for Search API (and maybe LLM API if defined, but it shouldn't).
        
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.async_support(true);
        let engine = Engine::new(&config).unwrap();
        let mut linker = Linker::new(&engine);
        
        // ONLY link Search Capability. DO NOT link Calendar.
        local::calendar_privacy::search_api::add_to_linker(&mut linker, |s: &mut HostState| s).unwrap();
        
        // Link WASI (Adapter needs this)
        add_to_linker(&mut linker).unwrap();
        
        // Setup state (needed for context, even if unused)
        let mut store = Store::new(&engine, HostState::new());
        
        let component_path = "../components/web_searcher.wasm";
        if !std::path::Path::new(component_path).exists() {
             eprintln!("Skipping test: {} not found.", component_path);
             return;
        }
        let component = Component::from_file(&engine, component_path).unwrap();

        // Attempt instantiation. 
        // If the component declares an import for `calendar-api`, this will fail because it's missing from the linker.
        let instance = linker.instantiate_async(&mut store, &component).await;
        
        match instance {
            Ok(_) => println!("SUCCESS: Web Searcher verified to work without Calendar Access."),
            Err(e) => panic!("SECURITY VIOLATION: Web Searcher failed to load without Calendar API! imports: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_leaky_agent_vulnerability() {
        // Test Strategy:
        // Try to instantiate Leaky Agent with a Restricted Linker (No Calendar).
        // It SHOULD FAIL because it negatively relies on that capability.
        
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.async_support(true);
        let engine = Engine::new(&config).unwrap();
        let mut linker = Linker::new(&engine);
        
        // Only link Search and LLM. OMIT Calendar.
        local::calendar_privacy::search_api::add_to_linker(&mut linker, |s: &mut HostState| s).unwrap();
        local::calendar_privacy::llm_api::add_to_linker(&mut linker, |s: &mut HostState| s).unwrap();
        
        // Link WASI
        add_to_linker(&mut linker).unwrap();
        
        let mut store = Store::new(&engine, HostState::new());
        
        let component_path = "../components/leaky_agent.wasm";
        if !std::path::Path::new(component_path).exists() {
             eprintln!("Skipping test: {} not found.", component_path);
             return;
        }
        let component = Component::from_file(&engine, component_path).unwrap();

        // Attempt instantiation. Must Fail.
        let result = linker.instantiate_async(&mut store, &component).await;
        
        if result.is_ok() {
            panic!("TEST FAILURE: Leaky Agent should rely on Calendar API, but it loaded successfully without it!");
        } else {
            println!("SUCCESS: Leaky Agent correctly failed to load without Calendar API (Verified Dependency).");
        }
    }
}
