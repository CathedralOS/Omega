# Independent completed-call result bounds

An independent completed call guarantees value == 7, so exact value - 7 is safe.
Every place uses that fact and operation. This row guards the contract checking
prerequisite discovered while advancing **CML4**; it does not close residual
cleanup acceptance.

The portable source family returns 7 in every member. Its wrapper publishes
`ensures result == 7`. Each invalid twin changes only the reserved member's body
to call that wrapper: the same conclusion would then depend on itself. Host
members, consuming subtraction and caller control flow remain identical. The
required rejection is `cannot prove ensures contract for exit from forwarded`.
Ordinary guarantees are partial correctness; nontermination alone is not a
reason to withhold a separately established promise.

| Place | Transport |
| --- | --- |
| [local](local/main.omg) | copied scalar |
| [field](field/main.omg) | record field |
| [parameter](parameter/main.omg) | authenticated argument requirement |
| [call result](call_result/main.omg) | direct completed call |
| [match join](match_join/main.omg) | either returned arm |
| [const name](const_name/main.omg) | the same bound named by `FLOOR` |
| [another state](other_state/main.omg) | checked state arrival |
| [another machine](other_machine/main.omg) | forwarding result contract |
| [another target body](target_body/main.omg) | four hosted consuming bodies |

Each place has `invalid/main.omg`. The root duplicates local. Every positive
binds all four hosted ProgramEntry targets, expects zero and must run with
`omega run --both`. The nine source-identical pass/fail pairs are pinned under
`tests/omega/pass/facts/independent_call_result_bound_through_<place>_exit` and
`tests/omega/fail/facts/independent_call_result_bound_through_<place>_rejected`
(nine each; the reject fixture is a byte-identical copy of `<place>/invalid/main.omg`).
A returned difference is tested only after the subtraction; that observation
does not supply its formation proof.

Requirements exercised:

- Establishment: an independently justified completed-result contract supplies
  the singleton used by Exact subtraction; no caller guard reconstructs it.
- Identity and lifetime: the row exercises capture and copying of the completed
  result. Exact call identity and mutation invalidation remain implementation
  obligations; these uniform singleton results do not independently distinguish
  different invocations or later writes.
- Composition: all nine transports carry the same bound into the subtraction.
- Soundness: every portable member participates in dependency checking. Copying
  result orderings into a synthesized return local cannot hide a citation cycle.
  Exact-edge induction, declared carrier ranges and body inference remain separate.
- Psi 05–07: proof decisions erase without changing the copied scalar,
  branch choice or outgoing result; Terminal execution checks that value.
- Omega 00–02: checked providers and ProgramEntry settle through the ordinary
  Build companion; integer and record layouts retain their declared ABI.
- Omega 03–06: subtraction, calls, state arguments, record homes and joins
  preserve the value across registers, activation frames and returns.
- Omega 07–09 and emission: encoded native control and result values must agree
  with Terminal execution. An intentionally wrong final oracle must trap in both.

Producer revisions, actual native/Terminal outcomes and invalid diagnostics belong
in the qualification record. Binding a hosted target does not establish execution
on that host; cross-host execution remains outside this row's local evidence.

On macOS ARM64, the root and all nine places execute at zero in both native and
Terminal engines. All nine circular-promise twins reject for the wrapper's
unproved result contract. The final-value corruption control traps in both.
