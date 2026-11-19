# Runtime-selected record editing

First and Last implementations return exclusive references into a borrowed
record array. The caller edits two fields through each returned reference,
changes implementation between calls, and checks both edits, the untouched
middle record and exact 64-bit identifiers. Expected exit: 0; a mismatch traps.

```sh
omega run --both samples/cli/collections/runtime_selected_record_edit/main.omg
```

The program runs with exit 0 natively and in Terminal interpretation on Intel
macOS. The returned references retain the original array backing. Anonymous
input descriptors and returned children have distinct custody even when formed
by the same statement; a named descriptor's cleanup realizes its own checked
loan release. Conflict and access-widening controls remain separate fixtures.
