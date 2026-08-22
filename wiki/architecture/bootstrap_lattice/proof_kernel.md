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

The current production certificate calculus includes disjunction introduction
with one explicit selected-arm index and one independently checked child proof,
matching the low-rung Beta/Gamma `inl`/`inr` rules. The checker rejects a
non-disjunction conclusion, an out-of-range arm, or a child conclusion that is
not exactly the selected disjunct. This logical rule grants no authority to
choose the artifact's canonical goal and by itself derives no ledger row.

The calculus also includes exact transitivity for terminal fixed-integer `<=`.
It checks both child derivations, their identical middle term, and the exact
outer endpoints. Together with the closed-integer primitive and disjunction
introduction, this can derive the negative arm of the canonical signed nonzero
goal from a tighter bound such as `d <= -2`. The rule mirrors the accepted
low-rung `int-le-trans` theorem, but the Rust checker remains an explicit
trusted implementation until a checked terminal-carrier bridge closes the
independent checker diamond.

Exact fixed-integer `<=` propositions may also transport one endpoint across
an independently derived equality. The certificate names endpoint zero or one,
retains the other endpoint exactly, and supplies separate checked relation and
equality children; the equality must connect the old and new endpoint in either
orientation. Non-`<=` relations, non-equality premises, another changed
endpoint, an unrelated equality, or any endpoint other than zero or one reject.
This is the bounded terminal-carrier instance of low-rung Leibniz `eqelim`; it
adds proof capability without granting a producer authority to choose a goal.

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
rows. A third exact-unique table declares the ten independent composition axes
for scalar, structural Unit, and boundary calls; one focused verifier module
composes already-validated contracts through that table rather than embedding
three call branches in general reconstruction. The trust graph hashes all three
inventories and their exact consumers. A fourth exact-unique table owns the
twelve proof-bearing scalar leaves. It retains direct denotation, six canonical
goal shapes, the normal-successor result equation, crash policy, fuel, and
frontier policy without inspecting predecessor definitions. General artifact
reconstruction consumes that typed observation, while one isolated migration
dispatcher still selects the current sufficient proposition. The trust graph
binds the proof-bearing table to exactly those twelve leaf nodes and binds the
dispatcher to every reducer it can select. This is a modularity checkpoint, not
a trust promotion: proof-bearing sufficient-form reducers still have to emit a
checked derivation of the unchanged canonical goals, and control algebra remains
a separate responsibility.

Exact-shift reconstruction follows the same production boundary internally:
a small precedence parent owns primitive shift dispatch, a direct-chain module
owns landed-count and interval foundations, and a cross-family module owns
cast, affine, divide/remainder, and shift composition. The reducer contract is
unchanged, and the trusted-migration node binds all three exact source files.
Exact conversion reconstruction is split analogously between a cast-precedence
parent, conversion-chain/interval foundations, and cross-family composition.
Its trusted-migration node also binds all three exact source files.

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

Specification arithmetic never introduces a partial term into the kernel.
Fixed-width integer and address embeddings denote unbounded `Int` values and
carry their source-carrier range as independently reconstructed facts. Exact
arithmetic is present only with its formation obligation discharged; Wrapping
denotes width-specific modular reduction; Saturating denotes a clamp to the
carrier bounds. Independent primitive definedness obligations, such as a
nonzero divisor, remain mandatory. Direct Trapping arithmetic is absent from
proposition syntax.
Its executable operation instead emits a separate primitive-specific crash
guard and, on normal return, the same exact mathematical result equation.

Each denotation row is versioned independently in the canonical semantic
ledger. Integer addition, subtraction, and multiplication trap exactly when
their mathematical result is outside the result carrier; division/remainder,
shifts, conversions, and floats use their own catalogued conditions. Crash
coverage separately proves the path-conditioned derived guard implies the
authored same-cause route disjunction. A certificate may cite these rows but
cannot choose the primitive semantics, substitute a generic out-of-range rule,
or turn a contract term into an executable effect.

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

The canonical `ExactDivisionDefined` kernel proposition is `1 <= d` for an
unsigned fixed carrier. For a signed carrier of at least two bits it is the
ordered disjunction `(d <= -2) OR (1 <= d) OR ((d <= -1) AND (MIN + 1 <= n))`.
For signed i1 it is only `(d <= -1) AND (0 <= n)`. This is a proposition
projection, not a reducer result: address carriers and mismatched operand types
reject. The complete carrier-total landed-literal family shared by exact
divide and remainder now reconstructs this canonical goal directly: unsigned
nonzero literals and signed literals other than zero and `-1` use only their
prior semantic equality and a closed integer-order judgment. The complete
signed `-1` exceptional family uses the same path when the dividend is also
landed as any literal above the carrier minimum; its recursive certificate
proves both terms of the third disjunct, or both terms of the `i1` conjunction,
from those exact equalities and closed order. An exact independently retained
`MIN + 1 <= dividend` proposition (`0 <= dividend` for `i1`) may replace the
dividend landing: it is cited directly as the second recursive premise from a
machine requirement or pre-site semantic axiom. No reducer result or wider
interval is imported; missing, stale, or weaker bounds reject. One retained
same-carrier literal lower bound `K <= dividend` may instead prove the floor
when closed order establishes `MIN + 1 <= K`; the certificate binds the exact
citation through `IntegerLessOrEqualTransitivity`. Reversed, mistyped, weaker,
or wrong-dividend facts reject. Exact prior safe-divisor propositions are now
canonical too: unsigned or signed `1 <= divisor`, and signed-width-at-least-two
`divisor <= -2`. Unsigned certificates cite their whole goal; signed
certificates cite and introduce the selected disjunct. The complete signed
joint-bound family selects the ordered third disjunct when both
`divisor <= -1` and `MIN + 1 <= dividend` are independently proved through the
supported exact citation or checked transitivity paths. It constructs those
two premises with conjunction introduction before disjunction introduction;
missing or redirected operands reject. A retained `divisor <= -1` may also pair
with an independently landed nonminimum dividend literal: closed integer order
and exact equality substitution prove the dividend floor. A minimum or
wrong-identity landing rejects. Exact literal equalities retained as machine
requirements use the same complete substitution path and are cited as
assumptions rather than semantic axioms. The selector examines every exact
same-carrier equality; zero-only, minimum-dividend, mistyped, or redirected
premises reject. The complete endpoint-transport family also pairs an exact
retained bound on `K` with an independently retained equality connecting `K` to
the canonical divisor or dividend endpoint in either orientation.
`IntegerLessOrEqualSubstitution` cites both premises and changes only that
endpoint. Dividend transport remains in the joint arm and requires its separate
`divisor <= -1` proof. A missing companion bound, unrelated equality, weak bound,
or changed untouched endpoint rejects. For signed `i1`, both canonical
conjuncts may be transported independently: `Kd <= -1` through `Kd == divisor`
and `0 <= Kn` through `Kn == dividend`. Both substitutions remain mandatory;
missing or crossed equalities reject. A complete nested endpoint family may
first derive the canonical bound on `K` from one stronger retained bound and a
closed same-carrier order fact, then transport it through equality. Its proof
nests `IntegerLessOrEqualTransitivity` beneath substitution; weak bounds,
missing equalities, or wrong endpoints reject. The next complete nested family
replaces that closed side with a second exact citation: unsigned `1 <= M` and
`M <= K`, or signed `K <= M` and `M <= -2`, followed by `K == divisor`.
The proof nests two-citation transitivity beneath endpoint substitution in
deterministic ledger order. Missing or disconnected middle relations, weak
signed ceilings, redirected equalities, or wrong endpoints reject. The signed
joint arm has the corresponding complete dividend sibling: exact
`divisor <= -1`, `MIN + 1 <= M`, `M <= K`, and `K == dividend`. Its ordered
conjunction cites the divisor bound directly and nests the two dividend-floor
citations beneath endpoint substitution. A missing or disconnected middle fact
rejects. The complete nested signed-`i1` family transports both mandatory
conjuncts from two exact citations each: `Kd <= Md`, `Md <= -1`,
`Kd == divisor`, and `0 <= Mn`, `Mn <= Kn`, `Kn == dividend`. Its proof is the
ordered conjunction of two transitivity-under-substitution nodes; either
missing middle relation rejects the whole goal. The signed
width-at-least-two joint arm is likewise complete when both conjuncts use
direct two-citation chains: `divisor <= K`, `K <= -1`, and
`MIN + 1 <= M`, `M <= dividend`. The proof introduces only arm 2 and constructs
its ordered conjunction from those two transitivity nodes; a missing or
disconnected citation rejects the entire arm. A signed `i1` divisor
fact alone cannot prove its two-premise conjunction. When exact prior
`divisor <= -1` and `0 <= dividend` propositions are both independently
retained, the complete `i1` family cites them and constructs that conjunction;
a missing premise or wrong operand identity rejects. Missing, reversed,
weakened, mistyped, or wrong-divisor facts reject. Missing or excluded evidence
rejects. A complete two-citation transitive family also accepts exact prior
`1 <= K` and `K <= divisor`, or signed `divisor <= K` and `K <= -2`, only when
the middle term and operand identities match exactly. Its proof is one
`IntegerLessOrEqualTransitivity` node over the two citations; missing,
disconnected, reversed, or redirected pairs reject. An exact prior canonical
goal may be cited directly, while an exact prior canonical arm is introduced
only at its ordered disjunct index. Reconstruction now mirrors the producer's
recursive `LessOrEqual`/conjunction/disjunction shape rather than maintaining
separate safe-divisor and exceptional selectors. Redirected goals, reordered
joint conjunctions, and wrong operands reject. No result equation participates.

Fixed endpoint substitution now uses matching side-local dispatch on each side
of the trust boundary. Independent `substitution/one` modules enumerate the
one-equality arm before the existing `substitution/two` siblings. Equality
orientation, source citation order, endpoint identity, inner-relation
precedence, outer `IntegerLessOrEqualSubstitution` shape, rejection, and both
fixed frontiers remain unchanged; the verifier reconstructs its own evidence.

The fixed two-equality endpoint sibling keeps its established
`integer_selection/substitution/two` API as a facade over independent
side-local `two/selection` owners. Each retains outer equality, orientation,
inner equality, then affine-relation order and exact fact non-reuse. Production
constructs the same inner-then-outer substitution bytes; verification
independently checks the final-alias affine relation. Endpoint identity,
rejection order, and the exact two-equality frontier remain unchanged.

The fixed one-alias order transport keeps its established
`alias_transport/one` API as a facade over independent side-local
`one/candidates` owners. Both retain assumptions before semantic axioms,
equality orientation, and indexed relation order before endpoint-substitution
completion. Production alone materializes citation proofs; verification
rebuilds the transported proposition independently. Proof bytes, rejection
order, and the exact one-alias frontier remain unchanged.

The existing
proof rules and proof-bundle v19 codec need no further vocabulary change.
All other exact divide/remainder reconstruction remains on its trusted reducer
until an untrusted producer can materialize kernel-checkable certificates for
the accepted affine/correlated families without importing operation evidence.
The producer-side common spine recursively composes exact prior citations,
integer-order leaves, conjunctions, and arbitrary ordered disjunctions and
kernel-checks the result. The remaining work is to replace each trusted
definition-chain, cast-sandwich, affine-join, and correlated forbidden-root
analysis outcome with a normalized witness that proves those atomic leaves;
the common compositor does not treat the reducer's proposition as a premise.

The first such normalization prerequisite is producer-visible but not a proof
rule. `IntegerAffineWitness` binds a signed fixed same-carrier root and target
to a nonempty, strictly increasing list of exact prior semantic-axiom indices.
The kernel independently validates each selected equality and replays only
exact add, subtract, or multiply-by-same-carrier-literal steps, recomputing the
checked `A * root + B` coefficients. Each definition also carries one aligned
optional literal-landing index. An absent index requires an inline typed signed
literal; a present index must select one strictly earlier exact same-carrier
equality between the non-chain SSA operand and its typed signed literal. The
root must be an SSA value; stale or reordered indices, malformed equations,
missing, late, redirected, ambiguous, or unused landings, carrier or target
drift, ambiguous orientation, unsupported roots, and checked-arithmetic
overflow reject. This
normalizes the definition-chain facts shared by direct affine analysis and the
two branches of same-root/correlated analysis without trusting an analyzer's
coefficients. It neither derives an atomic order proposition nor crosses a cast
or shift. A second producer-visible checker maps one independently established
canonical root `<=` proposition through the checked form to one exact atomic
target `<=` proposition. Positive coefficients preserve order, negative
coefficients reverse it, and zero coefficients use the root-bound orientation
to select one of the two sound constant-bound directions. The checker
recomputes the mapped endpoint with checked arithmetic and rejects malformed
root/literal/target shapes, overflow, or an endpoint outside the carrier. It
accepts no proof or citation authority by itself. `IntegerAffineBound` is the
versioned composition rule around those two existing checks. It owns one
recursively checked root-bound child and one `IntegerAffineWitness`; the kernel
rechecks the ordered definition word, maps the child conclusion, and records
every selected semantic axiom in accepted premise closure. Non-order or
wrong-root children, stale/reordered/malformed definitions, target/carrier
drift, arithmetic failure, and changed mapped conclusions reject. Proof-bundle
v19 retains tag 12 and canonically encodes the aligned optional landing
indices; the registered calculus is v16, and the Rust kernel is v8.
The calculus root and kernel implementation bind both the affine- and
cast-checker sources.
The first bounded producer family uses this rule for one to four prior signed
fixed affine definitions whose exact retained root bound maps directly to a
canonical safe-divisor arm. Reconstruction and production enumerate shortest
words first and advance only prefixes accepted by the affine witness checker;
within each depth, semantic-axiom indices remain strictly ordered. The kernel
independently checks continuity, algebra, the mapped conclusion, and
accepted-premise custody. Missing root custody, incomplete, reversed,
redirected, or stale words, wrong targets, and noncanonical mapped bounds
reject. Root custody may now also use one exact prior landed literal or
value-alias transport. A typed `root == literal` citation substitutes the root
into either endpoint of one closed reflexive relation; a value alias instead
combines one directly cited integer bound at the alias endpoint with its
independently cited equality. One exact two-citation order chain may instead
reconstruct the root bound through one shared SSA middle under a checked
transitivity child. Direct roots remain preferred, then landed literals, alias
transport, and transitivity; equality facts stay in ledger order, while bound
and second-leg indexes use their exact value endpoint. A missing bound,
equality, or order leg, unsafe or mistyped literal, identity, non-value,
disconnected, redirected, cross-carrier, or same-citation join rejects.
Three-or-more-alias or three-or-more-leg root reconstruction,
words of five or more definitions, joins, cast/shift compositions, and
correlated results remain producer work, so neither complete exact row changes
trust.
An exact mapped affine bound may also close to the canonical arm through one
typed closed-literal order bridge on the unchanged target endpoint. A stronger
lower bound places the primitive bridge before `IntegerAffineBound`; a stronger
upper bound places it after. Candidate mapping supplies no authority: the
kernel rechecks the exact affine conclusion and the enclosing transitivity
certificate. A nonclosed, mistyped, redirected, or weaker bridge rejects, and
no variable-endpoint or cited-fact search is added.
Affine completion now lives in dedicated, side-local `affine_custody` modules.
Producer and reconstruction independently own the fixed four-definition
witness frontier, exact mapped bound, and optional closed relaxation; no
authority is shared.
Affine evidence selection now lives in dedicated, side-local
`affine_selection` modules. Producer and reconstruction independently preserve
the exact preference order across direct, literal-landed, fixed one-/two-alias,
and exactly-two-leg transitive custody before invoking affine completion; no
generic path search or additional evidence shape is introduced.
Prior-evidence primitives now live in dedicated, side-local
`integer_evidence` modules. The producer alone owns citation indices and proof
nodes; reconstruction independently resolves retained integer literals and
replays closed order. Selectors depend on these leaf helpers without sharing
authority, changing precedence, or expanding the search frontier.
Canonical integer coordination now lives in dedicated, side-local
`integer_selection` modules. The producer independently builds the recursive
Truth/conjunction/disjunction/order proof shape before the public entry applies
the kernel check; reconstruction independently replays canonical proposition
shape and fixed bound dispatch. Each preserves its prior precedence and finite
evidence frontier.
Certificate-entry custody now lives in dedicated, side-local
`certificate_entry` modules. The producer exposes a selected proof only after
the kernel accepts its exact context, goal, assumptions, and semantic axioms;
reconstruction independently projects the canonical scalar goal before retained
selection. Invalid projection or failed checking yields no authority, and
neither side imports the other's decision.
The producer's 30 certificate regressions and reconstruction's 25 independent
selection regressions now live in side-local `tests` modules. Production
facades are 35 and 608 lines respectively, while every test name and assertion
is retained; no proof logic, authority, precedence, or search frontier moved
between sides.
Verifier control-flow evidence propagation now lives in a side-local
`path_facts` module. It alone decodes retained condition predicates, binds
successor parameters, emits edge equalities before rewritten facts, and
deduplicates propagated facts. The reconstruction parent still owns traversal,
merge intersection, and certificate selection; this extraction grants no proof
authority and changes no fact order.
Per-operation obligation reconstruction now lives in a side-local
`operation_facts` module. It preserves the exact goal-free, proof-bearing,
structural-effect, then call dispatch order; only the proof-bearing branch may
choose canonical certificate custody or trusted sufficient reduction before
recording the pre-result axiom snapshot. CFG traversal and return intersection
remain in the parent, and an unclaimed validated operation still fails closed.
Terminator custody now lives in a side-local `terminator_facts` module. It owns
the exact Jump/Conditional/return/crash dispatch, successor fact propagation,
scalar-result equality, nominal-cleanup obligations, structural-return facts,
and the rule that Crash contributes no normal exit. CFG scheduling and final
all-return intersection are separately owned below; cleanup order, axiom
snapshots, and noncanonical cleanup status are unchanged.
Immutable machine reconstruction context now lives in a side-local
`machine_context` module. It alone derives the existing path-fact enablement
predicate, exact value-type proposition context, machine-parameter custody set,
and block/machine identity indexes. Traversal consumes that read-only context;
operation and terminator modules retain their independent decision authority,
and no dispatch, fact, proof, or search order changes.
Deterministic machine fact flow now lives in a side-local `machine_flow` module.
It owns the existing sorted-ready topological schedule, per-block all-incoming
fact intersection, and final all-return fact intersection. The parent retains
operation-before-terminator traversal; no successor, fact, exit, proof, or
search order changes.
One exact prior value equality may also transport a completed affine bound from
its checked target alias to the canonical goal endpoint. The producer replaces
that one endpoint, constructs the bounded affine relation directly, and wraps
it in `IntegerLessOrEqualSubstitution`; reconstruction repeats the same exact
identity selection. A missing, redirected, crossed, or mistyped target equality
rejects. The affine relation builder cannot recurse into another target alias,
so this adds one wrapper only and no alias-chain search.

One fixed sibling may instead carry a completed affine bound across exactly two
distinct same-carrier target equalities. It nests two
`IntegerLessOrEqualSubstitution` nodes outside `IntegerAffineBound`; missing,
reused, redirected, cyclic, or mistyped equalities reject. The constructor
builds the affine relation directly at the final alias and never recurses
through the general order prover, so a third target alias remains outside the
family.

One bounded mixed root-custody sibling may instead compose exactly two prior
order citations at an alias endpoint, transport that completed bound through
exactly one retained value equality to the affine root, and then apply
`IntegerAffineBound`. Its proof nests `IntegerLessOrEqualTransitivity` beneath
`IntegerLessOrEqualSubstitution`; missing or disconnected order legs and absent
or redirected equalities reject. The constructor calls the affine builder
directly, so it cannot add another equality or order leg and does not introduce
recursive path search. Three-or-more-alias and three-or-more-leg custody remain
outside the producer.

One fixed two-alias sibling may instead transport one directly cited bound to
the affine root through exactly two distinct retained value equalities. Its
proof nests two `IntegerLessOrEqualSubstitution` nodes beneath
`IntegerAffineBound`; the root, middle alias, and bound alias must be distinct
same-carrier values. A missing, reused, redirected, crossed, cyclic, or mistyped
equality rejects. The constructor has no recursive alias walk, and a third
alias remains outside the producer.

Generic fixed two-alias transport now places ledger/index enumeration in
independent producer and verifier `alias_transport/two/candidates` modules.
The unchanged `alias_transport/two` entry APIs still receive the final
completion callback. Outer equality, orientation, inner equality, then indexed
bound order, exact fact non-reuse, nested substitution bytes, callback order,
rejection, and the two-alias frontier remain unchanged; the verifier derives
its retained bound independently.

One literal-ending sibling may land the affine root through exactly one
intermediate value alias and one exact same-carrier literal equality. It proves
a closed reflexive integer order, substitutes the alias, substitutes the root,
and only then applies `IntegerAffineBound`. Missing, redirected, reused, or
mistyped equalities reject, and a second value alias is not followed. This is
another fixed two-substitution path, not a recursive alias search.

The contiguous pure-cast core also has a non-serialized checked witness.
`IntegerCastChainWitness` selects a nonempty, strictly increasing sequence of
canonical semantic equalities from one SSA root to one SSA target. Every step
must be an adjacent 8/16/32/64 fixed `IntegerExactCast` that is neither an
identity nor a widening; declared carriers and actual term types must agree
exactly. The checked result owns the complete carrier sequence, exact selected
axiom indices, and the intersection of all carrier ranges. Thus narrowing and
cross-sign casts normalize only their surviving representable values and never
become a total or lossy conversion claim. Reversed, stale, reordered,
discontinuous, cyclic, address, non-native, and target-drifted words reject.
This one core is shared by direct one-cast sandwiches and contiguous multi-cast
prefix/suffix sandwiches, but it neither proves cast definedness nor checks the
surrounding arithmetic families. Mixed widening/cast words remain a distinct
normalization problem.

`IntegerCastBound` is the versioned integration for that core. One recursively
checked root-bound child and one nonempty contiguous word of partial
fixed-native exact-cast definitions map the same mathematical literal endpoint
into the final carrier. The kernel rechecks the complete cast witness and
conversion and records every selected definition in accepted premise closure.
A non-order or wrong-root child, empty, stale, reordered, discontinuous,
total/widening-shaped, or cyclic cast definitions, target/orientation drift, or
a changed endpoint rejects. Proof-bundle v19 retains tag 13; the registered
calculus is v16 and the Rust kernel v8. Producer and reconstruction independently
follow the unique exact-cast SSA definition spine backward from the goal,
reject ambiguous target definitions, and require its source-ordered ledger
word. They perform no recursive path or permutation search. Cast-chain custody
now lives in dedicated, side-local `cast_custody` modules. Producer and
reconstruction independently own unique-spine selection, exact witness/kernel
replay, and final `IntegerCastBound` completion; the broader evidence selectors
retain their existing order and proof shapes. Cast evidence selection now lives
in dedicated, side-local `cast_selection` modules.

Each side's cast-chain owner separates known-root word recovery from non-cast
source discovery behind its unchanged facade. Independent
`cast_custody/chain/definitions` modules recover the exact root-to-target
definition word; `cast_custody/chain/source` modules recover the unique source
and first cast index. Backward ledger traversal, ambiguity and reuse rejection,
source ordering, the semantic-axiom-length cycle bound, and the finite
single-spine frontier remain unchanged; no evidence authority crosses the
producer/verifier boundary.

Exact-cast custody completion also separates ordered goal-target enumeration
from per-target witness replay. Producer and reconstruction
`cast_custody/completion` parents retain left-endpoint before right-endpoint
order and value eligibility; independent side-local `completion/target`
modules recover the exact cast word and construct or check its bound
conversion. Only production materializes and kernel-checks `IntegerCastBound`.
Proof bytes, per-target rejection, and the finite unique-spine frontier remain
unchanged.

Producer and reconstruction independently preserve direct-bound,
landed-literal, fixed one-alias,
closed-strengthening, alias-landed-literal, then fixed two-alias precedence;
source-carrier literal remapping remains with cast custody. No proof shape or
search frontier changes. This completes contiguous cast-chain custody for exact
divide/remainder goals.

Closed-strengthened alias transport separates fact discovery from cast
completion. Independent producer and verifier
`alias_transport/cast/stronger/candidates` modules retain equality-first,
orientation-second, then bound-order enumeration and exact carrier/endpoint
eligibility. Their parents pass the same cited proof nodes or retained facts to
side-local completion. Citation identity, closed bridge/substitution bytes,
rejection order, the single-alias/single-bridge frontier, and cast-family
precedence remain unchanged.

Alias-landed-literal transport uses the same ownership boundary. Independent
producer and verifier `alias_transport/cast/literal/candidates` modules retain
root-equality-first, orientation-second, then distinct landing-equality order,
including exact fact non-reuse and carrier eligibility. Existing completion
owners receive the same two cited proof nodes or retained terms. Equality
identity, nested substitution bytes, target-endpoint order, rejection, the
single-alias/single-landing frontier, and cast-family precedence remain
unchanged.

One bounded source-affine composition may now provide
the cast child: producer selection follows the unique cast spine to its
non-cast source, remaps the canonical literal endpoint into that carrier, and
runs the existing finite affine selector against only the semantic prefix
strictly before the first cast. The resulting proof is exactly
`IntegerCastBound(IntegerAffineBound(...))`. Reconstruction independently
repeats the unique-spine, endpoint-remap, prefix-boundary, affine, and cast
checks. Missing or ambiguous cast sources, late affine definitions or literal
landings, unrepresentable endpoints, and either failed checker reject. The
existing direct/literal/fixed-alias cast precedence is unchanged, and no new
rule or proof-bundle-v19 field is introduced. This does not promote either
whole row.

This affine-to-cast selector separates endpoint orientation from resolved
completion. Independent producer and verifier `cast_selection/affine` parents
retain right-before-left endpoint order and value eligibility; side-local
`cast_selection/affine/completion` modules own source-spine recovery, literal
remapping, prefix-bounded affine custody, and cast completion. Source goals,
proof bytes, rejection order within each orientation, direct-cast precedence,
and the finite frontier remain unchanged.

The bounded dual accepts one directly cited same-carrier source
bound, a unique nonempty partial-cast spine, and one later finite affine word.
Producer selection remaps the cited literal into the cast target, constructs
`IntegerCastBound` from that exact assumption, and completes
`IntegerAffineBound` only when every affine definition and optional literal
landing is strictly after the final cast. Reconstruction independently repeats
the same direct-bound, cast, remap, affine, and strict-boundary checks. Existing
affine families retain precedence. Missing direct custody, ambiguous cast
spines, unrepresentable endpoints, and affine definitions or landings at or
before the last cast reject. Its proof is exactly
`IntegerAffineBound(IntegerCastBound(Assumption))`, with no new rule or v19
field. Shift/cast, joins, correlated results, and all other affine/cast shapes
remain trusted-reducer work; `fully-derived false` is unchanged.

One exact forward affine/cast/affine sibling composes those two checked
boundaries without adding proof vocabulary. Starting from one direct
same-carrier root-bound citation, production maps the exact endpoint through a
finite pre-cast affine word, the unique nonempty partial-cast spine, and a
finite post-cast affine word. The pre-cast definitions and literal landings
must all precede the first cast; the post-cast definitions and landings must all
follow the final cast. Its proof is exactly
`IntegerAffineBound(IntegerCastBound(IntegerAffineBound(Assumption)))`.
Reconstruction independently enumerates the same fixed affine frontiers and
rechecks both affine conversions, cast conversion, boundaries, and source
custody. Forward endpoint calculation selects candidates but grants no
authority because each existing kernel node rechecks the exact proposition.
Missing direct custody, ambiguous cast custody, an unrepresentable endpoint, or
boundary drift rejects. One-sided families retain precedence; no inverse
mapping, alias recursion, rule, or v19 field is added. Broader affine/cast
families and both exact rows remain trusted, and `fully-derived false` is
unchanged.

The producer and reconstruction implementations now place this cast-adjacent
affine responsibility behind matching small dispatch facades. Independent
side-local `cast/direct`, `cast/sandwich`, and `cast/endpoint` modules own the
direct-root case, the fixed affine/cast/affine case, and exact typed endpoint
remapping. The parents retain direct-before-sandwich precedence. Citation
order, strict definition boundaries, proof shapes, rejection behavior, and the
fixed witness frontier do not change, and no authority crosses the producer/
verifier boundary.

The direct cast-to-affine sibling likewise separates cast/root-bound
enumeration from completion. Matching producer and verifier `cast/direct`
parents retain semantic cast-root order, unique source-spine recovery, and
requirement order; independent `cast/direct/completion` modules own endpoint
remapping, exact-cast completion, and the strictly post-cast affine suffix.
Assumption identity, proof bytes, the last-cast boundary, rejection,
direct-before-sandwich precedence, and the finite frontier remain unchanged.

The fixed affine/cast/affine sibling likewise separates candidate enumeration
from completion. Matching producer and verifier `cast/sandwich` parents retain
cast-root, source-spine, requirement, and root-endpoint order; independent
`cast/sandwich/completion` modules own the mapped-prefix, exact-cast, then
affine-suffix composition. Citation identity, strict first/last-cast
boundaries, nested proof shape, rejection, and the fixed frontier are
unchanged.

Boundary-aware affine custody is also owned by matching side-local modules.
Producer and verifier `affine_custody/boundary` modules independently complete
strict post-boundary roots, and their `affine_custody/mapped` siblings
independently map exact pre-boundary roots to the requested target. Parent
modules retain ordinary root completion and unchanged re-exported APIs.
Citation order, strict boundary tests, proof shapes, rejection, and the fixed
four-definition frontier remain unchanged; the verifier does not consume the
producer's mapped proposition as authority.

Pre-boundary affine mapping likewise separates target candidate enumeration
from per-witness completion. Side-local producer and verifier
`affine_custody/mapped/completion` modules independently enforce the strict
definition and literal-axiom boundaries, validate the witness, and construct
or replay its exact mapped bound. Their `mapped` parents retain requested-
target and definition-word order. Proof bytes, candidate rejection, and the
fixed four-definition frontier remain unchanged.

Post-boundary affine custody mirrors the same responsibility split. Side-local
producer and verifier `affine_custody/boundary/completion` modules
independently enforce strict definition and literal-axiom boundaries before
delegating an eligible witness to ordinary affine-custody completion. Their
`boundary` parents retain goal-target and definition-word order. Proof bytes,
candidate rejection, and the fixed four-definition frontier remain unchanged.

Affine-witness candidate coordination likewise separates goal-target
enumeration from exact fixed-target completion. Side-local producer and
verifier `affine_custody/candidates/fixed` modules independently align literal
landings and form candidates for one requested target. The bounded
definition-word frontier is still computed once per parent invocation, and
target-first then word-order precedence, completion, rejection, and the fixed
frontier remain unchanged.

The root-bound child may also come from exactly one retained same-carrier
`root == literal` fact when that literal equals or strengthens the canonical
bound endpoint. The producer remaps the endpoint into the source carrier,
checks the closed bridge to the landed literal, substitutes the root endpoint
once, then applies `IntegerCastBound`; reconstruction independently selects the
same exact equality and rechecks the bridge. Direct bounds remain preferred.
Missing, redirected, mistyped, or weaker facts reject. One exact same-carrier
`root == alias` citation may instead transport one directly cited canonical
bound at that alias. Its fixed proof nests one
`IntegerLessOrEqualSubstitution` under `IntegerCastBound`; reconstruction
repeats the same exact equality/bound selection. Missing, redirected,
cross-carrier, or weaker bounds reject. The untrusted producer routes this
one-alias order transport for both cast and affine completion through one
indexed constructor; reconstruction independently mirrors that constructor,
so the family is no longer re-enumerated per completion rule. One closed
source-carrier endpoint
bridge may also strengthen the cited alias bound. Its fixed proof nests
`IntegerLessOrEqualTransitivity` under the one substitution; exact alias bounds
remain preferred. Production and reconstruction recheck the same bound, bridge,
and equality. They do not search alternate bounds or aliases, and a weaker
bridge rejects. One fixed sibling may instead land that alias through exactly
one same-carrier `alias == literal` citation. It proves the
closed canonical bridge, substitutes the alias, substitutes the root, then
applies `IntegerCastBound`; production and reconstruction select the same two
exact equalities. Missing, reused, redirected, mistyped, or weaker literals
reject. One fixed two-alias sibling may instead transport one directly cited
canonical bound through exactly two distinct same-carrier value equalities. It
nests two `IntegerLessOrEqualSubstitution` nodes under `IntegerCastBound`;
producer and reconstruction independently enumerate that exact three-citation
shape through their own local indexed constructor shared by cast and affine
completion. Those fixed one-/two-alias constructors now live in dedicated,
side-local `alias_transport` modules rather than the broader certificate and
reconstruction engines. The cast-specific closed strengthening and
alias-landed-literal shapes live beside them while retaining their distinct
transitivity and substitution proofs. They prefer every one-alias family and
perform no recursive or parameterized alias walk.
Missing, reused, redirected, crossed, cyclic, mistyped, or weaker facts reject.
A third alias, literal landing through two aliases, affine/cast, shift/cast,
joins, and correlated results remain outside this sibling; neither complete
exact row changes trust and `fully-derived false` remains.

The shared exact-shift core has a matching non-serialized checked witness.
`IntegerShiftChainWitness` selects a nonempty, strictly ordered sequence of
canonical exact-left or exact-right shift equalities from one fixed-native SSA
root to one SSA target. Every nonclosed count names an exact earlier canonical
equality landing that term; closed counts name no redundant fact. Count
carriers may vary across fixed-native widths, but every count must be
nonnegative and strictly less than the value width. The checked form retains
the exact ordered direction/count/index word rather than a cumulative count,
which would be unsound for mixed left/right composition. Nonexact operations,
carrier drift, unlanded, late, reversed, mistyped, negative, or out-of-range
counts, stale/reordered/discontinuous/cyclic definitions, and target drift
reject. This common core is usable by direct, cast-adjacent, affine-adjacent,
and divide/remainder-adjacent shift families. It accepts no proof authority,
establishes no root custody, and proves no overflow bound or interval preimage.

The last trusted correlated family also has a non-serialized checked custody
form. `IntegerCorrelatedForbiddenRootWitness` binds the dividend and divisor's
complete nonempty landed-literal affine walks, their shared direct signed
fixed-native parameter, strict source order and disjoint definitions, and the
exact two tight unary signature-bound axiom identities. It independently
replays every exact add/subtract/multiply definition and each nonclosed
sibling's prior canonical equality, recomputes both nonzero affine forms, and
solves the divisor's integer-lattice zero and `-1` equations. The latter is
forbidden only when the dividend form evaluates to the carrier minimum at the
same root. No forbidden root yields the ordered two-bound conjunction; roots
covering the entire retained interval yield falsehood; partial safety rejects.
Stale or redirected definitions/literals/bounds, correlation, branch order,
type or root drift, constant collapse, one-sided bounds, and arithmetic failure
all reject. The checked result retains exact branch, landing, bound, interval,
forbidden-root, and conclusion identities but accepts no proof authority.

A certificate conversion for the checked correlated result remains producer
work: `IntegerAffineBound` covers one affine target bound, not the correlated
two-branch lattice conclusion. Producer selection of richer composed root-bound
proofs also remains before either exact divide/remainder row can leave
`TrustedJudgment`. Proof-bundle v19 retains rule tag 13 for the complete
contiguous cast word; terminal codec v18 and installation record v24 remain
unchanged, and no trust status is promoted.

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
