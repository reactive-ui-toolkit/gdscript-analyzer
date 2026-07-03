//! T4.1 — the Godot-native message golden table.
//!
//! Every user-facing analyzer message that maps 1:1 onto a Godot 4.7 diagnostic is pinned here
//! against the REAL engine's verbatim output, captured from the official binary:
//!
//! - binary: **Godot v4.7.stable.official.5b4e0cb0f** (Windows) — the same version vendored in
//!   `vendor/godot/4.7-stable/` (see its `SOURCE.txt`)
//! - command: `godot --headless --path <proj> --check-only --script res://<probe>.gd`
//! - probed: 2026-07-03
//! - warning-class texts were captured by promoting each warning to ERROR level in the probe
//!   project (`[debug] gdscript/warnings/<name>=2` in `project.godot`) — `--check-only` prints
//!   warnings only through the error path. The trailing `(Warning treated as error.)` marker is
//!   the promotion artifact, NOT part of the message.
//!
//! The `p..`/`q..`/`r..` ids in comments name the probe scripts of that capture; the sources
//! here are the same shapes. When the vendored engine bumps (the `godot-api-sync` runbook),
//! re-probe and update this table in the same PR.
//!
//! Deliberate divergences from Godot, asserted here so they stay visible:
//! - Godot hard-ERRORS where the analyzer keeps the catalog's Warning default:
//!   property/constant-called-as-function (q18/q19), param redeclaration (r31, under
//!   `SHADOWED_VARIABLE`), and `class_name` hiding a global (r29). The TEXT is Godot's; the
//!   severity is gated per catalog so a registry-seam false positive can never block a build.
//! - Godot is SILENT where the analyzer is too (negative cases below): member misses on
//!   Object-class receivers (p34/p35 — the opt-in `UNSAFE_*` codes cover them), statements
//!   after `while true:` (r43), and reads of typed no-init locals (q11 — Godot 4
//!   zero-initializes them).
//! - Statements after `break`/`continue` (q04/q05): Godot is silent; the analyzer flags them
//!   with its own wording (extra coverage, pinned below so the wording stays distinct from the
//!   after-`return` text Godot owns).
//! - The indentation texts live in the syntax layer (`prepass.rs`) and are pinned by its tests.

use gdscript_base::FileId;
use gdscript_db::RootDatabase;

use crate::infer::analyze_file;

/// `(code, message)` pairs from the whole-file pass. The file is registered in the db (the
/// file's OWN types — enums, inner classes — resolve through salsa queries and need it);
/// `complete` arms the loader's workspace-completeness claim, which gates the
/// `UNDEFINED_FUNCTION` / `UNDEFINED_IDENTIFIER` pair (ADR-0005).
fn msgs(src: &str, complete: bool) -> Vec<(String, String)> {
    let api = gdscript_api::bundled();
    let mut db = RootDatabase::default();
    db.set_file_text(FileId(0), src, salsa::Durability::LOW);
    db.set_file_path(FileId(0), "res://probe.gd");
    db.sync_source_root();
    db.set_workspace_complete(complete);
    let root = gdscript_syntax::parse(src).syntax_node();
    let fi = analyze_file(&db, api, &root, FileId(0));
    fi.diagnostics
        .iter()
        .map(|d| (d.code.clone(), d.message.clone()))
        .chain(
            fi.raw_warnings
                .iter()
                .map(|w| (w.code.as_str().to_owned(), w.message.clone())),
        )
        .collect()
}

fn file_msgs(src: &str) -> Vec<(String, String)> {
    msgs(src, false)
}

fn project_msgs(src: &str) -> Vec<(String, String)> {
    msgs(src, true)
}

#[track_caller]
fn assert_emits(msgs: &[(String, String)], code: &str, message: &str) {
    assert!(
        msgs.iter().any(|(c, m)| c == code && m == message),
        "expected ({code}, {message:?})\ngot: {msgs:#?}"
    );
}

#[track_caller]
fn assert_absent(msgs: &[(String, String)], code: &str) {
    assert!(
        !msgs.iter().any(|(c, _)| c == code),
        "expected NO {code}\ngot: {msgs:#?}"
    );
}

// ---- compile-error classes -----------------------------------------------------------------

#[test]
fn builtin_method_miss_string() {
    // p01. Godot prints BOTH `Cannot find member "casll" in base "String".` and this — the
    // analyzer emits one diagnostic per site: the call-check text.
    let m = file_msgs("func t():\n\treturn \"abc\".casll()\n");
    assert_emits(
        &m,
        "UNDEFINED_METHOD",
        "Function \"casll()\" not found in base String.",
    );
}

#[test]
fn builtin_property_miss_vector2() {
    // p02 — the member-check text; the base IS quoted here (unlike the method form).
    let m = file_msgs("func t():\n\treturn Vector2(1, 2).zzz\n");
    assert_emits(
        &m,
        "UNDEFINED_PROPERTY",
        "Cannot find member \"zzz\" in base \"Vector2\".",
    );
}

#[test]
fn dictionary_method_miss() {
    // p32 — Dictionary gets NO "Cannot find member" companion in Godot (keyed-access sugar
    // absorbs the member check); the call check still fires.
    let m = file_msgs("func t() -> void:\n\tvar d: Dictionary = {}\n\td.greet()\n");
    assert_emits(
        &m,
        "UNDEFINED_METHOD",
        "Function \"greet()\" not found in base Dictionary.",
    );
}

#[test]
fn callable_method_miss() {
    // p33 — the ADR-0008 example, both recorded phrasings were real; this is the emitted one.
    let m = file_msgs("func t() -> void:\n\tvar c: Callable = func(): pass\n\tc.casll()\n");
    assert_emits(
        &m,
        "UNDEFINED_METHOD",
        "Function \"casll()\" not found in base Callable.",
    );
}

#[test]
fn undefined_function_bare_name() {
    // p04 — "base self" is literal: a bare call resolves against the script's own class.
    let m = project_msgs("func t():\n\treturn not_defined_fn()\n");
    assert_emits(
        &m,
        "UNDEFINED_FUNCTION",
        "Function \"not_defined_fn()\" not found in base self.",
    );
}

#[test]
fn undefined_identifier() {
    // p03
    let m = project_msgs("func t():\n\treturn not_defined_var\n");
    assert_emits(
        &m,
        "UNDEFINED_IDENTIFIER",
        "Identifier \"not_defined_var\" not declared in the current scope.",
    );
}

#[test]
fn type_mismatch_var_decl_init() {
    // p05 — Godot's generic first line (its second line is declaration-specific; the generic
    // one is common to every assignment shape, so the analyzer emits that).
    let m = file_msgs("func t() -> void:\n\tvar x: int = \"s\"\n\tprint(x)\n");
    assert_emits(
        &m,
        "TYPE_MISMATCH",
        "Cannot assign a value of type \"String\" as \"int\".",
    );
}

#[test]
fn type_mismatch_assignment_stmt() {
    // q28 — same generic text at a plain assignment.
    let m = file_msgs("func t() -> void:\n\tvar x: int = 1\n\tx = \"s\"\n\tprint(x)\n");
    assert_emits(
        &m,
        "TYPE_MISMATCH",
        "Cannot assign a value of type \"String\" as \"int\".",
    );
}

#[test]
fn type_mismatch_return() {
    // p36 — the return context swaps the verb.
    let m = file_msgs("func t() -> int:\n\treturn \"s\"\n");
    assert_emits(
        &m,
        "TYPE_MISMATCH",
        "Cannot return a value of type \"String\" as \"int\".",
    );
}

#[test]
fn class_name_hides_native_class() {
    // r29 — Godot hard-errors; the analyzer keeps Warning severity (see module doc).
    let m = file_msgs("class_name Node\n\nfunc t() -> void:\n\tpass\n");
    assert_emits(
        &m,
        "SHADOWED_GLOBAL_IDENTIFIER",
        "Class \"Node\" hides a native class.",
    );
}

// ---- warning classes (texts captured via the promotion trick) -------------------------------

#[test]
fn unused_variable() {
    // p07
    let m = file_msgs("func t() -> void:\n\tvar unused_thing = 3\n");
    assert_emits(
        &m,
        "UNUSED_VARIABLE",
        "The local variable \"unused_thing\" is declared but never used in the block. If this is intended, prefix it with an underscore: \"_unused_thing\".",
    );
}

#[test]
fn unused_local_constant() {
    // q01
    let m = file_msgs("func t() -> void:\n\tconst UNUSED_C: int = 3\n");
    assert_emits(
        &m,
        "UNUSED_LOCAL_CONSTANT",
        "The local constant \"UNUSED_C\" is declared but never used in the block. If this is intended, prefix it with an underscore: \"_UNUSED_C\".",
    );
}

#[test]
fn unused_parameter_names_the_function() {
    // p20 — the parameter form bakes in the enclosing function's name.
    let m = file_msgs("func t(unused_param: int) -> void:\n\tpass\n");
    assert_emits(
        &m,
        "UNUSED_PARAMETER",
        "The parameter \"unused_param\" is never used in the function \"t()\". If this is intended, prefix it with an underscore: \"_unused_param\".",
    );
}

#[test]
fn unused_private_class_variable() {
    // q02
    let m = file_msgs("var _priv: int = 3\n\nfunc t() -> void:\n\tpass\n");
    assert_emits(
        &m,
        "UNUSED_PRIVATE_CLASS_VARIABLE",
        "The class variable \"_priv\" is declared but never used in the class.",
    );
}

#[test]
fn unused_signal() {
    // q03
    let m = file_msgs("signal my_sig\n\nfunc t() -> void:\n\tpass\n");
    assert_emits(
        &m,
        "UNUSED_SIGNAL",
        "The signal \"my_sig\" is declared but never explicitly used in the class.",
    );
}

#[test]
fn unreachable_code_after_return_names_the_function() {
    // p08
    let m = file_msgs("func t() -> int:\n\treturn 1\n\tprint(\"after\")\n");
    assert_emits(
        &m,
        "UNREACHABLE_CODE",
        "Unreachable code (statement after return) in function \"t()\".",
    );
}

#[test]
fn unreachable_code_after_if_where_every_branch_returns() {
    // r42 — Godot uses the same after-return wording for an all-branches-return `if`.
    let m = file_msgs(
        "func t(b: bool) -> int:\n\tif b:\n\t\treturn 1\n\telse:\n\t\treturn 2\n\tprint(\"after\")\n",
    );
    assert_emits(
        &m,
        "UNREACHABLE_CODE",
        "Unreachable code (statement after return) in function \"t()\".",
    );
}

#[test]
fn unreachable_code_after_break_keeps_the_analyzer_wording() {
    // q04 — Godot is SILENT here; the analyzer's extra coverage keeps its own text so it can
    // never be mistaken for Godot's after-return message.
    let m = file_msgs("func t() -> void:\n\tfor i in 3:\n\t\tbreak\n\t\tprint(i)\n");
    assert_emits(
        &m,
        "UNREACHABLE_CODE",
        "Unreachable code (statement after a break, continue, or a diverging branch).",
    );
}

#[test]
fn unreachable_pattern() {
    // q06
    let m = file_msgs(
        "func t(v: int) -> String:\n\tmatch v:\n\t\t_:\n\t\t\treturn \"any\"\n\t\t1:\n\t\t\treturn \"one\"\n\treturn \"\"\n",
    );
    assert_emits(
        &m,
        "UNREACHABLE_PATTERN",
        "Unreachable pattern (pattern after wildcard or bind).",
    );
}

#[test]
fn shadowed_member_names_the_line() {
    // q08 — the member `speed` is declared on line 1.
    let m = file_msgs(
        "var speed: int = 1\n\nfunc t() -> void:\n\tvar speed: int = 2\n\tprint(speed)\n",
    );
    assert_emits(
        &m,
        "SHADOWED_VARIABLE",
        "The local variable \"speed\" is shadowing an already-declared variable at line 1 in the current class.",
    );
}

#[test]
fn shadowed_param_uses_godots_redeclaration_text() {
    // r31 — Godot hard-errors on this shape; the analyzer keeps Warning severity with the text.
    let m = file_msgs("func t(count: int) -> void:\n\tvar count: int = 2\n\tprint(count)\n");
    assert_emits(
        &m,
        "SHADOWED_VARIABLE",
        "There is already a parameter named \"count\" declared in this scope.",
    );
}

#[test]
fn shadowed_base_class_property() {
    // p23 — names the member kind and the base class.
    let m =
        file_msgs("extends Node\n\nfunc t() -> void:\n\tvar name: String = \"x\"\n\tprint(name)\n");
    assert_emits(
        &m,
        "SHADOWED_VARIABLE_BASE_CLASS",
        "The local variable \"name\" is shadowing an already-declared property in the base class \"Node\".",
    );
}

#[test]
fn shadowed_base_class_constant() {
    // q09
    let m = file_msgs(
        "extends Node\n\nfunc t() -> void:\n\tvar NOTIFICATION_READY: int = 1\n\tprint(NOTIFICATION_READY)\n",
    );
    assert_emits(
        &m,
        "SHADOWED_VARIABLE_BASE_CLASS",
        "The local variable \"NOTIFICATION_READY\" is shadowing an already-declared constant in the base class \"Node\".",
    );
}

#[test]
fn shadowed_global_builtin_function() {
    // q10
    let m = file_msgs("func t() -> void:\n\tvar abs: int = 1\n\tprint(abs)\n");
    assert_emits(
        &m,
        "SHADOWED_GLOBAL_IDENTIFIER",
        "The variable \"abs\" has the same name as a built-in function.",
    );
}

#[test]
fn shadowed_global_native_class() {
    // r33
    let m = file_msgs("func t() -> void:\n\tvar Node2D: int = 1\n\tprint(Node2D)\n");
    assert_emits(
        &m,
        "SHADOWED_GLOBAL_IDENTIFIER",
        "The variable \"Node2D\" has the same name as a native class.",
    );
}

#[test]
fn shadowed_engine_singleton_reads_as_native_class() {
    // r41 — Godot's `is_shadowing` labels singletons as native classes.
    let m = file_msgs("func t() -> void:\n\tvar Input: int = 1\n\tprint(Input)\n");
    assert_emits(
        &m,
        "SHADOWED_GLOBAL_IDENTIFIER",
        "The variable \"Input\" has the same name as a native class.",
    );
}

#[test]
fn unassigned_untyped_local() {
    // r35 — the UNTYPED no-init local is the candidate; a typed one is zero-initialized (q11).
    let m = file_msgs("func t() -> void:\n\tvar x\n\tprint(x)\n");
    assert_emits(
        &m,
        "UNASSIGNED_VARIABLE",
        "The variable \"x\" is used before being assigned a value.",
    );
}

#[test]
fn standalone_expression() {
    // q12
    let m = file_msgs("func t() -> void:\n\t1 + 2\n");
    assert_emits(
        &m,
        "STANDALONE_EXPRESSION",
        "Standalone expression (the line may have no effect).",
    );
}

#[test]
fn standalone_ternary() {
    // q13
    let m = file_msgs("func t(b: bool) -> void:\n\t1 if b else 2\n");
    assert_emits(
        &m,
        "STANDALONE_TERNARY",
        "Standalone ternary operator (the return value is being discarded).",
    );
}

#[test]
fn int_as_enum_without_cast() {
    // q14/r45 — identical text for script and engine enums; the engine-enum shape is used
    // because bare `resolve_type_name` has no file context for own-script enums (a recorded
    // under-warn: see TECH_DEBT.md).
    let m = file_msgs("func t() -> void:\n\tvar e: MouseButton = 2\n\tprint(e)\n");
    assert_emits(
        &m,
        "INT_AS_ENUM_WITHOUT_CAST",
        "Integer used when an enum value is expected. If this is intended, cast the integer to the enum type using the \"as\" keyword.",
    );
}

#[test]
fn enum_member_without_default_fires_only_when_the_enum_lacks_zero() {
    // r48 — `MouseButtonMask` declares no `0` value, so the implicit default is invalid…
    let m = file_msgs("var e: MouseButtonMask\n\nfunc t() -> void:\n\tprint(e)\n");
    assert_emits(
        &m,
        "ENUM_VARIABLE_WITHOUT_DEFAULT",
        "The variable \"e\" has an enum type and does not set an explicit default value. The default will be set to \"0\".",
    );
    // …r46 — while `MouseButton` has `MOUSE_BUTTON_NONE = 0` and Godot stays silent.
    let ok = file_msgs("var e: MouseButton\n\nfunc t() -> void:\n\tprint(e)\n");
    assert_absent(&ok, "ENUM_VARIABLE_WITHOUT_DEFAULT");
}

#[test]
fn enum_typed_local_no_init_is_unassigned_not_enum_default() {
    // r47 — Godot's ENUM_VARIABLE_WITHOUT_DEFAULT is MEMBER-only; an enum-typed local without
    // an initializer is tracked as UNASSIGNED_VARIABLE instead (even when the enum HAS a 0).
    let m = file_msgs("func t() -> void:\n\tvar e: MouseButton\n\tprint(e)\n");
    assert_emits(
        &m,
        "UNASSIGNED_VARIABLE",
        "The variable \"e\" is used before being assigned a value.",
    );
    assert_absent(&m, "ENUM_VARIABLE_WITHOUT_DEFAULT");
}

#[test]
fn onready_with_export() {
    // q16
    let m = file_msgs(
        "extends Node\n\n@onready @export var x: int = 1\n\nfunc t() -> void:\n\tprint(x)\n",
    );
    assert_emits(
        &m,
        "ONREADY_WITH_EXPORT",
        "\"@onready\" will set the default value after \"@export\" takes effect and will override it.",
    );
}

#[test]
fn confusable_identifier() {
    // q17 — the second character is CYRILLIC SMALL LETTER A.
    let m = file_msgs("func t() -> void:\n\tvar vаriable: int = 1\n\tprint(vаriable)\n");
    assert_emits(
        &m,
        "CONFUSABLE_IDENTIFIER",
        "The identifier \"vаriable\" has misleading characters and might be confused with something else.",
    );
}

#[test]
fn property_called_as_function() {
    // q18 — Godot names the member's TYPE, not its kind, and hard-errors (see module doc).
    let m = file_msgs("func t() -> void:\n\tvar n := Node.new()\n\tn.name()\n");
    assert_emits(
        &m,
        "PROPERTY_USED_AS_FUNCTION",
        "Name \"name\" called as a function but is a \"StringName\".",
    );
}

#[test]
fn untyped_declaration_variable_and_parameter() {
    // q21/q22 — no "The " prefix in Godot's strictness texts.
    let m = file_msgs("func t(p) -> void:\n\tvar x = 1\n\tprint(x)\n\tprint(p)\n");
    assert_emits(
        &m,
        "UNTYPED_DECLARATION",
        "Variable \"x\" has no static type.",
    );
    assert_emits(
        &m,
        "UNTYPED_DECLARATION",
        "Parameter \"p\" has no static type.",
    );
}

#[test]
fn inferred_declaration() {
    // q23
    let m = file_msgs("func t() -> void:\n\tvar x := 1\n\tprint(x)\n");
    assert_emits(
        &m,
        "INFERRED_DECLARATION",
        "Variable \"x\" has an implicitly inferred static type.",
    );
}

#[test]
fn unsafe_method_and_property_access() {
    // q24/q25 — these two were already Godot-verbatim before T4.1; pinned so they stay so.
    let m = file_msgs("func t(n: Node) -> void:\n\tn.some_fn()\n\tprint(n.some_prop)\n");
    assert_emits(
        &m,
        "UNSAFE_METHOD_ACCESS",
        "The method \"some_fn()\" is not present on the inferred type \"Node\" (but may be present on a subtype).",
    );
    assert_emits(
        &m,
        "UNSAFE_PROPERTY_ACCESS",
        "The property \"some_prop\" is not present on the inferred type \"Node\" (but may be present on a subtype).",
    );
}

#[test]
fn unsafe_call_argument_names_the_callee() {
    // q26
    let m = file_msgs(
        "func take(i: int) -> void:\n\tprint(i)\n\nfunc t() -> void:\n\tvar v: Variant = 1\n\ttake(v)\n",
    );
    assert_emits(
        &m,
        "UNSAFE_CALL_ARGUMENT",
        "The argument 1 of the function \"take()\" requires the subtype \"int\" but the supertype \"Variant\" was provided.",
    );
}

#[test]
fn integer_division() {
    // q22-era capture; text was already exact.
    let m = file_msgs("func t() -> void:\n\tprint(5 / 2)\n");
    assert_emits(
        &m,
        "INTEGER_DIVISION",
        "Integer division. Decimal part will be discarded.",
    );
}

#[test]
fn narrowing_conversion() {
    // r44; text was already exact.
    let m = file_msgs(
        "func gf() -> float:\n\treturn 1.5\n\nfunc t() -> void:\n\tvar x: int = gf()\n\tprint(x)\n",
    );
    assert_emits(
        &m,
        "NARROWING_CONVERSION",
        "Narrowing conversion (float is converted to int and loses precision).",
    );
}

// ---- call arity + argument errors (T4.3) ----------------------------------------------------

#[test]
fn too_many_arguments_own_function() {
    // p09
    let m = file_msgs("func one(a: int) -> void:\n\tprint(a)\n\nfunc t() -> void:\n\tone(1, 2)\n");
    assert_emits(
        &m,
        "TOO_MANY_ARGUMENTS",
        "Too many arguments for \"one()\" call. Expected at most 1 but received 2.",
    );
}

#[test]
fn too_few_arguments_own_function() {
    // p10
    let m = file_msgs(
        "func two(a: int, b: int) -> void:\n\tprint(a, b)\n\nfunc t() -> void:\n\ttwo(1)\n",
    );
    assert_emits(
        &m,
        "TOO_FEW_ARGUMENTS",
        "Too few arguments for \"two()\" call. Expected at least 2 but received 1.",
    );
}

#[test]
fn too_few_arguments_engine_method_counts_defaults() {
    // p14 — `add_child(node, force_readable_name = false, internal = 0)`: one REQUIRED
    // parameter; Godot's "at least" is the default-less count.
    let m = file_msgs("func t() -> void:\n\tvar n := Node.new()\n\tn.add_child()\n");
    assert_emits(
        &m,
        "TOO_FEW_ARGUMENTS",
        "Too few arguments for \"add_child()\" call. Expected at least 1 but received 0.",
    );
}

#[test]
fn too_many_arguments_utility_function() {
    // p21
    let m = file_msgs("func t() -> void:\n\tprint(absf(1.0, 2.0))\n");
    assert_emits(
        &m,
        "TOO_MANY_ARGUMENTS",
        "Too many arguments for \"absf()\" call. Expected at most 1 but received 2.",
    );
}

#[test]
fn invalid_argument_type_is_a_hard_error() {
    // p06 — Godot prints the generic `Cannot pass …` line too; the analyzer emits the
    // site-specific text (the callee name is known here).
    let m =
        file_msgs("func take(i: int) -> void:\n\tprint(i)\n\nfunc t() -> void:\n\ttake(\"s\")\n");
    assert_emits(
        &m,
        "TYPE_MISMATCH",
        "Invalid argument for \"take()\" function: argument 1 should be \"int\" but is \"String\".",
    );
}

#[test]
fn variadic_calls_never_arity_error() {
    // p15 — `print` is vararg.
    let m = file_msgs("func t() -> void:\n\tprint(1, 2, 3)\n");
    assert_absent(&m, "TOO_MANY_ARGUMENTS");
    assert_absent(&m, "TOO_FEW_ARGUMENTS");
}

// ---- negative cases: the analyzer must be silent exactly where Godot is ---------------------

#[test]
fn object_class_member_miss_is_not_a_hard_undefined() {
    // p34/p35 — Godot is silent on Object-receiver misses at engine defaults (a script can
    // attach members at runtime); only the opt-in UNSAFE_* pair may speak.
    let m = file_msgs("func t() -> void:\n\tvar n := Node.new()\n\tn.fooo()\n\tprint(n.fooo_p)\n");
    assert_absent(&m, "UNDEFINED_METHOD");
    assert_absent(&m, "UNDEFINED_PROPERTY");
}

#[test]
fn typed_no_init_local_read_is_not_unassigned() {
    // q11 — Godot 4 zero-initializes typed locals.
    let m = file_msgs("func t() -> int:\n\tvar x: int\n\treturn x\n");
    assert_absent(&m, "UNASSIGNED_VARIABLE");
}

#[test]
fn code_after_while_true_is_not_flagged() {
    // r43 — Godot is silent; the flow analysis also always falls through a loop (its body may
    // run zero times), so the analyzer matches.
    let m = file_msgs("func t() -> void:\n\twhile true:\n\t\tpass\n\tprint(\"after\")\n");
    assert_absent(&m, "UNREACHABLE_CODE");
}
