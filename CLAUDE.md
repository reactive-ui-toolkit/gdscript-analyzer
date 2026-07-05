# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

# gdscript-analyzer — repo guide for contributors (incl. AI assistants)

A Rust **library** that statically analyzes GDScript (Godot 4.x) — a "Roslyn for Godot".
It is engine-/protocol-neutral: queries take byte offsets and return plain data (POD); each
client (the LSP server, CLI, WASM playground, Node addon, and the `guitkx` markup toolchain) maps
results to its own protocol. **A library, not a server.** The full plan lives in [`plans/`](plans/)
— start at [`plans/README.md`](plans/README.md).

**State:** published on the `0.x` line (workspace at `0.5.2`); the analyzer is **feature-complete
through Phase 6**. Live today: parse + type diagnostics, type-aware hover, completion, the full
LSP-grade query surface (symbols, goto, find-refs, rename, signature help, folding, inlay hints,
code actions), the Godot 4.x engine model, scene-aware node-path typing, a `gdformat`-compatible
formatter, and semantic tokens. Remaining work is integration-driven — see [`plans/TODO.md`](plans/TODO.md).

## Golden rules
- **Conventional Commits** on PR titles. Squash-merge turns the PR title into the commit that
  `release-plz` reads, so the title drives the version bump + changelog: `feat(syntax): …`,
  `fix(ide): …`, `docs: …`; `feat!:` / `BREAKING CHANGE:` for breaks. Scope = crate name.
- **Changesets** for the npm surface: any change affecting `@gdscript-analyzer/*` needs a
  `.changeset/*.md` (`pnpm changeset`). Rust-only changes don't.
- **ADRs**: an architecturally consequential decision lands as a numbered ADR in
  [`docs/src/adr/`](docs/src/adr/) in the same PR.
- **Portability is law.** The core crates (`base, syntax, api, scene, fmt, db, hir, ide, session`)
  MUST compile to `wasm32` — no `std::fs`, no `Instant::now()`/`SystemTime::now()`, no threads in
  the hot path. CI enforces `cargo check -p gdscript-ide --target wasm32-unknown-unknown` on every PR.
- **Never weaken a lint or test to make CI pass.** Fix the cause. Lints are denied in CI
  (`clippy -D warnings`); whitelist domain vocabulary in `clippy.toml` (`doc-valid-idents`) rather
  than sprinkling `#[allow]`.

## Workspace
Crates depend **only downward**:

```
gdscript-base                POD types (FileId, TextRange, LineIndex, query results)
 └ gdscript-syntax           logos lexer + lossless cstree parser (CST + AST)
    ├ gdscript-api           Godot engine model — GENERATED from extension_api.json
    ├ gdscript-scene         .tscn/.tres parser → node-path typing
    ├ gdscript-fmt           gdformat-compatible formatter
    └ gdscript-db            inputs / project model / salsa incremental layer
       └ gdscript-hir        name resolution + gradual type inference + warnings
          └ gdscript-ide     ← the public API (AnalysisHost / Analysis)
             └ gdscript-session   internal URI-keyed layer → serde_json::Value (NOT a stable API)
                ├ gdscript-ffi   Node addon (napi-rs)     → @gdscript-analyzer/core
                └ gdscript-wasm  browser (wasm-bindgen)   → @gdscript-analyzer/wasm
                └ gdscript-gdext Godot GDExtension (gdext) → reactive-ui-analyzer addon zip
gdscript-lsp   standalone LSP server  ┐ native clients of gdscript-ide
gdscript-cli   `gdscript` CLI         ┘ (not in the wasm32 gate)
```

- `gdscript-ide` is the **public API** (`AnalysisHost` owns files; a cheap cloneable `Analysis`
  snapshot answers queries returning POD + byte offsets). Architecture: [`plans/01-ARCHITECTURE.md`](plans/01-ARCHITECTURE.md).
- `gdscript-session` holds **all** binding logic (URI↔`FileId` interner, document lifecycle, JSON
  serialization) so it is unit-testable with plain `cargo test`. The napi/wasm crates are thin
  delegators that can't be tested natively (no `libnode`/wasm runtime at link time). It is an
  internal layer — not a stable API; consumers depend on `gdscript-ide`.
- `gdscript-api` is **generated** — never hand-edit `crates/gdscript-api/src/generated.rs` or the
  `engine_api.bin` / `engine_docs.bin` blobs beside it. Run `cargo xtask codegen-api`.
- `bindings/node` and `bindings/wasm` package the FFI crates for npm; `xtask` is repo automation.

## Commands
- `cargo xtask ci` — the full local gate (fmt + `clippy -D warnings` + test + wasm-check + deny).
  Run before every PR. Mirrors `.github/workflows/ci.yml`.
- `cargo test` — runs the **default-members** (excludes the FFI bindings, which build on their own
  targets). Single crate/test: `cargo test -p gdscript-syntax <name>`.
- `cargo xtask fixtures` — re-bless the `expect-test` golden snapshots (runs tests with
  `UPDATE_EXPECT=1`); **review the diff before committing**. Ad-hoc: `UPDATE_EXPECT=1 cargo test`.
- `cargo xtask differential` — cross-validate the parser against `tree-sitter-gdscript` (native-only,
  `tree-sitter-oracle` feature). Divergences are triaged in
  `crates/gdscript-syntax/tests/KNOWN_DIVERGENCES.md`.
- `cargo xtask codegen-api` — regenerate the engine model from the newest vendored `extension_api.json`.
- `cargo wasm-check` (alias) — the CI portability guard on its own.
- `cargo xtask dist` / `release --dry-run` — build/release ergonomics; publishing is CI-only
  (`release-plz` for crates, changesets for npm).

## Testing conventions
Golden tests use rust-analyzer's **`expect-test`** (`.cst` and inline snapshots); bless with
`cargo xtask fixtures` or `UPDATE_EXPECT=1 cargo test`. Parser correctness is additionally guarded by
the **differential oracle** against tree-sitter (`cargo xtask differential`). Robustness fuzzing uses
`proptest`; native perf benches use `criterion` (under `benches/`).

## Syncing with Godot
Engine knowledge comes from `vendor/godot/<version>/extension_api.json` (+ `doc/classes/*.xml` for
hover docs). The `godot-api-sync` GitHub Action watches Godot releases, vendors the new dump, runs
`codegen-api`, and opens a PR. Runbook: [`plans/GODOT-SYNC.md`](plans/GODOT-SYNC.md).

## Build-environment note (Windows)
You need a linker: either install VS "Desktop development with C++" (MSVC), or
`rustup default stable-x86_64-pc-windows-gnu` (bundled mingw linker). Build from **PowerShell**,
not Git Bash — MSYS's `/usr/bin/link` shadows the real linker and produces confusing errors.

## Branches
Default branch `master` (protected). Integration branch `dev`. Feature branches → PR into `dev`.
