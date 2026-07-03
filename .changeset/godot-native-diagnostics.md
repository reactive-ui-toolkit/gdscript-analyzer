---
"@gdscript-analyzer/core": patch
---

Godot-native diagnostics (guitkx parity T4.1–T4.3): every analyzer message with a 1:1 Godot analog now uses the engine's verbatim 4.7 phrasing, pinned by a probe-derived golden table; parse errors quote tokens Godot-style (`Expected ":" after "if" condition.`) instead of leaking Rust enum names; new `TOO_FEW_ARGUMENTS` / `TOO_MANY_ARGUMENTS` errors plus Godot's `Invalid argument for "f()" function: …` type error on statically-resolved calls (default- and vararg-aware — `print` and `...rest` functions never flag; corpus-clean on 138 demo projects); `UNASSIGNED_VARIABLE` and `ENUM_VARIABLE_WITHOUT_DEFAULT` now match the engine's actual trigger conditions (untyped/enum-typed no-init locals; members only when the enum lacks a `0` value).
