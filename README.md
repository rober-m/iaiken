## TODOs

### aiken-repl

- [x] Build repl library using temporary aien-project::Project
- [x] Handle execution history (up/down arrows)
- [x] Provide introspection functionality (ctx, re-definitions)
- [x] Create `aiken-repl` binary
- [x] Remove noise from results
- [ ] Create README

### iaiken

- [x] Create install kernel functionality
- [x] Create basic `kernel_info` communication
- [x] Create `execution` communication
- [x] Track execution_count and emit execute_input + stream/execute_result
- [x] Implement minimal Aiken executor (shell out) and map stdout/stderr to IOPub
- [x] Integrate aiken-repl
- [x] Hanlde execute error path properly (IOPub error + execute_reply error)
- [ ] Implement completions
- [ ] Implement uninstall (remove kernelspec dir)
- [ ] ~~Add syntax highlighting~~ Not part of the kernel's job
- [ ] Add tracing logs + env filter and trim printlns
- [ ] Create README



