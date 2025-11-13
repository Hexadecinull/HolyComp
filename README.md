# HolyComp

HolyComp is an implementation of the HolyC programming language (the language used in TempleOS), focused on faithful language semantics and portability across modern hosts (Linux, macOS, Windows). HolyComp aims to provide both:

- An interpreter (REPL and file-mode) for fast iteration and prototyping.
- A compiler backend (LLVM) for JIT/AOT compilation to native code.

Key design decisions
- Scope: Faithful HolyC language semantics that "run anywhere" on modern OSes. TempleOS binary/runtime compatibility is out of scope for initial phases (can be revisited later).
- Primary language: Rust — used for the frontend (lexer, parser, AST), interpreter (bytecode VM + REPL), and compiler backend.
- Code generation: LLVM (via inkwell) for JIT and AOT compilation.
- Focus areas (initial priority): pointers & memory model, structs/arrays, function calling semantics, then inline asm, stdlib, tooling.

Why Rust + LLVM
- Rust gives memory safety without garbage collection, a strong type system, and excellent cross-platform toolchain via cargo.
- LLVM provides a mature code-generation and optimization pipeline. inkwell binds well with Rust and makes JIT/AOT practical.
- This combo lets us develop a fast interpreter and a production-quality compiler backend, while keeping the codebase maintainable.

Repository layout
- /frontend
  - lexer.rs
  - parser.rs
  - ast.rs
  - types.rs (later)
- /interpreter
  - vm.rs (bytecode VM)
  - runtime.rs (heap, malloc/free hooks)
  - repl.rs
- /compiler
  - codegen.rs (LLVM IR emitter using inkwell)
  - abi.rs (calling convention layer)
- /stdlib
  - implementations of builtins (print, I/O shims, file APIs)
- /tests
  - unit tests for parser, interpreter, codegen
  - a compatibility testset: real HolyC source snippets
- Cargo.toml
- README.md (this file)
- ROADMAP.md

Quick development process (recommended)
1. Write a precise subset spec and test-suite derived from real HolyC examples.
2. Implement lexer + parser + AST in Rust with tests (use pest or nom).
3. Implement an interpreter (tree-walk or bytecode VM) that passes the test-suite.
4. Implement LLVM codegen (JIT first) and ensure parity with interpreter on tests.
5. Add AOT code emission (object files / executables).
6. Implement pointer/memory model, structs, inline asm hooks, stdlib and higher-level features.
7. Add CI (GitHub Actions) to run tests on Linux/macOS/Windows.

Getting started (developer commands)
- Install Rust (stable) and Cargo.
- Create a new cargo workspace:
  cargo new --lib holyc_frontend
  cargo new --bin holyc_interpreter
  cargo new --bin holyc_compiler
- Run tests with:
  cargo test --workspace

License and contribution
- MIT license.
- We will adopt a test-driven approach; please add test cases with any PRs.

