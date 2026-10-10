# ensures_policy_results

First sample pinning `ensures result ==` on folded policy
arithmetic: exact u8 sum, `in Wrapping`/`in Saturating`
operand casts, and a wrapped-shift mix — each ensures clause
observes the policy-selected fold.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
