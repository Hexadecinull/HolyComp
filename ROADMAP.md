# HolyComp Roadmap

This document tracks the phased development plan for HolyComp — a faithful,
portable HolyC implementation in Rust with an interpreter and LLVM compiler backend.

---

## Phase 0 — Foundation ✅ *Done*

- [x] Workspace layout: `holyc_frontend`, `holyc_interpreter`, `holyc_compiler`, `holyc_stdlib`
- [x] Full manual lexer (all tokens, operators, literals, comments, preprocessor directives)
- [x] Recursive-descent + Pratt expression parser
- [x] Complete AST (`Expr`, `Stmt`, `TopLevel`, `Module`)
- [x] Type system (`HolyType` with `size_of`, signed/float predicates)
- [x] `RuntimeError` + `Value` in `holyc_stdlib` (canonical, shared)
- [x] Tree-walk interpreter: all control flow, arithmetic, function calls
- [x] REPL with rustyline (multi-line buffering, `:help` / `:reset` commands)
- [x] `Print` / `printf`, math builtins (`Sin`, `Cos`, `Sqrt`, `Pow`, `Abs`)
- [x] `printf`-style formatter (`%d %u %x %X %f %g %c %s %%`)
- [x] ABI classification layer (ready for inkwell wiring)
- [x] LLVM codegen scaffold (`CodegenCtx`, `emit_ir`)
- [x] CI: lint + 3-OS test matrix + MSRV + nightly + security audit
- [x] `.gitignore`, `.gitattributes` (LF normalisation, Linguist overrides)
- [x] Compatibility test suite: `hello.HC`, `arithmetic.HC`, `loops.HC`, `switch.HC`, `fibonacci.HC`

---

## Phase 1 — Pointer & Memory Model ✅ *Done*

**Goal:** `*ptr`, `&var`, `arr[i]`, `MAlloc` / `Free` fully working in the interpreter.

- [x] Add a flat heap (`Vec<u8>` + bump allocator) to `Interpreter`
- [x] Implement `MAlloc(size)` → real heap address; `Free(ptr)` → mark free
- [x] Implement `*ptr` dereference and `&var` address-of in `vm.rs`
- [x] Implement `arr[i]` subscript for heap-allocated arrays
- [x] String literals → heap-allocated null-terminated bytes; `%s` reads from heap
- [x] `MemSet(ptr, val, len)` and `MemCpy(dst, src, len)` real implementations
- [x] Add `--heap-size` CLI flag (default: 64 MiB)
- [x] Add `pointer.HC` compat test

---

## Phase 2 — Structs & Member Access ✅ *Done*

**Goal:** `class Foo { I64 x; };  Foo f; f.x = 1;` works end-to-end.

- [x] Struct layout engine: field offsets, padding, `sizeof(ClassName)`
- [x] `TypeEnv` map: `String → StructLayout` threaded through interpreter and parser
- [x] `.field` and `->field` access in `vm.rs` via heap offsets
- [x] Nested structs and struct arrays
- [x] `typedef` resolves through `TypeEnv`
- [x] Add `structs.HC` compat test

---

## Phase 3 — LLVM JIT Backend

**Goal:** `holyc-compile --jit foo.HC` executes at native speed.

- [ ] Add `inkwell` dependency (`llvm17` or current stable)
- [ ] Wire `CodegenCtx::compile` for arithmetic functions (no pointers yet)
- [ ] Implement `compile_expr` for `IntLit`, `FloatLit`, `BinOp`, `Call`
- [ ] Implement `compile_stmt` for `VarDecl`, `Return`, `If`, `While`, `For`
- [ ] Map HolyC types → LLVM types (`I64 → i64`, `F64 → double`, `U0 → void`)
- [ ] Use ABI layer (`abi.rs`) to set calling conventions on generated functions
- [ ] JIT execution via `inkwell::execution_engine::ExecutionEngine`
- [ ] Parity tests: every `tests/compat/*.HC` must produce identical output via both interpreter and JIT

---

## Phase 4 — AOT Compilation

**Goal:** `holyc-compile foo.HC -o foo` produces a native executable.

- [ ] Emit LLVM object files via `TargetMachine::write_to_file`
- [ ] Link with system linker (cc / lld shim)
- [ ] `--emit-llvm` flag: write `.ll` IR to disk
- [ ] `--emit-asm` flag: write `.s` assembly
- [ ] Cross-compilation: `--target x86_64-unknown-linux-gnu` etc.
- [ ] Add `justfile` with `build`, `run`, `compile` recipes

---

## Phase 5 — Inline Assembly

**Goal:** `asm { … }` blocks pass through to LLVM inline asm.

- [ ] Lex and preserve raw asm text through the AST (currently a stub)
- [ ] Map `asm { … }` → `llvm::InlineAsm` via inkwell
- [ ] AT&T syntax only for initial support (mirrors TempleOS x86 asm)
- [ ] Constraint string inference from surrounding variable types
- [ ] Add `asm_test.HC` compat test

---

## Phase 6 — Standard Library Expansion

- [x] `StrCpy`, `StrCmp`, `StrCat`, `StrStr`, `StrToI64`
- [x] `FileOpen`, `FileClose`, `FileRead`, `FileWrite`, `FileSeek` (host OS shims)
- [x] `Time` / `DateStr` (host clock)
- [x] `Rand` / `SRand`
- [x] `MemCmp`
- [x] `SPrint(buf, fmt, …)` — formatted string to buffer
- [x] Document every builtin in `docs/stdlib.md`

---

## Phase 7 — Tooling & Developer Experience

- [ ] `--check` mode: parse + type-check, no execution
- [ ] Structured diagnostics with source spans (line/column, caret underline)
- [ ] `holyc fmt` — auto-formatter
- [ ] VS Code / Neovim tree-sitter grammar for HolyC
- [ ] Language Server Protocol (LSP) stub (hover types, go-to-def)
- [ ] Code coverage via `cargo-tarpaulin`

---

## Phase 8 — TempleOS Compatibility Layer *(stretch goal)*

- [ ] Research TempleOS ring-0 calling convention differences
- [ ] Graphics stub: map `GrPrint`, `GrLine`, etc. to host framebuffer or SDL2
- [ ] Spike: run a simple TempleOS `.HC` application unmodified

---

## Versioning

| Version | Milestone                          |
|---------|------------------------------------|
| 0.1     | Phase 0 complete                   |
| 0.2     | Phase 1 complete (current)         |
| 0.2     | Phase 2 complete                   |
| 0.3     | Phase 3 LLVM JIT complete          |
| 0.4     | Phases 4-7 complete                |
| 0.5     | Tooling: fmt, LSP, tree-sitter (current)|
| 0.3     | Phase 3 — JIT working              |
| 0.4     | Phase 4 — AOT working              |
| 0.5     | Phase 5 + 6                        |
| 1.0     | Phase 7 complete, stable CLI       |
