# Crate layout

The workspace is a flat `crates/*` virtual workspace (matklad's "Large Rust
Workspaces"), with thin `bindings/{node,wasm}` packages and an `xtask/` build
crate. **Each crate depends only downward** — lower crates know nothing about
the layers above them. The authoritative version is in
[`plans/01-ARCHITECTURE.md`](https://github.com/yanivkalfa/gdscript-analyzer/blob/master/plans/01-ARCHITECTURE.md) §1.

## The layer table

| Crate | Responsibility | Depends on | wasm-safe? |
|---|---|---|---|
| `gdscript-base` | POD types: `FileId`, `TextSize`/`TextRange`, `LineIndex`, position/range conversions, the serde result structs shared with clients. No logic. | — | ✅ |
| `gdscript-syntax` | Lexer (`logos`) + indentation pre-pass + hand-written recursive-descent parser → lossless `cstree` CST + typed AST. Error recovery. | base | ✅ |
| `gdscript-api` | The Godot engine model generated from `extension_api.json` + doc XML: classes, inheritance, methods, properties, signals, enums, singletons, utility functions, builtins — plus the hand-authored GDScript layer (keywords, annotations, builtins) the dump omits. | base | ✅ |
| `gdscript-scene` | `.tscn`/`.tres` text parser → node-tree model for `$Path`/`%Unique` node-path typing. | base | ✅ |
| `gdscript-fmt` | A `gdformat`-compatible formatter: `format`/`format_range`, safe-by-construction (falls back to the original source if it can't prove the reformat is meaning-preserving). | base, syntax | ✅ |
| `gdscript-db` | Input layer: a virtual file system (`FileId` → text, **injected**, never `std::fs`), the project model, `apply_change`. Salsa inputs + tracked queries for incremental recompute. | base, syntax, api | ✅ |
| `gdscript-hir` | Semantic layer: lower AST → HIR, scope tree, name resolution, gradual type inference, the GDScript warning checks (62 codes and counting). | base, syntax, api, db | ✅ |
| `gdscript-ide` | The **feature** layer and **public API**: `AnalysisHost` + immutable `Analysis`, one method per IDE feature, POD results. **The crate external Rust consumers depend on, and the wasm-check target.** | all above | ✅ |
| `gdscript-session` | Internal URI-keyed layer over `gdscript-ide` → `serde_json::Value` results (document lifecycle, JSON serialization). Unit-tested natively; **not** a stable API — consumers depend on `gdscript-ide`. Shared by the napi and wasm bindings so they can't drift apart. | ide | ✅ |
| `gdscript-ffi` | The napi-rs v3 Node binding (`@gdscript-analyzer/core`) — a thin `#[napi]` delegator over `gdscript-session`. `publish = false` (packaged via `bindings/node`). | session | n/a (is the binding) |
| `gdscript-wasm` (`bindings/wasm`) | The wasm-bindgen browser binding (`@gdscript-analyzer/wasm`) — a thin `#[wasm_bindgen]` delegator over `gdscript-session`. | session | n/a (is the binding) |
| `gdscript-lsp` | A real, standalone, spec-compliant LSP server binary. The only place that knows `lsp-types`/JSON-RPC. `publish = false`. | ide | native |
| `gdscript-cli` | `check`/`lint`/`format`/`symbols` for CI/pre-commit. `publish = false`. | ide | native |
| `xtask` | Build automation: `codegen-api`, `fixtures`, `dist`, `release` helpers, the local `ci` gate. | — | native |

## Dependency direction

```text
base ◀── syntax ◀── db ◀── hir ◀── ide ◀── session ◀── ffi ◀── (bindings/node)
  ▲         ▲       ▲       ▲       ▲          └────── wasm ◀── (bindings/wasm)
  ├── api ──┘───────┘───────┘       ├── lsp
  ├── scene ────────────────────────┘
  └── fmt ──────────────────────────┘── cli
```

A crate may only `use` crates to its left. Adding an upward edge is an
architectural change and should be questioned in review.

## Current state

Every crate above is real, shipped code — not a stub. The core crates compile,
lint, test, and pass the wasm portability check with substantial domain logic
behind them (parsing, inference, warnings, formatting, scene typing,
incremental recompute); `gdscript-ffi`/`gdscript-wasm`/`gdscript-lsp`/`gdscript-cli`
are published binaries/packages. See [`CLAUDE.md`](https://github.com/yanivkalfa/gdscript-analyzer/blob/master/CLAUDE.md)
and [`plans/ROADMAP.md`](https://github.com/yanivkalfa/gdscript-analyzer/blob/master/plans/ROADMAP.md) for the current
phase-by-phase status.

## Publishing note

Internal crate names use the `gdscript-` prefix. The public Rust crate is
`gdscript-ide`. The npm scope is `@gdscript-analyzer/*`. Non-library crates
(`gdscript-ffi`, `gdscript-session`, `gdscript-lsp`, `gdscript-cli`, the
bindings) carry `publish = false` — they ship as binaries or npm packages
instead of crates.io crates.
