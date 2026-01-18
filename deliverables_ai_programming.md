# Deliverables: AI Agent Programming Use Case

This document outlines the Capability Contract strategy for an **AI Agent that writes software**. This is a high-stakes domain where "Agency" meets "Arbitrary Code Execution."

## 1. The Problem Space

An "AI Software Engineer" agent needs powerful tools to be useful:
*   Reading the codebase.
*   Writing/Editing files.
*   Running commands (builds, tests).
*   Installing dependencies (network access).

**The Risk**:
*   **Destructive Acts**: `rm -rf /` or deleting the wrong directory.
*   **Supply Chain Attacks**: Installing malicious packages.
*   **Exfiltration**: Sending `~/.ssh/id_rsa` or env vars to a remote server.
*   **Resource Hijacking**: Using `shell:exec` to install a crypto miner.

## 2. Design Goal: "Containment of Creativity"

We want the agent to be creatively free to solve coding problems but structurally unable to perform operations outside the "Project Sandbox."

## 3. The Capability Contract (WIT Contracts)

Instead of giving the agent `wasi:filesystem` and `wasi:cli` (root access), we define semantic interfaces.

### Core Capabilities

#### A. `codebase` (The "Safe" Filesystem)
Instead of "File System", we expose "Project System".
*   **Scope**: Strictly jailed to the project root.
*   **Constraints**:
    *   `read(glob: string) -> list<file>`: Can handle `.gitignore`.
    *   `write(path: string, content: string)`: *Versioning aware*. Maybe writing creates a "Patch" or "Staged Change" rather than overwriting disk bytes immediately?
    *   **No Access**: Cannot traverse `../`, cannot see `.git/config` (secrets).

#### B. `shell-runner` (The "Scoped" Terminal)
Instead of `sh` access, we allow "Task Execution".
*   **Allowed**: `make test`, `cargo build`, `npm install`.
*   **Denied**: `curl`, `wget`, `ssh`, arbitrary executable paths.
*   **Output**: Streamed textual output (stdout/stderr).

#### C. `package-manager` (The Protocol Filter)
Instead of `wasi:http` (Open Internet), we proxy dependency resolution.
*   **Allowed Domains**: `crates.io`, `registry.npmjs.org`, `maven.org`.
*   **Rate Limits**: Prevent bulk scraping.

## 4. Proposed WIT Interface (`coding-agent.wit`)

```wit
package local:programming-agent;

interface codebase {
    record file-diff {
        path: string,
        // The agent proposes a "diff" or "replacement", host handles the IO
        new-content: string, 
    }
    
    // Agent can inspect the project structure
    list-files: func(dir: string) -> list<string>;
    read-file: func(path: string) -> string;
    
    // Agent submits changes as a "Plan" or "Batch"
    // The Host can then show the user a Diff View before applying
    submit-changes: func(changes: list<file-diff>) -> result<_, string>;
}

interface shell {
    // Agent requests to run a "safe" command
    // Host validates against a whitelist (e.g. only 'cargo', 'make', 'npm')
    run-task: func(command: string, args: list<string>) -> string;
}

world software-engineer {
    import codebase;
    import shell;
    
    // The agent exports a "Work" function
    export solve-issue: func(issue-description: string) -> string;
}
```

## 5. Security Architecture (The Two-Agent Model)

For high assurance, we can split the "Coding Agent" into two components:

1.  **The Architect (WASM Component A)**:
    *   **Inputs**: Issue description, file tree.
    *   **Outputs**: A "Plan" (List of files to modify).
    *   **Capabilities**: `read-only` access to codebase. `llm-inference`.
    *   **NO** write access. **NO** shell access.

2.  **The Developer (WASM Component B)**:
    *   **Inputs**: The Architect's Plan.
    *   **Capabilities**: `write` access (scoped). `shell` access (to verify tests).
    *   **Constraint**: Cannot change files *not* in the Architect's plan?

## 6. User Experience (CapCon UI)

When you run `dev-agent --fix-bug "Null pointer in main.rs"`, the Capability Contract Review detects the requirements:

```text
[CAPABILITY CONTRACT REVIEW]
Agent 'Junior Dev' requests valid permissions:

  - [ ] codebase:read (Scoped to ./src)
  - [ ] codebase:write (Proposed Patches only)
  - [ ] shell:exec (Whitelisted: 'cargo test')
  
[SAFE] This agent cannot access the internet or delete files outside ./src.
Do you approve? [y/N]
```
