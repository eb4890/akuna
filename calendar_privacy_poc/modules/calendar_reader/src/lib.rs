use wit_bindgen::generate;

generate!({
    world: "calendar-reader-world",
    path: "../../wit/calendar.wit",
    exports: {
        "local:calendar-privacy/calendar-api": Component,
    }
});

struct Component;

// When exporting an interface (not a top-level func), the trait is usually inside `exports::...::InterfaceName`.
impl exports::local::calendar_privacy::calendar_api::Guest for Component {
    fn get_free_slots() -> Vec<exports::local::calendar_privacy::calendar_api::TimeWindow> {
        vec![
            exports::local::calendar_privacy::calendar_api::TimeWindow {
                start: "2023-11-01T09:00:00Z".to_string(),
                end: "2023-11-01T10:00:00Z".to_string(),
                is_free: true,
            }
        ]
    }
    
    fn get_events_sensitive() -> Vec<exports::local::calendar_privacy::calendar_api::CalendarEvent> {
         vec![]
    }
}


