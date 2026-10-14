# Cyclic reference and claim rotation

`rotate_references` swaps exclusive references to disjoint projected fields on
ranked backedges. The caller checks both unchanged storage on the zero-iteration
path and the final writes after three iterations. `rotate_claims` passes a
linear owner through an ordinary structural call result on every iteration,
then swaps it with the other carried claim. Zero, odd and even counts check
that the selected return keeps the original claim's contents and that both
claims are disposed at the real exit.
`rotate_envelopes` repeats the call-result transfer with an affine sibling and
a reference to caller storage beside the linear child. Odd and even rotations
check each component and its owner association after the old result home retires.
Scalar returning guards run after both tokens are consumed. Ordinary and selected
Omega boundary helper bodies exercise both Boolean arms. A leading affine local
is consumed by the selected return call on one arm and implicitly disposed on
the other; the caller checks the actual returned values.

Native and Terminal execution must both exit 0. No Console or foreign binding
is involved; an incorrect referent or claim binding crashes with Trap.
The target roster additionally checks all four Terminal installations against
one unchanged artifact. Other targets still require their own native hosts.

```sh
omega run --both tests/omega/pass/control_flow/runtime_cyclic_reference_and_claim_rotation_exit/main.omg
python tools/corpus_gate.py --both --filter runtime_cyclic_reference_and_claim_rotation_exit
python tools/corpus_gate.py --filter cyclic_arrival_duplicates_exclusive_reference,cyclic_arrival_restores_consumed_field,conditional_scalar_return_preserves_live_claim
```

The invalid controls start with legitimate disjoint references and complete
linear custody. Only the backedge duplicates a loan or forwards the owner
after consuming its linear child. These rejections test safety independently
of whether the cycle declares a termination measure. A separate primitive
conditional-return control leaves its linear token unconsumed and must reject
that live debt.
