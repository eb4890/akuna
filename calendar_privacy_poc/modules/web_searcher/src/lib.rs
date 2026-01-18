use wit_bindgen::generate;

generate!({
    world: "web-searcher-world",
    path: "../../wit/calendar.wit",
    exports: {
        "local:calendar-privacy/search-api": Component,
    }
});

struct Component;

impl exports::local::calendar_privacy::search_api::Guest for Component {
    fn search(query: String) -> Vec<exports::local::calendar_privacy::search_api::SearchResult> {
        vec![
            exports::local::calendar_privacy::search_api::SearchResult {
                title: format!("Result for {}", query),
                url: "https://example.com/result".to_string(),
                snippet: "Some generic event".to_string(),
            }
        ]
    }
}


