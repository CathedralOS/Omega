# Cross-carrier declared ranks

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../../../../TASKS.md).

Fact: a private helper preserves the invocation rank on a tail edge, and every
complete cycle strictly descends while advancing the explicit counter once.
The original inputs 4→7 and 7→7, both selected elements, rank range
0..=18446744073709551615, counter 3→6 and caller-visible canaries are unchanged.
The selected measure takes a u64 and produces a u8. Its ordinary helper proves
`(result as u64) == 7 - value` and returns `(7 - value) as u8` under value≤7.
The rank range is mathematical; its ceiling need not fit the result carrier.

The [frozen probe](../../../parameter/computed_measure/cross_carrier/probe/main.omg)
is byte-identical to the first failing program. Exact fixed base
6ac592511e88fd440bc4bef8a55c854f9102b087 refuses all nine positive places with
ranking/source-complete-witness diagnostics. The completed producer runs
all nine places and that frozen probe in Terminal Psi and native macOS ARM64;
the nine no-progress twins reject. The focused corpus records14 runnable
positives agreeing at exit0 and13 intended refusals. Three existing neighboring
programs also run in both engines. No other-host execution is claimed.

Each `computed_measure/cross_carrier/invalid/` twin changes only progress +1 to
+0. Corpus pins are `nested_machine_joint_rank_cross_carrier_<place>_*` under
`tests/omega/{pass,fail}/facts/`. `joint_rank/controls/cross_carrier_*` also
checks explicit conversion versus a missing conversion, typed versus anonymous
result arms, a helper's own Requires and a narrowing operation's own safety.

- Establishment and soundness: the exact input type and helper Requires form
  before result guarantees. Every partial subtraction and narrowing conversion
  retains its own safety obligation. A representable number cannot silently
  change its already-landed carrier; anonymous result leaves may land normally.
- Identity and lifetime: rank application binds the captured u64 coordinate and
  its exact caller facts; result bounds belong to the completed u8 observation.
  Both selected elements, untouched neighbors and canaries stay observable.
- Composition: the whole component orders produced ranks; private coordinates
  are selected in the input carrier. Every complete cycle must descend. Source
  and destination observations retain independent call/cast receipts.
- Terminal production: the ordinary computation root retains its formal input
  type and produced result type. Its completed result reaches the cyclic header
  as the existing private rank parameter. No implicit cast or alternate emitter
  is introduced; identity-forward measures keep their same-carrier restriction.
- Omega realization: the same fact checks the authored counter and storage
  oracles through Terminal and native execution. Existing layout, ABI, frame
  and instruction selection own the physical details. This bounded repair does
  not establish independent proof-kernel or cross-host qualification.

The parent remains open for signed cross-carrier ranks, other declared bodies,
nonscalar and dependent orders, the other invocation-contract obligations and
product-source migration. Raw source hashes, baseline failures, actual producer
identities and outcomes are retained in `build/cross-carrier-rank/`.
