# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the specification and language guide; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Question numbers are mutable queue positions, not permanent decision identities.
Code, canaries, and settled documentation must cite a stable named decision or
the governing guide section rather than an owner-question number. A settled
decision's durable identity does not change when this queue is pruned.

Before a proposed surface becomes an owner question, audit whether it is
implemented, whether any authored source uses it, and whether ordinary Omega
already expresses the customer. An unimplemented, unused spelling that adds no
capability beyond existing checked machines is retired rather than redesigned.
Hypothetical future utility does not by itself preserve syntax; a concrete
customer requiring a distinct capability may propose a new surface later.

Every `OWNER-BLOCKED` escalation must name an independently motivated product
requirement or credible external use case. Existing corpus use is not required.
A test, experiment, benchmark, or implementation task cannot be the sole
motivation, and machinery introduced only to support such work is removed or
kept non-authoritative rather than promoted into an owner decision.

Apply the same test to security machinery. Omega owns only claims it can
enforce at its actual compiler, package, and artifact boundaries. A proposal
that merely restates host operating-system, credential, transport, or operator
trust must be deleted or delegated to that owner rather than dressed as an
Omega guarantee. If the boundary or enforceable claim is genuinely ambiguous,
promote that narrow ambiguity here before adding machinery.

A compelling proposal for a new bootstrap language, replacement rung, or
alternate dialect belongs here before implementation, even as an experiment.
Apply the [bootstrap scope checkpoint](AGENTS.md#scope-checkpoints): identify
the concrete compiler customer or required proof obligation, compare existing
languages and simpler refactors, and account for total audit cost and machinery
displaced. Await an owner decision; a task or prototype is not approval.

## 1. Epsilon expanded-storage admission

<a id="epsilon-static-storage-policy"></a>

Should interpreted Epsilon retain a separate cap on fully expanded logical
storage, or bound only the resources its evaluator actually consumes?

**Standing requirement:** [Epsilon section 10](bootstrap/4_epsilon/LANGUAGE.md#10-resource-classification)
requires post-checking expanded-storage admission, deterministic array-length
attribution, and the bounded `ApplicationStaticStorageBytes` demand witness.
The switch to an evaluator explicitly retained unrelated resource decisions
(`c1f09eddaa`, historical decision D125). This is a proposed policy revision,
not an unanswered permission to choose a larger numeric capacity. The existing
requirement remains in force until answered.

The customer is the interpreted first Omega compiler D. Its
[`AlphaTapeBuffer`](bootstrap/5_omega/representations.epsilon) contains eight
fixed arrays. With a hypothetical one-byte `u8`, four-byte `i32`, no-padding
layout, those arrays alone represent 141,674,226 bytes, excluding scalar fields
and metadata. That is logical-size arithmetic, not measured memory or a selected
Epsilon layout. D's authored extents and capacity checks remain source semantics.
The evaluator's [sparse storage](bootstrap/4_epsilon/implementation_notes.md)
does not materialize those arrays in full; conversely, repeated updates to a
small array can consume substantial cumulative immutable Gamma allocation.
An expanded-size cap therefore does not replace actual resource containment.

A disposable probe at `f048ab4155` on macOS arm64 confirms this distinction:
a checked 529-byte program declares
`[[[u8; 2147483647]; 2147483647]; 2147483647]`, reads the unwritten last cell,
writes `65` there, checks a separate zero cell, and outputs `A`. The exact
current execution receipt (`dd4985c0eb6e1f30bc2178f90dd30e25ae7b842fb544137f606a44e622000f22`)
returns process zero and private observation `000000000041` in about 0.81 seconds.
Its logical byte-cell count is 9,903,520,300,447,984,150,353,281,023.
This is diagnostic execution, not final-profile conformance or evidence that a
selected static limit was violated. No permanent profiling tool was added.

1. **Recommended: remove the separate expanded-size admission requirement for
   interpreted Epsilon.** Retain exact source array bounds, zero homes, value
   semantics, evaluator refinement, and explicit fail-closed limits on actual
   request, storage, stack, output, and execution resources. This avoids adding
   a logical layout/sizing pass, its attribution rules, and corresponding proof
   and conformance obligations solely for an unrealized dense representation.
   Actual-resource containment and the final observation boundary still need
   implementation; this is not a proposal for unlimited execution or host policy.
2. **Retain the current requirement.** Select the profile's byte measure,
   storage-root accounting, and extent, then implement bounded type-graph sizing
   and canonical attribution before execution. Keep the sparse runtime: no
   Alpha backend, native ABI, or eager allocation is required. This preserves
   an independently predictable logical-size admission rule, at the cost of a
   separate analysis that does not bound cumulative interpreter allocation.

No implementation-size or speedup estimate has been established for either
choice. Only replacement of this policy awaits an owner ruling. The
[P3 resource task](TASKS_BOOTSTRAP.md#p3---delta-to-epsilon) remains open; checking
and runtime conformance, lower-chain resource analysis, and independent proof
work do not depend on this answer.
