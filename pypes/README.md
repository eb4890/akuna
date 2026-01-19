# Pypes: Safe WASM Plumbing

**Pypes** is a runtime and static analysis tool for wiring together WebAssembly (WASM) components. It allows you to define complex agent architectures using a declarative TOML configuration and enforces strict safety policies *before* execution.

## Features

*   **Declarative Plumbing**: Define your component architecture in a simple `pypes.toml` file.
*   **Static Safety Analysis**: automatically detects and blocks dangerous capability combinations before the agent runs.
    *   **Lethal Trifecta Detection**: Prevents a single component from having `Untrusted Input` + `Internal Data` + `Exfiltration` capabilities.
    *   **Deadly Duo Detection**: Prevents a single component from having `Untrusted Input` + `Destructive` capabilities.
*   **Implicit Data Diodes**: Capabilities are only granted if explicitly wired. If you don't wire `search -> file_writer`, that path effectively doesn't exist.

## Installation

Prerequisites: Rust (cargo).

```bash
cd pypes
cargo build --release
# Binary is at ./target/release/pypes
```

## Usage

Run a blueprint:

```bash
./pypes --config my_agent.toml
```

Verify a blueprint without running it (useful for CI/CD or "Manifest Review"):

```bash
./pypes --config my_agent.toml --verify-only
```


## AI Agent Mode (Contract Generator)

`contract_agent` simulates an offline AI that takes a natural language request, generates a capability contract (Blueprint), and verifies it before execution.

```bash
# Safe: Read Calendar
./target/debug/contract_agent --prompt "Find a time for lunch"

# Unsafe: Search Web + Delete Calendar (Deadly Duo)
./target/debug/contract_agent --prompt "Search for spam and delete it from calendar"
# Result: ❌ Contract REJECTED by Safety Policy.

# Safe: Search Web + Propose Deletion (Proposal Pattern)
./target/debug/contract_agent --prompt "Search for spam and safely delete it"
# Result: ✅ Contract Verified SAFE.
```



Blueprint files use TOML. They have two sections: `[components]` and `[wiring]`.

### Example: Safe Calendar Agent

```toml
[components]
# Define your WASM modules
agent = "modules/agent.wasm"
calendar = "modules/calendar_reader.wasm"

[wiring]
# Connect imports to exports.
# Format: "Consumer.ImportInterface" = "Provider.ExportInterface"

# 1. The Agent can read the calendar (Safe internal access)
"agent.local:calendar/read" = "calendar.local:calendar/read"

# 2. The Calendar component needs actual filesystem access (provided by Host)
"calendar.wasi:filesystem/types" = "host.wasi:filesystem/types"

# Note: The agent is NOT wired to "host.wasi:http", so it cannot exfiltrate data.
```

## Safety Concepts

### Lethal Trifecta
A vulnerability pattern where an attacker can steal sensitive data via prompt injection.
*   **Conditions**: A component has:
    1.  **Untrusted Input** (e.g., Search results, Emails)
    2.  **Internal knowledge** (e.g., Calendar, Files)
    3.  **Exfiltration** (e.g., Network access)
*   **Pypes Action**: If this combination is detected in the dependency graph, `pypes` **rejects** the blueprint.

### Deadly Duo
A vulnerability pattern where an attacker can cause irreversible damage.
*   **Conditions**: A component has:
    1.  **Untrusted Input**
    2.  **Destructive Capability** (e.g., Delete File, Send Email)
*   **Pypes Action**: **Rejects** the blueprint.

## Project Structure

*   `pypes/`: The CLI runner (Host).
*   `pypes_analyser/`: The core library performing graph analysis and policy verification.
