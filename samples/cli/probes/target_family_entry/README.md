# target_family_entry

The root selects a member handle per target: every `entry` body is
target-scoped and calls only its own target's `helper` — neither
spelling nor the call graph is rewritten to make one target's body the
entry.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
