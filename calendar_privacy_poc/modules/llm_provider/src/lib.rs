use wit_bindgen::generate;

generate!({
    world: "llm-provider-world", // We need to add this world to wit
    path: "../../wit/calendar.wit",
    exports: {
        "local:calendar-privacy/llm-api": Component,
    }
});

struct Component;

impl exports::local::calendar_privacy::llm_api::Guest for Component {
    fn predict_state(context: String) -> exports::local::calendar_privacy::llm_api::UserState {
        // Simple mock logic
        if context.contains("tired") {
            exports::local::calendar_privacy::llm_api::UserState::Tired
        } else {
            exports::local::calendar_privacy::llm_api::UserState::Energetic
        }
    }

    fn completion(prompt: String) -> String {
        // Simple mock logic for Leaky Agent
        if prompt.contains("search") {
             "Search for 'Secret Project'".to_string()
        } else {
             format!("I received: {}", prompt)
        }
    }
}
