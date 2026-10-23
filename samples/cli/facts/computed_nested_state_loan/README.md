# Computed nested state loan

A computed nested leaf loan captures each selector once before the later argument and preserves caller storage through a state transfer.

Parent: **PLACED-ACCESS-NATIVE-OPS**. This row establishes exact-once evaluation of a computed nested borrowed argument on a named-state edge when selector contracts fix its coordinates. Broader indexed access and foreign-host acceptance remain open.

The root loans `rows[choose_row(cursor)].data[choose_column(cursor)]` into `apply`, followed by `replacement(cursor)`. The selectors promise 2 and 1 and increment a counter. `apply` stores 42 through the loan. Exit 0 requires the selected caller cell to be 42, its neighbor zero, its tag 7, and the counter 3. All nine places retain the same selector contracts, extents (3 rows, 2 columns), and operation; they additionally require effect trace 123 and preserve an initialized adjacent cell. The match variant executes both Boolean arms.

The earlier cloud worktree was unavailable on resume. The root was reconstructed from its preserved semantic checkpoint and frozen before compiler edits. On current baseline `d6cf554fd1cbf20551983d29bae26f5cf3b81664`, Linux x86-64, the root and all nine places report native signal 4 and Terminal `Trap` (CLI 255). A separate diagnostic root with only the counter oracle changed to 1 exits 0 in both engines, proving the write and neighbors are correct while both selectors are skipped. The authored positive oracle remains 3. The initial integration to `87ec9c7603` changed tools, fixtures and goldens, with no Rust/compiler changes.

Canonical fixed coordinates had displaced retained reference formation. The repair gives evaluated operands precedence over storage-only state transfer, reusing ordinary reference establishment and checked backing/lifetime provenance. The borrowed-arrival exemption in target custody replay follows the exact captured parent occurrence on every incoming borrowed source. Equal backing supplies no such exemption; unknown predecessors, access restrictions, descendant interference and lifetime debts remain checked.

## Places and controls

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [local/main.omg](local/main.omg) | [invalid](local/invalid/main.omg) |
| field | [field/main.omg](field/main.omg) | [invalid](field/invalid/main.omg) |
| parameter | [parameter/main.omg](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| call_result | [call_result/main.omg](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| match_join | [match_join/main.omg](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| const_name | [const_name/main.omg](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| other_state | [other_state/main.omg](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| other_machine | [other_machine/main.omg](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| target_body | [target_body/main.omg](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

Each twin changes only `&mut rows[...]` to `&rows[...]` at the state argument. It must reject with `but the caller lends only immutable access`; setup or unrelated rejections do not count. Mirrored fixtures are under `tests/omega/{pass,fail}/facts/computed_nested_state_loan_through_*`.

```sh
omega run --both samples/cli/facts/computed_nested_state_loan/main.omg
python3 tools/sample_native_gate.py --facts --filter computed_nested_state_loan
python3 tools/corpus_gate.py --both --filter computed_nested_state_loan
python3 tools/corpus_gate.py --filter computed_nested_state_loan
```

## Requirements and evidence

- **Establishment:** exact selector result contracts establish in-bounds coordinates before loan formation. Their effectful computations still execute. Each place carries the selector inputs' exact facts into the operation without a post-hoc caller guard.
- **Identity and lifetime:** the receiving state writes the selected original caller cell; neighbors and tag remain unchanged. Checked backing and parent relationships retain its lifetime.
- **Composition:** local bindings, record fields, parameters, contracted call results, both match arms, named constants, another state, a forwarding machine and four target-scoped bodies retain the same operation and facts.
- **Soundness boundaries:** each authority-only twin rejects. Exact occurrence identity, access weakening and unknown/mixed incoming predecessors have focused target-custody controls; equal backing alone is insufficient.
- **Checked-to-Terminal Psi (`Psi 05–07`):** retained reference formation evaluates selectors once before the later argument and transports the captured descriptor. Actual Terminal exit 0 and counter/trace agreement are required.
- **Providers, geometry and calling plans (`Omega 00–02`):** companions bind all four hosted targets. Nested array/record geometry and state argument placement retain the selected address; target declarations are not foreign-host execution evidence.
- **Instructions, homes and frames (`Omega 03–06`):** actual Linux x86-64 execution exercises selector calls, captured address storage and receiving-state writeback with live effects and disjoint neighbors. Corpus execution exercises independent target custody replay.
- **Encoding and image (`Omega 07–09`):** the emitted Linux x86-64 image must actually exit 0 and agree with Terminal. Compile-only output is insufficient.

Canonical authority: [evaluation schedule](../../../../wiki/spec/language/expressions.md#evaluation-schedule), [captured places](../../../../wiki/spec/terminal-psi/loans.md#captured-places-and-available-premises) and [referent identity](../../../../wiki/spec/terminal-psi/structural_access.md#referent-identity). Producer identities, exact commands and raw outcomes are retained in the task's qualification evidence. Windows x86-64, macOS ARM64 and Linux ARM64 execution are unverified here.
