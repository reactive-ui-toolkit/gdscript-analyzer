# gdscript-gdext

The **Godot GDExtension binding** of gdscript-analyzer: a `cdylib` exposing one
`GdscriptAnalyzer` class (plain `RefCounted`, `tool`) whose methods delegate 1:1 to
[`gdscript_session::Session`](../../crates/gdscript-session) — the same core the napi
(`@gdscript-analyzer/core`) and wasm (`@gdscript-analyzer/wasm`) bindings wrap. Three hosts,
byte-identical analysis.

- **In-process, zero config**: loads via a `.gdextension` descriptor; called by function pointer.
  No server, no Node, no ports.
- **Plain data class, not a `ScriptLanguageExtension`** — structurally avoids the
  language-registration and hot-reload hazards of script-language plugins. Ships
  `reloadable = false` (load-once; editor restart on upgrade).
- **Offsets are UTF-8 bytes** (half-open, 0-based lines), like every binding: the GDScript client
  owns byte ↔ line/column conversion.
- The engine model is **bundled** (native builds of `gdscript-api` embed `engine_api.bin`);
  `load_engine_api` exists only for API symmetry with wasm.

## Build

```powershell
cargo build -p gdscript-gdext --release
# -> target/release/gdscript_gdext.dll (.so / .dylib per OS)
```

Drop the library next to `addon/reactive_ui_analyzer/gdscript_analyzer.gdextension` (under
`bin/`, using the names the descriptor lists) and copy the folder into a Godot project's
`addons/`. The `release-gdext` workflow does exactly this per-OS and attaches the assembled
addon zip to each `v*` release.

## Testing

The shim's *inputs* are covered by `gdscript-session`'s unit tests. The Variant boundary itself
cannot be `cargo test`ed (gdext builtins require a live engine), so it is behavior-tested
end-to-end by the **reactive_ui_editor headless suite** in
[ReactiveUI-Godot](https://github.com/yanivkalfa/ReactiveUI-Godot) (`tests/guitkx_editor_test.gd`,
analyzer sections), including a parity fixture asserting gdext results match the session's JSON
for identical inputs. Keep `cargo clippy -p gdscript-gdext --all-targets -- -D warnings` green;
CI checks this crate on every PR.
