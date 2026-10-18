# Guarded record and line loans

A selected Complete or End edge retains the mutable Application record and shared 64-byte line loans, so storing count in application.ledger.count and reading line[0] observes 37 + 5 = 42.

Every positive and reject twin copies that fact sentence. The bound is 64 and
the operation is the original record store followed by the byte-plus-record
read. The original package-only root probe is unchanged; it is checked, not
counted as native execution. Each new application exits 0 only if both the
Complete and End deliveries observe 42. Before each invocation, the caller
sets its record count to 99; after each return, it must observe count 5 in the
original record. Full and Invalid return false.

| Place | Positive | Distinguishing delivery |
| --- | --- | --- |
| local | `local/main.omg` | caller-local record and state-local line owners |
| field | `field/main.omg` | the line owner is a Holder field |
| parameter | `parameter/main.omg` | an extra ordinary invocation forwards the mutable Application parameter |
| call result | `call_result/main.omg` | the shared line reference is returned from a lifetime-related call |
| match join | `match_join/main.omg` | both match arms yield the shared line reference |
| const name | `const_name/main.omg` | named INDEX is exactly zero for the same write and read |
| another state | `other_state/main.omg` | an extra state transports both references |
| another machine | `other_machine/main.omg` | a separate machine performs the same record store and read |
| another target body | `target_body/main.omg` | all four hosted target bodies perform that operation |

Every positive has its reject twin at
`<place>/controls/shared_record/main.omg`. The twin consistently attenuates
only the Application loan from mutable to shared while retaining the same
storage, selected case edges, bound, store and read. The expected diagnostic
is `assignment cannot write application because it is not mutable in this
state` (with the compiler's backticks). A signature mismatch, proof refusal,
package failure or native-production failure does not satisfy the control.

Run the nine applications and their controls with:

```sh
python3 tools/sample_native_gate.py --filter cli/facts/guarded_record_line_loan
```

The focused regression fixtures are
`tests/omega/{pass,fail}/facts/guarded_record_line_loan_through_*`.
Their positive native and interpreter oracles are exit 0; their negative
oracles identify the rejected assignment. Use the `guarded_record_line_loan`
corpus selector. Cross-target source checking is distinct from matching-host
native execution; a Linux x86-64 observation does not claim execution on the
other three hosts.

The existing rules are [borrow access](../../../../wiki/spec/language/ownership.md#borrows-and-aliases),
[carried-loan transport](../../../../wiki/spec/language/lifetimes.md#carried-loan-transport),
and [state arrivals](../../../../wiki/spec/language/state_contracts.md#bindings-and-arrivals).
No language rule is added. The compiler now resolves logical local-borrow roots
through the same current-production query as physical local names. State loan
chains retain backing homes by their checked successor membership, including
when an owned reference descriptor carries the address; cleanup releases those
carriers before the backing. This replaces a direct-argument-place recognizer
that discarded the field owner on the lending edge. The independent retained
audit originally reported `ReferenceDisposalConflict` for the field witness;
the witness and the audit remain unchanged across the repair.
