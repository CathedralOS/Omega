# Whole record arguments in declared calls

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../../../../../TASKS.md).

> A private helper preserves the invocation rank on a tail edge, and every complete cycle strictly descends while advancing the explicit counter once.

The measure calls ordinary `distance(value: Progress)` instead of reading its
fields directly. Its public `requires lower <= upper` forms before its public
result equation can enter the proof. The complete captured record remains
separate from the scalar projections demanded by that contract. Runtime lowering
copies the actual owned input through the ordinary structural argument plan;
it does not reconstruct a record from those projections.

Seven iterations advance the caller counter from 3 to 10. Equal endpoints do
not advance it. Both paths check canary 641 and endpoint equality. Every twin
changes only the helper increment from 1 to 0.

| Place | Positive | Nondecreasing twin |
| --- | --- | --- |
| local | [source](../../../../local/declared_measure/computed_record/call/main.omg) | [source](../../../../local/declared_measure/computed_record/call/invalid/main.omg) |
| field | [source](../../../../field/declared_measure/computed_record/call/main.omg) | [source](../../../../field/declared_measure/computed_record/call/invalid/main.omg) |
| parameter | [source](main.omg) | [source](invalid/main.omg) |
| call_result | [source](../../../../call_result/declared_measure/computed_record/call/main.omg) | [source](../../../../call_result/declared_measure/computed_record/call/invalid/main.omg) |
| match_join | [source](../../../../match_join/declared_measure/computed_record/call/main.omg) | [source](../../../../match_join/declared_measure/computed_record/call/invalid/main.omg) |
| const_name | [source](../../../../const_name/declared_measure/computed_record/call/main.omg) | [source](../../../../const_name/declared_measure/computed_record/call/invalid/main.omg) |
| other_state | [source](../../../../other_state/declared_measure/computed_record/call/main.omg) | [source](../../../../other_state/declared_measure/computed_record/call/invalid/main.omg) |
| other_machine | [source](../../../../other_machine/declared_measure/computed_record/call/main.omg) | [source](../../../../other_machine/declared_measure/computed_record/call/invalid/main.omg) |
| target_body | [source](../../../../target_body/declared_measure/computed_record/call/main.omg) | [source](../../../../target_body/declared_measure/computed_record/call/invalid/main.omg) |

The [nested public-call composition](controls/nested_call/main.omg) gives the
record-taking helper an ordinary scalar helper, with its own public contract.
The [missing-formation control](controls/missing_formation/main.omg) removes the
call's independently required ordering; the desired rank and the callee's
result equation cannot establish that input requirement.

## Requirements and evidence

The fixed baseline is `b9f7b5f007015207843632512dbf4311bf36b3d9`. Its retained
optimized producer was built at `3f3146d64af80e1a5ef25b6df4be285b37f88dba`,
with identical compiler source bytes; CLI SHA256
`bf0368b7202c9075b472b9f96a84a1d197420e8a3bb8afe86af393fdd84d3ea2`.
It wrongly refuses all nine positives for missing independent entry formation.
Fresh macOS ARM64 observations under `build/record-call/row-results.json` show
all nine positives exiting 0 in both engines and all nine twins rejecting strict
cycle descent. The nested-call composition also exits 0 in both engines; its
twin and the missing-formation control reject. Actual producer digests, source
hashes, commands and output remain in those records and the publication evidence.

- Establishment: the selected public precondition is proved on independently
  captured actuals before result bounds or guarantees are installed.
- Identity and lifetime: one whole typed snapshot has its own source scope;
  coordinate-keyed scalar reads neither replace it nor authorize storage access.
  Every completed arrival supplies a fresh complete input. Saved denotations
  preserve their original values while importing their complete operand graphs.
- Composition: each private edge compares independently produced ranks under its
  exact current arguments. Public contracts retain their declaration namespace;
  their expression handles never become caller-owned field bindings.
- Soundness: nominal type, numeric policy, formation, pure/total call admission,
  copy authority and strict descent remain independent. Missing ordering rejects.
- Psi 06–08: checking retains the whole runtime input beside scalar read bindings.
  The declaration call uses its own checked origin and an admitted owned-copy
  receipt; statement loans or temporary captures cannot be fabricated. The
  ordinary call scheduler and structural argument lowering consume that receipt.
- Omega 00–02: existing record layout and calling plans carry the actual record,
  endpoints and canary. Caller and captured-state parameter order are independent;
  the shared owned-home resolver honors completed formal-to-actual bindings.
- Omega 03–06: caller-visible counter and canary checks expose clobbered homes or
  incorrect cycle lowering. Wider activation/WCSU acceptance remains open.
- Omega 07–09: every positive requires native/Terminal agreement at exit 0.
  Pins are `facts/nested_machine_joint_rank_record_call_<place>_{exit,rejected}`
  plus the nested-public-call and missing-formation controls.

The runtime receipt covers borrow-free owned copies whose source and result are
unrestricted. It preserves ordinary callee cleanup. Inputs requiring loans,
affine/linear transfers need their own actual custody plans; this row supplies
no evidence for them. The [indexed-input continuation](indexed/README.md)
retains whole-root selector bounds and disjoint current fields across a
selected store, with its own nine places and refusal controls. All programs bind
four hosted targets; execution on other hosts and broader parent acceptance
remain unverified.
