# Consuming from the Browser

> **Status: live.** `@gdscript-analyzer/wasm` is published and this is its real,
> shipped surface — try it in
> [the playground](https://yanivkalfa.github.io/gdscript-analyzer/playground/)
> ([`plans/ROADMAP.md`](https://github.com/yanivkalfa/gdscript-analyzer/blob/master/plans/ROADMAP.md)
> tracks what's still ahead on the road to `1.0`).

For in-page analysis — web playgrounds, browser-based editors (Monaco /
CodeMirror), or any client that can't load a native addon — there is a
WebAssembly build:

```bash
npm i @gdscript-analyzer/wasm
```

## How the wasm build is produced

The shipped route is a dedicated **`bindings/wasm`** crate built with
**wasm-bindgen** (`wasm-pack build --target web`), sharing the engine-neutral
`gdscript-session` core with the Node binding — not the napi-rs
`wasm32-wasip1-threads` target originally sketched in
[ADR-0003](../adr/0003-napi-rs-v3-dual-target.md) (see that ADR's amendment
note). This keeps the artifact small with no `SharedArrayBuffer`/COOP-COEP
requirement — the approach Biome and Ruff take. See
[`plans/01-ARCHITECTURE.md`](https://github.com/yanivkalfa/gdscript-analyzer/blob/master/plans/01-ARCHITECTURE.md) §4.

## The shape across the boundary

As in Node, the wasm module holds a stateful, URI-keyed `Analyzer` session and
returns results **as native JS values** (via `serde-wasm-bindgen`, JSON-compatible
mode) per query. Strings and structs cross the boundary *by copy*, so a query
returns only its feature result — never a full AST per call.

```js
import init, { Analyzer } from "@gdscript-analyzer/wasm";

await init(); // load + instantiate the .wasm module

const az = new Analyzer();
az.openDocument("inmemory://main.gd", "extends Node\nfunc f(): pass\n", null);
const diagnostics = az.diagnostics("inmemory://main.gd"); // byte offsets in, native JS out
```

`Analyzer` also exposes `changeDocument`/`closeDocument`,
`setProjectConfig`/`setWorkspaceComplete`/`setWarningOverride`,
`loadEngineApi(bytes)` (to enable engine-class completion/hover — see below),
and one method per IDE feature (`completions`, `hover`, `signatureHelp`,
`codeActions`, `gotoDefinition`, `findReferences`, `rename`,
`workspaceSymbols`, `semanticTokens`, `foldingRanges`, `inlayHints`, `format`,
`formatRange`, `syntaxTree`) — see the
[wasm package README](https://github.com/yanivkalfa/gdscript-analyzer/blob/master/bindings/wasm/README.md)
for the full, current list.

## Engine data is shipped separately

The Godot engine model (`extension_api.json`) is several megabytes, so it is
**not** `include_bytes!`'d into the wasm module. Fetch the bundled binary blob
and hand it to `loadEngineApi(bytes)` to enable engine-class
completion/hover — see the wasm package README for where to source it.

## Portability is enforced from day one

The core crates must compile to `wasm32` — no `std::fs`, no
`Instant::now()`/`SystemTime::now()`, no threads in the hot path, and
`getrandom`'s JS backend only in the wasm binding. CI runs
`cargo check -p gdscript-ide --target wasm32-unknown-unknown` on every PR. The
full rules are in [`plans/01-ARCHITECTURE.md`](https://github.com/yanivkalfa/gdscript-analyzer/blob/master/plans/01-ARCHITECTURE.md) §7.
