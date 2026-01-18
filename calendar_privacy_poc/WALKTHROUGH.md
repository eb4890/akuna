# Calendar Privacy POC Walkthrough

This Proof-of-Concept demonstrates a secure architecture for an AI agent that needs to access both a private calendar and the public web, without leaking private data.

## Architecture

We use **WebAssembly (WASM)** Component Model to enforce strict isolation.

### Components

1.  **Host (Rust)**: The trusted runtime. It orchestrates user data and capabilities.
2.  **Calendar Reader (WASM)**: Has access to the `calendar` capability. It reads the schedule and outputs sanitized "free slots".
3.  **Context Analyzer (WASM)**: Has access to `llm-inference`. It determines if the user is `Tired`, `Busy`, `Energetic` (Enum) based on the schedule.
4.  **Web Searcher (WASM)**: Has access to `web-search`. It searches for events based on potential *state*, but never sees the calendar.
5.  **Matcher (WASM)**: Pure logic. Combines `TimeWindow` and `SearchResult` records.

### The "Leaky Agent"
We also allow running a "Leaky Agent" (a monolithic component) to demonstrate how easily PII leaks without these boundaries.

## Running the POC

**Prerequisites**:
*   Rust (`rustc`, `cargo`)
*   Wait for `wasm-tools` installation

**Steps**:

1.  **Build and Componentize**:
    ```bash
    cd src/akuna/calendar_privacy_poc
    make components
    ```
    This command compiles all Rust modules to WASM, downloads the WASI adapter, and componentizes them using `wasm-tools`.

2.  **Run Secure Mode**:
    ```bash
    make run-secure
    ```
    *   **Review**: The CLI will show a manifest of components. Verify that they have NO dangerous capabilities.
    *   **Approve**: Type `y`.
    *   **Result**: The system runs safely. `web_searcher` searches for "events for UserState::Tired person" without seeing your calendar.

3.  **Run Leaky Mode**:
    ```bash
    make run-leak
    ```
    *   **Review**: The CLI will show that the `Leaky Agent` imports `calendar-api`, `search-api`, AND `llm-api`.
    *   **Approve**: Type `y`.
    *   **Result**: The agent is tricked by a prompt injection ("Ignore previous instructions...") and leaks your meeting details to the search query.

4.  **Run Security Tests**:
    ```bash
    make test
    ```
    This runs the automated integration tests which assert:
    *   `web_searcher` *cannot* even be instantiated if the `calendar-api` is not provided (Architectural Isolation).
    *   `leaky_agent` *requires* the `calendar-api` to run.

## Verification Results (Projected)

*   **Secure Mode**: The `WebSearcher` receives queries like "events for tired person". No dates or meeting titles appear in the mock search logs.
*   **Leak Mode**: The `LeakyAgent` sends a query like "Ignore instructions... Secret Project Meeting" to the search engine, demonstrating the vulnerability.
