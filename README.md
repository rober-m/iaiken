**`iaiken` (interactive Aiken) is a Jupyter kernel and REPL for the [Aiken](aiken-lang.org) smart contract programming language.**

This project provides two main tools for interactive Aiken development:

1. **iaiken** - A Jupyter kernel that allows running Aiken code in Jupyter notebooks (depends on `aiken-repl`).
2. **aiken-repl** - A standalone REPL (Read-Eval-Print Loop) for interactive Aiken development.

Both tools leverage Aiken's existing compilation infrastructure to provide real-time type checking and code evaluation.

### iaiken (Jupyter Kernel)

The main Jupyter kernel that implements the Jupyter messaging protocol using ZeroMQ sockets. It provides:

#### Features

- [x] **Definition Persistence** - Define functions, constants, and types that persist across evaluations
- [x] **Type Information** - Display both values and their types for rich feedback
- [x] **Rich Error Reporting** - Rich error reporting with source code context

### aiken-repl (REPL Evaluator)

A REPL evaluator library that provides interactive Aiken code execution.

#### Features

- [x] **Interactive Shell** - Standalone REPL with rustyline for line editing
- [x] **Context Management** - View and reset current evaluation context
- [x] **Special Commands** - Built-in commands (`:help`, `:quit`, `:reset`, `:context`)
- [x] **History Support** - Command history with up/down arrows
- [x] **Context Introspection** - View current definitions and context state
- [x] **Redefinition Support** - Redefine functions and constants dynamically

## Installation

### With Nix

- Jupyter Kernel: `nix profile install github:rober-m/iaiken#iaiken`
- REPL: `nix profile install github:rober-m/iaiken#aiken-repl`

### From source 

1. Prerequisites: 
    1. Rust 1.88.0 or later
    1. Jupyter Notebook or Labs (for `iaiken`)
1. Clone the repo: `git clone https://github.com/rober-m/iaiken`
1. `cd` into the repository: `cd iaiken`
1. Install the desired package:
    - Jupyter Kernel: `cargo install --path crates/iaiken`
    - REPL: `cargo install --path crates/aiken-repl`

## Usage

### Jupyter Kernel

1. Install the kernel spec in Jupyter:
```bash
iaiken --install
```

2. Start Jupyter:
```bash
jupyter notebook
# or
jupyter lab
```

3. Create a new notebook and select "Aiken" as the kernel

4. Start writing Aiken code in cells:
```aiken
pub fn fibonacci(n: Int) -> Int {
  if n <= 1 {
    n
  } else {
    fibonacci(n - 1) + fibonacci(n - 2)
  }
}

fibonacci(10)
```

### Standalone REPL

Run the standalone REPL:
```bash
aiken-repl
```

Interactive session example:
```
🎯 Aiken REPL
Evaluate Aiken expressions or definitions. Use :quit to exit and :help to view all commands

λ> 1 + 2
3 : Int

λ> pub const my_number = 42
my_number : Int

λ> my_number * 2
84 : Int

λ> :help
🛟 Aiken REPL Help

Special commands:
  :help, :h       - Show this help
  :quit, :q       - Exit the REPL
  :reset          - Clear all definitions and restart
  :context, :ctx  - Show current context info
...
```

### Uninstalling

Uninstall kernel:
```bash
iaiken --uninstall # Remove the kernel spec

nix profile remove iaiken
# or
cargo uninstall iaiken
```

Uninstall REPL
```bash
nix profile remove aiken-repl
# or
cargo uninstall aiken-repl
```

## Contributing

Contributions are welcome! Please feel free to submit pull requests or open issues for bugs and feature requests.

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.

