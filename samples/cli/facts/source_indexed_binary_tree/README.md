A two-child Tree constructor preserves Total <= 10 * Nodes; Nodes <= 8 proves exact 80u64 - Total.

This fact advances **PROOF-KERNEL-CORE** and **PROOF-CONTRACT-MIGRATION** in
[TASKS.md](../../../../TASKS.md). It exercises the source constructor's dependent
child-position function, beyond the zero/one-child vector fragment. The
[W-based profile](../../../../wiki/spec/proofs/inductive_profile.md) requires an
independently checked child in the indexed fiber of **every** position, and an
induction hypothesis for every recursive child. This is the existing calculus;
source positivity or a successful source predicate does not admit a new rule.

The unchanged frozen [root](main.omg) supplies independent erased
`Tree<1,2>` and `Tree<1,4>` values to a constructor of `Tree<3,7>` with head 1.
An always-left function cannot inhabit the right child's required fiber. Checked
induction proves `Total <= 10 * Nodes`; the generic `bounded` consumer uses
`Nodes <= 8` to justify its exact `80u64 - Total`, producing 73. The runtime
comparison observes that result; it does not supply the subtraction's proof.
The [empty-child witness](empty_children/main.omg) preserves the earlier frozen
`Tree<1,7>` program unchanged, including its two independent empty inputs.

At fetched main `4bd2529e755945aee52ea3f443928513b5701467`, the optimized actual
`bc3c278555a37dd85da45c2eed3c729628a6fbb0` producer (compiler/library inputs
identical to main) refuses the root: `source constructor child-function production
for multiple recursive fields is not implemented yet`. The old constructor owner
handled zero or one child. Baseline qualification receipts preserve that actual
failure, the original empty-child failure and the rejecting controls. Producer,
profile, source hashes, execution and independent receiving records belong in the
claim/qualification evidence, not a rewritten historical result.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [main](local/main.omg) | [invalid](local/invalid/main.omg) |
| field | [main](field/main.omg) | [invalid](field/invalid/main.omg) |
| parameter | [main](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| call result | [main](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| match join | [main](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| const name | [main](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| a nested machine | [main](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| another machine | [main](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| another target body | [main](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

Each twin changes only the constructed head from 1 to 2, keeping the exact sum
index, entry conditions, child arguments, consuming operation and oracle. It must
reject the missing constructor predicate derivation. The additional
[swapped-fiber control](swapped_fibers/main.omg) exchanges the differently indexed
child arguments and must reject the exact left-field application mismatch.
The root and empty-child witness also have constructor-input twins. All positive
Build files bind the four hosted targets and declare exit 0. Entry bindings alone
do not establish execution on four host architectures.

The nine paired fixtures are
`tests/omega/{pass,fail}/facts/source_indexed_binary_tree_through_<place>_{exit,rejected}`.
The root and parameter pass fixtures request `main mathematical-actuals` through
the ordinary corpus inspection path: source-formed carrier keys and source formal
rosters precede canonical serialization and actual independent receiving.

The completed Linux x86-64 capture with optimized producer
`92056ecd9701e193d25984955d0205ae70c50b78` ran all eleven public positives
(root, original empty-child witness and nine places) with native/Terminal exit 0;
all twelve public constructor-input/fiber controls rejected. The ordinary corpus
record contains fourteen checked and eighteen rejecting fixtures, including seven
focused arithmetic controls. All twelve runnable Tree fixtures executed at exit 0
in both engines. The additional direct-call Boolean-scope fixture and its
head-only twin pin closed constructor schemas under an irrelevant lexical
parameter, independently of a match join; they do not add a fact row.

The row required two general composition repairs alongside the dependent
position selector. Scalar entailment retains an exact positive rational multiple
of an established polynomial bound, checking every monomial and the constant
offset; wrong sign, insufficient ceiling, missing premise and an extra monomial
still reject. Lowering retains a prepared constructor's already checked closed
schema when its exact dependency roster is empty, including inside Boolean
scope. Nonempty schemas retain exact authored erased-parameter application; no
missing proof is synthesized and no open carrier is admitted.

Ordinary source-bound independent receiving succeeded for the root, parameter
and Boolean-scope fixtures with 50,653, 52,586 and 54,313 steps respectively under
the unchanged 65,536-step budget. The Boolean-scope artifact has three carriers,
two indexed roots and two source-bound invocation formals; its canonical artifact
is 171,365 bytes (171,236 semantic). These are focused costs, not a general
performance result. Changing only the runtime result oracle from 73 to 74 causes
a typed Terminal trap and the agreeing native illegal-instruction presentation.
Historical failed recordings and their producer identities remain in the claim
evidence; a recorded refusal was not counted as a passing positive.

Applicable obligations and acceptance:

1. **Establishment:** the constructor owes its full case constraints and both
   exact indexed child fibers before it exists. Its closed schema abstracts the
   source values, child evidence and premises. No goal, post-hoc guard or
   ungranted axiom supplies the proof.
2. **Identity and lifetime:** source field identity fixes declaration order;
   distinct child fibers cannot be exchanged or replaced by layout equality.
   Erased immutable inputs retain their exact source bindings and grant no
   runtime storage access or authority.
3. **Composition:** source induction checks each recursive citation's premises
   and strict descent. The delivered bound survives the nine places into the
   same subtraction. Both arms of the Boolean join and every state arrival owe
   their ordinary obligations.
4. **Soundness:** the finite-position selector uses the existing dependent
   `Two`/pair/empty constructions. Ordinary empty-context signature/kernel checks
   still admit the complete `Pi(position). IW(next(label,position))` function.
   Predicate derivations, transitive assumptions and source correspondence keep
   their independent owners; source recursion is not a trusted certificate.
5. **Psi 05–07:** mathematical child functions and actual arguments remain in
   checked canonical certificates and source-bound receiving; runtime erasure
   contributes no executable child lane. Terminal execution must retain the
   scalar bound's behavior and agree with the native result.
6. **Omega 00–02:** ordinary hosted Build/ProgramEntry settlement and existing
   `u64` layouts/call plans realize the consumer. Erased children introduce no
   physical ABI argument or provider authority.
7. **Omega 03–06:** the scalar calls, joins, field projection and state edge
   retain the exact subtraction and ordinary activation storage rules. This row
   adds no target instruction or stack policy.
8. **Omega 07–09 and emission:** the public command is `omega run --both
   <place>/main.omg`. Actual Linux x86-64 Terminal/native exit 0, immutable native
   products and rejecting twins are required before row closure; other host
   executions remain unverified until observed there.

This row does not close the parent proof layer. Derivation families, mutual and
nested recursion, type/level indices, arbitrary runtime-dependent carrier and
source telescope transport, and the full kernel/metatheory migration acceptance
remain open. Closed independently source-formed input/result carriers and exact
existing grants remain the receiving boundary; no arbitrary dependent/open
carrier application is claimed.
