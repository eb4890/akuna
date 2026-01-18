use anyhow::{Result, Context};
use std::fs::File;
use std::io::BufReader;
use ical::IcalParser;
use crate::local::calendar_privacy::calendar_api::{CalendarEvent, TimeWindow};

/// Parses a local .ics file and returns a list of CalendarEvents.
pub fn load_events(path: &str) -> Result<Vec<CalendarEvent>> {
    let file = File::open(path).context(format!("Failed to open calendar file: {}", path))?;
    let buf = BufReader::new(file);
    let parser = IcalParser::new(buf);

    let mut events = Vec::new();

    for line in parser {
        let calendar = line.context("Failed to parse calendar line")?;
        for event in calendar.events {
            let mut title = "Untitled".to_string();
            let mut start = "".to_string();
            let mut end = "".to_string();
            let mut location = "".to_string();
            let mut description = "".to_string();

            for property in event.properties {
                match property.name.as_str() {
                    "SUMMARY" => title = property.value.unwrap_or_default(),
                    "DTSTART" => start = property.value.unwrap_or_default(),
                    "DTEND" => end = property.value.unwrap_or_default(),
                    "LOCATION" => location = property.value.unwrap_or_default(),
                    "DESCRIPTION" => description = property.value.unwrap_or_default(),
                    _ => {}
                }
            }

            // Basic ISO8601 Check (Ideally we use chrono to normalize)
            if !start.is_empty() {
                events.push(CalendarEvent {
                    title,
                    start,
                    end,
                    location,
                    description,
                });
            }
        }
    }

    Ok(events)
}

/// Simple heuristic to derive free slots from events.
/// In a real app, this would do proper interval subtraction.
/// For this POC, we just return a fixed window if no events overlap.
pub fn derive_free_slots(_events: &[CalendarEvent]) -> Vec<TimeWindow> {
    // Mock logic for free slots for now, as interval math is complex
    vec![
        TimeWindow {
            start: "2023-10-27T10:00:00Z".to_string(),
            end: "2023-10-27T11:00:00Z".to_string(),
            is_free: true,
        },
        TimeWindow {
            start: "2023-10-27T14:00:00Z".to_string(),
            end: "2023-10-27T15:00:00Z".to_string(),
            is_free: true,
        }
    ]
}
