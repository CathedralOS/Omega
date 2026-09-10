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

Ordinary selected comparisons and floating Match arms share one Terminal
`IeeeFloatCompare` operation with six explicit relations. The scalar computation
graph evaluates the subject once and each reached pattern once; arm identity
and exact selected provider commitment survive alongside the operation.
Canonical verification and the Terminal interpreter implement both formats,
including NaNs and signed zeros. The ordinary native graph carries the same
comparison through Abstract, Target and Legalized operations, preserving exact
operation identity into selected instructions and its physical coverage child.
Reproduce the portable boundary with:

```text
mbx run -p omega -- inspect-terminal --machine choose --target macos_arm64 tests/omega/pass/expressions/match_float_patterns/main.omg
```

The same fixture has an ordinary free Unit application root; its original
`choose` and `identity` machines remain callable with runtime arguments.
The CLI requires normal project package-review acceptance; the native canary
supplies explicit reviewed fixture inputs without changing a developer's
acceptance records:

```text
mbx run -p omega -- --output-only --target macos_arm64 tests/omega/pass/expressions/match_float_patterns/main.omg
mbx nextest run -p compiler --test canary_suite -E 'test(float_match_native_publication)' --no-fail-fast
mbx nextest run -p omega-native-differential-test --test ieee_comparisons --no-fail-fast
```

Native comparison selection uses signed and unsigned ordering of the IEEE bit
encodings, explicitly excluding both NaN intervals and equating signed zeros.
It does not reinterpret an IEEE comparison as a mathematical equality fact.
The existing integer comparisons and Boolean materializers avoid dependence on
ambient FP controls, including flush-to-zero behavior for subnormals. This is a
correctness-first expansion, not an optimized hardware-comparison sequence.
Adding direct ISA comparison forms can reduce code size later, but must retain
the same NaN, zero, control and independent byte-replay obligations.

The checked-source interpreter consumes target-neutral comparison executions
retained by selected dispatch after exact intrinsic-plan validation. It rejoins
each record to the current root, generational arm, source operands, requirement
and selected plan commitment, then compares the saved subject and reached
pattern through `FloatSemantics`. It does not manufacture source expressions
or re-evaluate operands. Raw checked source without settled execution remains
rejected; wildcard-only dispatch invokes no equality. The integration cases
include both formats, NaNs, signed zeros, array projections, effectful subjects
and patterns, unselected trapping bodies, and substituted execution records:

```text
mbx nextest run -p compiler --test canary_suite --no-fail-fast -E 'test(float_match_checked_interpreter) | test(float_match_interpreter::)'
mbx nextest run -p checked-interpreter --test value_dispatch --no-fail-fast
```

This interpreter milestone does not close crash-qualified equality or
checked-adapter Match execution; those require their own arm-local custody.

Float call/return ABI transfers and block arguments use the ordinary scalar
graph, with exact floating register views at call boundaries and raw-bit local
transport. Provider selection alone is still not execution evidence: both the
direct and retained native entrances independently join authored comparison
occurrences to the selected intrinsic, then retain their exact boundary
application coverage through native publication. The lower receipt-coupled
ProgramEntry API still rejects comparison occurrence custody it cannot consume.
Physical comparison children cover every attributed instruction interval and
independently replayed private spill gaps, without absorbing another authored
operation or control edge. Their machine, object, and final-image bytes agree.
The [native comparison tests](../../../../tests/native-differential/tests/ieee_comparisons.rs)
exercise the unchanged call-bearing match and all six relations in both formats;
matching-host execution and cross-target byte replay remain separate checks.
The internal stack-argument test enters through one C register argument, then
uses generated ten-argument calls. Internal AAPCS64 transport is not a claim
that private symbols implement Darwin C's differently packed stack arguments.

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
