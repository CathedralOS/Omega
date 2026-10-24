# Explicit non-owning reference erasure

Fact: explicit non-owning reference erasure writes 73 to the original record
without minting routed membership.

Parent: `DOMAIN-ACCESS-AUTHORITY` in [TASKS.md](../../../../TASKS.md).
This isolates ordinary explicit erasure before implementing retained read/write
policy. No access clauses are declared here, so acceptance cannot establish
policy enforcement or establish post-mutation qualification restoration under
the [invariant-window rules](../../../../wiki/spec/language/dependent_values.md#invariant-windows).

The [root witness](main.omg) starts with an issued record, explicitly borrows it
under the bare carrier, writes 73 and observes the original field. It makes no
claim that `Issued` remains true after writing. [Build](build.omg) binds all four
hosted targets. Acceptance is:

```sh
omega run --both samples/cli/facts/non_owning_reference_erasure/main.omg
```

The fresh optimized baseline at
`67c52e168edf0dab9a95660adfec01b9fd4c4b89` refuses the unchanged root at
`main.omg:12:40`: "a qualification reference view must retain existing source
qualifications in BOTH directions for mutable storage; `as` cannot mint routed
provenance". `omega run --both` exits 200 before either engine executes.
The immutable producer digest is
`0597ba558223a3f3e0cdc8929fbf357f9cac57c57e7b2030fed10f8687bd47d8`;
build receipt, profile and raw refusal are retained under `build/`.

The nine places are `local`, `field`, `parameter`, `call_result`, `match_join`,
`const_name`, `other_state`, `other_machine`, and `target_body`. Each contains
`main.omg` and its `invalid/main.omg` twin. The twin changes the view contract
to the unrelated routed `Reissued` domain: erasure must not become issuance.
The const-name place supplies the same 73 through `WRITTEN`; it does not
fabricate a compile-time exclusive reference. The matching fixtures are under
`tests/omega/{pass,fail}/facts/non_owning_reference_erasure_through_<place>_*`.

Optimized execution reaches native and Terminal exit 0 in all nine
places on macOS ARM64. The previously frozen match join exposed a native
selection veto on qualified arrival metadata; full arrival identity and access
must be checked, not metadata absence. The pinned producer observes 14 native
and Terminal successes (root, nine places and four supplementary controls), and
25 reason-sensitive refusals. A fresh comparison of 864 fixtures against clean
main `873d5df486fe76efbbfd65e0b173bdf032bdc645` loses no outcomes or expected
fragments and gains 33. Pre-existing golden failures are not claimed green.
Producer metadata, raw observations, immutable products and comparison records
are retained in the owning qualification. Only nine unrelated discovery
`sample.json` files were untracked during that comparison; compiler, library,
corpus fixtures and harness inputs were unchanged at its recorded revision.

## Requirements and controls

- Establishment: `issue` establishes the exact incoming `Issued` value. An
  unrelated route cannot be introduced by casting the reference.
- Identity/lifetime: every store targets the caller's original storage; the
  caller observes 73 after the view and its transported aliases finish.
- Composition: the same erasure/store survives the nine transports. Additional
  input annotations are not synthesized normal-return promises. Explicit
  `ensures` remains checked, and no-write calls retain unaffected evidence.
- Soundness: original lender field/element declarations survive intermediate
  views and aliases. Controls under `fail/recast/non_owning_erasure_preserves_*`
  cover scalar and routed fields, local owners, stored/reborrowed aliases,
  projected views, arrays, and a helper replacing a qualified field with an
  unissued record. That helper must not resurrect its input annotation on return.
  The separate false-reestablishment control rejects an unproved `ensures`;
  authored re-establishment and unchanged-input controls execute successfully.
  Joined shared references cannot acquire store permission. Existing ordinary
  bounds, write-only observation, custody and loan controls remain applicable.
- Checked-to-Terminal Psi: ordinary affine reference descriptors and explicit
  referent paths carry the erased view; no runtime policy bits or new carrier
  layout is introduced. Actual interpreter execution observes the store.
- Layout/provider/ABI: one-field `u32` geometry and existing write-only pointer
  calling plans; all four hosted `ProgramEntry` bindings. Native observations
  here are macOS ARM64, not executions on unavailable hosts.
- Instruction/frame realization: original backing pointers survive calls,
  joined reference arrivals, and target-scoped bodies without granting reads.
- Encoding/native emission: direct emitted macOS ARM64 images return the
  authored 0 and agree with Terminal Psi. Corruption controls are checking
  obligations, not runtime observations.

The record-predicate and retained scalar-field controls exercise predicate
forgetting and ordinary storage invariants separately. This row does not claim
completion of every primitive-reference signature/lowering path. A separate
development scalar probe still encountered an existing unadmitted signature
shape; it is not counted as passing execution evidence.

Return to `domain_access_authority` and Cathedral after this prerequisite.
Domain-owned access enforcement and compiler qualification-restoration work
remain open under the existing invariant-window and provenance rules.
