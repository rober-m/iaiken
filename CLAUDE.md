# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Jupyter kernel implementation for the Aiken programming language, written in Rust. Aiken is a smart contract language for Cardano that compiles to Untyped Plutus Core. The kernel enables running Aiken code in Jupyter notebooks through the Jupyter messaging protocol.

## Commands

### Build and Development
```bash
# Build the project
cargo build

# Run the kernel (requires connection file from Jupyter)
cargo run -- --connection-file=<path-to-connection-file>

# Install kernel specification
cargo run -- --install

# Uninstall kernel specification  
cargo run -- --uninstall

# Check compilation without building
cargo check

# Run with release optimizations
cargo build --release
```

### Testing
```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture
```

## Architecture

### Core Components

**Main Entry Point (`src/main.rs`)**
- CLI argument parsing using Clap
- Three operation modes: run kernel, install, or uninstall
- Async main function using tokio runtime

**Connection Management (`src/connection.rs`)**
- Handles Jupyter connection file parsing
- Sets up ZeroMQ socket connections (shell, control, iopub, stdin, heartbeat)
- Uses zeromq crate (pure Rust implementation) instead of traditional zmq bindings
- All sockets use async/await pattern with tokio

**Message Protocol (`src/messages/mod.rs`)**
- Implements Jupyter messaging protocol v5.4 structures
- `MessageHeader`: Contains msg_id, session, username, date, msg_type, version
- `JupyerMessage<T>`: Generic message wrapper with header, parent_header, metadata, content
- `ExecuteRequest`: Specific message type for code execution requests
- `ConnectionConfig`: Parses Jupyter connection files with transport, IP, ports, signing key

### ZeroMQ Socket Architecture

The kernel uses 5 different ZeroMQ socket types following Jupyter protocol:
- **Shell Socket (ROUTER)**: Main request/reply channel for execution, introspection, etc.
- **Control Socket (ROUTER)**: High-priority control messages like shutdown
- **IOPub Socket (PUB)**: Broadcast channel for output, execution state, etc.
- **Stdin Socket (ROUTER)**: Handles input requests from kernel to frontend
- **Heartbeat Socket (REP)**: Simple ping/pong to detect kernel health

### Current Implementation Status

The project has basic infrastructure but is incomplete:
- ✅ CLI parsing and connection file handling
- ✅ ZMQ socket setup and binding
- ❌ Message handling loop (TODO in connection.rs:43)
- ❌ Aiken code execution integration
- ❌ Kernel specification installation
- ❌ Complete Jupyter message types

### Key Dependencies

- **tokio**: Async runtime for handling concurrent ZMQ connections
- **zeromq**: Pure Rust ZeroMQ implementation (chosen over zmq crate)
- **serde/serde_json**: JSON serialization for Jupyter protocol messages
- **clap**: Command-line argument parsing with derive macros
- **anyhow**: Error handling and context
- **uuid**: Generate unique message IDs

### Development Notes

- Uses Rust 2024 edition
- All async operations use tokio
- Error handling uses anyhow for ergonomic error context
- Follows Jupyter client messaging specification v5.4
- ZMQ addresses are built dynamically from connection config (tcp://ip:port format)

### Next Implementation Steps

1. Implement message handling loop in `run_kernel()`
2. Add remaining Jupyter message types (kernel_info_request, etc.)
3. Integrate Aiken compiler/interpreter for code execution
4. Implement kernel specification creation for installation
5. Add proper message signing using HMAC-SHA256
6. Handle execution state broadcasting via IOPub socket