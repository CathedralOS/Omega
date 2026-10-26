# Repeated nested child type

Both `Buffer.marker` and `Buffer.cell` have type `Cell`. They contain marker 91 and mutable value 37 respectively. Registration writes only the second Cell to 41 and retains the exact outer backing. Completion writes it to 73; the first Cell remains 91 and the earlier copied scalar remains 37.

The ordinary record traversal must retain full parent paths and distinguish both occurrences of the same child type. A child type repeated in siblings is not a recursive type cycle. The active ancestry of the first occurrence ends before the second is visited.

The C fixture retains a typed union of `struct buffer *` and `struct repeated_buffer *`, with a per-slot discriminator. `nestlease_register_repeated` takes the second C type explicitly; the original registration API and completion behavior remain available. The common completion checks the discriminator before selecting the corresponding typed field access. There are no pointer casts or aliasing through the original C record. Static assertions pin both outer sizes and each field offset.

The C normal self-test mixes both shapes in separate live slots, completes them in reverse order, and reuses one capacity for the repeated shape. All original nine deliberate-trap controls still run. Source and runtime evidence remain separate: the positive needs actual native exit 0 and Terminal exit 0, while its twin inserts exactly one forbidden read of `buffer.cell.value` before Pending completes.

Optimized macOS ARM64 producer `bc8b7aeed7891890e64e3444eaf03b1c5b4f4ba4` establishes native 0 and Terminal 0; the twin rejects for its active local borrow. Other-host execution belongs to the parent row qualification. Every authored positive entry binds all four hosted targets. `positive.json`, `negative.json`, provider receipt and raw streams preserve exact replay identities outside the repository.
