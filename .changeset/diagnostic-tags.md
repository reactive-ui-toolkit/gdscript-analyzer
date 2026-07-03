---
"@gdscript-analyzer/core": patch
---

Diagnostics now carry LSP `DiagnosticTag`s: the unused/unreachable family (`UNUSED_*`, `UNREACHABLE_CODE`, `UNREACHABLE_PATTERN`) serializes `tags: [1]` (Unnecessary) so editors dim the range; the key is omitted when empty, leaving the pre-tag wire shape untouched for every other diagnostic.
