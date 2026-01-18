# Capability Contracts: Calendar Privacy POC

A Proof-of-Concept for **Capability Contracts** ("CapCon"), demonstrating how to build safe AI agents using **WebAssembly (WASM)**, **WIT (Wasm Interface Type)**, and the **Component Model**.

## The Problem

We want an AI agent that can:
1.  **Read your Calendar** (to know your schedule).
2.  **Search the Web** (to find events).
3.  **Propose Events** (that fit your schedule).

**The Risk**: A "Leaky Agent" (monolithic design) could be tricked via prompt injection to read your calendar and send the details to a search engine query (e.g. "Search for 'Secret Meeting with CEO'").

**The Solution**: Architectural Isolation.
*   **Calendar Reader**: Sees Calendar. Cannot Search.
*   **Web Searcher**: Sees Search. Cannot Calendar.
*   **Matcher**: Sees pure data from both. Matches them locally.

## Project Structure

*   `host/`: The Rust runtime that orchestrates the components.
    *   Enforces the "Capability Contract" (Security Policy).
    *   Includes a CLI to review permissions before execution.
*   `modules/`: The WASM components.
    *   `calendar_reader`
    *   `web_searcher`
    *   `context_analyzer`
    *   `matcher`
    *   `leaky_agent` (A purposefully insecure implementation for comparison)
*   `wit/`: The Interface Definitions (`.wit`). The "Contract".

## Quick Start

### Prerequisites
*   Rust (`cargo`)
*   `wasm-tools` (Install via `cargo install wasm-tools`)

### Running the Demo

1.  **Build Everything**:
    ```bash
    make components
    ```

2.  **Run Secure Mode** (Safe):
    ```bash
    make run-secure
    ```
    *   Follow the prompt to approve the contract.
    *   Observe that `WebSearcher` searches for generic terms ("events for tired person") without seeing specific calendar details.

3.  **Run Leaky Mode** (Unsafe):
    ```bash
    make run-leak
    ```
    *   Observe that `LeakyAgent` requests *both* `calendar` and `search` capabilities.
    *   The prompt injection succeeds, and it leaks "Secret Project Meeting" to the simulated search engine.

## Documentation

*   [Walkthrough](WALKTHROUGH.md): Detailed step-by-step guide and implementation details.
*   [Design](DESIGN.md): Conceptual background on Capability Contracts.
