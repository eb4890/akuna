use anyhow::Result;
use clap::Parser;
use pypes_analyser::{verify, Blueprint};
use std::collections::HashMap;

#[derive(Parser)]
#[clap(author, version, about)]
struct Args {
    /// The natural language request (e.g., "Find a time for lunch")
    #[clap(short, long)]
    prompt: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    println!("🤖 Agent received request: \"{}\"", args.prompt);

    // 1. Simulate LLM generation
    println!("🤔 Analyzing request and generating capability contract...");
    let blueprint = generate_blueprint_from_prompt(&args.prompt);
    
    // Display the specific proposed contract
    let toml_str = toml::to_string_pretty(&blueprint)?;
    println!("\n📋 Proposed Contract (Blueprint):\n{}", toml_str);

    // 2. Verify
    println!("🛡️  Running Safety Verification (Pypes Analyser)...");
    match verify(&blueprint) {
        Ok(_) => {
            println!("✅ Contract Verified SAFE.");
            println!("🚀 Executing Agent with these capabilities...");
            // Stub execution
        },
        Err(violations) => {
            println!("❌ Contract REJECTED by Safety Policy.");
            for v in violations {
                println!("   ⚠️  [{:?}] {}", v.violation, v.details);
            }
            println!("STOPPING. The agent will not run.");
        }
    }

    Ok(())
}

fn generate_blueprint_from_prompt(prompt: &str) -> Blueprint {
    let lower = prompt.to_lowercase();
    let mut components = HashMap::new();
    let mut wiring = HashMap::new();

    // Default: Always need the Agent core
    components.insert("agent".to_string(), "modules/agent.wasm".to_string());

    // Heuristics (Mock LLM)
    let needs_calendar = lower.contains("calendar") || lower.contains("time") || lower.contains("schedule");
    let needs_search = lower.contains("search") || lower.contains("find") || lower.contains("look for");
    let needs_delete = lower.contains("delete") || lower.contains("remove") || lower.contains("cancel");
    let needs_email = lower.contains("email") || lower.contains("send");

    if needs_calendar {
        components.insert("calendar".to_string(), "modules/calendar.wasm".to_string());
        // Link Agent -> Calendar
        wiring.insert("agent.local:calendar/read".to_string(), "calendar.local:calendar/read".to_string());
        // Link Calendar -> Host FS (Internal Data)
        wiring.insert("calendar.wasi:filesystem/types".to_string(), "host.wasi:filesystem/types".to_string());
    }

    if needs_search {
        components.insert("search".to_string(), "modules/search.wasm".to_string());
        // Link Agent -> Search
        wiring.insert("agent.local:search/query".to_string(), "search.local:search/query".to_string());
        // Link Search -> Host HTTP (Exfiltration / Untrusted)
        wiring.insert("search.wasi:http/outgoing-handler".to_string(), "host.wasi:http/outgoing-handler".to_string());
        
        // Note: In our tagging logic, 'search' export might imply untrusted input to the consumer.
    }

    if needs_delete {
        let is_proposal = lower.contains("propose") || lower.contains("safely");
        
        // If 'calendar' is already added, we just add the wire. If not, add it.
        if !components.contains_key("calendar") {
            components.insert("calendar".to_string(), "modules/calendar.wasm".to_string());
            wiring.insert("calendar.wasi:filesystem/types".to_string(), "host.wasi:filesystem/types".to_string());
        }
        
        if is_proposal {
             // Safe Proposal Pattern: Wires to a proposal interface (mocked)
             wiring.insert("agent.local:calendar/propose_delete".to_string(), "host.local:calendar/propose_delete".to_string());
        } else {
             // Dangerous Direct Delete
             wiring.insert("agent.local:calendar/delete".to_string(), "calendar.local:calendar/delete".to_string());
        }
    }

    if needs_email {
        components.insert("emailer".to_string(), "modules/emailer.wasm".to_string());
        wiring.insert("agent.local:email/send".to_string(), "emailer.local:email/send".to_string());
        wiring.insert("emailer.wasi:http/outgoing-handler".to_string(), "host.wasi:http/outgoing-handler".to_string());
    }

    Blueprint {
        components,
        wiring,
        workflow: None,
    }
}
