# Pypes Developer Experience

## The Orchestrator Problem
**Current State**: The `orchestrator.wasm` component has imperative code that calls calendar, LLM, and search. This creates a **privileged component** with access to multiple capabilities—a security hotspot.

**The Question**: Can the TOML be rich enough to express the coordination logic, eliminating the need for orchestrator components?

**Answer**: Yes, with **Declarative Workflows**.

---

## Declarative Workflows in TOML

Instead of writing a WASM orchestrator, the user defines a **pipeline** in the blueprint:

```toml
[components]
calendar = "components/calendar_reader.wasm"
llm = "components/llm_provider.wasm"
search = "components/web_searcher.wasm"

[workflow]
[[workflow.steps]]
id = "get_schedule"
component = "calendar"
function = "get-free-slots"

[[workflow.steps]]
id = "predict_user_state"
component = "llm"
function = "predict-state"
input = "{{ get_schedule.output | summarize }}"

[[workflow.steps]]
id = "search_restaurants"
component = "search"
function = "search"
input = "restaurants near {{ predict_user_state.output.location }}"
```

### Benefits
1.  **No Privileged Component**: Each component only has access to its declared capabilities.
2.  **Static Analysis**: The entire flow is visible in the TOML—Pypes Analyser can verify data paths.
3.  **User-Friendly**: The `contract_agent` can generate these workflows from natural language.

### Advanced Features
*   **Conditionals**: `if = "{{ step1.output.is_free == true }}"`
*   **Loops**: `foreach = "{{ get_schedule.output }}"`
*   **Error Handling**: `on_error = "fallback_step"`

---

## The `pypes-sdk` CLI

To enable the ecosystem, we provide a frictionless developer experience:

### 1. Scaffold a New Skill
```bash
pypes-sdk new weather-skill
```
Generates:
```
weather-skill/
├── weather.wit          # Interface definition
├── src/lib.rs           # Stub implementation
├── Cargo.toml           # Pre-configured for wasm32-wasip1
├── build.rs             # Automates wit-bindgen + wasm-tools
└── manifest.toml        # Permissions/capabilities
```

### 2. Local Testing
```bash
pypes-sdk test
```
Runs the skill in a local sandbox with mock inputs (e.g., `get-weather("London")` → fake API response).

### 3. Publish to Registry
```bash
pypes-sdk publish --registry https://skills.akuna.com
```
*   Signs the WASM with the developer's key.
*   Uploads `weather.wasm`, `weather.wit`, and `manifest.toml`.
*   Returns a URI: `remote:weather@1.0.0`

---

## Example: "Stock Tracker" Without Orchestrator

### User Request
"Create an agent that checks my stock portfolio and emails me if any stock drops 5%."

### Contract Agent Output (`agent.toml`)
```toml
[components]
stocks = "remote:stock-market@2.1.0"
email = "remote:email-sender@1.0.0"

[workflow]
[[workflow.steps]]
id = "fetch_portfolio"
component = "stocks"
function = "get-portfolio"

[[workflow.steps]]
id = "check_threshold"
component = "stocks"
function = "filter-by-drop"
input = "{{ fetch_portfolio.output }}"
threshold = 0.05

[[workflow.steps]]
id = "send_alert"
component = "email"
function = "send"
condition = "{{ check_threshold.output | length > 0 }}"
to = "user@example.com"
body = "Stocks dropped: {{ check_threshold.output }}"
```

### Pypes Runtime Execution
1.  Downloads `stock-market.wasm` and `email-sender.wasm` (if not cached).
2.  Instantiates components in sandboxes.
3.  Executes workflow steps sequentially, passing outputs via the proxy.
4.  **No orchestrator component exists**—the runtime itself is the orchestrator.

---

## Implementation Roadmap

### Phase 1: pypes-sdk
*   [ ] Implement `new`, `test`, `publish` commands.
*   [ ] Auto-generate `build.rs` for wit-bindgen + wasm-tools.

### Phase 2: Declarative Workflows
*   [ ] Extend TOML schema with `[workflow]` section.
*   [ ] Implement workflow executor in `pypes` runtime.
*   [ ] Add templating engine for `{{ }}` syntax.

### Phase 3: Ecosystem
*   [ ] Host public registry at `skills.akuna.com`.
*   [ ] Build skill browser UI (like npm registry).
