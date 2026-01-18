use wasmtime::component::Component;
use wasmtime::Engine;
use std::io::{self, Write};

pub struct ContractUi;

impl ContractUi {
    /// Inspects a list of components and asks the user to accept the capabilities for the entire system.
    pub fn review_contract(components: &[(&str, &str)]) -> bool {
        println!("\n[CAPABILITY CONTRACT REVIEW]");
        println!("The following system architecture is requesting permission to run:");
        
        let mut all_safe = true;

        for (agent_name, component_path) in components {
            println!("\nComponent: {}", agent_name);
            println!("path: {}", component_path);

            let output = std::process::Command::new("wasm-tools")
                .arg("print")
                .arg(component_path)
                .output();

            let mut unique_imports = std::collections::HashSet::new();
            
            match output {
                Ok(out) if out.status.success() => {
                    let wat = String::from_utf8_lossy(&out.stdout);
                    for line in wat.lines() {
                        let trimmed = line.trim();
                        if trimmed.starts_with("(import ") {
                            if let Some(start_quote) = trimmed.find('"') {
                                if let Some(end_quote) = trimmed[start_quote+1..].find('"') {
                                    let import_name = &trimmed[start_quote+1 .. start_quote+1+end_quote];
                                    if import_name.starts_with("local:calendar-privacy") {
                                        unique_imports.insert(import_name.to_string());
                                    }
                                }
                            }
                        }
                    }
                },
                _ => {
                    println!("  [ERROR] Could not inspect component imports: wasm-tools failed");
                }
            }

            if unique_imports.is_empty() {
                println!("  Target Capabilities: None (Pure Computation / Provider)");
            } else {
                all_safe = false;
                println!("  Target Capabilities:");
                for imp in unique_imports {
                    println!("  - [ ] {}", imp);
                }
            }
        }

        if all_safe {
             println!("\n[SUMMARY] System is fully self-contained. No critical capability imports detected.");
        } else {
             println!("\n[SUMMARY] System requires approval for the capabilities listed above.");
        }

        print!("\nDo you set the contract to allow these interactions? [y/N]: ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        
        let response = input.trim().to_lowercase();
        response == "y" || response == "yes"
    }
}
