# gdscript-analyzer

**gdscript-analyzer is a Rust library that parses and semantically understands
GDScript (Godot 4.x)** and exposes an engine-independent query API —
completion, hover, diagnostics, go-to-definition, find-references, rename, type
inference, and more — that any tool can embed: natively, in Node via
[napi-rs](https://napi.rs), in the browser via WebAssembly, or from other
languages through a C ABI.

Think of it as **"Roslyn / rust-analyzer for Godot"**: the reusable *analysis
brain*, deliberately separated from any one editor or server.

## A library, not a server

The single most important design decision is that this is a **library, not an
LSP server**. The analysis engine is protocol-neutral: you give it file
contents and byte offsets, and it returns plain old data (POD) structs. It knows
nothing about LSP, JSON-RPC, or any particular editor.

*Clients* — a standalone LSP server, a CLI, a web playground, or a markup
toolchain such as guitkx — each map those neutral results onto their own
protocol. This is rust-analyzer's discipline: the `ide` crate "knows nothing
about LSP"; only a thin server crate does. See
[ADR-0001](adr/0001-rust-library-not-server.md) and
[`plans/00-VISION-AND-SCOPE.md`](https://github.com/yanivkalfa/gdscript-analyzer/blob/master/plans/00-VISION-AND-SCOPE.md).

## Who consumes it

In rough priority order:

1. **guitkx** — the ReactiveUI-for-Godot markup toolchain, our first client and
   validation harness. It needs GDScript intelligence inside markup `{expr}`
   blocks *without a running Godot editor*.
2. **A standalone GDScript LSP server** (`gdscript-lsp`) — both a real product
   and the reference client for the API.
3. **A CLI** (`gdscript-cli`) — `check` / `lint` / `format` / `symbols` for CI
   and pre-commit hooks.
4. **A web playground** — Rust→WASM, in-browser analysis of pasted GDScript.
5. **The wider community** — other editors and other-language consumers.

## Why it exists

Before this project, the only way to get *semantic* GDScript intelligence was
to run the Godot editor and talk to its built-in LSP over TCP. Every other tool
in the space was either syntactic-only, Python/.NET-locked, or editor-bound.
The empty quadrant — **semantic-grade + engine-independent + Rust→multi-target
+ library-first** — is what this project fills. The original landscape
analysis is in [`plans/00-VISION-AND-SCOPE.md`](https://github.com/yanivkalfa/gdscript-analyzer/blob/master/plans/00-VISION-AND-SCOPE.md).

## Project status — feature-complete, published on the 0.x line

The analyzer is **feature-complete through its planned phases** and ships on
both registries (crates.io workspace + `@gdscript-analyzer/core` on npm, moving
in lockstep — 0.6.0 at the time of writing). Live today:

- **Diagnostics** — parse + type errors and the gateable GDScript warning
  catalog, with **Godot's own verbatim message texts** (probed against the real
  4.7 binary and golden-pinned), arity/argument checking on resolved calls,
  project-aware `UNDEFINED_*` absence diagnostics, LSP rendering tags
  (unused/unreachable code dims), and warning gating that matches the engine's
  defaults (see the [Warning Reference](reference/warnings.md)).
- **The full LSP-grade query surface** — type-aware hover, completion,
  document/workspace symbols, go-to-definition, find-references, rename,
  signature help, folding, inlay hints, code actions, semantic tokens.
- **The Godot 4.x engine model** (generated from `extension_api.json`, with an
  auto-sync pipeline for new engine releases), scene-aware node-path typing,
  and a `gdformat`-compatible formatter.
- **Every consumer from the list above** — the guitkx toolchain embeds it in
  production, `gdscript-lsp` and `gdscript-cli` ship as binaries, and the wasm
  package runs it in the browser.

Remaining work is integration-driven; see
[`plans/ROADMAP.md`](https://github.com/yanivkalfa/gdscript-analyzer/blob/master/plans/ROADMAP.md) and the repository's `TECH_DEBT.md`.

## Next steps

- [Install](guide/install.md) the crate or npm package.
- [Quickstart](guide/quickstart.md) — analyze a `.gd` string.
- Consuming the library from [Rust](consume/rust.md), [Node](consume/node.md),
  or [the browser](consume/browser.md).
- [Contributing](contributing/architecture.md) and the
  [Architecture Decision Records](adr/README.md).
