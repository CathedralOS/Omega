# RC diagnostics — closure record

Status: closure ledger for the **RC-DIAGNOSTICS-CLOSURE** row — whether
the linux x86-64 fail-canary gate has closed and what the row retires
into. The live measurement record stays in
[`rc_diagnostics_linux_x86_64.md`](rc_diagnostics_linux_x86_64.md); this
note records the retirement disposition. It authorizes no new gate and
changes no fence.

## The gate

The release-matrix diagnostics row runs the full fail corpus:

```text
cargo nextest run -p compiler --test canary_suite \
  proof_and_float_suites::proof_and_domain_canaries::fail_canaries_reject_with_expected_diagnostic_fragment
```

(the completion doc's recorded filter omits the middle
`proof_and_domain_canaries` module; the test lives one level deeper).

## Closure evidence

Green witnesses on linux x86-64, cargo-nextest (mbx unavailable on this
host):

| revision | result | recorded by |
| --- | --- | --- |
| `72fc66d6c326` | PASS, 109.3s, zero drift | measurement draft re-witness |
| `53817f8759e5` | PASS, 115.6s, zero drift | measurement draft re-witness |
| `771d0469a1c` | PASS, 112.6s, full fail corpus | RC-DIAGNOSTICS-GATE row |

The earlier red records are retained in the measurement draft:
`e76d715c8e` (10 drifted: 8 stale `expected.txt` + 2 silent
acceptances), `1edade1a480` (11 drifted — added
`comptime/fuel_exhausted_const_array_length`), and the interim
`d8a4c5603fe` (4 drifted, including the `quotient_*` respells and the
`machine_self_call_recursion_rejected` silent acceptance).

## How the drift set closed

Every fixture in the recorded set now rejects with its pinned fragment:

- Six wording fixtures respelled and `providers/slot_plan_ambiguous`'s
  `build.omg` re-bound to all four targets so its two-covering-plans
  ambiguity fires again — `3ba619a81098` (RC-DIAGNOSTICS-GATE).
- `generics/colon_bound_rejected` re-bound through a `domain<T: copy>`
  head so the original colon-bound check fires — `3a505ad6ff5e`.
- `ownership/linear_ambiguous_state_result_mapping` re-pinned to the
  production-route refusal and `calls/guarded_value_call_terminal_rejected`
  graduated to `pass/calls/guarded_value_call_computed_argument_exit`
  after upstream discharged the spliced-continuation obligation —
  `273e0d18e9bc`.
- `domains/boundary_operator_mutation_invalidates_domain` respelled to
  forward the stored `&mut` field through a `&mut` binding —
  `d74f2145b9` / `2e1db3ba3e` (RC-DIAGNOSTICS-STABILITY's landed slice).
- The interim `d8a4c5603fe` additions (`proofs/quotient_*` wording and
  `calls/machine_self_call_recursion_rejected`) were repaired by
  intermediate main commits before the first green re-witness.

## Retirement disposition

The row's closure condition — the gate green on a witnessed commit — is
met. What this row does **not** cover stays with its owners:

- **Host scope.** The gate is one matrix row. The release contract still
  requires all eight gates on one clean commit across the four required
  hosts; the non-linux legs belong to the parent **RC-DIAGNOSTICS** row's
  matrix work, not this record.
- **Fresh-witness cadence.** The green runs above are per-revision
  filtered witnesses of a corpus that drifts as diagnostics improve;
  re-measurement is the live measurement draft's cadence, not a gate on
  this record. At this record's stamp the current tip (`d650f2e45ac0`
  through `8e6e213d6348`) cannot produce a fresh witness at all: the
  workspace does not compile — `04f2fdbb853` added
  `StructuralTypeShape::ElementView` without the
  `optimization-unit/.../structural_encoding.rs:379` match arm. That is
  a build pairing in flight on another lane, not a gate regression: no
  canary's behavior changed. The record is retired on the witnessed
  greens above.
- **Folded negative-case obligations** belong to
  **RC-SOURCE-SEMANTICS** per the measurement draft's own boundary.

This row is a measurement/disposition record — no fixture files, corpus
membership, or expected.txt fragments were modified by it.
