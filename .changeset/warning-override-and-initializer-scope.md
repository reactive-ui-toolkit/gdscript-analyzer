---
"@gdscript-analyzer/core": patch
---

New `setWarningOverride("engine-defaults" | "strict" | "none")` selects the warning-gating profile — `engine-defaults` matches Godot's own default levels so an editor never warns where the engine wouldn't. A declared name is now visible to its own initializer's lambda bodies (`var f = func(): f.call()` is legal GDScript), which also stops a broken-initializer parse (`var toggle = func(broken: pass`) from cascading a false `UNDEFINED_IDENTIFIER` on the swallowed lines.
