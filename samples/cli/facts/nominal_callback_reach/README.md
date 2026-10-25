# nominal_callback_reach

A nominal `satisfies`-bound machine binder contributes its selected callback's
reach row and exact result through a generic traversal. `walk<Step>` binds
`Step` to `bump` under `StepContract::step`; the same fact is transported
through all nine places: the callback's value arrives as a local binding, a
record field, a callee parameter, a call result, a match join arm, a const-named
argument, another state, another machine, and per-target caller bodies.

Parent task: STATIC-CALLBACK-REACH-RETENTION — the closed reach application
retains the original binder telescope, the nominal requirement identity, the
selected contract commitment, and the exact emitted call record through
lowering, codec reload, interpretation and native execution, including when
the selected callee is inlined or absent from the emitted machine set.

Outer command:

    omega run --both samples/cli/facts/nominal_callback_reach/<place>/main.omg

Reject twins (`<place>/invalid/`): `walk<noise>` selects a machine without
`satisfies StepContract::step`, so checking rejects the unsatisfied binder.
