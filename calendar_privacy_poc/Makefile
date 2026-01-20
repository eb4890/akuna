# Makefile for Calendar Privacy POC

# Configuration
WASM_TARGET = wasm32-wasip1
BUILD_DIR = target/$(WASM_TARGET)/debug
COMPONENTS_DIR = components
ADAPTER = wasi_snapshot_preview1.reactor.wasm
ADAPTER_URL = https://github.com/bytecodealliance/wasmtime/releases/download/v18.0.0/$(ADAPTER)

# Modules list (orchestrator, context_analyzer, matcher removed - replaced by declarative workflow)
MODULES = calendar_reader web_searcher leaky_agent llm_provider

# Default target
all: build-modules components

# 1. Build WASM Modules
build-modules:
	@echo "Building WASM modules..."
	cargo build --target $(WASM_TARGET) \
		-p calendar_reader \
		-p web_searcher \
		-p leaky_agent \
		-p llm_provider

# 2. Download Adapter
$(ADAPTER):
	@echo "Downloading WASI Adapter..."
	curl -L -O $(ADAPTER_URL)

# 3. Create Components (using wasm-tools)
components: build-modules $(ADAPTER)
	@echo "Creating WASM Components..."
	@mkdir -p $(COMPONENTS_DIR)
	@for mod in $(MODULES); do \
		echo "Componentizing $$mod..."; \
		wasm-tools component new $(BUILD_DIR)/$$mod.wasm -o $(COMPONENTS_DIR)/$$mod.wasm --adapt ./$(ADAPTER) || \
		echo "Warning: Could not componentize $$mod. Ensure wasm-tools is installed."; \
	done

# Run via pypes
run: components
	@echo "Run via Pypes:"
	@echo "  pypes run secure.toml"

# Clean
clean:
	@echo "Cleaning..."
	cargo clean
	rm -rf $(COMPONENTS_DIR)
	rm -f $(ADAPTER)

# Helper to check tools
check-tools:
	@echo "Checking requirements..."
	@which wasm-tools > /dev/null || echo "Error: wasm-tools not found!"
	@which cargo > /dev/null || echo "Error: cargo not found!"

