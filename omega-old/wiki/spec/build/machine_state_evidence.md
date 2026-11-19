# Machine-state realization evidence

The evaluated [call and state plans](calling_plans.md) are the published boundary
promise. Actual register and machine-state use is evidence about one provider
artifact, not an additional caller contract.

## Final-artifact obligation

Instruction selection, allocation and emission must honor the state ceiling.
It applies to the final program, including inlined or specialized code,
thunks, veneers, generated stubs and admitted indirect leaves.

```text
actual_transitive_footprint ⊆ permitted_transitive_use
actual_clobbers ∩ unsaved_interrupted_state = ∅
```

The backend accounts for each emitted instruction using its target specification
or checked-assembly catalog entry, including operand loads, stack, flags and
control transfers. Unknown instruction effects reject. Generated entry and exit
code must account for register reads and writes, stack and instruction-pointer
use, and the call to the intended body. The entry's ceiling covers this code
and its synchronous callees. Preservation depends on the clobbered subset.

The compiler may change allocation within those bounds. Contextual
specialization may produce a callee under a restricted ceiling without adding a
generic type application. Static and dynamic code obey the same limits. The
trust report distinguishes effects derived from checked code from claims
explicitly admitted for opaque providers.

<a id="certificate-and-replay"></a>
<a id="final-artifact-validation-placement"></a>

## Native emission and backend testing

The native backend enforces these constraints as it constructs the image.
For example, when encoding a branch it must resolve its target and select a
displacement that fits. Ordinary `omega build` and the compilation step of
`omega run` must publish the emitted bytes directly. Publication and native
artifact identity must be independent of a second pass over those bytes and
of any digest recording its result.

Backend regression tests check observable behavior or format rules against an
independent expectation. For example, a static Linux arm64 image must place
its load segments so a kernel with 64 KiB pages can map them separately.
The [ELF regression test](../../../omega-rust/omega/compilation/backend/images/image-elf/src/static_executable.rs#L332)
checks the emitted headers even on a host with smaller pages.
CI runs such regressions through the backend test suite. Keep a more elaborate
test helper only when it catches a specific defect a simpler assertion or
execution test would miss. Whole-image replay belongs outside the build's
validator options and is not a per-PR CI requirement.

A receiver that grants execute permission applies its own
[installation checks](executable_installation.md#placement-lifecycle) to the
actual placement.

## Exit realization

Return-control evidence must match `CallPlan::entry_control`; the restored-state
set must match `StatePlan`. An opaque provider supplies the accepted claim under
a root-reported trust receipt or adequate-hardware-isolation receipt. Missing,
unreported, or mismatched evidence rejects before forming provider execution.
