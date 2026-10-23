# Additional input qualification versus stored invariant

Fact: an additional `Before` input qualification permits writing 73 without
promising `Before` on return. Every place starts with the same `Cell.value == 0`
entry fact, calls the same qualified write-only update, and observes 73 afterward.
The comparison is a runtime oracle, not a guard supplying the input fact.

Parent: `DOMAIN-ACCESS-AUTHORITY` in [TASKS.md](../../../../TASKS.md), specifically
the ordinary validity and loan contracts required by its controlled writer.
This row establishes the prerequisite distinction, not access-policy enforcement.
The governing rules are [qualified input admission](../../../../wiki/spec/language/domains.md#declaration-and-membership),
[mutable arguments and field defaults](../../../../wiki/spec/language/dependent_values.md#mutable-receivers-and-declared-field-domains)
and [write-only authority](../../../../wiki/spec/terminal-psi/structural_access.md#write-only-authority).

## Witnesses and controls

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [source](local/main.omg) | [source](local/invalid/main.omg) |
| Field | [source](field/main.omg) | [source](field/invalid/main.omg) |
| Parameter | [source](parameter/main.omg) | [source](parameter/invalid/main.omg) |
| Call result | [source](call_result/main.omg) | [source](call_result/invalid/main.omg) |
| Match join | [source](match_join/main.omg) | [source](match_join/invalid/main.omg) |
| Const name | [source](const_name/main.omg) | [source](const_name/invalid/main.omg) |
| Another state | [source](other_state/main.omg) | [source](other_state/invalid/main.omg) |
| Another machine | [source](other_machine/main.omg) | [source](other_machine/invalid/main.omg) |
| Target body | [source](target_body/main.omg) | [source](target_body/invalid/main.omg) |

Each twin changes only `overwrite`'s contract by adding `ensures cell in Before`.
It must reject that promise after writing 73, rather than fail parsing, entry
admission, carrier matching or ordinary borrowing. The extra
[false-entry control](invalid_entry/main.omg) changes only initial value 0 to 1
and must reject `overwrite`'s input requirement. Corresponding fixtures live in
`tests/omega/{pass,fail}/facts/additional_input_domain_mutation_*`.
The separate `fail/domains/write_only_declared_field_domain_requires_restoration`
fixture protects a stored `Holder.cell: Cell in Before`: that default still
requires restoration on normal return.

Acceptance for each positive is `omega run --both <place>/main.omg`, exit 0.
Build binds all four hosted targets; native observations are host-specific.

## Evidence and requirements

Initial optimized producer
`ec7f75640e2e0eed5d6558889ef487ca7f4e2d1f` refused the local source's qualified
record projection and its otherwise-provable write-only input admission. A
geometry-only repair exposed the mistaken implicit root handback promise.
Fixed optimized producer `4bbcf73a381bcf10ca6cd24754f4ff161c8b20f8`
(SHA-256 `ed4bf655031c68033e26e11f075794c61253d77ac5ab5c613928a780297f463a`,
opt-level 1) observes native 0 and Terminal Psi 0 in all nine places on macOS
ARM64. Nine explicit-postcondition twins reject `ensures`; initial value 1
rejects `requires`. Declared-field corruption, including a bare-helper route,
still rejects restoration; invalid stored-reference qualification rejects
construction, and qualified write-only field observation remains forbidden.
Retained native images and Terminal artifacts are hashed with the producer-bound
qualification records. The 153-fixture comparison against clean baseline
`7276fbb60b5cf17423e4978f299e22fdd5ba68ba` lost no outcomes; after integration,
155 relevant fixtures produced 70 checked and 85 expected rejections with no
lost success or wrong rejection reason. Raw execution records retain their
actual producer provenance. Compiler-dependent golden rows will be recorded
in a scoped follow-up after this compiler is published.

- Establishment: the actual source's zero field proves `Before` at the exact
  call, not the formal annotation. Initial value 1 fails that admission.
- Identity and lifetime: write-only references address the caller's original
  record, which the caller observes after loan completion. No stale `Before`
  fact is republished from the static parameter type.
- Composition: constructor, field, parameter, call, join, constant, state,
  helper and target-scoped transport carry the same entry obligation. Explicit
  postconditions remain obligations; declared storage defaults remain distinct.
- Soundness: no write-only read, default-field corruption, semantic erasure,
  conserved-claim disposal or access-policy privilege is supplied by comparing
  storage identities. Ordinary admission and original-storage restoration remain
  separate checks; the controls above exercise their failure boundaries.
- Checked-to-Terminal Psi: the same normal write and call plans execute in the
  interpreter. The `Before` precondition is checked rather than made into an
  invented return certificate. No new runtime metadata is introduced.
- Layout/provider/ABI: existing `u32` record geometry and write-only pointer
  arguments; all four hosted `ProgramEntry` bindings. Other hosts are not yet
  execution observations for this change.
- Instruction selection/frame realization: ordinary field stores, helper calls,
  restored loans and caller loads; no special case for this carrier or value.
- Encoding/native emission: actual emitted macOS ARM64 programs return 0 and
  agree with Terminal Psi. Other targets retain their authored bindings, not a
  claim of execution on a different host.

Return to the separate `domain_access_authority` fact and Cathedral after this
prerequisite. Nothing here establishes the full domain-policy acceptance or a
new rule preserving routed provenance through mutation.
