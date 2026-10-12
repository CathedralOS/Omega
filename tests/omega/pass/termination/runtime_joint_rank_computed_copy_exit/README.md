The count entry transports remaining and remaining + 0 into a named state.
The selected copy preserves rank across the call to step; step decrements it
under n > 0 before calling count again. Every joint cycle therefore contains
strict descent, and the ordinary arrival requirements establish the
ranged member's endpoints. Count needs no explicit rank window.

The original Main machine bodies are preserved from the former
rank_range_call_computed_copy_unproven failure fixture. The ordinary program
entry invokes Main::main and crashes Trap on a result other than zero. The
adjacent Build selects that entry for each hosted target.

Run `omega run --both main.omg`; native and Terminal must both exit zero.

The four `u32 [0..=9]` parameters from
`88d4c1e4f77:tests/omega/fail/termination/rank_range_call_computed_copy_unproven/main.omg`
are `Main::count.remaining`, `pair.left`, `pair.right`, and `Main::step.n`.
Each retains its plain unsigned carrier and an own-arrival `requires <= 9`;
the two state parameters share a conjunction. Their zero floors follow from
`u32`. Count's unbounded natural witness and step's explicit rank window are
unchanged. The full authored program still returns zero in both engines.

The neighboring `rank_range_declared_step_unbounded` retains three
`u32 [0..=9]` declarations from `c788455baa2`: `walk.remaining`, `pair.live`
and `pair.saved`, now arrival requirements. Its unconstrained stride permits
zero, so the recurrent subtraction cannot establish strict descent. Its old
expected fragment already missed that diagnostic on the migration base; the
control now pins the actual strict-decrease refusal without changing its body
or granting a positive stride premise. Supplying ten to count separately
refuses its ordinary arrival requirement; that probe is restored.
