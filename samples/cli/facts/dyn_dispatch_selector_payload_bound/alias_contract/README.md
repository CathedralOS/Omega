# A named public result bound

These examples use a named mathematical predicate to state the same public
`result <= 88u64` guarantee as the [parent production](../README.md). The dynamic
selector still chooses the executable implementation; the predicate only
justifies the later `88u64 - result` operation.

Fact: Dispatching a trait method through a dynamic trait carrier (dyn / trait conformance) whose contract establishes result <= 88u64 preserves the return bound across the selector and data lanes, discharging exact subtraction 88u64 - result.

`within_payload_bound(value, ceiling)` is a transparent checked definition of
`value <= ceiling`, following [named mathematical definitions](../../../../../wiki/spec/proofs/mathematical_bindings.md#named-mathematical-definitions).
The implementation members publish ordinary integer
comparisons. Every positive calls both implementations with inputs 10 and 20
and checks differences 50, 60, 40 and 30. Both match branches therefore have
distinct observable results. The target place uses ordinary named
implementations and four explicit Build choices, with architecture tags 17/29
that detect a swapped selection.

The root and all nine places retain four hosted entries and expected exit 0 in
both engines. Each twin changes only the published bound to 89 in the requirement
and both selected member signatures. The executable bodies and runtime inputs
stay unchanged. That weaker promise must reject at exact subtraction.
The [parent table](../README.md) links every positive and reject twin.

**BUILD-OWNED-TARGET-SELECTION** owns this source migration;
**RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY** owns preservation of the named
public result guarantee and the exact selector through checking and execution.
This is a second contract production of the existing fact, not another row.
The parent's requirements cover establishment, exact inputs and selections,
composition, proof boundaries, Terminal Psi, provider/layout/calling plans,
instructions and actual native execution here as well.

Earlier baseline `e06334c63dc4a811b0bcf9f530cbc91b14b23eac`, using optimized
producer `b3a895c26149208c72bfa1f186e0c26bee9486ca59746de925d0f7fb5f8f5520`,
refused the local subtraction after losing the public predicate. The earlier
repair's Linux observations and existing fixture goldens remain historical;
they do not validate these changed programs. The current focused run covers all
nineteen projects in this production alongside the nineteen parent projects.
Fresh migration results are pending in the parent claim's retained evidence.

Matching fixture pairs remain under
`tests/omega/{pass,fail}/facts/dyn_dispatch_payload_alias_through_*`.
Broader operational contracts, product parity and other-host execution remain
separate parent acceptance.
