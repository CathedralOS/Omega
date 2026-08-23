# Beta refinement reconstruction

This directory owns untrusted reconstruction of the Beta source-to-Alpha
artifact refinement obligation. `beta_symbolic.py` derives source meaning;
`alpha_symbolic.py` derives the meaning of the compiled Alpha tape;
`alpha_refinement_check.py` independently pins both derivations and asks the
low-rung proof kernel to check their equivalence. The curated
`refinement-samples/` and deterministic generators exercise that cross-rung
edge. See [`REFINEMENT.md`](REFINEMENT.md) for its exact claim and limits.

`symbolic_loop_check.py` remains the focused source-side check that pins Beta
loop summaries to executable reference meaning over concrete input grids. It is
refinement support, not Beta's canonical interpreter and not Alpha opcode
conformance.

The shared parser and concrete interpreter remain under
`bootstrap/rungs/beta/reference/`. Reconstruction may consume that meaning
surface, but it neither compiles Beta nor grants an artifact authority.
Support binaries are compiled with the persisted lattice-built `bc.tape`
through `bootstrap/rungs/beta/artifact_env.sh`; the refinement owner does not
rebuild or depend on the disposable Rust Beta producer.

Run `ownership-test.sh`, `symbolic-loops.sh`, `refinement.sh`, and
`refinement-cert-diamond.sh` from any working directory.

`bc-artifact-structure.alpha` is the first whole-artifact obligation checker
rooted below `bc`. It walks the reachable control-flow graph of the persisted
Alpha tape, permits jump-skipped inline data, and rejects unknown/truncated or
overlapping instructions plus invalid direct targets. Its focused gate includes
mutated negative controls and the exact tape-hole payload boundary. This closes
instruction framing and direct-control-target reconstruction only; memory
bounds, call/return discipline, complete stream semantics, and terminal-class
correspondence remain open.
