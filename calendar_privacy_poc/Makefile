# Makefile for Calendar Privacy POC

# Configuration
WASM_TARGET = wasm32-wasip1
BUILD_DIR = target/$(WASM_TARGET)/debug
COMPONENTS_DIR = components
ADAPTER = wasi_snapshot_preview1.reactor.wasm
ADAPTER_URL = https://github.com/bytecodealliance/wasmtime/releases/download/v18.0.0/$(ADAPTER)

# Modules list
MODULES = calendar_reader web_searcher context_analyzer matcher leaky_agent

# Default target
all: build-modules components build-host

# 1. Build WASM Modules
build-modules:
	@echo "Building WASM modules..."
	cargo build --target $(WASM_TARGET) \
		-p calendar_reader \
		-p web_searcher \
		-p context_analyzer \
		-p matcher \
		-p leaky_agent

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
	@# Note: The above tries lib<name>.so first because cargo often outputs cdylib/libraries that way on Linux for wasm32-wasi if crate-type is cdylib. 
	@# If crate-type is bin, it's just <name>.wasm. We assume library/cdylib.

# 4. Build Host
build-host:
	@echo "Building Host..."
	cargo build -p host

# 5. Run Modes
run-secure: all
	@echo "Running Secure Mode..."
	cargo run -p host -- --mode secure

run-leak: all
	@echo "Running Leaky Agent Mode..."
	cargo run -p host -- --mode leak

# 6. Test Suite
test: components
	@echo "Running Security Test Suite..."
	cargo test -p host

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
