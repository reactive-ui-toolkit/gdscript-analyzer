# Reactive UI Analyzer (native GDScript analysis for the Godot editor)

Prebuilt [gdscript-analyzer](https://github.com/yanivkalfa/gdscript-analyzer) binaries packaged as
a Godot **GDExtension**: one `GdscriptAnalyzer` class (a plain `RefCounted`) exposing the full
analysis surface — diagnostics, completions, hover, go-to-definition, find-references, rename,
signature help, inlay hints, symbols, semantic tokens, folding, code actions, and formatting —
**in-process**: no server, no Node runtime, no configuration.

Built as the deep-intelligence layer for the **Reactive UI Editor** Godot addon
(`reactive_ui_editor`, from [ReactiveUI-Godot](https://github.com/yanivkalfa/ReactiveUI-Godot)):
with this addon installed next to it, embedded GDScript inside `.guitkx` files gets type-aware
completion, hover, and diagnostics. `reactive_ui_editor` feature-detects it
(`ClassDB.class_exists("GdscriptAnalyzer")`) and degrades gracefully to markup-only intelligence
when it is absent — this addon is an optional enhancement, never a requirement. It is equally
usable standalone from any GDScript tool code.

## Install

1. Copy `addons/reactive_ui_analyzer/` into your project's `res://addons/`.
2. Restart the editor (GDExtensions load at startup; `reloadable = false` is deliberate).
3. There is nothing to enable — it is a library, not an `EditorPlugin`. Verify with
   `print(GdscriptAnalyzer.version())` anywhere in tool code.

Requires **Godot 4.4+**. Platforms: Windows x86_64, Linux x86_64/arm64, macOS universal
(x86_64 + arm64). The macOS libraries are unsigned — on first load, allow them under
System Settings → Privacy & Security (or `xattr -dr com.apple.quarantine` the addon folder).

## Export note

This is **editor tooling**. Exclude `addons/reactive_ui_analyzer/*` in your export presets
(Resources → "Filters to exclude") so games don't carry an analysis library they never call.

## API sketch (GDScript)

```gdscript
var az := GdscriptAnalyzer.new()
az.open_document("untitled:probe.gd", "var x := 1\n", "")   # uri, text, res_path ("" = none)
print(az.diagnostics("untitled:probe.gd"))                   # -> Array of Dictionary
print(az.completions("untitled:probe.gd", 10))               # byte offset, UTF-8
az.close_document("untitled:probe.gd")
```

All offsets are **UTF-8 byte offsets** (half-open ranges, 0-based lines) — convert from
`CodeEdit` line/column at the call boundary. Results are plain `Dictionary`/`Array` mirroring the
analyzer's JSON POD; navigation results carry a `uri` per file. Query from the **main thread**
only.

Versioning follows the gdscript-analyzer workspace (this addon zip is attached to each
[`v*` release](https://github.com/yanivkalfa/gdscript-analyzer/releases)).
