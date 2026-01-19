use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::Direction;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Blueprint {
    pub components: HashMap<String, String>,
    pub wiring: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ViolationType {
    LethalTrifecta, // Untrusted + Internal + Exfiltration
    DeadlyDuo,      // Untrusted + Destructive
}

#[derive(Debug)]
pub struct SafetyViolation {
    pub component: String,
    pub violation: ViolationType,
    pub details: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Capability {
    UntrustedInput, // User prompt or Web results
    InternalData,   // Calendar, Files
    Exfiltration,   // HTTP, Network
    Destructive,    // Delete, Write
    Proposal,       // Human Verification (Safe)
}

pub fn verify(blueprint: &Blueprint) -> Result<(), Vec<SafetyViolation>> {
    let mut violations = Vec::new();

    // 1. Build Graph
    // Nodes are components (including "host").
    // Edges are dependencies (Consumer -> Provider).
    let mut graph = DiGraph::<&str, ()>::new();
    let mut node_map = HashMap::new();

    // Add components
    for name in blueprint.components.keys() {
        let idx = graph.add_node(name.as_str());
        node_map.insert(name.as_str(), idx);
    }
    // Add host if not present (implicit)
    if !node_map.contains_key("host") {
        let idx = graph.add_node("host");
        node_map.insert("host", idx);
    }

    // Add edges from wiring
    // wiring: "consumer.import" = "provider.export"
    for (consumer_key, provider_key) in &blueprint.wiring {
        let consumer_name = consumer_key.split('.').next().unwrap_or(consumer_key);
        let provider_name = provider_key.split('.').next().unwrap_or(provider_key);

        if let (Some(&c_idx), Some(&p_idx)) = (node_map.get(consumer_name), node_map.get(provider_name)) {
            // Edge: Consumer depends on Provider
            if !graph.contains_edge(c_idx, p_idx) {
                graph.add_edge(c_idx, p_idx, ());
            }
        }
    }

    // 2. Identify Base Capabilities (Leafs) based on interfaces/imports
    // We basically tag the PROVIDER side of a wire.
    // If "host.wasi:http..." is provided, then the Host provides Exfiltration.
    // But in the graph, we just see Consumer -> Host.
    // We need to associate the CAPABILITY with the PROVIDER logic being accessed.
    
    // Better approach:
    // Tag specific 'provider_keys' with capabilities.
    // Map components to the capabilities they *consume*.
    
    let mut component_caps: HashMap<&str, HashSet<Capability>> = HashMap::new();
    
    // Initialize empty sets
    for name in blueprint.components.keys() {
        component_caps.insert(name.as_str(), HashSet::new());
    }
    // Assume the 'main' component (if there is one?) gets UntrustedInput (User Prompt).
    // For now, we'll assume ANY component that acts as a "logic" node might receive user input?
    // Let's refine: The user usually talks to ONE entrypoint. 
    // We'll mark 'host' as safe, but interfaces from host might be dangerous.

    // 3. Analyze Wiring to seed capabilities
    for (consumer_key, provider_key) in &blueprint.wiring {
        let consumer_name = consumer_key.split('.').next().unwrap();
        // provider could be "host" or another component
        // let provider_name = provider_key.split('.').next().unwrap();

        let caps = infer_capabilities(provider_key);
        
        if let Some(set) = component_caps.get_mut(consumer_name) {
            for cap in caps {
                set.insert(cap);
            }
        }
    }

    // 4. Propagate Transitive Capabilities
    // If A depends on B, A gains B's capabilities?
    // YES. If A calls B, and B can Read Calendar, A can effectively Read Calendar (by asking B).
    // (This is a conservative approximation: B might sanitize, but for plumbing safety we assume worst case).
    
    let mut changed = true;
    while changed {
        changed = false;
        // We clone to iterate safely
        let current_caps = component_caps.clone();
        
        for (consumer_name, consumer_caps) in component_caps.iter_mut() {
            if let Some(&c_idx) = node_map.get(consumer_name) {
                // Find all providers for this consumer
                let neighbors = graph.neighbors_directed(c_idx, Direction::Outgoing);
                for p_idx in neighbors {
                    let provider_name = graph[p_idx];
                    if let Some(provider_caps_set) = current_caps.get(provider_name) {
                        for &cap in provider_caps_set {
                            if consumer_caps.insert(cap) {
                                changed = true;
                            }
                        }
                    }
                }
            }
        }
    }
    
    // 5. Check Violations
    for (name, caps) in &component_caps {
        // Assume Entrypoint gets UntrustedInput implicitly? 
        // Or should we rely on explicit wiring?
        // Let's assume if it has Exfiltration + Internal, it's ALREADY bad if we assume User Input is always present or flows freely?
        // Actually, "Lethal Trifecta" requires Untrusted Input.
        // Let's assume "UntrustedInput" comes from:
        // 1. Explicit wiring to a 'User' source (not yet modeled).
        // 2. OR 'Exfiltration' sources (HTTP) usually imply 'Untrusted' return values (search results).
        
        let has_untrusted = caps.contains(&Capability::UntrustedInput);
        let has_internal = caps.contains(&Capability::InternalData);
        let has_exfiltration = caps.contains(&Capability::Exfiltration);
        let has_destructive = caps.contains(&Capability::Destructive);

        // Trifecta
        if has_untrusted && has_internal && has_exfiltration {
             violations.push(SafetyViolation {
                component: name.to_string(),
                violation: ViolationType::LethalTrifecta,
                details: format!("Component '{}' has access to Untrusted Input, Internal Data, and Exfiltration.", name),
            });
        }

        // Deadly Duo
        if has_untrusted && has_destructive {
            violations.push(SafetyViolation {
                component: name.to_string(),
                violation: ViolationType::DeadlyDuo,
                details: format!("Component '{}' has access to Untrusted Input and Destructive Capabilities.", name),
            });
        }
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

fn infer_capabilities(interface: &str) -> Vec<Capability> {
    let mut caps = Vec::new();
    
    // Heuristics based on interface names
    // In a real system, this would be a lookup against a curated registry.
    
    // Exfiltration / Untrusted Source
    if interface.contains("http") || interface.contains("search") || interface.contains("network") {
        caps.push(Capability::Exfiltration);
        caps.push(Capability::UntrustedInput); // Responses are untrusted
    }
    
    // Internal Knowledge
    if (interface.contains("calendar") || interface.contains("filesystem") || interface.contains("read")) && !interface.contains("propose") {
        caps.push(Capability::InternalData);
    }
    
    // Destructive
    // IMPORTANT: 'propose' is NOT destructive because it requires human approval.
    if (interface.contains("delete") || interface.contains("write") || interface.contains("modify")) && !interface.contains("propose") {
        caps.push(Capability::Destructive);
    }
    
    // Proposal (Safe)
    if interface.contains("propose") {
        caps.push(Capability::Proposal);
    }
    
    // Special case: LLM inference (usually compute, but if wired to others...)
    // Treat LLM as benign by default, it just processes data.
    
    caps
}
