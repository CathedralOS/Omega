# Source import custody and queue research — 2026-10-21

Question: can import discovery borrow its stable toolchain custody directly,
and can duplicate queue keys avoid allocating another path without hashing twice?
Prior experiment: [source metadata transfer](../import_assembly/README.md).

Queries:
- `site.doc.rust-lang.org std collections HashSet implemented HashMap value ()`
- `site:doc.rust-lang.org std collections hash_map VacantEntry key into_key`

Read the Rust standard-library documentation beyond search snippets:
[HashSet](https://doc.rust-lang.org/std/collections/hash_set/struct.HashSet.html),
[VacantEntry](https://doc.rust-lang.org/std/collections/hash_map/struct.VacantEntry.html).
HashSet wraps a HashMap with unit values. VacantEntry exposes the owned lookup key
by reference before insertion. Moving `(PathBuf, DependencyScope)` into an entry
lets only a new admission copy its pending-load key. This preserves one lookup and
the existing storage layout. A contains-then-insert approach adds a second lookup
for unique keys; shared path pointers add another allocation/ownership mechanism.
Neither is needed. This is a hypothesis about allocation work, not elapsed time.

Custody snapshots exist solely to split borrowing of immutable custody and mutable
resolved imports. Store the existing custody type directly in SourceStorage and
borrow that field; registration, exact-file precedence and longest-directory-first
selection remain unchanged. No cache or reduced security check is involved.

Acceptance: compare current-main and each mechanism separately through real source
assembly, with exact output/diagnostic hashes in unique and shared checkpoint modes.
Retain only demonstrated allocation elimination with preserved scoped identity,
ordering, custody and runtime outcomes. Report full CLI timing spread separately.
