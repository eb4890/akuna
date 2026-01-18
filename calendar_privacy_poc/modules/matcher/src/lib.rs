use wit_bindgen::generate;

generate!({
    world: "matcher-world",
    path: "../../wit/calendar.wit",
    exports: {
        world: Component,
    }
});

struct Component;

impl Guest for Component {
    fn reconcile(
        slots: Vec<local::calendar_privacy::calendar_api::TimeWindow>, 
        events: Vec<local::calendar_privacy::search_api::SearchResult>
    ) -> Vec<local::calendar_privacy::search_api::SearchResult> {
        if slots.is_empty() {
            return vec![];
        }
        events
    }
}


