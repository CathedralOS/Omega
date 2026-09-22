# Pipeline rewrites ownership audit

Audit of every rewrite-bearing module tree against the ownership rules in
[AGENTS.md](../../AGENTS.md) — pipeline crates (`X-to-Y` / `X-to-X`) own
transformations and private working state; representations hold durable IR;
semantics crates hold meaning and independent verification; backends own
unavoidable ISA/ABI/object-format detail. Recorded on 2026-09-20 on linux
x86-64 at `40e9234d97` (origin/main). Read-only audit: no transform source was
modified.

## Rewrite-bearing surface

| Module tree | Crate layer | Declared role | Verdict |
| --- | --- | --- | --- |
| `abstract-operations-to-abstract-operations/src/rules` | pipeline (X-to-X) | executable entrance — `PSI_PASS_CATALOG` sole enable/order table | Owned correctly |
| `.../rules` (10 pass files: control_flow_cleanup, copy_propagation, dead_scalar_elimination, global_value_numbering, proof_check_elision, sparse_conditional_constant_propagation, state_specialization, plus support/tests) | pipeline | per-pass local rule order | Owned correctly |
| `abstract-operations-to-abstract-operations/src/ranked_rewrites` (countdown_invariant_constant_relocation, loop_invariant_scalar_motion) | pipeline | stage group — ranked-cycle custody rewrites | Owned correctly |
| `selected-instructions-to-selected-instructions/src/rewrites` (93 entries: name.rs + name/ submodule pairs) | pipeline (X-to-X) | stage group — selected-CFG rewrites + replay evidence | Owned correctly |
| `selected-instructions-to-register-homes/src/rewrites` (rematerialization) | pipeline (X-to-Y) | stage group | Owned correctly |
| `optimization-unit/src/optimization_unit/rewrite` (model/candidate/codec/canonical_encoding) | representation | "immutable Psi rewrite candidate taxonomy" — data + invariant-preserving access only | Allowed — no `apply_*`/`transform_*`/`propose_*` fns; construction of immutable candidates is representation data. Watch item: candidate construction here is the closest thing to transform logic outside `pipeline/`; keep application verbs out. |
| `optimization-core/src/decisions` + `promotions/` | representation | decision/promotion records | Allowed — durable evidence, no transforms |
| `optimization-unit-semantics/src/candidates/rewrite_accounting` | semantics | executable entrance — provenance/custody accounting reconstructed independently of producers | Owned correctly — independent acceptance is the semantics role |
| `terminal-verifier/tests/ranked_scc`, `terminal-psi-to-abstract-operations/tests/ranked_native` | tests | fixtures | Out of scope (test data) |

## Name overlap that is not rewrite ownership

- `machine-emission/src/exit_contract/validation_rules` — exit-contract
  validation predicates in a backend crate; not optimization rewrites.
- `proof-admission/src/integer_rules` — proof-citation normalizations in Psi
  semantics; not optimization rewrites.
- `packages/manager/**/decisions` — lockfile decision records; unrelated.

Future inventories should key on the module-role tag, not directory names.

## Orphan check (script)

For each rewrite dir above: every subdirectory must have a sibling `name.rs`
or a `mod name;` declaration in the parent's `mod.rs`, and every `*.rs` file
must be declared or have a submodule dir. Result: zero orphans in
`rules`, `ranked_rewrites`, `selected-instructions-*/src/rewrites`
(all 93 entries declared). The `ORPHAN-REWRITE-MODULES-CATALOG` work already
machine-checks the sis2sis roster via `module_catalog.rs`; this audit extends
the same check by script over the other three trees.

## Architecture gates already covering this

`tests/architecture/entrypoint_module_layout.rs` pins the
`Optimizer module role:` header convention (every audited `mod.rs` carries
one) and lists `optimization_unit/rewrite/model`; `layering.rs` holds
dep-direction gates plus targeted rewrite-structure checks (e.g. the
`ranked_rewrites/countdown_invariant_constant_relocation` and
`selected_lowering/literal_fold` rows). The audit found no live violation of
either.

## Conclusion

Rewrite ownership is clean at this head: all transformation code lives in the
three pipeline optimization crates, representations hold only taxonomy and
evidence, and the semantics crate owns independent accounting. No orphan
modules and no misplaced apply/transform entry points. One watch item:
candidate *construction* inside `optimization-unit/rewrite` is data-shaped
today — keep transform application verbs out of that tree or revisit the
placement.
