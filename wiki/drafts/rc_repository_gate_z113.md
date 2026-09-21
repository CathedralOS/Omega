# RC-REPOSITORY-GATE — linux x86-64 leg (z113)

Incremental record of the `RC-REPOSITORY` release gate block on linux x86-64
at `bf1a1a8d6cc` (origin/main, 2026-09-20) plus this leg's repair. Prior legs:
[rc_repository_gate_z92.md](rc_repository_gate_z92.md) (`ff2f489bbff`),
[rc_repository_gate_closure_z173.md](rc_repository_gate_closure_z173.md),
[rc_repository_gate_linux_x86_64.md](rc_repository_gate_linux_x86_64.md).

**This leg repaired the `custody_mutation_matrix` unfenced residual** (22 of 23
undriven inventory families) and re-measured the two cheap gate commands. The
gate remains red overall: every surviving residual sits inside a live sibling
claim.

| # | Command | Result at this tip |
| --- | --- | --- |
| 1 | `cargo fmt --all -- --check` | **GREEN** — the `trait_operators.rs` drift recorded at z92 was repaired by its fence owner; zero residual at `bf1a1a8d6cc` |
| 2 | `cargo clippy --workspace --all-targets -- -D warnings` | not re-run; last measured at `ff2f489bbff` with all residuals fenced (§2 of z92 record) |
| 3 | `cargo nextest run -p omega-architecture-test --all-targets --no-fail-fast` | 574/576 — residuals now exactly two fenced items (below) |
| 4 | canary `retired_domain_when_surface_is_absent_from_authored_corpus` | not re-run; PASS at z92 |
| 5 | `cargo check --workspace --all-targets` | not re-run workspace-wide; `cargo check -p image-emission --all-targets` clean after this leg's edit |
| 6 | `cargo nextest run --workspace --lib --no-fail-fast` | not re-run; last measured at `ff2f489bbff` (11 failures, all sibling-lane clusters) |

## Repair landed by this leg

`installation_function_nested_custody.rs` declared 22 `*FieldForTest`
inventories and also drove them through `run_one_field_substitution_matrix` —
the ratchet requires a cross-file consumer ("no OTHER file drives it"), the
same declaration/driver split as `installation_field_substitution_fields.rs` +
`installation_field_substitutions.rs`. This leg moves the 22
`custody_field_inventory!` declarations into the new sibling module
`installation_function_nested_custody_fields.rs` (registered in
`tests/artifacts.rs`); the nested-custody file now imports the 22 enum names
explicitly and keeps driving the matrices — satisfying the ratchet with zero
behavior change.

Witnessed: `cargo nextest run -p omega-architecture-test -E
'test(~every_declared_custody_field_inventory)'` — violation list shrinks from
23 entries to the one fenced residual below. `cargo nextest run -p
image-emission --test artifacts -E
'test(~installation_function_nested_custody)'` — 18/18 PASS (all matrix
drivers intact after the move).

## Remaining red residuals — all fenced

- `custody_mutation_matrix`: `compilation-report/src/pcc/native_evidence/custody_tests.rs`
  `NativePlacedImageEvidenceFieldForTest` — fenced to CUSTODY-MUTATION-COVERAGE
  (claim `6f8aad97`, exp 2026-09-21T05:23Z).
- `glob_self_imports`: `psi/semantics/validation`
  `value_custody/expression_types/result_type.rs` — the single surviving glob
  file, fenced to MATCH-SELECTIVE-LOWERING (claim `f3edafb2`, exp
  2026-09-21T07:39Z); declared by its own fence-holder at `3bf8be9383`.
- clippy sites per z92 §2: RUNTIME-VALUE-GENERICS, TPR6,
  MATH-FOUNDATION-BINDINGS fences.
- workspace `--lib` failures per z92 §6: sibling-lane clusters.

## Residual posture

Unfenced drift reachable from this host is repaired. The gate goes green when
the fenced legs land: MATCH-SELECTIVE-LOWERING's `result_type.rs` glob and
CUSTODY-MUTATION-COVERAGE's `NativePlacedImageEvidenceFieldForTest` matrix are
the last two architecture residuals; clippy/lib residuals stay with their
recorded owners.
