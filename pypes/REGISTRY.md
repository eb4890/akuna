# Registry POC Usage Guide

## Overview
The Registry POC enables Pypes to download WASM components from remote registries at runtime.

## Component Setup

Components are referenced in blueprints using the `remote://` URI scheme:
```toml
[components]
my_skill = "remote://registry.example.com/skill-name@version"
```

## Registry Structure

Each remote skill must be accessible via HTTPS at:
```
https://registry.example.com/skill-name/version/
├── component.wasm
├── manifest.toml
└── interface.wit (optional)
```

## Manifest Format

The `manifest.toml` must include:
```toml
[package]
name = "skill-name"
version = "1.0.0"
registry = "registry.example.com"

[checksums]
component = "sha256:HEXSTRING"

[permissions]
capabilities = ["network", "env"]
```

## Cache Location

Downloaded components are cached in:
```
~/.pypes/cache/
├── registry.example.com/
│   ├── skill-name@1.0.0/
│   │   ├── component.wasm
│   │   └── manifest.toml
```

## Testing

To test with a local registry:
1. Create a directory with `component.wasm` and `manifest.toml`
2. Run a simple HTTP server: `python3 -m http.server 8080`
3. Reference in blueprint: `remote://localhost:8080/test-skill@1.0.0`

## Security

- All components are verified against manifest checksums before execution
- Future: Cryptographic signatures
