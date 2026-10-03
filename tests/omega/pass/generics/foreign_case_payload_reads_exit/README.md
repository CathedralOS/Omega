# Selected foreign payload reads

This preserves the stronger version of the generic case-selection witness:
both helpers return a field from their selected record payload instead of a
constant. `Second` puts a `u64` before its `i32` field, so both payload layouts
must remain exact. Do not replace these reads with case-specific constants.

```sh
omega run --both tests/omega/pass/generics/foreign_case_payload_reads_exit/main.omg
python3 tools/corpus_gate.py --both --filter generics/foreign_case_payload_reads_exit
```

Expected native and checked-interpreter exit: 70. At `0ee51ed8feb`, the
interpreter returned 70 but native production rejected `case payload observation
requires an established case`. Both now execute with the authored exit.
Package review is a separate prerequisite of the public command.

Stage 05 carries the existing case-leaf read source into scalar-field emission,
which performs the copy and read at the expression's execution point. The
successor-only precomputation and nested value namespace have been removed.
Terminal replay independently requires the copied carrier's exact active case;
the decoded-artifact regression substitutes a well-typed inactive case and
requires rejection. SQUALR-HEADLESS retains the application acceptance.
