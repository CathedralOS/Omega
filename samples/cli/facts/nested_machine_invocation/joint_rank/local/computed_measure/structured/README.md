# Structured declared rank bodies

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../../../../TASKS.md).

Fact: a private helper preserves the invocation rank on a tail edge, and every
complete cycle strictly descends while advancing the explicit counter once.
This production keeps the call-measure row's inputs4→7/7→7, both selectors,
range0..=18446744073709551615, counter3→6 and caller-visible canaries. The
measure selects `rank(value)` below6 and `rank(value)-1` otherwise; the crossing
from5 to6 strictly decreases by2. Every twin changes only strict progress+1 to+0.

The [frozen smaller probe](../../../parameter/computed_measure/structured/probe/main.omg)
is byte-identical to the original baseline probe. All nine places retain their
existing transport shapes in `computed_measure/structured/`, with matching
`invalid/` twins.

The fixed base `05812db44c356c4bb96427b4e8dd286a895f8128` refuses all nine
positives at rank-range/source-complete-witness checking. The repaired producer
executes all nine with native exit 0 and Terminal Psi exit 0 on macOS ARM64;
all nine no-progress twins reject with the preserving-cycle diagnostic. Other
host execution remains unverified. All programs bind the four hosted targets.

The corpus pins use `nested_machine_joint_rank_structured_<place>_{exit,rejected}`
under `tests/omega/{pass,fail}/facts/`. Additional programs in
`joint_rank/controls/structured_*` exercise short-circuit selection, partial
arithmetic formed only in its selected arm, a reversed formation guard,
nonmonotone descent, a partial subject, and branch-local callee requirements.

- Establishment: the subject forms before any branch fact; each selected arm
  forms its calls and arithmetic independently before its value is retained.
- Identity and lifetime: guards and values name captured scalar snapshots;
  branch premises never become ambient facts after the join. The mutable
  selected element, untouched neighbor and caller-visible canary are checked.
- Composition: both selectors and active/completed invocations run; complete
  cycles advance exactly three times, and the twin removes only progress.
- Soundness: callee Requires forms before importing Ensures; invalid subject,
  branch-requirement and guarded-arithmetic controls retain their refusals.
  Shared proof-work exhaustion declines a query, never discards a feasible arm.
  The supplementary owned-ingress program carries an unrelated owned token,
  erased proof input and erased scalar requirement through the prelude; its
  twin changes only the erased actual from 19 to 18 and fails the Requires.
- Psi lowering: ordinary retained scalar control selects the arm and completes
  its calls. A one-shot invocation prelude observes the initial rank; every
  cyclic arrival passes its completed rank through an explicit private header
  parameter. No later SSA value is assigned to a pre-observation block.
- Omega realization and emission: both engines agree on the authored counter,
  selection and storage oracles on macOS ARM64. The existing 100000-cycle
  native witness also exits 0 with a 256 KiB stack limit. This is focused
  execution evidence, not independent proof-kernel or other-host qualification.

The parent outcome remains open for other declared bodies, cross-carrier
measures, nonscalar orders and dependent distances, and its other invocation
contract and product-source obligations. Raw source identities, producers,
baseline refusals and execution observations are retained in
`build/structured-rank/` and the PR evidence.
