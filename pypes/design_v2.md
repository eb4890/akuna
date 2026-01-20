# Pypes 2.0: The "Browser for Agents" Architecture

## Vision
Pypes 2.0 evolves from a static plumbing tool into a dynamic runtime for **Downloadable Agent Skills**. Just as a web browser safely executes untrusted JavaScript from any server, Pypes 2.0 will safely execute untrusted WASM Components (skills) downloaded from a remote registry.

## Core Architecture: Dynamic Guarded Proxy

The central innovation is the move from compile-time `bindgen!` to runtime Interface loading.

### 1. Dynamic Composition (No Recompile)
Instead of baking types into the `pypes` binary, the runtime will:
1.  **Parse WIT at Runtime**: Use `wit-parser` to read `.wit` files provided by the downloaded skills.
2.  **Synthesize Types**: Dynamically construct `wasmtime::component::Type` definitions in memory.
3.  **Bridge via Values**: Use a generic `Val`-based proxy that validates traffic against these dynamic types.

**Benefit**: A user can add a `finance_skill.wasm` (and its `finance.wit`) to their agent, and Pypes runs it immediately without a new build.

### 2. The Contract Agent Workflow
The User does not write TOML manually.
1.  **Negotiation**: User tells `contract_agent`: "I need an agent to track my stocks and email me summaries."
2.  **Synthesis**: `contract_agent` builds a Blueprint (`agent.toml`) selecting necessary skills (e.g., `remote:stock-market`, `remote:email-sender`).
3.  **Verification**: The blueprint is statically verified for safety (e.g., "Stock Market cannot read Email Inbox directly").
4.  **Execution**: Pypes Runtime reads the TOML, downloads missing skills/WITs, and executes.

### 3. Component Registry
Skills are treated as software packages.
*   **Artifacts**: A skill consists of a `component.wasm`, `interface.wit`, and a `manifest.toml` (permissions).
*   **Protocol**: `pypes install explicit:finance@1.0.0`
*   **Verification**: The runtime verifies cryptographic signatures of skills before loading.

### 3. Policy Middleware (The "Guard")
Since the proxy handles all inter-component traffic, we enforce policies *on the wire*:
*   **Data Budget**: "Orchestrator can send max 1KB/request to LLM."
*   **DLP**: "Block any string matching credit card regex."
*   **Schema Validation**: "Ensure the JSON output matches the requested schema."

## Implementation Roadmap

### Phase 1: The Registry POC
*   [ ] Define a simple remote layout (S3 bucket or HTTP server).
*   [ ] Implement `pypes fetch <skill_url>` to download and verify WASM/WIT.

### Phase 2: Dynamic WIT Loader
*   [ ] Remove `bindgen!` from `main.rs`.
*   [ ] Implement a `WitLoader` struct using `wit-parser`.
*   [ ] Re-implement `func_new_async` using `wasmtime::component::Val` mapped to the loaded WIT types.

### Phase 3: The Guarded Proxy
*   [ ] Implement `Middleware` trait for the linker.
*   [ ] Add `max_payload_size` to the Blueprint TOML.
*   [ ] Enforce limits in the proxy loop.

## Example Flow
1.  **User**: "I want my agent to have stock market access."
2.  **Action**: User adds `skills = ["remote:stock-market"]` to `agent.toml`.
3.  **Pypes**:
    *   Downloads `stock-market.wasm` and `finance.wit`.
    *   Parses `finance.wit` to understand `get-stock-price(symbol: string) -> float`.
    *   Wires `orchestrator` to `stock-market`.
    *   **Enforce**: "Stock market skill cannot access Camera or File System" (Sandbox).

## 4. Multi-Party Contracts
Pypes 2.0 supports scenarios where multiple humans (agents) must agree to a shared contract, such as scheduling a meeting or a joint financial transaction.

1.  **Dual Signatures**: The blueprint must be agreed to by all parties. Each party runs their bit of the contract.
2.  **Atomic Agreement**: The contract defines a "Meeting" capability that requires approval from both Alice's and Bob's calendar agents.
3.  **Execution Model**:
    *   The contract runs in each party's environment with the agreed communication channels.

**Example**:
*   **Goal**: Schedule a lunch.
*   **Contract**: "Allow querying both calendars for free slots. Allow booking ONLY if slot is mutually free and within lunch hours."
*   **Flow**:
    1.  Alice proposes contract C.
    2.  Bob reviews and agrees to C.
    3.  Agent X executes C, querying both calendars and proposing a time. Or agent X and Y execute different parts of C and communicate.
    4.  C permits booking strictly because the logic (verified by hash) allows it.
    5. There will be static checking to make sure that neither party is in a position to lethal trifecta the other.

*  **Future**: This will execute in Trusted Execution Environments (TEE) with attestation to ensure that each party is running the contract.
