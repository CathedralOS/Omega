# Floating-point realization

[Numeric semantics](../../../../wiki/spec/language/numeric_values.md#floating-formats-and-operations)
defines the required behavior. This note identifies realization evidence and
current boundaries, not alternate floating semantics.

## Selected plans and target features

Core [float requirements](../../../../source/library/core/float_operations.omg)
have explicit native satisfiers for arithmetic, comparison, classification,
conversion, square root, min/max, negate, and multiply-then-add. Directed
add/subtract/multiply/divide/square-root satisfiers exist across the four native
profiles; AArch64 additionally realizes nearest and directed FMA. A directed
operation saves/restores controls around the single operation; source has no
ambient mode change.

Generic x86-64 remains SSE2-baseline. The opt-in `AvxFma3` deployment joins one
exact profile and canonical AVX+FMA3 requirement to register-only
`VFMADD132SS`/`VFMADD132SD`. Both format slots retain raw-bit cancellation
receipts distinguishing fused from separately rounded results. An encoder or
checked build selection alone is not hardware execution evidence.

[x86_fma_plan_association.rs](src/pipeline/x86_fma_plan_association.rs) joins the
complete selected `ProviderPlan`, compiler-intrinsic provenance, exact requirement,
format/slot, and admitted profile. Repeated uses may deduplicate the selected
association but retain distinct operation occurrences. Non-x86, cross-profile,
wrong-format, and multiply-then-add substitution cannot borrow FMA admission.

## Operation and control custody

A supported source lane carries independent landed-literal FMA locals through
Terminal raw-bit constants and FMA operations. Exact per-occurrence proposals
cover the source-ordered Terminal operation roster one-to-one. The ordinary
Abstract/Target/Assigned/machine pipeline retains the selected plan and admitted
provider; assignment owns XMM homes. Wider shapes require their own checked
correspondence and cannot silently fall into an optimized-direct entrance.

The x86 FMA function envelope saves complete MXCSR, installs `0x1f80`
(nearest-even, masked exceptions, FTZ/DAZ disabled), and restores incoming state.
Object decoding independently checks operand loads, controls, FMA bytes, and
instruction intervals; final native replay rejoins the canonical Terminal graph
and selected plan. Permitted internal Unit calls remain within the function
envelope. A supported returning foreign leaf uses its own nested envelope.

Returning foreign calls preserve complete MXCSR or AArch64 FPCR around argument
setup/call and restore it after outbound-stack release, before scalar result
normalization. Sequential calls can reuse an aligned frame slot while retaining
distinct ordered instruction intervals. AArch64 uses an eight-byte slot with
exact `MRS`/`STR` and `LDR`/`MSR` sequences. Direct syscalls do not acquire this
returning-foreign envelope. Callback entry canonicalization/restoration remains
separate work; a calling-policy predicate is not execution evidence.

## Differential coverage

The [float canaries](tests/canary_suite/float_plans_and_policies.rs) and
[proof/float suites](tests/canary_suite/proof_and_float_suites.rs) retain selected
provider identities, semantic edge observations, explicit target roots, and
reproducible images. x86 baseline receipts do not include fused/directed
operations merely because other selected cohorts exercise them. AArch64
receipts distinguish fused/unfused and directed behavior.

Only execution on a matching host supplies the native execution leg; ELF or
PE/COFF construction and repeated identical images are different evidence.
Broader source/control flow, callback controls, and independently realized
software/feature-qualified alternatives require their own end-to-end custody.
