# Operand effects and restored reference slots

Owner: **NESTED-MACHINE-UNIFICATION**, [remaining acceptance](../../../../../../../../../TASKS.md).

The same private-cycle fact survives an effectful decrement operand. `step`
saves the selected payload reference, exchanges two descriptor slots, then
restores their original references before returning exactly one. The pending
scalar store decrements the original payload. An observable audit increases
once per iteration; the spare payload, both guards and the carrier canary stay
intact. Seven iterations advance the caller's counter from 3 to 10; the empty
path preserves 10 and performs no audit writes.

Every place has an `operand_effects/` child beside its existing stored-reference
production. Its `invalid/` changes only the operand's exact result contract and
returned value from one to zero. Descriptor writes and restoration are identical;
the resulting preserving cycle must reject its rank obligation. The corresponding
fixtures are `tests/omega/{pass,fail}/facts/nested_machine_joint_rank_operand_effects_*`.

The fixed baseline is `c2e8bc13ca91f0181762f058596439091ff602d4`. The unchanged
local program refuses because `step` has no complete normal-exit backing
relation. Inspection identifies a second load of a saved reference through its
subsequently replaced descriptor slot. Completing each load once repairs the
relation; complete operand effects then distinguish exact restoration from a
changed rank subject. A write footprint alone proves neither.

The source contracts are [assignment scheduling](../../../../../../../../../wiki/spec/language/expressions.md#evaluation-schedule),
[subject identity](../../../../../../../../../wiki/spec/language/state_contracts.md#mutation-and-subject-identity),
[rank preservation](../../../../../../../../../wiki/spec/language/termination.md#subject-preservation)
and [original referents and incoming rosters](../../../../../../../../../wiki/spec/terminal-psi/structural_access.md#referent-identity).
The positive restores the original descriptor roster; swapping equal-typed
references without restoring it is not this acceptance.

- Establishment: `step` establishes its exact returned scalar; the positive
  subtracts it under the helper's nonzero arrival contract. No caller equality
  guard supplies the proof after the operation.
- Identity and lifetime: saved references retain their captured bindings;
  each newly appended pointer projection needs its own load. A completed
  singleton slot relation can prove restoration; unknown, coarse, debt-only
  or uncorrelated alternatives cannot.
- Composition: operand writes invalidate current observations before the store;
  publication requires the current coordinate to name the captured destination.
  All recurrent edges and normal returns retain their ordinary obligations.
- Soundness boundaries: rank evidence grants no reference access, loan closure
  or restoration. The zero-result twins still reject complete-cycle descent.
- Psi 05–07: the programs retain the operand's calls, slot stores, reference
  captures and terminal cycle through Terminal Psi and its interpreter.
- Omega 00–02: all four hosted targets have authored ProgramEntry bindings;
  ordinary call plans retain original borrowed backing and descriptor words.
- Omega 03–06: native execution uses the existing reference, register-home and
  frame paths; this row adds no stack bound or new WCSU claim.
- Omega 07–09: the counter, audit, spare and guard oracle makes omitted calls,
  wrong descriptor copies or wrong stores observable. Runtime evidence here is
  macOS ARM64; other hosted native engines remain unmeasured.

Run each positive with `omega run --both <place>/main.omg`; it declares exit 0.
Keep exact producer digests and engine observations in the qualification record.
The broader parent remains open for indexed/case private coordinates, further
independent reference loads, other callee obligations, portable resource
qualification, source migration and the product compiler.
