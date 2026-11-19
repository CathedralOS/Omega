# Helpers can update fields through saved references

A helper can update a bounded field through a reference saved in a local
variable. It can temporarily write 61, restore a value at most 60, and return.
The caller can then use the record's bound to prove `70 + value` fits `i32`.
The compiler used to lose the field's location while producing executable code.

The compiler may omit a local variable that only names borrowed storage.
It now records the original object, field path, access permission and indices
captured when the reference was formed. Constructing another borrow follows
that record through the same path as a call argument. It consumes the omitted
outer pointer crossing while retaining real pointer loads inside the value.
Shared, mutable and write-only loans use one planner. The old recognizer
limited to three child references is removed.

The [predicate-helper examples](../../../samples/cli/facts/data_where_standing/predicate_helper/README.md)
and [write-only examples](../../../samples/cli/facts/data_where_standing/write_only/README.md)
cover all nine transport places each. Every positive checks both extreme input
results and unchanged nonzero neighbors. Both match selections run; selected
implementations have distinct tags. Each invalid counterpart removes the bound
or returns before restoring it.

All 18 positives pass ordinary `omega run --both` on macOS ARM64, explicitly
reporting native exit 0 and Terminal exit 0. All 36 invalid counterparts reject
with their own required diagnostic fragments. The 54 sample and fixture pairs
have identical program and Build files. These results establish the saved-reference
repair; the write-only examples already checked on the baseline and provide
regression coverage.

[Hosted source checking](https://github.com/CathedralOS/Omega/actions/runs/41774629213)
accepts 24 programs and correctly rejects 37 controls, including nearby existing
borrow cases. It reports no wrong reasons, lost successes, unsafe acceptances or
oracle failures. The 54 edited corpus rows retain this run's actual source stamp;
every other row is unchanged. Eight additional programs also agree in both
engines, covering saved scalar loans, reference aliases, selector evaluation
order, child-borrow restoration and nested pointer loads. Two additional
controls reject using a parent while a child remains live.

The executable producer is `cd5340c6d0348e868e34d68ee85f250c431f79ba`, with CLI
SHA-256 `02777d19f59b383102c937ddc9cd930d47196332853a7ec7696de56724a8f9fd`,
embedded optimization level 1 and the repository's pinned Rust toolchain.
The publication base is `0378c9bcf14f9e63f4c4dd67bc60a0ec06da6cd2`.
A later source edit removes only a redundant lowering check of the recorded
outer crossing. Both checked producers guarantee that marker. Independent
review verified that consuming it directly preserves every tested valid path;
the observations keep their original producer identity.

The initial optimized baseline `2713116252386e98ce5e87961aca764f8760b217`
refuses eight predicate-helper places; const-name already checks. The first
repair, `d6448cfae742e9142043bacd0a2be6a3891820b6`, still refuses those eight and
the historical scalar-loan example. Both failed results are retained.
The first local runtime setup used an inherited anchor-library path and invoked
neither engine. The successful retry explicitly uses this worktree's library.
The additional Console fixtures completed ordinary package acceptance before
their native and Terminal runs; both report their authored exit 70.

Raw receipts are retained in the shared checkout's ignored
`build/scalar-continue-20261102/` folder: `saved-loans-hosted-41774629213/`,
`saved-loans-c6-range-download-41774629213.json`,
`saved-loans-final-authored-inputs.json`,
`saved-loans-runtime-correct-library-samples.json`, the corresponding raw-command
records and consumed locks, `saved-loans-negative-sample-applicability.json`,
`saved-loans-corpus-recording.json` and
`saved-loans-c6-marker-revalidation-removal.json`. The compact ledger is
`saved-loans-completed-evidence-ledger.json`; additional engine receipts are
listed in `saved-loans-runtime-all-extra-summary.json`.

Other hosted targets retain their authored entry bindings without native
execution here. The parent **CANONICALIZE-SCALAR-RANGE-CONTRACTS** migration
remains open. GUI qualification remains deferred as requested.
