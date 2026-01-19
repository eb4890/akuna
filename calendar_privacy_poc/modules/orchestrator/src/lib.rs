use wit_bindgen::generate;

generate!({
    world: "orchestrator-world",
    path: "../../wit/calendar.wit",
    exports: {
        world: Component,
    }
});

struct Component;

impl Guest for Component {
    fn run() -> String {
        use local::calendar_privacy::{calendar_api, llm_api, search_api};

        // 1. Calendar Reader
        let slots = calendar_api::get_free_slots();
        let slot_count = slots.len();

        // 2. Context Analyzer
        // Note: passing string because llm_api expects string context
        let state = llm_api::predict_state("14:00 context"); 
        
        // 3. Web Searcher
        let query = format!("events for {:?} person", state);
        let results = search_api::search(&query);
        let result_count = results.len();

        format!("Orchestrator Finished: Found {} slots, User is {:?}, Found {} events.", slot_count, state, result_count)
    }
}
