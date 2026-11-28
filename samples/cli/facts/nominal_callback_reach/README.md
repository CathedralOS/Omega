# nominal_callback_reach

The parameter `const Step: StepContract::step` carries the selected callback's
reach and result through a generic traversal. `walk::<bump>` selects the
implementation. The nine cases carry the callback's value through a local
binding, a record field, a callee parameter, a call result, a match join arm,
a const-named argument, another state, another machine, and per-target caller bodies.

Parent task: STATIC-CALLBACK-REACH-RETENTION — the closed reach application
retains the original binder telescope, the nominal requirement identity, the
selected contract commitment, and the exact emitted call record through
lowering, codec reload, interpretation and native execution, including when
the selected callee is inlined or absent from the emitted machine set.

Outer command:

    omega run samples/cli/facts/nominal_callback_reach/<place>/main.omg

Reject twins (`<place>/invalid/`): `walk<noise>` selects a machine without
`satisfies StepContract::step`, so checking rejects the unsatisfied binder.

## Build-owned target-body migration

The `target_body` place and its twin carry no source target prefix.
`BUILD-OWNED-TARGET-SELECTION` replaces the prefixed family with ordinary named
implementations (`X86...` and `Arm...`) of a boundary-trait requirement, selected
by explicit target-keyed Build rows. The operation, inputs, contracts and exit
oracle are unchanged. The entry wrapper and each implementation check a tag
(17 for x86-64, 29 for AArch64), so a wrong Build row traps. Observed evidence is
Linux x86-64 native and Terminal Psi agreement for the positive and source
rejection for the twin; the other hosted targets keep their authored rows,
unexecuted here.
