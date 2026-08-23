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
mutated negative controls and the exact tape-hole payload boundary. It also
reconstructs ordered procedure regions from direct-call targets and proves the
static call/return shape: entry zero is the unique root, only that root halts,
every callee region has a return, call continuations stay in the caller, and
non-call edges cannot cross procedure boundaries. This closes instruction
framing, direct-control-target reconstruction, and static call/return nesting.
Dynamic call-depth bounds, data-stack and memory bounds, complete stream
semantics, cyclic progress, and terminal-class correspondence remain open.

`bc-block-control.alpha` and `bc-effect-sites.alpha` form the next whole-artifact
slice. The gate concatenates those responsibility-specific modules into one
Alpha checker, so the effect checks reuse the exact source scan, block table,
tape bytes, and decoded-instruction coverage rather than trusting a second
private reconstruction. The checker resolves the source's procedure-local
entry/state/`to`/`when` graph and validates canonical block/transition locations
and successor shapes. It also binds 612 lexical source effect sites—309 ordinary
calls, two reads, five writes, 113 fixed-string emits, and 183 explicit
returns—to their exact artifact instructions. The emit check decodes and compares
all 829 literal bytes and validates the exact jump/address/length/helper macro.
Compiler-synthesized prelude, helper, and 70 fallthrough epilogues account for
the remainder, so every one of the artifact's 423 calls, two reads, six writes,
254 returns, and sole halt has exactly one owner.

The mapper supplies locations only; `bc-block-control.sh` packages the exact
repository source and artifact itself. Structurally valid branch/call retargets,
I/O register/opcode mutations, helper mutations, unreachable literal changes,
emit pointer/length changes, and malformed witnesses all reject. This proves
static effect-site custody and the fixed-emit macro when reached. Argument and
expression values, frame contents and dynamic call depth, return values,
non-literal I/O values, global trace order/reachability, terminal classes,
memory/stack bounds, and cyclic progress remain open.
