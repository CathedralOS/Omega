# Proof kernel

[Lattice overview](bootstrap_lattice.md) | [Delta language rung](rungs/delta.md)

> **Status: WORKING PROTOTYPE.** Independent Beta and Gamma implementations
> accept valid certificates, reject invalid ones, and are exercised by logic,
> equality, operational-seam, fuzz, and cross-implementation gates.

The proof kernel is an assurance service used across the bootstrap lattice. It
is deliberately not a Greek-letter rung: it does not extend the programming
language chain, and programs do not elaborate through it. Producers at several
rungs emit certificates; the kernel checks them.

```text
Alpha → Beta → Gamma → Delta       language/bootstrap spine
              ↘       ↙
                proof kernel       cross-cutting assurance
```

The source lives in [`compiler/proof-kernel/`](../../../compiler/proof-kernel/).
The principal implementations are:

- `check.beta`: logical proof checking;
- `eq.beta`: definitional equality and conversion;
- `checker.gamma`: an independent Gamma implementation used for the checker
  diamond.

The kernel accepts a proposition and a certificate and returns accept or reject.
Proof search, elaboration, SMT integration, compilers, and other certificate
producers remain untrusted: they gain no authority by producing a candidate
certificate.

## Proof checking is not artifact verification

The proof kernel answers one deliberately small question:

```text
Does this certificate derive proposition P under these explicit premises?
```

It does not decide which proposition a terminal-Psi artifact ought to prove.
That is the job of the Psi-aware artifact verifier:

```text
canonical Psi artifact
    -> validate structure, identities, and contracts
    -> reconstruct every operation, edge, and return obligation
    -> validate every authorized admission against the consuming profile
    -> require exactly one permitted discharge for every obligation
    -> invoke the proof kernel for certificate-derived facts
    -> VerifiedTerminalModule
```

The proof bundle may carry a proposition for decoding and checking, but that
proposition must exactly match an obligation independently reconstructed from
the fingerprinted artifact. A certificate cannot omit an access, weaken an
author contract, reclassify a derivable fact as admitted, or prove an unrelated
theorem and attach it to different code.

The certificate is therefore self-contained as a derivation while remaining
bound to the exact artifact and obligation identities. The small proof kernel
need only understand the canonical proposition and derivation calculus. A
component must still understand terminal Psi well enough to reconstruct the
right propositions.

### Canonical semantic ledger and untrusted reduction

The deployment endpoint does not trust the current Rust verifier to choose a
convenient sufficient proposition. One total low-rung definition consumes the
canonical terminal-Psi bytes and emits an ordered semantic ledger containing:

- one canonical goal for every proof-bearing operation, edge, return, contract,
  conservation event, and admission site;
- only local primitive denotations, authored contracts, and checked positional
  substitutions; and
- premise origins, prerequisites, establishment points, value/place versions,
  validity scopes, invalidations, and acyclic justification dependencies.

Local operation rules live in a closed, typed declarative schema language with
no opaque callbacks. An ordinary leaf-operation addition supplies one auditable
row; a new control, validity, effect, or frontier concept is explicitly a ledger-
algebra revision. A row states direct denotation and canonical goals but never a
multi-operation interval summary.

The untrusted reducer may compute such a summary, but must emit a derivation from
the exact ledger premises to the unchanged canonical goal. A cast fed by three
SSA definitions therefore receives the three local equations; the reducer proves
their affine composition. A partial operation's result equation becomes
available only on its normal successor after its safety obligation, never as a
premise for that same obligation.

At acyclic joins, a merge evidence token requires matching valid tokens on every
predecessor. Ranking prevents cyclic justification, while dominance and
version/invalidation checks separately establish availability. Cyclic control
uses invariant establishment and preservation obligations rather than the merge
rule. Calls independently check complete `requires` enumeration and exact
capture-free instantiation over caller arguments, places, versions, moves,
reborrows, outcomes, crash routes, and evidence lifetimes.

The production Rust migration starts with one closed declarative operation
inventory in `psi-terminal-semantics`. It owns stable identity and direct local
equations for the goal-free scalar cohort plus a separate exact-unique
structural/effect table for Boolean field custody, port-write effects, and
affine-place establishment. The latter keeps result, custody, action, external
effect, fuel, and frontier axes independent and emits distinct fact, effect, or
frontier observations. The verifier traverses artifacts and consumes those
rows, while the trust graph hashes the same inventories. This is a modularity
checkpoint, not a trust promotion: proof-bearing sufficient-form reducers still
have to derive the unchanged canonical goals, and call and control algebras
remain separate responsibilities.

The authoritative ledger is established for every deployed artifact either by
executing the low definition or by checking a derivation of the same result.
Optimized verifier agreement is a diagnostic oracle and grants no authority.

### Current arithmetic reconstruction (trusted migration state)

For proof-gated arithmetic the operation carries operands, result type, and an
obligation identity. The artifact verifier derives the proposition from terminal
carriers and path-local semantic facts; producer interval metadata is never an
axiom. Native lowering is authorized only after the exact proposition is
certified. Unsupported relations fail closed.

The sufficient-form families below describe the current Rust implementation,
not the final trust boundary. Until each family proves the canonical goal with a
kernel-checked certificate, its exact implementation and version is a distinct
trusted-judgment dependency in the executable trust closure. Reclassifying the
reduced proposition as an admitted program premise is not a substitute for
recording which implementation decided it.

One total-conversion composition permits a nonempty admitted affine,
homogeneous signed-product, exact-shift, or carrier-total landed-divisor prefix
to cross a finite nonempty chain of strict valid fixed-native integer widenings
and feed a nonempty admitted suffix from the same four families. Widening is
numeric identity: the verifier validates every ordered edge, intersects the
target preimage with the original source carrier, and then invokes only the
selected source inverse or complete-hull algebra. Each exact operation keeps
independent evidence; widening definitions are semantic structure, never proof
authority. Empty mathematics is falsehood, divide/remainder partial overlap
and checked replay failure remain unadmitted, and zero is local to the current
obligation after complete shape validation.

A heterogeneous conversion spine may contain both strict valid fixed-native
integer widenings and validator-legal partial fixed-native exact casts between
nonempty admitted source and target computations, provided both conversion
kinds occur. Numeric identity permits one ordered carrier-intersection replay,
but does not merge proof authority: every partial cast reconstructs from the
source root through its own preceding conversion prefix, while every target
interval reconstructs through the complete conversion word before the source
inverse or complete-hull algebra. Widening carries no evidence; every source
operation, cast, and target operation remains independently evidenced. A
source divide/remainder hull proves a cast only by complete containment, never
by partial overlap or falsehood. Pure conversion chains and narrower forms
retain priority. Empty affine/product/shift mathematics is falsehood, checked
failure is no admission, and malformed or stale conversion definitions fail
closed.

A distinct signed-affine rule covers direct, pre-cast, and post-cast one-sided
chains containing both an exact add/subtract offset and a negative landed
factor. Ordered shrinking replay composes checked sign/magnitude `(A, B)` for
`A * root + B`; `A < 0` reverses the interval preimage, `MIN` requires no host
negation, and `A == 0` decides only the current proposition. Every arithmetic
prefix and partial cast retains independent evidence. Mathematical empty
preimages are falsehood, while checked coefficient, offset, division, or
interval failure admits no family. Homogeneous signed products, nonnegative
affine chains, two-sided sandwiches, and conversion chains remain distinct.

One two-sided signed-affine rule crosses exactly one partial cast between
signed fixed-native carriers. Source and target are nonempty left-associated
landed-literal affine chains. A source that contains both an offset and a
negative factor may feed any target affine prefix; otherwise the source must
remain nonnegative and the current target prefix must contain both an offset
and a negative factor. The verifier composes checked sign/magnitude
`(As, Bs)` and `(At, Bt)`, pulls the target carrier backward through the target
form, intersects the exact cast source/target carriers, and then pulls through
the source form. Either negative coefficient reverses endpoints, `MIN` uses
only magnitude, and either zero coefficient settles only the current
proposition after complete shape validation. Every source operation, cast,
and target operation retains independent evidence. Empty mathematics is
falsehood; checked composition, division, or interval failure is no admission.
All-nonnegative, one-sided, homogeneous-product, and conversion-spine rules
retain priority.

A same-root affine fork/join admits one fixed-native exact add or subtract
whose two operands are disjoint nonempty landed-literal affine branches rooted
at the exact same direct machine parameter. Each branch must already belong to
an independently admitted direct offset, product, or mixed affine family. The
verifier walks the source-ordered branch definitions separately and composes
checked sign/magnitude forms `Al * root + Bl` and `Ar * root + Br`; the join is
then `(Al + Ar) * root + (Bl + Br)` or `(Al - Ar) * root + (Bl - Br)`. Only the
join's carrier preimage is reconstructed by that combined form. Every branch
operation and the join retain distinct evidence, so cancellation or a zero
combined coefficient cannot authorize an unsafe prefix. Empty preimages are
falsehood, while checked composition or malformed definition walks are no
admission. Distinct roots remain outside this one-dimensional proof family.

A distinct-root signature-bounded affine fork/join admits the same outer
fixed-native exact add or subtract when the two disjoint, nonempty,
source-ordered landed-literal affine branches end at different direct machine
parameters of the same carrier. The verifier selects the tightest landed unary
lower and upper signature bounds for each root, intersects each pair with the
carrier, and maps each interval forward through its branch's checked signed
affine form. It then uses the Minkowski sum or difference of those independent
ranges. Complete containment in the join carrier reconstructs the canonical
conjunction of the selected root bounds; a wholly disjoint join range is
falsehood; partial overlap is no admission. Relational cross-root premises,
missing one-sided bounds, computed roots, shared roots, overlapping or
reordered branch definitions, and checked arithmetic failure remain fenced.
Every branch operation and the outer join retain independent evidence. This
is the distinct-root sufficient form for the exact-add and exact-subtract
ledger entries below; subtraction reverses the right forward interval before
the Minkowski join.

A distinct-root signature-bounded signed affine product join admits one outer
fixed-native exact multiply over two disjoint, nonempty, source-ordered
landed-literal affine branches rooted at different direct signature
parameters. Both roots must supply landed unary lower and upper bounds; the
verifier selects the tightest endpoints, maps each interval forward through
its checked signed affine form, and computes the exact hull of the four corner
products. Complete containment emits the canonical conjunction of the four
selected bounds, a wholly disjoint hull is falsehood, and partial overlap or
checked corner overflow is no admission. Every branch operation and the outer
multiply retain independent evidence. Same-root quadratic correlation,
relational cross-root premises, one-sided bounds, unsigned carriers, and
malformed branch walks remain fenced. This is a separate sufficient form for
the exact-multiply ledger entry below.

A same-root signature-bounded signed affine quadratic product join separately
admits one outer fixed-native exact multiply over two disjoint, nonempty,
source-ordered landed-literal affine branches rooted at the same direct
signature parameter with nonzero coefficients. From the tightest landed unary
lower and upper bounds, the verifier composes the checked integer quadratic
and evaluates its exact discrete range at both interval endpoints and the
in-range floor and ceiling adjacent to its rational vertex. Complete
containment emits the canonical conjunction of the two selected bounds, a
wholly disjoint range is falsehood, and partial overlap or checked
coefficient, vertex, or evaluation failure is no admission. Every branch
operation and the outer multiply retain independent evidence. Constant
collapse, distinct roots, relational premises, one-sided bounds, unsigned
carriers, and malformed branch walks remain fenced. This correlated nonlinear
sufficient form precedes the distinct-root rectangle form for exact multiply.

A same-root signature-bounded signed affine divide/remainder safety join admits
one outer fixed-native exact divide or remainder over two disjoint, nonempty,
source-ordered landed-literal affine branches rooted at the same direct signed
signature parameter with nonzero coefficients. From the tightest landed unary
lower and upper bounds, the verifier solves the checked integer equations for
the divisor's zero and `-1` roots. The latter is forbidden only when the
correlated dividend evaluates to the carrier minimum at that exact lattice
point. No in-range forbidden root emits the canonical conjunction of the two
selected bounds. Forbidden roots covering every integer in the selected
interval emit falsehood; partial safety or checked equation, range-size, or
evaluation failure is no admission. Every branch operation and the outer
divide or remainder retain independent evidence. Constant collapse, distinct
roots, relational premises, one-sided bounds, unsigned carriers, and malformed
branch walks remain fenced. This nonconvex correlated sufficient form precedes
generic runtime-divisor fallback for the exact-divide and exact-remainder
ledger entries below.

| Operation | Independently reconstructed sufficient forms |
| --- | --- |
| Exact cast | the source value is within the target carrier. A finite chain of at least two validator-legal partial fixed-native exact casts may start at one direct machine parameter: each prefix follows only ordered shrinking cast definitions, intersects every carrier from the root through the current target, and emits canonical root bounds without importing any earlier cast proof. The same finite cast core may follow one already-admitted nonempty affine, homogeneous signed-product, exact-shift, or carrier-total exact-divide/remainder prefix: every cast prefix intersects all carriers reached so far and maps that interval through only the selected prefix family's existing verifier-owned inverse algebra. Conversely, the complete cast chain may feed one nonempty affine, homogeneous signed-product, exact-shift, or landed-safe-literal divide/remainder suffix: every suffix prefix validates the entire ordered cast shape, intersects every carrier, and applies only its existing post-cast inverse algebra. Both sides may be nonempty in the unified composition: every source prefix, cast prefix, and target prefix walks ordered shrinking definitions, intersects the complete cast carrier chain, and applies only the selected target inverse followed by the selected source inverse or complete-hull algebra. Every prefix operation and cast retains independent evidence; empty affine/product/shift preimages are falsehood, checked replay failure is no admission, and divide/remainder still requires complete hull containment. Mathematical empty intersection is falsehood and malformed transfer is no admission. When one partial fixed-native cast consumes a retained finite left-associated same-carrier exact-add/subtract literal-offset chain rooted at a direct parameter, the verifier follows ordered shrinking-prefix definitions, accumulates the checked mathematical offset, shifts the target interval back by that offset, intersects it with the source carrier, and reconstructs the surviving direct-root bounds independently of every prefix operation's own evidence. A retained finite left-associated same-source-carrier exact-multiply chain may likewise feed one partial fixed-native cast: the verifier accumulates independently landed nonnegative factors with checked arithmetic, maps the target interval back through the cumulative product, intersects it with the source carrier, and keeps every multiply-prefix proof independent. Product zero makes only the cast true, product one uses the ordinary target/source intersection, and larger products use the signed or unsigned inverse target bounds. A unified same-source-carrier affine chain containing both offset and multiply operations may also feed the cast: the verifier replays checked `A * root + B`, maps the target interval back through positive `A`, or decides a zero-coefficient cast solely from target representability of `B`, without importing any prefix proof. That source affine chain may continue through the cast into one finite target affine suffix: each target prefix independently maps the target carrier through its checked affine form, intersects with the source carrier, and maps through the complete source form. Zero coefficients decide only the current obligation after full sandwich validation. The cast may also separate a nonempty source affine chain from a nonempty target shift chain, or a nonempty source shift chain from a nonempty target affine chain: ordered target replay, explicit target/source intersection, and complete source replay reconstruct only the current obligation while every source operation, cast, and target operation keeps independent evidence. Finite same-source-carrier exact-left- or exact-right-shift chains may also feed the cast with independently landed heterogeneous legal counts. Left shift uses inverse target bounds or a zero result at source width; right shift uses the arithmetic/zero-fill target preimage, saturating to zero for unsigned sources and `-1` or zero for signed sources. A mixed-only finite left-associated chain containing both shift kinds may also feed the cast: the verifier starts from the target/source carrier intersection and maps it backward through every ordered inverse-left or inverse-right definition. Mathematical emptiness is falsehood; checked transfer failure is no admission. A finite exact-divide/remainder chain may feed the cast only when verifier-replayed toward-zero quotient and dividend-sign remainder interval hulls map the full source carrier wholly inside the target; guard-sensitive nonconvex preimages remain outside this family. Every prefix proof remains independently mandatory. |
| Exact right shift | `0 <= count < width`. In a retained finite left-associated chain, every link reconstructs independently from its own landed in-range count; the prior shifted-value definition is an operand, not proof authority. One direct partial fixed-native cast may root the same finite chain: the cast retains independent representability evidence, while each shift prefix remains independently true from only its own landed legal count. This independent count proof is unchanged when the post-cast chain contains both shift kinds. |
| Exact left shift | legal count plus carrier-tight no-overflow bounds. In a retained finite left-associated chain, the verifier follows only ordered shrinking-prefix definitions to a direct same-carrier parameter, checks every independently landed in-range count, accumulates counts with checked arithmetic, and reconstructs each link's cumulative carrier-tight root bound; a cumulative count at least the value width admits only the zero root. A direct partial fixed-native cast may instead root such a finite nonempty same-value-carrier chain: the cast proves representability separately, while every shift prefix shifts the target interval right by its checked cumulative count and intersects it with the source carrier; heterogeneous fixed-native count carriers remain legal. One direct-root mixed family admits any finite left-associated chain containing both exact-left and exact-right shifts. Each left prefix independently maps its safe input interval backward through every prior canonical definition: inverse left shift uses ceiling/floor division by the power of two, inverse arithmetic or zero-fill right shift uses `[a*2^k, (b+1)*2^k-1]`, and every step intersects the carrier. The same mixed-only chain may be rooted at one direct partial fixed-native cast: each left prefix walks the ordered definitions back to the cast and intersects the resulting target interval with the source carrier before emitting canonical source-root bounds. More generally, nonempty shift chains may occur on both sides of one partial cast: each target-left prefix replays the complete target prefix to the cast, intersects target and source carriers, and replays the complete source prefix to a direct parameter; every source shift, cast, and target shift proof remains independent. A nonempty source affine chain may replace that source shift prefix: every target-left obligation replays to the cast, intersects carriers, then maps through the complete checked source affine form, without importing source or cast proofs. A finite landed add/subtract/nonnegative-multiply prefix may instead precede a finite shift suffix containing at least one left shift: after replaying prior shifts, the verifier composes checked `A * root + B` and maps the surviving interval back to the direct same-carrier root. `A == 0` decides only the current left-prefix obligation. No cast, arithmetic-prefix, or earlier shift proof is imported, so later cancellation, zero factors, or shifts cannot erase unsafe prefixes. Mathematical emptiness is falsehood; checked transfer failure is no admission. |
| Exact add | a known addend and its complementary carrier bound; unsigned `left <= MAX - right`; signed positive/negative variants with the sign fact that makes the bound operation total. In a retained left-associated mixed exact-add/subtract chain, the verifier follows ordered shrinking-prefix definitions to a direct same-carrier parameter, combines landed same-carrier right literals as additions or mathematical negations of subtrahends, and reconstructs every prefix from the checked cumulative offset. A direct partial fixed-native cast may instead root such a finite nonempty same-target-carrier chain; the cast proves direct representability independently, while every arithmetic prefix walks back to the canonical cast definition and reconstructs its own target interval shifted by the checked cumulative offset and intersected with the source carrier. The post-cast chain may widen to the unified mixed affine family only when offset and multiply operations both occur: each prefix replays checked `A * source + B` against the target/source intersection, retaining every earlier proof across zero or cancellation. A finite exact-shift prefix may instead feed a finite add/subtract/nonnegative-multiply suffix: each arithmetic prefix maps the carrier backward through checked `A * shifted_root + B`, then replays every ordered shift definition to the direct root. The source shift chain may cross one partial cast before the target affine suffix; each target prefix maps through its checked affine form, intersects target and source carriers, then replays the complete source shift definitions without importing any prior evidence. Every shift and arithmetic proof remains independent; a zero coefficient decides only the current proposition after full shape validation. One outer add may also join two disjoint already-admitted affine branches rooted at the same direct parameter; checked addition of their coefficient/offset forms reconstructs only the join while preserving every branch obligation. |
| Exact subtract | a known right operand and its complementary carrier bound; unsigned `right <= left`; signed positive/negative variants with the sign fact that makes the bound operation total. The same finite post-cast mixed-chain rule reconstructs every subtract prefix independently, using the mathematical negation of each landed subtrahend; cancellation never replaces an earlier prefix obligation. One outer subtract may also join two disjoint already-admitted affine branches rooted at the same direct parameter; checked subtraction of their coefficient/offset forms reconstructs only the join while preserving every branch obligation. |
| Exact multiply | a known factor and the carrier-tight interval; a positive runtime factor plus `MIN / factor <= value <= MAX / factor` for signed carriers, or the upper bound for unsigned carriers; runtime factor `-1` plus `MIN + 1 <= value`; a signed factor at most `-2` plus `MAX / factor <= value <= MIN / factor`. In a retained finite left-associated chain, the verifier follows only ordered shrinking-prefix definitions to a direct same-carrier parameter, accumulates explicitly landed nonnegative right factors with checked arithmetic, and reconstructs each link's cumulative carrier-tight root bound independently. A direct partial fixed-native cast may instead root such a finite nonempty same-target-carrier chain: the cast proves representability separately, while every multiply prefix divides the target interval by its checked cumulative product and intersects it with the source carrier; product zero or one makes only that prefix true, and a later zero cannot erase earlier evidence. The direct, pre-cast, and post-cast homogeneous placements also admit signed-carrier chains containing at least one negative landed factor. The verifier accumulates a checked sign/magnitude product, including `MIN`, and maps `[L,U]` through positive `P` as `[ceil(L/P), floor(U/P)]` or through negative `P` as `[ceil(U/P), floor(L/P)]`. Zero decides only the current proposition; every prior prefix and cast remains independently evidenced. A wider retained affine chain may interleave exact add/subtract with exact multiply only when both families occur, whether rooted directly or at one direct partial cast: the verifier replays every ordered prefix as checked `A * root + B`, maps the carrier back through positive `A`, or decides a zero-coefficient prefix solely from `B`, while preserving every earlier obligation. |
| Exact divide/remainder | a known safe divisor; `1 <= divisor`; `divisor <= -2`; or `divisor <= -1` together with `MIN + 1 <= dividend`. In a retained finite mixed divide/remainder chain, every link reconstructs independently from its own safe divisor; an earlier result definition is an operand, not proof authority. One direct partial fixed-native cast may root the same finite chain: the cast retains independent representability evidence and every prefix remains independently true from only its own divisor proof. Either root form may contain direct same-carrier runtime divisors when at least one occurs: each runtime divisor independently supplies its positive or at-most-`-2` proposition, while the joint `-1`/dividend exception is available only to the first direct-root operation whose dividend bound is independently reconstructed. Computed and post-cast dividends import no such authority. Two disjoint nonempty affine branches on the same direct signed fixed-native signature root may instead use the complete unary root interval and exact forbidden-root lattice test described above; every branch prefix remains independently evidenced. |
| Wrapping/Saturating divide/remainder | a known nonzero divisor, `1 <= divisor`, `divisor <= -2`, or `divisor <= -1`; policy defines the signed `MIN`/`-1` result |

The consolidated divide/remainder cross-cast rule admits a nonempty landed-
literal exact-divide/remainder chain on either side of one partial fixed-native
cast when the other side is a nonempty affine or shift chain. When
divide/remainder precedes the cast, the toward-zero quotient and dividend-sign
remainder hull of the full source carrier must fit the target carrier. Each
target-affine or target-left safe interval containing that complete hull is
truth; a disjoint interval is falsehood; partial overlap remains unadmitted
because it would require a guard-sensitive nonconvex preimage. In the converse
direction, the source affine or shift chain and cast reconstruct independently,
and each target divide/remainder proposition depends only on its own landed
safe divisor. No operation imports another operation's proof or evidence.

The corresponding direct same-carrier family omits the cast but retains the
same four nonempty divide/remainder-to-affine/shift compositions. A leading
divide/remainder chain supplies its verifier-replayed carrier-total hull to
each following affine or left-shift safe interval: containment is truth,
disjointness is falsehood, and partial overlap remains unadmitted. In the
converse direction, affine or shift proofs replay from the direct parameter as
usual, while each following divide/remainder proof depends only on its landed
safe divisor. Every operation and every evidence item remains independent.

A finite nonempty landed-literal exact-divide/remainder chain may likewise
cross one validator-legal partial fixed-native exact cast into another finite
nonempty landed-literal exact-divide/remainder chain. The cast obligation
replays the complete source carrier through the ordered toward-zero quotient
and dividend-sign remainder hull transfers and is admitted only when the final
hull lies wholly in the target carrier. There is no partial-overlap or
falsehood widening for that cast. Every source operation, the cast, and every
target operation retains separate evidence; each target proposition depends
only on its own landed safe divisor.

Path facts must reach the operation through verified terminal control. Count
masking, machine overflow behavior, or a producer claim cannot discharge an
obligation. The verifier passes only the reconstructed proposition and exact
semantic axioms to the small kernel.

## Trust and meaning

Kernel acceptance defines certificate validity, not program behavior. Checked
bridges must connect each proposition and progress judgment to the exact version
of the terminal-Psi execution semantics they claim to describe:

```text
safety / partial correctness
    exhaustive derivation + sound rows + valid premises + checked obligations
    => no execution prefix violates the selected safety policy and every
       completed outcome satisfies its contract

progress / total correctness
    well-founded orders + per-edge descent + complete SCC/call closure
    + accepted environmental progress premises
    => every published termination guarantee holds
```

Logical fuel discharges neither theorem: sponsor exhaustion suspends at the
unpaid site and later resumes, so it is scheduling and attribution rather than
termination evidence.

These bridges are the central open proof obligations. Current seam tests compare
kernel derivations with independent operational decisions and reject perturbed
claims; they are evidence and regression gates, not the final metatheorems. An
interpreter oracle is also silent when execution is implemented correctly but a
ledger row denotes that execution incorrectly.

Each universally quantified row theorem is low-rung metatheory proved once, not
a quantified proposition repeated in every artifact bundle. It cites exact
digests for the row, state model, mathematical definitions, operational clauses,
and generic composition theorem. A derived status is computed only from an
accepted proof with matching dependencies. Conservative semantic extensions may
transport an unaffected proof through a checked extension theorem; relevant
changes require a new proof, while old artifacts retain their pinned semantics
identity.

Every accepted dependency forms a closed acyclic trust graph whose leaves are
explicit registered roots with kind, semantic subject, digest/version, owner,
scope, rationale, and accepting policy. Unknown leaves reject. Trusted verifier
implementations, unproved schema rows, locally proved rows awaiting the global
composition theorem, accepted admissions, and irreducible semantic axioms remain
distinguishable rather than collapsing into one `trusted` label.

This bridge is relative to terminal Psi's abstract operational semantics. The
proof that x86-64 or AArch64 lowering realizes that execution belongs to the
separate native-refinement closure and carries its own ISA and hardware roots;
portable PCC never imports those assumptions.

The kernel remains on the audited bootstrap lineage. The Beta and Gamma versions
are independently implemented and compared, so moving the directory or removing
its former rung name does not weaken its assurance role.

A later Psi implementation may provide a faster third implementation of the
same kernel. It joins the checker diamond only over the exact shared calculus
and certificate semantics, and only when it is independently implemented; a
port of an existing checker tests the compilation path but is not implementation
diversity. The Beta and Gamma implementations remain the low-rung reference
route unless a separately justified trust transition replaces them.

## Recursive and normalization certificates

The Rust kernel checks recursive components and algebraic normalization using
the settled shapes below. Source automation does not yet emit those records,
and terminal Psi does not yet reconstruct or retain them; that bridge remains
open without making the source entailment engine trusted.

A recursive certificate is organized by the strongly connected component of
the proof-call graph. The component cites its ranking relation and one proof of
that relation's well-foundedness. Each application edge within the component
then proves only its local obligation:

```text
ranking_relation(callee_measure, caller_measure)
```

The component rule makes every member's contract available only at strictly
smaller measures. A call outside the component is an ordinary contract
application. Thus self induction and mutual induction use the same kernel rule,
while an unmeasured proof cycle cannot disguise itself as a sequence of
ordinary citations. The well-foundedness proof is shared component evidence;
the decrease proof is edge evidence. Both participate in provenance.

A normalization certificate identifies the selected conformance and exact law
evidence used to justify the canonicalization. Replaying a total normalizer may
compress primitive inference steps, but it does not erase premises. The trust
closure of every cited law is inherited by the normalized conclusion, including
admitted laws.

The review synopsis is a deterministic projection of the checked certificate,
not a source-side explanation pass. It names the certificate fingerprint and
renders its recursive components, closure rules, cited laws, and trust closure.
Source attribution may decorate certificate nodes, but cannot substitute for a
certified derivation.

The tree-certificate checker records exact rule families and cited
assumption/semantic-axiom propositions. The terminal artifact layer
fingerprints the accepted proof bundle and renders its current review synopsis
only from a `VerifiedTerminalModule`, so changing an accepted proof route
changes both its fingerprint and trust record. The recursive-component checker
requires a canonical member/edge set, one selected relation and
well-foundedness route, and exact per-edge decrease evidence. The normalization
checker pins the selected conformance and law set, verifies every law route,
and requires the conclusion to cite every law premise. Both retain admissions
in provenance. Terminal vocabulary, source production, verification, and the
synopsis still need to carry these two record families end to end.

## Scope discipline

- The kernel contains proof rules and deterministic certificate checking.
- Automation and proof search live outside it and must materialize evidence.
- New logical capabilities require negative tests and an operational or semantic
  seam where one exists.
- During pre-release development the certificate producer and every checker move
  together. Stale certificates reject; this page describes only the current
  calculus.

The remaining certification bridge, terminal reconstruction closure, and
soundness work is tracked once under P3 in
[`TASKS.md`](../../../TASKS.md).
