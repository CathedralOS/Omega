# Ordinary numeric provider override

This is a regression control for `BUILD-OWNED-TARGET-SELECTION`: Source checking
retains the Product occurrence's Core meaning when Build explicitly selects an
ordinary implementation of a public numeric requirement. It adds no new fact row
and does not close the parent task.

[The positive](main.omg) selects `CheckedNan` for all five hosted targets. Its
unchanged public guarantee uses `F32::meaning`; its body computes `value != value`.
The entry requires the selected provider to return false for a finite value and
true for NaN. [The reject twin](../../../fail/build/ordinary_numeric_provider_override/main.omg)
changes only that provider body to `false` and must fail its guarantee.

[The guarded helper](guarded_helper/main.omg) additionally checks public result
equation composition under the original scalar entry precondition. Its
[partial-helper control](../../../fail/build/ordinary_numeric_provider_override/partial_helper_own_goal/main.omg)
omits that precondition and must fail call formation; an ensures goal cannot
establish the precondition of its own partial operand. No proof uses a Source
body unfolding or a post-hoc caller guard.

[The invocation control](../../../fail/build/ordinary_numeric_provider_override/provider_invocation/main.omg)
adds an ordinary `Input` call to the selected numeric provider and must fail the
public numeric requirement's empty invocation ceiling. The
[mutable-home control](../../../fail/build/ordinary_numeric_provider_override/stale_field/main.omg)
changes a referenced field before consuming it through a partial helper; its
entry bound cannot justify the helper's post-write precondition.

The frozen original was wrongly refused by producer
`d76c76eb8e2cb5896fb6a837e73d59339cc63595` with 608 unrelated ambiguous Core
operator diagnostics. Producer `bad7372331fa46b28b64408e21dfc5376d31e8d6`
(optimized CLI SHA-256
`132bafa01148b46ac71ee6a1a60e80f9c6c06506a5e7276257a094d72697fd7c`)
checks the unchanged original and rejects both intended invalid controls; its
finite and NaN witnesses each run with native and Terminal Psi exit 0 on
`macos_arm64`. Fresh pin observations are retained with the parent qualification
under `build/target-retirement/ordinary-numeric-override`.
