# Rewrite selected instructions

`optimize_selected_instructions` receives instruction selection's admitted
result, runs the requested rewrite phase, and publishes a
[`SelectedBody`](../../representations/selected-instructions/published) for
register allocation. An identity run still publishes completed liveness and
live ranges; a rewrite can also publish its allocation-legality facts.

The stage returns that representation and optional `SelectedOptimizationReplay`
evidence separately. Only the native driver and requested verification use the
evidence. Allocation does not link this stage implementation.

Start at [`src/selected_rewrites/selected_optimization.rs`](src/selected_rewrites/selected_optimization.rs).
The shared [`analyses/`](analyses) implementation computes liveness, ranges and
legality for rewrites and register-pressure recovery. Its private staged views
are analysis scratch; completed inter-stage facts belong to the representation.
