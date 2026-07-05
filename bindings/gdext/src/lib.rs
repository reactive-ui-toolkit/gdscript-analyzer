//! `gdscript-gdext` — the Godot GDExtension binding (godot-rust / gdext), a **thin wrapper** over
//! [`gdscript_session::Session`].
//!
//! All real logic — the URI→`FileId` interner, the document lifecycle, the serialization of query
//! results — lives in the pure-Rust, fully unit-tested `gdscript-session` core. This crate is a
//! near-trivial `#[func]` delegator plus ONE bespoke piece: the `serde_json::Value` → Godot
//! `Variant` shim (`json_to_variant`), because gdext has no built-in serde↔Variant bridge. It
//! mirrors the napi binding (`gdscript-ffi`) and the wasm binding (`gdscript-wasm`), which wrap
//! the same `Session` — three hosts, byte-identical analysis results.
//!
//! GDScript holds one `GdscriptAnalyzer` (a plain data `RefCounted`, **not** a
//! `ScriptLanguageExtension` — structurally immune to the language-registration/hot-reload
//! hazards of script-language plugins), pushes documents by **URI string**, and queries by URI +
//! **UTF-8 byte offset**; every query returns a Godot `Dictionary`/`Array`/`String` mirroring the
//! session's JSON POD. Half-open `[start,end)` ranges, 0-based lines; **the GDScript client owns
//! the byte ↔ line/column conversion** (a `LineIndex` port), exactly like the JS clients own
//! their UTF-16 mapping. `null`-returning queries (`hover`, `signature_help`, `syntax_tree`,
//! `format`, `format_range`) map a Rust `None` to Godot `null`.
//!
//! Threading: the held salsa state is not `Sync`; Godot calls into the object from the main
//! thread only (the class does not opt into multi-threaded instance sharing). Do not call it
//! from worker `Thread`s.
//!
//! The engine model (`extension_api.json`, rkyv-encoded) is **bundled** in native builds of the
//! core — no runtime fetch and no `load_engine_api` call is needed here (the method exists for
//! completeness/diagnostics and mirrors the wasm binding's API).

// gdext's #[gdextension] entry point expands to `unsafe` FFI glue; that is this crate's only
// `unsafe`. The binding object is opaque to Rust callers and needs no `Debug`.
#![allow(unsafe_code, missing_debug_implementations)]
// Exported #[func] methods take owned `GString`s by the gdext ABI; the thin delegation only
// borrows them, so clippy's by-value lint is a false positive here (same rationale as the napi
// binding).
#![allow(clippy::needless_pass_by_value)]

use gdscript_session::Session;
use godot::prelude::*;

/// Marker type for the GDExtension entry point (`gdext_rust_init`, named in the `.gdextension`
/// descriptor's `entry_symbol`).
struct GdscriptAnalyzerExtension;

#[gdextension]
unsafe impl ExtensionLibrary for GdscriptAnalyzerExtension {}

/// Convert a borrowed [`serde_json::Value`] into a Godot [`Variant`]:
/// object → `Dictionary`, array → `Array`, string → `String`, number → `int`/`float`,
/// bool → `bool`, null → `null`. Integral JSON numbers become Godot `int` (i64) so GDScript
/// consumers can index with them directly; everything else becomes `float`.
fn json_to_variant(value: &serde_json::Value) -> Variant {
    match value {
        serde_json::Value::Null => Variant::nil(),
        serde_json::Value::Bool(b) => b.to_variant(),
        serde_json::Value::Number(n) => n.as_i64().map_or_else(
            // Not an i64: u64 overflow or a real float — f64 covers both for Godot.
            || n.as_f64().unwrap_or(f64::NAN).to_variant(),
            |i| i.to_variant(),
        ),
        serde_json::Value::String(s) => s.to_variant(),
        serde_json::Value::Array(items) => {
            let mut arr = VarArray::new();
            for item in items {
                arr.push(&json_to_variant(item));
            }
            arr.to_variant()
        }
        serde_json::Value::Object(map) => {
            let mut dict = VarDictionary::new();
            for (key, val) in map {
                dict.set(&key.to_variant(), &json_to_variant(val));
            }
            dict.to_variant()
        }
    }
}

/// `Option`-returning session queries: `None` → Godot `null`.
fn opt_json_to_variant(value: Option<serde_json::Value>) -> Variant {
    value.map_or_else(Variant::nil, |v| json_to_variant(&v))
}

/// Clamp a GDScript `int` offset into the session's `u32` byte-offset domain (negative → 0).
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn to_offset(offset: i64) -> u32 {
    offset.clamp(0, i64::from(u32::MAX)) as u32
}

/// A live, URI-keyed analysis session for GDScript source, callable from GDScript.
///
/// Construct once with `GdscriptAnalyzer.new()`, push documents with
/// `open_document`/`change_document`/`close_document` (+ `set_project_config`), then query.
/// The Rust `AnalysisHost` (and its salsa cache) stays alive across edits. A plain data
/// `RefCounted` `tool` class: usable from `@tool` editor plugins and from games alike, in-process,
/// no server, no configuration.
#[derive(GodotClass)]
#[class(tool, init, base=RefCounted)]
pub struct GdscriptAnalyzer {
    session: Session,
}

#[godot_api]
impl GdscriptAnalyzer {
    // ---- introspection ------------------------------------------------------------------------

    /// The analyzer version (the workspace version this library was built from), e.g. `"0.6.0"`.
    /// Lets a host addon log/verify the native layer it feature-detected. Static — callable as
    /// `GdscriptAnalyzer.version()` without an instance.
    #[func]
    fn version() -> GString {
        GString::from(env!("CARGO_PKG_VERSION"))
    }

    // ---- document lifecycle -------------------------------------------------------------------

    /// Open or replace a document by `uri`. Pass `res_path` (`res://…`) on first open to enable
    /// cross-file `preload` / `extends` / autoload resolution; pass `""` for none. It is recorded
    /// once and ignored on later opens (re-sending would needlessly invalidate the resource-path
    /// registry).
    #[func]
    fn open_document(&mut self, uri: GString, text: GString, res_path: GString) {
        let res_path = res_path.to_string();
        let res_path = if res_path.is_empty() {
            None
        } else {
            Some(res_path)
        };
        self.session
            .open(&uri.to_string(), &text.to_string(), res_path.as_deref());
    }

    /// Replace a document's text by `uri` (its `res://` path is unchanged). An unknown `uri` is
    /// interned as a new document (an upsert).
    #[func]
    fn change_document(&mut self, uri: GString, text: GString) {
        self.session.change(&uri.to_string(), &text.to_string());
    }

    /// Close (remove) a document by `uri`. A later re-open assigns a fresh file id.
    #[func]
    fn close_document(&mut self, uri: GString) {
        self.session.close(&uri.to_string());
    }

    /// Set the project's `project.godot` text (enables `[autoload]` singleton resolution).
    #[func]
    fn set_project_config(&mut self, text: GString) {
        self.session.set_project_config(&text.to_string());
    }

    /// Declare whether the opened file set is the **whole project** (every `.gd` under the
    /// project root). Pass `true` only after feeding every project file — it arms the
    /// absence-based `UNDEFINED_FUNCTION` / `UNDEFINED_IDENTIFIER` diagnostics (silent otherwise:
    /// a partial view can never prove a name is defined nowhere).
    #[func]
    fn set_workspace_complete(&mut self, complete: bool) {
        self.session.set_workspace_complete(complete);
    }

    /// Select the warning-gating profile: `"engine-defaults"` matches Godot's own default levels,
    /// `"strict"` promotes the safety opt-ins to WARN, `"none"` restores the auto default
    /// (standalone ⇒ strict, project ⇒ engine defaults). Unknown strings are ignored
    /// (forward-compatible).
    #[func]
    fn set_warning_override(&mut self, mode: GString) {
        self.session.set_warning_override(&mode.to_string());
    }

    /// Whether `uri` is currently open. Lets a client distinguish "file not tracked" from a
    /// genuine empty result (array queries return `[]` and nullable queries `null` for both).
    #[func]
    fn is_open(&self, uri: GString) -> bool {
        self.session.is_open(&uri.to_string())
    }

    /// Install an `extension_api` engine-model blob (rkyv bytes). Returns `false` if the bytes
    /// fail to decode. **Normally unnecessary**: native builds bundle the engine model; this
    /// exists for completeness and mirrors the wasm binding's runtime-load path.
    #[func]
    fn load_engine_api(&mut self, bytes: PackedByteArray) -> bool {
        self.session.load_engine_api(bytes.as_slice())
    }

    // ---- queries (Dictionaries/Arrays mirroring the session's JSON POD) ------------------------

    /// Parse + type diagnostics for `uri`, as an `Array` of `Dictionary`
    /// (`{ range: { start, end }, severity, code, message, source, fixes }`; byte offsets).
    #[func]
    fn diagnostics(&self, uri: GString) -> Variant {
        json_to_variant(&self.session.diagnostics(&uri.to_string()))
    }

    /// The document outline for `uri`, as an `Array`.
    #[func]
    fn document_symbols(&self, uri: GString) -> Variant {
        json_to_variant(&self.session.document_symbols(&uri.to_string()))
    }

    /// Foldable regions for `uri`, as an `Array`.
    #[func]
    fn folding_ranges(&self, uri: GString) -> Variant {
        json_to_variant(&self.session.folding_ranges(&uri.to_string()))
    }

    /// Inlay type hints for `uri`, as an `Array`.
    #[func]
    fn inlay_hints(&self, uri: GString) -> Variant {
        json_to_variant(&self.session.inlay_hints(&uri.to_string()))
    }

    /// Semantic tokens for `uri`, as an `Array`.
    #[func]
    fn semantic_tokens(&self, uri: GString) -> Variant {
        json_to_variant(&self.session.semantic_tokens(&uri.to_string()))
    }

    /// Format the whole document: the formatted text as a `String`, or `null` when the source
    /// does not parse cleanly (never formats broken code).
    #[func]
    fn format_document(&self, uri: GString) -> Variant {
        self.session
            .format(&uri.to_string())
            .map_or_else(Variant::nil, |text| text.to_variant())
    }

    /// Format the byte range `[start,end)`: `{ range: { start, end }, text }` (a minimal
    /// replacement patch), or `null` when the source does not parse cleanly.
    #[func]
    fn format_range(&self, uri: GString, start: i64, end: i64) -> Variant {
        opt_json_to_variant(self.session.format_range(
            &uri.to_string(),
            to_offset(start),
            to_offset(end),
        ))
    }

    /// Completion items at byte `offset`, as an `Array` of `Dictionary`.
    #[func]
    fn completions(&self, uri: GString, offset: i64) -> Variant {
        json_to_variant(
            &self
                .session
                .completions(&uri.to_string(), to_offset(offset)),
        )
    }

    /// Hover info (type/docs) at byte `offset`, or `null`.
    #[func]
    fn hover(&self, uri: GString, offset: i64) -> Variant {
        opt_json_to_variant(self.session.hover(&uri.to_string(), to_offset(offset)))
    }

    /// Signature help inside a call at byte `offset`, or `null`.
    #[func]
    fn signature_help(&self, uri: GString, offset: i64) -> Variant {
        opt_json_to_variant(
            self.session
                .signature_help(&uri.to_string(), to_offset(offset)),
        )
    }

    /// Quick-fix code actions at byte `offset`, as an `Array`.
    #[func]
    fn code_actions(&self, uri: GString, offset: i64) -> Variant {
        json_to_variant(
            &self
                .session
                .code_actions(&uri.to_string(), to_offset(offset)),
        )
    }

    /// Definition site(s) of the symbol at byte `offset`, as an `Array` (each carries a `uri`).
    #[func]
    fn goto_definition(&self, uri: GString, offset: i64) -> Variant {
        json_to_variant(
            &self
                .session
                .goto_definition(&uri.to_string(), to_offset(offset)),
        )
    }

    /// Every reference to the symbol at byte `offset`, as an `Array` (each carries a `uri`).
    #[func]
    fn find_references(&self, uri: GString, offset: i64) -> Variant {
        json_to_variant(
            &self
                .session
                .find_references(&uri.to_string(), to_offset(offset)),
        )
    }

    /// Workspace-wide rename of the symbol at byte `offset` to `new_name`: the edit set as a
    /// `Dictionary` (keyed per file `uri`), or an error-shaped `Dictionary` when refused.
    #[func]
    fn rename(&self, uri: GString, offset: i64, new_name: GString) -> Variant {
        json_to_variant(&self.session.rename(
            &uri.to_string(),
            to_offset(offset),
            &new_name.to_string(),
        ))
    }

    /// Fuzzy workspace symbol search for `query`, as an `Array` (each carries a `uri`).
    #[func]
    fn workspace_symbols(&self, query: GString) -> Variant {
        json_to_variant(&self.session.workspace_symbols(&query.to_string()))
    }

    /// The debug syntax tree for `uri` as a `String`, or `null` for an unknown `uri`.
    #[func]
    fn syntax_tree(&self, uri: GString) -> Variant {
        self.session
            .syntax_tree(&uri.to_string())
            .map_or_else(Variant::nil, |tree| tree.to_variant())
    }
}
