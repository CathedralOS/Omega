# Ingress ownership completion research — 2026-10-22

Scope: checkpoint syntax pages, speculative parser transactions, bounded token scratch, and remaining spelling owners. Base is in base.txt. Prior source-working-set and retained-syntax research are reused.

Read official Rust Arc documentation (https://doc.rust-lang.org/std/sync/struct.Arc.html#method.make_mut) and bumpalo API (https://docs.rs/bumpalo/latest/bumpalo/struct.Bump.html#method.reset). Arc provides clone-on-write but applying it to an entire forest recreates a full copy on the first append. Freeze individual nonempty pages at the checkpoint boundary; child additions remain owned, and edits detach only touched pages. Avoid a per-node Arc.

Bump reset does not run destructors for its values. Omega syntax carries boxed names, vectors and shared decoded/source owners, so untyped reset is not a drop-in replacement. An append transaction must drop rejected rows, restore all parallel table frontiers, preserve published handles, and never roll back an unrelated mutation. Measure whether direct accepted construction removes more work than checkpoint bookkeeping adds.

Batch fusion must preserve ordered source/page identities, source custody, parse worker stack size, and lexical-error-before-parser-error priority. Recycle token record/range capacity within each worker; decoded buffers that escape into syntax stay owned there and cannot be overwritten.

Acceptance: exact logical roots, spans, diagnostics and custody, independent child mutation, transaction rollback and nested-transaction controls, retained literals after scratch reuse, and native/interpreter runtime agreement. Compare fresh and shared checkpoints, parser allocation/live bytes, all-worker assembly traffic and ordinary CLI paired timing. Existing baseline failures are attributed, never weakened.
