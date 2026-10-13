# proof_order_fact

'Machines as proofs': empty-bodied `distinct_indices` discharges `ensures i != j` from `requires i < j` via built-in order rules — pure proof artifact, no runtime code.

Compile-checked only: no standard-library dependency and
no \`Main::exit\` — run a check target, not native.
