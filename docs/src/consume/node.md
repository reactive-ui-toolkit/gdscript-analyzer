# Consuming from Node

> **Status: live.** `@gdscript-analyzer/core` is published and this is its real,
> shipped surface ([`plans/ROADMAP.md`](https://github.com/yanivkalfa/gdscript-analyzer/blob/master/plans/ROADMAP.md)
> tracks what's still ahead on the road to `1.0`).

Node consumers install the napi-rs native addon:

```bash
npm i @gdscript-analyzer/core
# pnpm add @gdscript-analyzer/core
```

The addon is a real **native `.node` binary** built with
[napi-rs v3](https://napi.rs) — no WASM overhead, full native speed. This is the
path that powers Node-based LSP servers, including guitkx's. Per-platform
prebuilt binaries (`@gdscript-analyzer/core-linux-x64-gnu`, `-darwin-arm64`,
`-win32-x64-msvc`, …) are pulled in automatically through
`optionalDependencies`, so there is no compile step for consumers.

## napi vs wasm

There are **two thin binding crates** sharing one engine-neutral core
(`gdscript-session`, a URI-keyed session over `gdscript-ide`): `gdscript-ffi`
(napi-rs v3 → the Node native addon you're reading about here) and
`gdscript-wasm` (wasm-bindgen → the [browser package](./browser.md)). Sharing
`gdscript-session` means the two surfaces can't drift apart — only the thin
`#[napi]`/`#[wasm_bindgen]` delegation differs. For Node you almost always want
the **native addon** (this package): it is faster, has no
`SharedArrayBuffer`/COOP-COEP requirements, and reads files on the host side.
Reach for the [wasm package](./browser.md) only for the browser or a
sandboxed/edge runtime where a native addon can't load. See
[ADR-0003](../adr/0003-napi-rs-v3-dual-target.md) for the original binding
decision and its amendment.

## The shape across the boundary

The binding keeps a **stateful, URI-keyed session** alive inside Rust so the
analysis cache survives edits. The JS side opens/changes/closes documents by
URI and runs queries by URI + byte offset; results come back as **native JS
objects** (via napi's `serde_json::Value` bridge — no client-side
`JSON.parse`). The surface is intentionally small and flat — strings and
structs cross the boundary *by copy*, so a query returns only its feature
result, never a whole AST.

```js
import { AnalysisHandle } from "@gdscript-analyzer/core";

const az = new AnalysisHandle();
az.openDocument(
  "inmemory://player.gd",
  "extends Node\n\nfunc _ready() -> void:\n\tprint(1 + 1)\n",
  "res://player.gd", // optional res:// path, or null
);

// Byte offsets in; native JS objects out. The client maps byte offsets -> UTF-16.
const diagnostics = az.diagnostics("inmemory://player.gd");
const symbols = az.documentSymbols("inmemory://player.gd");
const hover = az.hover("inmemory://player.gd", 42);
console.log(diagnostics, symbols, hover);
```

`AnalysisHandle` also exposes `changeDocument`/`closeDocument`,
`setProjectConfig`/`setWorkspaceComplete`/`setWarningOverride`, and one method
per IDE feature (`completions`, `signatureHelp`, `codeActions`,
`gotoDefinition`, `findReferences`, `rename`, `workspaceSymbols`,
`semanticTokens`, `foldingRanges`, `inlayHints`, `format`, `formatRange`,
`syntaxTree`) — see the [Node package README](https://github.com/yanivkalfa/gdscript-analyzer/blob/master/bindings/node/README.md)
for the full, current list.

## Position encoding (the footgun)

The core emits **byte offsets**. LSP uses **UTF-16** code units. The binding
glue ships a byte→UTF-16 converter (backed by `gdscript-base`'s `LineIndex`) —
do the conversion at the boundary, not in your application code. This is
discussed in [`plans/01-ARCHITECTURE.md`](https://github.com/yanivkalfa/gdscript-analyzer/blob/master/plans/01-ARCHITECTURE.md) §4.
