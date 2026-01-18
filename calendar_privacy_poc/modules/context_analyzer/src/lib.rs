use wit_bindgen::generate;

generate!({
    world: "context-analyzer-world",
    path: "../../wit/calendar.wit",
    exports: {
        world: Component,
    }
});

struct Component;

// The export name is based on the WIT world export name.
// Since it's a top-level export function `analyze`, unbound functions are usually trait methods on `Guest`.
// But wait, top-level exports in a world map to `Guest` trait.
impl Guest for Component {
    fn analyze(schedule: Vec<local::calendar_privacy::calendar_api::TimeWindow>) -> local::calendar_privacy::calendar_api::UserState {
        use local::calendar_privacy::calendar_api::UserState;
        
        let count = schedule.len();
        let summary = format!("User has {} free slots.", count);
        
        // Imported functions are usually at the root of the generated mod or in packages.
        // `wit-bindgen` 0.16 usually puts imports in namespaces matching the WIT package.
        match local::calendar_privacy::llm_api::predict_state(&summary) {
            UserState::Tired => UserState::Tired,
            UserState::Busy => UserState::Busy,
            UserState::Energetic => UserState::Energetic,
            UserState::Traveling => UserState::Traveling,
            _ => UserState::Unknown,
        }
    }
}


