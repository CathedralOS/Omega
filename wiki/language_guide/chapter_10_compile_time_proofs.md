# Chapter 10: Compile-Time Proofs

Compile-time proofs are not a second programming language. A `proposition`
declares a proof formula. Ordinary machines establish and consume those
formulas through their contracts; a machine used only to establish facts emits
no runtime code.

The basic shape is:

```text
requires + body facts -> ensures
```

If the checker can prove that implication, the machine is a proof artifact. If
it cannot, the contract is only an unchecked promise and must be rejected or
treated as an explicit boundary.

## Proposition declarations

`proposition` is the declaration category for a proof formula. It has no
runtime result, value position, layout, effects, work attribution, termination
contract, or executable body. Proposition applications appear wherever an
ordinary contract fact may appear:

| Declaration | Meaning |
|---|---|
| `machine` | named, contracted computation or transition system |
| `proposition` | proof formula |
| `domain` | qualification of one carrier |
| `trait` | requirement and evidence interface |

Internally, proposition families inhabit the proof-only `Prop` universe. A
proof is evidence inhabiting one proposition application. `requires` and
`ensures` expose that evidence as erased input and output facts on the
machine's entry and terminal edges; they do not add runtime parameters or
result fields.

Erasure does not make proof evidence a runtime carrier. Runtime representation,
occurrence minting, and carried authority are separate questions: a selected
opaque representation says how a value's bits move, an authorized operation
says who may create a valid occurrence, and a proposition or domain records the
fact that occurrence establishes. None substitutes for another.

```omega
proposition rat_equivalent(left: Rat, right: Rat);

machine preserve_equivalence(left: Rat, right: Rat)
requires
    rat_equivalent(left, right)
ensures
    rat_equivalent(right, left)
{
    ...
}
```

A primitive fact-only proposition ends with `;`. A witness-bearing proposition
publishes one canonical carrierless interface through an `evidence` clause.
The interface is owner-authorized public proof content, not an executable body,
an ordinary generic bound, or the producer implementation. Any conformance
selected while establishing the proposition must supply that complete
interface. Different conformances may carry different witnesses without
changing the proposition's nominal symbol.

The clause is signature content:

```omega
proposition converges_together<machine Left, machine Right>(
    left: CauchySeq<Left>,
    right: CauchySeq<Right>
) evidence ConvergenceEvidence<Left, Right>;
```

Like every independently nameable declaration, a proposition is package-private
unless marked `pub`. Publishing a bodyless proposition exports the family name
and signature, not a universally true instance. Trust and admission attach to
the exact evidence-producing boundary or unchecked `ensures` edge; they never
attach merely because a proposition family is public.

`evidence` answers one question: what may proof-only code project from a term
of this proposition? A witness-bearing proposition names exactly one
interface. The interface is not a result type or ordinary `where` constraint,
and `=` remains the distinct transparent-alias form. A bare carrier such as a
modulus machine would not state the laws tying that carrier to this proposition
application; the interface publishes the carrierless members and their laws as
one elimination contract.

`evidence` is contextual after a proposition signature; it is not globally
reserved as an identifier.

The evidence interface is nevertheless normalized and fingerprinted. Revising
it is a breaking proof-interface change even though the proposition retains its
name. Runtime extraction of a witness requires an ordinary `Type`-level package;
proof evidence cannot be opened into runtime computation merely because its
representation erases.

A transparent proposition definition uses `=`:

```omega
proposition cauchy<machine Sequence>(value: CauchySeq<Sequence>) =
    converges_together<Sequence, Sequence>(value, value);
```

The right side is an existing proof expression and must be eligible in a fact
position. The alias expands before semantic normalization, creates no new
proposition identity, and inherits the expansion's requirements, trust,
fact-or-witness classification, and evidence interface. Its source name remains
available for diagnostics and debug maps.

Generic proposition parameters use the same category explicitly:

```omega
trait Reflexive<C, proposition Relation>
where
    proposition Relation(left: C, right: C);
```

This is distinct from a resultless operation requirement such as
`where machine Visit(item: &T);`, which describes an executable procedure.
Applications of either a `bool`-returning machine or a proposition are facts in
`requires` and `ensures`; the proposition form additionally permits facts with
no decision procedure. A bare Boolean expression in a fact position means that
it is `true`, so `decides(a, b)` and `decides(a, b) == true` normalize to the
same fact. No Boolean-to-proposition bridge operation is required.

Fact-only versus witness-bearing is part of normalized proposition identity.
Transparent aliases do not enter fingerprinted terminal Psi; primitive
proposition symbols, their binders, classification, and normalized evidence
interface do.

Evidence retains validity scope and trust provenance. A retained proof term is
always copyable: consumable authority belongs to an affine or linear `Type`
carrier, which may have zero runtime layout, rather than to `Prop`. Copyability
does not make a proof timeless. A term may still be borrow-scoped,
entry-scoped, invalidated by a write, or tied to a live lease; all copies share
that validity and expire together. Admission marks the evidence chain, not the
proposition name, so two proofs of the same formula may carry different hidden
witnesses and trust, and a deployment profile may accept one and reject the
other.

## Named evidence terms

Every checked `ensures P` establishes an erased proof term for `P`. A
witness-bearing proposition additionally gives that term projectable members.
Naming a `requires` clause binds the exact incoming term; naming an `ensures`
clause declares an exact outgoing term:

```omega
machine transitive<machine First, machine Middle, machine Last>(
    first: CauchySeq<First>,
    middle: CauchySeq<Middle>,
    last: CauchySeq<Last>
)
requires
    left_evidence: converges_together(first, middle)
    right_evidence: converges_together(middle, last)
ensures
    result_evidence: converges_together(first, last)
{
    result_evidence = ComposedEvidence<
        left_evidence.modulus,
        right_evidence.modulus
    >;
}
```

An unnamed `requires P` asks only that `P` be established. A named
`requires proof: P` additionally retains one exact proof term so the body may
project or forward its hidden witness. Naming therefore changes the public
proof-call surface and is a breaking API revision even though the proposition
required is unchanged.

The incoming names are local aliases over positional erased proof parameters.
A caller supplies them in clause order after the call's `;` separator; no
visible-fact search, conformance search, or name matching occurs. The separator
marks the boundary between ordinary `Type` arguments and `Prop` inhabitants.
It is omitted when the proof lane is empty. An evidence-only call retains the
separator, as `callee(; proof)`, so a proof term cannot be confused with an
ordinary argument:

```omega
let (;
    result_evidence: combined_evidence
) = transitive(
    first,
    middle,
    last;
    first_evidence,
    second_evidence
);
```

Projection is ordinary member syntax. Repeating `left_evidence.modulus`
projects the same opaque symbol because both expressions use the same retained
term. Forwarding the binding preserves that term. Separate introductions may
carry different terms even when they inhabit the same nominal proposition.
No `open` form or ambient producer inference exists.

A named `ensures` binding is definitely assigned exactly once on every exit
whose outcome guard makes that clause applicable. Assignment selects a named
complete producer conformance privately in the proof body. The checker still
checks the nominal proposition and the producer's complete normalized evidence
rows. The checked frontend accepts this introduction directly for a concrete
subjectless conformance alias and retains its exact conformance, trait,
canonical instantiated argument identities, and realization rows. Thus an
application whose evidence declaration is `Evidence<T>` selects
`Evidence<i32>` only when its exact proposition binder argument is `i32`;
another argument and an unresolved open endpoint both reject.
Forwarding instead uses ordinary assignment:

```omega
result_evidence = existing_evidence;
```

This form is an erased identity binding, not a runtime load/store and not a new
proof introduction. The target must be a named `ensures` term of the current
machine, the source must be an exact named `requires` term of that machine, and
their normalized proposition application and evidence interface must match.
The outgoing slot then denotes the incoming term itself. A visible matching
fact cannot replace the source assignment. Checked lowering already enforces
that every named output is assigned exactly once on every ordinary outcome of
the finite named-state graph. Assignment is ordered at its source statement and
carried across named transitions; assigning twice rejects, while a crash-only
outcome produces no outgoing proof lane and need not assign it.

An outcome guard is declared as one group keyed by an exact case of the
machine's declared result sum:

```omega
machine Search::find(items: &[Item], target: Item) -> SearchResult
ensures
    SearchResult::Found -> {
        in_bounds: result.index < items.len;
        items[result.index] == target;
    }
{
    ...
}
```

At most one authored group names a case in one declaration layer. Named
selectors remain unique across the complete machine contract. The case path is
resolved only in the declared result type and normalizes to the exact nominal
case; it is not inferred from the proposition, visible facts, assignment sites,
or body shape. A non-sum result, unknown case, duplicate case group, Boolean
guard, or duplicate selector rejects. Moving a guarantee to another case and
renaming a public selector are breaking proof-interface revisions; reordering
groups or rows and renaming a caller-local term are not.

Named and unnamed rows have the same path coverage and different producer
discharge forms. On every ordinary exit producing the guarded case, a named row
is assigned one exact evidence term, while an unnamed row is proved from that
exit's path facts after substituting the concrete result payload. A proof at a
shared join covers the row only when all qualifying incoming paths establish
it. Other result cases neither assign nor prove the group, and a crash-only exit
produces no result case. The braces are contract organization only: no source or
artifact aggregate, package, group value, projection, multiplicity, or group
identity exists.

Name a `requires` clause only when its body projects or forwards the term.
Changing `requires P` to `requires proof: P` adds an explicit erased input and
is a breaking call-interface revision. Named `ensures` labels are public output
selectors. Renaming one breaks callers that select it. Adding a named guarantee
does not break existing callers: an unselected proof term is not retained, while
the proposition still enters the caller's fact catalog.

## Evidence output lanes

Calls keep `Type` results and `Prop` evidence in separate output lanes, mirroring
the input-side `;` separator. A call never constructs a source-visible aggregate
containing both universes. The ordinary form binds only the declared runtime
result and retains no projectable outgoing witness:

```omega
let quotient = divide(numerator, denominator);
```

Every applicable `ensures` proposition still enters the caller's fact catalog.
When the caller needs the exact witness for projection or forwarding, it names
that public `ensures` slot after `;`:

```omega
let (
    quotient;
    nonzero_evidence: proof
) = divide(numerator, denominator);
```

The slot name is public API; the name after `:` is the caller-local term.
Selected evidence outputs are named rather than positional because capture is
optional and selective. An omitted slot contributes its fact but creates no
caller-local term. A same-name shorthand may omit `: local_name`. Proposition
terms are copyable, so capturing or omitting one adds no runtime operation,
storage, cleanup, or fuel.

An evidence-only call leaves the `Type` lane empty:

```omega
let (;
    result_evidence: combined_evidence
) = prove_result();
```

The call still executes exactly once. Its runtime effects, crashes, and fuel are
those of the ordinary call and callee body. There is no generated output type,
reserved `value` field, package projection, partial package move, or package
identity. Runtime results retain their declared Type and ordinary multiplicity;
captured proof terms retain their proposition, exact witness identity, validity
scope, and derivation provenance independently.

Outcome-guarded evidence is selectable only in the applicable outcome arm. The
same separator divides the case's runtime payload from its proof bindings:

```omega
transition allocate(size) {
    Success {
        extent;
        granted_evidence: grant
    } -> use(extent; grant)

    Error { error } -> report(error)
}
```

The proof slot does not exist on inapplicable paths. Definite assignment remains
per outcome: the producer assigns each named `ensures` term exactly once on each
exit where its guard applies. A caller may select any subset of applicable proof
outputs, and adding another guarantee does not force existing patterns to grow.

Every named or unnamed proposition in the guarded group enters the caller fact
catalog only after flow establishes that exact result case. Omitting a named
selector omits only the caller-local evidence term; it does not make the fact
unconditional and does not leak it into sibling arms. An unnamed row likewise
mints no caller-local term. The guarantee validity scope is the intersection of
the result occurrence, every normalized value occurrence referenced by its
proposition, and any scope retained by its evidence interface. A fact over
borrowed content is borrow-scoped and invalidated by an intersecting write; a
fact solely about an owned immutable result may remain timeless even when the
implementation originally computed that result from borrowed input.

Requirement guarantees are inherited by a satisfying machine. The satisfier's
authored case group adds rows; omission never removes or weakens inherited rows,
and an exact restatement rejects as redundant. The effective concrete contract
merges the pinned requirement rows with the additions. Calls through the
requirement see its published surface, while direct calls may use the stronger
concrete surface.

Trait machine requirements may publish named `requires` and `ensures` lanes.
They use the same syntax and call separator as concrete machines; there is no
trait-specific proof package or forwarding form. The requirement owns the
ordered input propositions, evidence interfaces, and public output selectors.
Incoming binding names remain callee-local aliases and a satisfier may rename
them without changing the requirement application. Outgoing selector names are
part of the requirement's public proof API; a satisfier cannot rename, omit, or
replace them, and changing one is a breaking revision.

Every proposition application in a requirement lane must close over subjects
bound by that requirement's ordinary parameters, result, static telescope, or
declared proposition parameters. A named lane does not bind a hidden runtime
subject or carry an otherwise expired occurrence between calls. Evidence over a
borrowed or revisioned subject retains the ordinary intersection of validity
scopes.

A satisfying machine proves and assigns the inherited lanes under the same path
coverage rules as a concrete declaration. Its private producer conformance and
additional direct-call guarantees remain implementation content. A default
realization obeys the same rule. Static and dynamic requirement calls expose
only the requirement-owned witness: dynamic selection may establish the opaque
witness promised by the requirement, but no satisfier-private evidence term,
producer identity, or varying projection becomes public. The proposition's
declared evidence interface is the complete elimination surface.

The artifact keeps proposition identity, evidence-term identity, and
derivation provenance separate. The first names the claim, the second preserves
the exact hidden witness across projection and forwarding, and the third records
how the claim was established and which admitted premises it trusts. Terminal
Psi already carries forwarded terms as dense source-handle-free vocabulary
identities over the exact proposition application and a structured carrierless
interface; the application and term interface must agree, and forwarding
contributes one row. Canonical positional rows for the selected terminal
machine's named `requires` and `ensures` lanes now refer to that exact ID, and a
forwarded pair shares one ID. A selected producer instead carries a separate
canonical proof-bundle provenance identity keyed to its fresh ensured term and
retaining its exact conformance, evidence trait, and normalized rows. That
provenance changes proof identity, not semantic identity or runtime behavior.
Each ensured realization pipeline retains its public output selector beside the exact
term ID. A call-site capture row binds a selected callee lane to one fresh
caller-local term; omitted lanes mint no term. The ordinary result remains on
the canonical runtime `Call` operation, and proof rows add no executable work.
Projection of the complete conformance surface remains unfinished.

## Explicit relevance

Relevance belongs to a binding occurrence, independently of its type,
validity scope, and provenance. For a `Type` binding it is also independent of
that type's multiplicity. An erased field uses the same
bracket-property convention as other binding properties:

```omega
data Certified<T> {
    value: T;
    proof [erased]: Valid<T>;
}
```

The checker retains `proof` in the typed and proof calculi but lowering gives
it no field offset, address, runtime read, or runtime cleanup. Erased data may
be consumed by contracts, proofs, other erased bindings, and statically checked
authorization for an effectful operation; it may not determine runtime data or
control. This noninterference rule is checked through every call and
projection. `[erased]` is therefore a relevance judgment, not a promise that
the implementation happens not to inspect the field.

A proof-machine result likewise exists only in proof computation. It may feed
another proof machine or an erased initializer, and a statement-position call
may cite the machine for its established facts, but its result cannot initialize,
return, branch, or otherwise determine runtime data.

Erasure does not discharge Type obligations. An explicitly erased Type ghost
may remain affine or linear, borrow-scoped, lease-scoped, content-bearing, or
provenance-bearing, and its obligations remain in the compiler frontier until
explicitly consumed. What is forbidden is a runtime destructor or cleanup
action that relies on erased representation. A containing value cannot leave
scope while an erased linear Type obligation remains live. Proposition proof
terms are different: they are intrinsically proof-only and always copyable.

A structurally zero-layout Type value needs no `[erased]` marker merely to cost
zero bytes. It remains an ordinary value and carries ordinary ownership and
multiplicity. Conversely, `[erased]` cannot be used to delete the bytes of a
representable runtime value; it is a checked relevance assertion for a
specification-only occurrence.

Construction normally supplies an erased term even though it produces no
runtime code. Omission is derived only for a structurally visible and
accessible nullary constructor; the compiler does not invoke a general
inhabitance judgment or synthesize a zero/default value. Ordinary authored
field defaults remain ordinary defaults.

For currently resolved and nameable checked-shape holders and non-generic
evidence types, the executable slice elaborates an omitted erased initializer
only when exactly one payloadless constructor with no common fields determines
the term. Holders may also be closed synthesized generic records when the
instance is selected by an explicitly typed local initializer, direct exact
assignment, exact return, or one parameter signature shared by every
same-name free-call candidate. A syntactically direct `self.method(...)` call
uses the same rule over the exact enclosing attached-data owner; an explicitly
typed local receiver or direct `self.field` receiver uses its exact nominal
owner. The implicit receiver is not a value argument. Computed, chained, and
dynamic receivers remain fail-closed. This contextual record elaboration does
not infer type arguments from fields. Pure and mixed common-field/
case generic sums admit multiple exact closed instances per generic base in the executable slice. A
closed annotated local, direct assignment, agreeing free/direct-self call
parameter, or return destination selects construction identity; an exact local,
parameter, or attached-self-field subject selects
destructure identity. Other bare constructor contexts retain the unique-
instance fallback and fail closed when more than one identity is possible.
Nested concrete generic payload records reach the same synthesis fixpoint. The
semantic typed tree receives the constructor term
before proof and multiplicity checking. Ambiguous, absent, generic, or
otherwise ineligible evidence constructors still require an explicit term;
omission never invokes a default or general inhabitance search. Native erasure
admits non-generic transparent records, sums, mixed common-field/case shapes,
those closed generic-record instances, and the exact closed generic-sum cohort.
Closed plain records, sums, and mixed shapes may also have attached machines
when every attachment is an ordinary checked body with no unresolved machine
parameters. This includes
a closed synthesized instance of a generic record whose bodyful attached
machine is cloned and fully substituted for that instance; the generic template
itself has no runtime storage. Their machine storage and runtime contained-machine
topology use the erased-stripped fields, while semantic ownership and proof
obligations retain every field. Erased payloads do not
change tags or case numbering; they remain visible to semantic exhaustiveness
and obligation checking. The compiler fails closed for unresolved generic uses,
ambiguous generic record/sum construction contexts, computed, chained, or
dynamically selected receivers, placed views, wire/codec and ABI faces, and attached
machines over unresolved generic uses, non-checked supply modes, or unresolved
machine parameters.

Runtime layout, ABI classification, codec shape, and placement offsets use the
erased-stripped form. Nominal type identity and semantic fingerprints retain
the erased binding and its type. A placement gives an erased field no physical
offset; any fact it carries must instead be established by the checked or
admitted placement plan.

## Machines As Proofs

This machine proves a simple ordering fact:

```omega
machine distinct_indices(
    i: u64,
    j: u64
)
requires
    i < j
ensures
    i != j
{
}
```

The empty body is valid only if the checker can prove the guarantee from the
requirement and built-in arithmetic/order rules.

This machine proves a closed arithmetic fact:

```omega
machine pythagorean_3_4_5()
ensures
    3nat * 3nat + 4nat * 4nat == 5nat * 5nat
{
}
```

The checker reduces both sides to the same `Nat` value, then closes the equality
by reflexivity. The body does not need to simulate computation.

A theorem-only machine has no `Type` result. Its parameters state universal
variables, `requires` states hypotheses, and `ensures` states conclusions. A
machine returns a value only when it genuinely computes that value in addition
to proving its contract. Algebraic law slots and quotient-congruence theorems
are theorem-only; a dummy `-> Self` result is not induction evidence and must
not be required merely to carry a proof.

## Typed Facts

Proof facts must be typed.

```omega
3nat * 3nat
```

is math over `Nat`.

```omega
3i32 * 3i32
```

is machine arithmetic and carries machine obligations such as width and
overflow behavior.

The same operator spelling can exist in both worlds. The operand types decide
which proof rules apply.

Calls and projections in fact position are denotational terms. For example,
`add_int(a, b).pos` denotes the `pos` field of the pure call result; it creates
no runtime temporary, move, or loan. Such a call must be total and pure. A
fact's validity is the intersection of the validity and revision scopes of
every occurrence it references, transitively through those calls. An
intersecting write or revision transition invalidates the affected fact.

This does not transfer custody into `Prop`. A proposition may mention a linear
value or result, and copies of that proposition remain erased and copyable;
the actual `Type` occurrence retains its independent multiplicity and custody.
"No loan" here means no new runtime loan: a fact depending on an existing loan
still expires with that loan.

## Total Specification Arithmetic

Every term in `requires`, `ensures`, a domain predicate, or a guarded `crashes`
route is total. Exact arithmetic is admitted after its ordinary formation
obligations are proved. Wrapping and Saturating arithmetic remain admissible
after any primitive obligations outside their overflow policy are proved; for
example, neither policy makes division by zero a result. Direct Trapping
arithmetic is not a proposition term: its failure transfers runtime control,
and contracts do not execute.

A domain `requires` row must resolve to `Prop`. A machine returning `bool` is a
value term, not an implicit proposition and not a validator invocation hidden
inside qualification. Transparent proposition bodies may contain eligible total
pure machine calls as denotational terms under the fact-call rule above. When a
transparent proposition projects one of its parameters, substituting a call
result for that parameter retains the exact call-and-projection eligibility
certificate rather than hiding the call behind the proposition name. The call's
contracts and validity scopes remain in the proof, while no runtime call is
emitted merely because the proposition is used.

Use an explicit proof view when a contract needs unbounded mathematics:

```omega
requires
    embed(left) + embed(right) <= embed(i32::Maximum)
```

For every fixed-width integer and address carrier, `embed` produces proof
`Int`. Unsigned and address embeddings additionally establish nonnegativity and
their exact carrier upper bound. The projection has no runtime representation,
does not alter the source qualification, and cannot determine runtime data or
control. Floats instead use `Float::meaning32` or `Float::meaning64`, whose
`FloatMeaning` result preserves finite rational value, signed zero, infinity,
and NaN explicitly.

Removing a policy with `as` is a different statement:

```omega
requires
    embed(right) >= 0
    embed(left) <= embed(i32::Maximum) - embed(right)
ensures
    result == (left as i32) + (right as i32)
```

The result expression uses Exact fixed-width arithmetic after the earlier facts
prove its intermediate result representable. An Exact operation cannot use the
proposition containing that same operation to justify its own formation, and
this is not shorthand for the unbounded expression above.

Proof `Nat` remains the natural carrier for induction, counts, and nonnegative
resource coordinates. Its ordinary subtraction is Exact and forms only when
the right operand is proved no greater than the left. Clamping is spelled
`Nat::saturating_sub(left, right)`; bare `Nat - Nat` never silently truncates.
An exact `Int as Nat` conversion similarly requires a nonnegative source.

Executable Trapping arithmetic independently creates a compiler-derived crash
site. A specification occurrence creates no crash edge. Authored `crashes`
routes are total may-ceilings, and coverage checks each derived guard `D`
against the authored alternatives `C_i` by requiring
`D implies (C_1 or ... or C_n)`. See
[Total Specification Arithmetic](../design_briefs/total_specification_arithmetic.md)
for the complete policy bridges and Terminal-Psi rules.

## Proof-Only Data

`Nat` and its kin are currently classified as proof-only: unbounded, with no
machine layout, no ZII obligation, and no runtime existence. Recursive data is
legal, and recursion is the present structural reason no finite layout can be
derived:

```omega
data Nat {
    case Zero;
    case Succ(n: Nat);   // recursive: no layout is derivable — proof-only
}
```

Working rules:

- **Proof-only is computed, never spelled.** A type is proof-only when it is
  recursive (directly or mutually) or any field's type is proof-only. There
  is no marker; writing recursive data is the opt-in, and diagnostics name
  the classification ("`Nat` is proof-only: recursive data has no layout").
- A proof-only value may appear **only in fact positions** — machine or domain
  `requires`, `ensures`, `where` clauses — and in proof-stratum machine bodies.
  It never has a size, an address, or a zero value.
- A machine whose signature mentions a proof-only type is itself proof-only:
  it is evaluated by the checker, never lowered.
- **The checker computes where values exist and rearranges where they do
  not.** `Nat`/`Int`/`Rat` facts evaluate with exact unbounded arithmetic
  (`3nat * 3nat` reduces to `9nat`); facts over axiomatized carriers such as
  `Real` normalize symbolically under the carrier's declared algebra. The
  operand type picks the mode.
- A pure, total, measured machine over ordinary machine types is **dual-use**:
  it runs at runtime *and* serves as a fact atom the engine reasons about.
  Most theorems about `u64` code cite dual-use machines directly and never
  need `Nat` at all; `Nat` appears when a claim is genuinely about unbounded
  mathematics.

The explicit-relevance migration preserves the structural fact while removing
the accidental universe split. A recursive or otherwise unlayoutable `Type`
may occupy an erased binding and participate in proof computation, but cannot
occupy a runtime-relevant binding. Explicit binding relevance takes precedence
during migration; the existing recursive-propagation rule remains legacy
inference for unannotated declarations until their surfaces are migrated.
Constructor choices count as representation as well as fields, so an
all-fieldless sum such as `bool` does not become erased by vacuity.

Core ships the roster: `Nat`, `Int`, `Seq<T>`, `Bag<T>`, and `Rat`. Every finite
nonzero float embeds into signed `Rat` exactly (binary values are dyadic
rationals), while signed zero, infinity, and NaN inhabit the separate
proof-level `FloatMeaning` cases. Float verification invokes executable
`FloatSemantics` functions whose finite branches are exact Rat arithmetic plus
one format rounding step. Its `FiniteNonZero` payload is `Rat::NonZero`, so
the proof carrier has no overlapping zero representation. `Int` is the uniform
proof embedding target for fixed-width integers and addresses. Its order has no
floor, so ranking views over it must produce a
well-founded `Nat` rank or carry a proven floor.

Core's `Rat` stores a signed `IntPair` numerator and a positive `Nat`
denominator; `mk_signed_rat` cancels the pair's shared offset and reduces the
remaining magnitude with the denominator. Its Cauchy-facing metric still
avoids division. `rat_gap(p, q)` is the nonnegative absolute cross-product
numerator gap, and
`rat_close(p, q, precision) == Nat::Zero` states
`|p-q| <= 1/precision` by comparing `precision * gap` with the common
denominator in Nat's explicit saturating-subtraction order. Its reflexive and
symmetric laws are ordinary
checked machines; they are the metric substrate for the constructed `Real`
corpus, not compiler-known arithmetic.

The supporting natural metric is ordinary core code as well. `nat_gap(a, b)`
computes symmetric absolute difference from the two saturating-subtraction
directions, and
`nat_gap_triangle(a, b, c)` proves
`nat_gap(a, c) <= nat_gap(a, b) + nat_gap(b, c)` in the settled
`Nat::saturating_sub(left, right) == Nat::Zero` order spelling. Its proof uses
nested structural
case states; every value leaf is checked, and recursion remains admissible only
when strict-subterm provenance survives every state-parameter forwarding edge.
Proof citations are statement-ordered: an earlier checked citation can
establish a later citation's `requires`, but a later statement can never justify
an earlier call. No Nat metric law is built into the checker.

Rational triangle is likewise division-free. `rat_gap_triangle_scaled(p,q,r)`
lifts all three gaps to the shared denominator and proves
`q.den * gap(p,r) <= r.den * gap(p,q) + p.den * gap(q,r)`. It is an ordinary
composition of Nat gap homogeneity, commutative-semiring factor rearrangement,
and `nat_gap_triangle`. Citing it substitutes symbolic member places into the
consumer's frame (`p.den` becomes the actual argument's `.den`); the names in a
theorem declaration are never observable at a citation site.

The order layer used above is checked core code as well.
`mul_le_mul_right(a,b,k)` transports `a <= b` through a common multiplier;
`mul_le_cancel_right(a,b,k)` reflects the order when `k` is positive. The
first proof is requires-bearing induction: its induction hypothesis is visible
to an authored per-arm citation only when every premise instantiated at the
smaller self-call is already established at that statement boundary. Earlier
citations may make the conditional hypothesis available to later citations;
an unproved or membership-shaped premise contributes no hypothesis.

`rat_close_triangle_split(p,q,r,e)` is the reciprocal-precision triangle:
closeness of `p,q` and `q,r` at `e+e`, plus positivity of `q.den`, proves
closeness of `p,r` at `e`. It scales the denominator-shared gap triangle,
combines both premise bounds, cancels `q.den`, and then cancels the concrete
factor two. No division or hidden ordered-ring tactic enters the proof.

The first sequence-facing atoms are ordinary generic machines too.
`cauchy_at<Sequence, Modulus>(precision, i, j) == Nat::Zero` states the
same-generator point obligation after `i` and `j` have reached the static
modulus. `converges_together_at<Left, Right, Modulus>` states its
heterogeneous two-generator twin. Their arbitrary precision and index inputs
are the universal variables; positive precision and both modulus bounds are
ordinary `requires`. There is no hidden quantifier, runtime callable, or
compiler-known notion of convergence in this surface. Their same-generator
reflexivity and heterogeneous symmetry facts are checked generic theorem
machines and remain citable at concrete generator/modulus selections.

`converges_together_at_triangle_split<Left,Middle,Right,Modulus>` lifts the
doubled-precision Rat theorem to one shared middle index. Both precision levels,
all modulus thresholds used by the premises and conclusion, and the actual
`Middle(index).den` positivity fact remain explicit requirements. Static-machine
application member places preserve the selected generator during citation
substitution; a positivity fact about another generator does not alias it.

The pointwise corpus supplies the mathematical kernel for the quotient below.
The remaining language layer packages an existential modulus plus its universal
law as carrierless proof evidence. A named convergence term projects one stable
opaque modulus symbol characterized by its law; it does not run a convergence
decider or expose the selected conformance in runtime layout. Repeating the
projection on that term yields the same symbol, while distinct evidence terms
may carry distinct witnesses without changing proposition or quotient
identity.

The ordered implementation dependency is explicit: proof-side proposition
families and typed index telescopes land before evidence-bearing quotient
formation. See
[Law-Bearing Relations, Evidence, And Quotients](../design_briefs/law_bearing_relations_and_quotients.md).

A quotient coarsens a type: sort its values into buckets of things a
proven equivalence calls interchangeable, and the buckets become the
values — read `%` as it already reads everywhere else, modulo. Wrapping
arithmetic is the familiar instance: `u32` addition is integer addition
with numbers differing by 2^32 counted the same.

```omega
data Real = CauchySeq % ConvergesTogether;
```

This is the bodyless `data` declaration (the `const X = ...;` shape): the
right side is a type expression, and `%` is its one new form. `CauchySeq` is a
proof carrier family whose typed index telescope contains its generator
machine. `ConvergesTogether(a, b)` is a proposition over representative
values. Its representatives may be `CauchySeq<A>` and `CauchySeq<B>` with
different generator indices while sharing the same family identity. Rat is the
same model with an empty index telescope. Quotient carrier matching never
admits an instance of a different family.

The proposition's evidence is a retained term produced by a privately selected
conformance and projected entirely in the proof stratum:

```text
ConvergenceEvidence<A, B>
|- modulus(precision: Nat) -> Nat       opaque proof symbol
`- close_after(...)                    checked universal law
```

The mathematical name `ConvergesTogether(a, b)` is a witness-bearing
proposition whose declaration names the carrierless evidence interface.
Ordinary signatures do not expose the underlying selected conformance.
Convenience names such as `Cauchy(s)` may be transparent proposition aliases.
Because the entire evidence term has no runtime carrier, its named input and
output bindings need no storage owner, table, allocation, or cleanup. Merely
having no runtime table slots would not suffice for an ordinary runtime
instance.

Relation properties are ordinary explicit conformances. `Reflexive`,
`Symmetric`, and `Transitive` are independent requirements;
`Equivalence<C, R>` composes all three and redeclares none. Preorders and
partial orders reuse the same component properties. Law evidence is selected
through those conformances rather than discovered from proof-machine names.

`%` consumes the carrier family, proposition relation, and one explicitly
passed `Equivalence` conformance. It never searches visible conformances or
selects an individual law satisfier. Quotient formation remains
carrier-only (`seq as Real`; `42 as Real` does not compile — that road runs
through `Rat` and a constant stream). Proven `ConvergesTogether(a, b)`
establishes logical equality between `a as Real` and `b as Real`. Equality on
the quotient means "same bucket," never "same representative". This logical
fact does not synthesize an executable structural `==` operation.

The quotient declaration names that evidence in its static `where` surface:

```omega
data Real = CauchySeq % ConvergesTogether
where
    ConvergesTogether satisfies
        Equivalence<CauchySeq, ConvergesTogether>
        as CauchyEquivalence;
```

Here `as CauchyEquivalence` references an existing named conformance; it does
not declare one and does not enter quotient identity.

Every path in this formation surface remains an authored declaration
selection. The carrier, quotient relation, repeated `where` relation, sealed
`Equivalence` trait, and trait arguments follow the quotient data declaration's
visibility. The selected conformance is private formation custody rather than
quotient API identity, but selecting it across a package boundary still
requires an ordinary direct dependency and a public conformance declaration.

Equivalence licenses the quotient type, not operations on it. A lifted
operation explicitly selects the representative machine and one ordinary
checked theorem machine. The theorem's parameters state its universal
variables: quotient-bearing positions appear as left/right representative
pairs, while each ordinary pass-through position is one binder reused in both
calls. Its `requires` names the exact selected relations and makes both
representative calls legal; its `ensures` states congruence in the requested
result relation.

For example, a partial representative operation proves its theorem under both
call preconditions:

```omega
machine fraction_divide_respects(
    left_numerator: Fraction,
    right_numerator: Fraction,
    left_denominator: Fraction,
    right_denominator: Fraction
)
requires
    FractionEquivalent(left_numerator, right_numerator);
    FractionEquivalent(left_denominator, right_denominator);
    left_denominator in Fraction::NonZero;
    right_denominator in Fraction::NonZero
ensures
    FractionEquivalent(
        Fraction::divide(left_numerator, left_denominator),
        Fraction::divide(right_numerator, right_denominator)
    )
{
    // proof
}
```

The compiler derives that expected contract from the representative operation,
public-to-representative argument correspondence, selected quotient relations,
and requested result quotient. It validates the exact named theorem after
selection. It never discovers a theorem by visibility or shape, and no
`Respects` interface, variadic proof binder, arity-indexed trait family, or
runtime dictionary exists.

The quotient owner selects both machines in an ordinary body through one of
two sealed core operations:

```omega
machine Rational::divide(
    numerator: Rational,
    denominator: Rational
) -> Rational
requires
    denominator in Rational::NonZero
{
    Quotient::define<
        Fraction::divide,
        fraction_divide_respects
    >(numerator, denominator)
}
```

The quotient owner authors public precondition `Q`; it is never derived from
the implementation-scoped representative precondition `P`.
`Quotient::lift<F, Theorem>` is the wrapper form: `Q` must imply `P` for both
representative applications, but may be stronger, and arguments may be adapted
explicitly. `Quotient::define<F, Theorem>` is the faithful-definition form:
`Q` and `P` must be equivalent, runtime arguments correspond
position-for-position with no constants, permutation,
duplication, or omission, and the intrinsic result reaches every normal return
unchanged. The compiler checks these facts over normalized IR, so aliases and
state forwarding do not change the classification. A rejected `define` points
to `lift` when the body is an honest wrapper.

For a partial operation, universally proving `Q -> P` supplies legality for
both equivalent representatives. No separate domain-invariance law or source
biconditional is required. Both legality premises remain in the selected
theorem itself because its calls must denote under its own contract,
independently of any later lift.

The selected theorem is resultless proof-static authority. It must be checked,
pure, crash-free, suspension-free, blocking-free, and terminating. Selecting it
emits no theorem call, proof object, representative pair, or dictionary.
Checked and terminal identity retain the operation, correspondence, exact
relations, theorem application, lift/define kind, and contract/result-flow
certificates.

Implementation status: `%` formation now requires the exact proposition
relation and explicitly named, sealed `Equivalence` conformance shown above.
Its closed Reflexive, Symmetric, and Transitive rows must state the canonical
contracts and depend transitively only on checked proof machines; generic
relation binders are matched by exact category and order. There is no Boolean
relation, structural law-discovery, authored `Equivalence` lookalike, ambient
selection, or admitted/boundary proof fallback.

The checked operation boundary recognizes only the sealed wrapper spelling and
retains the exact resolved `F`, exact named theorem, and lift/define kind. It
does not yet admit or execute the request: exact theorem-contract validation,
contract correspondence, and normalized result flow remain required. Bare
calls on representatives or quotient values cannot discover a structurally
similar proof machine.

For the narrow total direct `define` shape, a separate proof-only preparation
API can now erase the completed validation evidence into source-free canonical
correspondence. `TerminalModule` retains the complete extracted batch in
canonical identity order, its codec binds every certificate and rederives its
identity, and normal representation validation independently reconstructs and
replays each row. The explicit producer attachment is not an ordinary machine-
lowering path. This is not executable admission: the rows name no Terminal
machine or operation, normal validation still rejects the request, and no
representative call is lowered. General `lift` implication—including the
explicit transport authority still required for membership and opaque
proposition families—and checked executable Terminal lowering remain open.

A quotient may retain an arbitrary representative unchanged at runtime and may
therefore share its ABI without performing normalization. The representative
is nevertheless opaque. Quotient formation suppresses synthesized structural
equality, ordering, hashing, serialization, reflection, pattern matching, and
every other representation-derived observer. Struct and case literals cannot
forge a quotient value; casting an exact carrier instance with `as Quotient` is
the sole construction path. Logical quotient equality is the declared
relation. Executable equality is an ordinary lifted quotient operation and must
prove `equals(x, y) == true <-> R(x, y)` through
`DecidesEquivalence`; that law also entails the required result-congruence
theorem. The named operation may bind fixed `==` through the ordinary
[`operator` declaration head](chapter_5_expressions_evaluation.md#operators).
Other observer roles state their role-specific correctness as ordinary
contracts until a named interface exists.

Compile-time evaluation preserves the exact representative supplied by quotient
construction. An ordinary `const` may therefore materialize that carried
representative without proving a canonical form; this is no stricter than
runtime construction and grants no new observer. Canonicalization is required
only when a consumer asks for representative-independent identity, including a
stable serialized/wire form, public ABI promise, canonical const-index atom,
structural interning/hashing, or reproducible raw bytes. Equivalent quotient
constants may otherwise contain different opaque representatives.

Initial lifting is deliberately pure and terminating. Observable effects,
crash routes, suspension, blocking, and progress behavior need a richer
behavioral respect relation and cannot be justified by result congruence alone.
Likewise, initial quotient carriers contain no affine/linear `Type` content or
owned/routed custody: quotient equality may not make distinct authority or
provenance occurrences substitutable.

Carrier declarations do not assign global relational roles to their static
parameters. A proposition may quantify independent left and right index packs,
or use one shared pack, according to the relation it declares. A selected
constructor relator is correspondingly heterogeneous, `Lift<I,J,R>`, and the
quotient owner chooses the exact named lift for the known quotient/container
pair. The exact selection is retained in semantic identity, and an uncovered
pair rejects at instantiation.

Transparent non-dependent products lift recursively. Dependent fields lift in
dependency order: facts established for earlier left/right fields determine
whether later proposition applications coincide or require an authored
transport theorem. The quotient owner discharges transport required by its
chosen relation; the proposition owner controls the laws available for opaque
propositions. A relation depending on erased `Type` content remains proof-only
unless checked evidence shows that content is determined by the runtime
projection, in which case a runtime decider may be derived.

An attached proof-carrier operation used this way has a by-value receiver and
does not install a representative-facing method or reify a representative on
the quotient. A borrowed or mutable receiver remains a forbidden runtime use
of proof-only data. Copyable runtime carriers may receive pure executable
quotient operations through the same sealed lifting gate; the representative
still never becomes source-visible.

A boundary axiom may be cited as an environmental assumption elsewhere, but
cannot admit either an equivalence conformance or a selected operation theorem
for a checked quotient. Both require checked proof machines. A false quotient
equality propagates by substitution without the containment boundary available
to an admitted resource claim.

A literally bodyless free machine is not a theorem. Checked theorem machines
have bodies, including an empty `{ }` body when their conclusions follow from
entry facts. An accepted axiom is an explicit bodyless `boundary machine` and
retains admitted provenance. A proof machine that ensures a witness-bearing
proposition must supply its declared evidence; the formula cannot be retained
without the witness that its eliminators require. An `Equivalence` conformance
or selected operation theorem depending on admitted evidence never licenses a
quotient formation or lift.

## Proof Views

`embed(value)` is a compiler-owned fact-position term former with canonical
semantics, like `old(&place)`. It is not executed, overridden, selected, or
declared as a bodyless boundary machine. A package-declarable proof-term-symbol
surface, if ever justified independently, must be designed explicitly rather
than inferred from the temporary Real scaffold.

Runtime data often needs a mathematical view before it can be reasoned about.

For slices, useful proof views include:

```text
Seq(items)    ordered finite sequence view
Bag(items)    finite multiset/counting view
Range(len)    finite index space
```

These are ordinary proof-only types from core — recursive data plus
extraction lemmas, not compiler-known forms. They do not allocate at runtime;
they let contracts talk about math without pretending that proof binders are
runtime loops.

`Sorted` is an ordinary domain defined by a predicate machine (see Quantified
Facts below); the views exist so contracts can talk about order and counting
without inventing runtime loops. Sorting is naturally expressed as:

```omega
machine Sort::bubble_sort_preserving(
    before: &[Nat],
    items: &mut [Nat]
)
requires
    Bag(items) == Bag(before)
ensures
    Seq(items) in Sorted
    Bag(items) == Bag(before)
{
}
```

The `before` value is explicit. A caller that wants to preserve an arbitrary
computed value can make or carry such a snapshot itself.

Contracts also have the narrower proof-only `old(place)` form. It selects the
callable-entry revision of a structural place so a postcondition can relate
that place's prior and current content. It is not a runtime snapshot, does not
duplicate the place or its value, and initially does not accept an arbitrary
computed expression. For example, `old(&extent)` gives a content projection a
stable pre-state subject while preserving the exact owned occurrence.

`old` is derived from the same place-revision model used by scoped facts and
borrow certificates; it is not a second history mechanism. Terminal Psi
retains the structural place, its callable-entry revision, and the current
place separately. It is the sole source pre-state term former, is admitted only
in fact position where a callable-entry revision exists, and packages cannot
implement or override it. The retired proof spelling `entry(place)` and the
retired explicit machine-member `entry` grammar are not aliases.

## Helper Machines

Large proofs should be decomposed through helper machines with small contracts.

```omega
machine Sort::compare_swap(
    before: &[Nat],
    items: &mut [Nat],
    index: u64
)
requires
    index + 1 < items.len
    Bag(items) == Bag(before)
ensures
    items[index] <= items[index + 1]
    Bag(items) == Bag(before)
{
}
```

The preservation fact is explicit. If a caller needs a before-state, it passes
one in. Nothing in this chapter relies on an implicit snapshot keyword.

A sorting proof is built from smaller facts:

```text
compare/swap orders one adjacent pair
compare/swap preserves Bag(items)
one pass moves the largest remaining item to the end
repeated passes establish Seq(items) in Sorted
Bag(items) stays equal to the explicit before value
```

## Quantified Facts

> **Quantifiers are not keywords.** Universal claims over
> all values are machine parameters (a theorem over `(n: u64)` is checked
> symbolically once). Element-wise facts are element types and window facts
> (chapter 7). Relational facts over sequences are **predicate machines** plus
> one extraction lemma each. Existentials are witness-carrying out-params.
> `forall`/`exists` remain parse errors; the quantified shape lives in the
> engine, not the surface.

A relational property is defined by an ordinary measured machine:

```omega
machine sorted(items: &[i32]) -> bool
terminates by items -> Slice::Length;
{
    transition items.len <= 1 {
        true  -> true
        false -> items[0] <= items[1] && sorted(items[1..])
    }
}

domain [i32]::SortedAscending
    requires sorted(self);
```

The definition also specifies the decider: a checked validator runs it (or a
loop the checker proves refines it), and the successful path uses `as` only
after the predicate is established.

Consuming the fact at an arbitrary index needs one **extraction lemma** per
predicate — an induction, written once by the predicate's author:

```omega
machine sorted_extracts(items: &[i32], i: u64, j: u64)
requires sorted(items) == true && i < j && j < items.len
ensures items[i] <= items[j]
terminates by i -> Nat::IncreasingTo(j);
{ ... }
```

After that, the engine holds the quantified fact-shape natively and every use
is mechanical, under two closed rules:

- **Instantiation** happens only at index atoms in scope at the obligation —
  deterministic, budgeted, never searched. A missing instance is a normal
  "cannot prove" naming the index it needed.
- **The delta rule**: extending a quantified fact by one element (a
  validator's loop step, a table's append) costs one definitional unfold.
  Loop invariants over sequences ride state arrival contracts (chapter 11).

Instances injected by the lemma are ordinary atom-facts, so the
difference-bound engine composes them — transitivity, everything-left-of-mid,
min-at-ends are downstream chains, not further lemmas.

## Induction Is Ranked Recursion

A proof-stratum machine recurses under the same rule as every machine: a
`terminates by` ranking, checked at every cycle (chapter 3). Read as a proof, the
machine *is* the induction: transition dispatch is the case analysis
(exhaustiveness enforced — no missed constructor), the measured cycle is the
appeal to the induction hypothesis, and a state's arrival contract (parameter
facts plus state `requires`, chapter 11) is the hypothesis itself, proven at
every in-edge. Nothing was added to the language to express induction; the
state machine was already its shape.

Every recursive edge whose contract is consumed is an induction edge. This
includes a resultless statement citation, an explicitly discarded call, and a
call nested in a value expression. The callee's `ensures` may enter the proof
context only after that exact direct or mutual-cycle edge proves a strict
decrease under the component's ranking. Consequently an unmeasured theorem
cannot cite itself at unchanged arguments and use its own conclusion to close
its goal.

Induction may also be indexed by a finite unsigned count while its theorem is
about proof-only data. On an arm guarded by `n > 0` (or `n >= 1`), a recursive
argument `n - 1` is the checked predecessor. The structural checker treats that
argument as an opaque index, imports the recursive contract there, and can then
unfold or cite `Nat` lemmas around the recursive result. This is a bridge at the
recursive edge, not an implicit conversion between `u64` and structural `Nat`.

## Termination Proofs

Termination is a proof over every cycle in the reachable machine/state graph,
not an `ensures` proposition evaluated after a return and not a reach-row
member.

```omega
machine walk(items: &[Nat])
terminates by items -> Slice::Length;
{
}
```

The ranking argument is ordinary proof vocabulary:

- choose explicit subjects;
- select a well-founded ranking view; and
- prove every cyclic edge makes the produced rank strictly smaller.

Direction belongs to the view rather than a blessed `decreases` or `increases`
keyword. `Nat::Descending`, `Nat::IncreasingTo(limit)`,
`Tree::ProperSubtree`, and lexicographic views all satisfy the same checker
role. A standalone `measure` declaration supplies a named custom view and
multiple measures per carrier are legal.

Proof-stratum machines use exactly the same `terminates by` source and checking
rule as runtime machines. Their eligibility differs only at lowering: measured
non-tail recursion is legal when evaluation remains in the proof/compile-time
stratum and is rejected if runtime lowering is requested.

The normalized artifact separates the public termination guarantee from the
private ranking witness. A witness change invalidates its provider proof cache,
not caller or external requirement-binding identity. See chapter 9 and
[Termination, Ranking, And Progress](../design_briefs/termination_ranking_and_progress.md).

## Citing Proofs

A fact the engine cannot derive may be discharged by citing a proof machine's
contract, instantiated at the operands. This is the only connection between
proof-stratum theorems and runtime code, and it has no syntax of its own — a
cited theorem is a fact like any other:

```omega
machine Walker::step(&mut self)
requires self.n >= 1 && self.n <= 6148914691236517205
ensures self.n == collatz_step(n0)    // refinement: the u64 op IS the ideal op
{ ... }
```

Working rules:

- A theorem over parameters applies at any operands satisfying its
  `requires` — instantiation is machine application, not search.
- An `ensures` may equate a runtime place with a pure machine's result (a
  *refinement* fact): the runtime operation provably computes the
  mathematical function on the domain where its witnesses fit. Prove once
  over the ideal type; embed per width by supplying each width's bound.
- Runtime code that cites no proofs pays nothing and sees nothing.

Carrying a theorem to a site is an ordinary statement call — a fact-only
machine invoked for its `ensures`, which enters the flow facts and erases at
codegen:

```omega
mask_is_mod(self.head, self.cap);            // erased; its ensures now in scope
self.slots[self.head & (self.cap - 1)] = x;  // proves against those facts
```

Erasure does not remove the citation edge from recursion checking. A citation
inside the same recursive component imports an induction hypothesis only with
the strict-decrease certificate described above; syntax position and result
use do not weaken that rule.

This explicit form is the default: the proof structure
stays visible in the text. When an obligation fails for want of a known
lemma, the diagnostic names it by shape match. A rewrite extension —
proven equations joining the engine's term reading — is parked in the
design brief, to be revisited only if ergonomics demand it.

## Evidence And Trust

Facts are proven, computed, deferred, or accepted — and each tier is a
distinct compiler behavior, never a label:

- **Proven** (the engine, a derivation, a cited theorem): no declaration
  exists. Most facts live here invisibly.
- **Evaluated**: the compiler runs an ordinarily terminating machine in the
  hermetic target-semantic evaluator. Deterministic work is metered for live
  progress, warnings, and any root-selected ceiling; long or unlimited
  evaluation remains legal when root policy permits it. Results and canonical
  usage records are cached separately.
- **Deferred** ("prove later", written by tooling): a waiver of exactly one
  compiler-derived obligation — nothing new becomes citable. Warns on every
  build; fatal at **package release** (publishing with an open deferral is
  the hard error — "release" is a package-manager moment, not a build
  configuration; debt never crosses a package boundary). Hash-pinned to the
  code under it: edits kill the deferral and it must be re-taken.
- **Accepted**: a `boundary machine` — a contract with no body, the proof
  system's face of the boundary culture (chapter 19): explicitly trusted and
  reported, but not thereby proven or audited.

```omega
boundary machine collatz_cert_checked()
ensures check_collatz_cert(cert_blob_b41c) == true
```

Working rules:

- **The statement carries all specificity.** Trust the narrowest thing — an
  execution claim ("this checker accepts this certificate", the
  certificate's identity inside the statement) rather than the theorem it
  implies; a userspace proof machine lifts the narrow claim to the broad
  one. The trust report cannot be vaguer than the claim, because it *is*
  the claim.
- **There is no inline `assume`.** Boundary machines are the only home for
  unproven facts. Grant locality: **own-package boundary machines are active
  in dev builds**, carrying a standing warning until granted; boundary
  machines arriving **from packages are inert until granted** — a library's
  boundary machines surface as requests when the package is added, and a
  package can never self-grant.
- **Grants flow from the root.** The final build's `build.omg` accepts one
  package claim set through the ordinary `Build` API. The build lockfile — the
  same machine-written lockfile that pins package resolution; one receipt file,
  not two — fingerprints the package plus its complete normalized claim set.
  Adding, removing, or changing any claim invalidates the acceptance and
  presents the exact diff. No hash is hand-written; `build.omg` stays the only
  file a human authors.
  A provider-slot grant binds only the provider plan selected for that slot;
  unselected and partial candidates remain dev-active and cannot acquire the
  selected plan's receipt merely because they implement the same boundary.
- **Acceptance is not proof of review.** A lock resolution, reviewer string,
  signature, LLM verdict, or proof certificate cannot establish that the
  package as a whole was competently audited. Certificates establish only the
  exact propositions independently reconstructed and checked by their kernel.
  The accepted project state and its surrounding organizational controls are
  the authority for package admission.
- **The engine can veto.** A boundary statement the engine can refute — one
  contradicting declared ranges, domains, or another accepted statement —
  is a compile error, grants notwithstanding.
- **Blast radius is reported.** The trust report names which conclusions
  rest on which boundary machines; facts derived without touching one stay
  in the unconditional tier, visibly. Export status is irrelevant — the
  report sees every grant, private or public. Routed provider qualifications
  remain equally specific: their rows bind the exact provider-plan
  fingerprint and requirement to the accepted parameter or returned result,
  authority flow, domain, carry policy, predicate-discharge requirement, and
  grant provenance.
- **The grant row is the language's `unsafe`.** A granted false statement
  can corrupt anything proofs protect — bounds, domains, and through
  corrupted memory, everything downstream. Reach restrictions cannot be waived by
  facts (they ride the call graph, and a boundary machine has no body),
  but a false range fact reaches the same place dynamically. Omega has no
  `unsafe` keyword because this is the one unsafe door: root-only, pinned,
  reported, tripwired.
- **Runtime-decidable boundary claims get oracle tripwires** in proof
  builds: a test run that witnesses a violation traps naming the machine
  that lied.

Certificates need no construct of their own: a certificate is wire data,
its checker is a measured machine, its soundness is a theorem
(`check(c) == true` implies the claim), and establishment is the
`evaluated` tier — or a proved `as` qualification through a certificate domain
(`domain [u8]::ValidCert requires check(self);`), the validated-decode pattern
of chapter 8 applied to proofs. A build that can afford the check *proves*
the claim outright; one that cannot accepts the narrow execution claim
above and lifts it by theorem.

Trust has a data face too. `boundary data` declares a type whose source
representation is externally admitted rather than structurally defined. It
does not mean “imported layout” or “exported layout,” and the keyword does not
encode traffic direction. A `boundary machine` is likewise classified by its
supply mode—external realization through `satisfies`, or an admission-bearing
claim—rather than by an inbound/outbound reading of `boundary`. An abstract
carrier-owned provider slot instead uses explicit `boundary requirement`.

The N5 `omega::language::core::real` package is temporary axiomatic scaffolding;
it is not precedent for a claim-free bodyless boundary-machine category. Its
current contents include:

```omega
boundary data Real;                                    // opaque proof-only carrier
boundary machine Real::add(a: Real, b: Real) -> Real;  // no ensures: a symbol — claims nothing
boundary machine real_add_commutative(a: Real, b: Real)
ensures Real::add(a, b) == Real::add(b, a);            // an axiom: one trust row
```

The carrier is proof-only. The claim-free `Real::add` spelling merely introduces
a temporary proof symbol; it must not mint a general language surface. N6/N8
replace it with the constructed Cauchy quotient and ordinary checked
operations. Each bodyless law carrying `ensures` is an admission-bearing axiom,
not a proved theorem, and remains one disclosed trust row until an ordinary
proof-machine body replaces it. Consumers then swap admission for checked
import.

Core ships classical logic itself this way: excluded middle is a boundary
machine, granted like anything else — nothing is granted by default, not
even logic (project templates carry the line). A build that never grants it
is constructive, and its trust report says so.

## Automation And Boundary

Omega source normally shows the proof strategy rather than every logical rule.
That is especially important when reading recursive proof machines. An
ordinary call contributes its checked `ensures` through contract application.
A call within the same recursive proof component is different: using that
contract would be circular unless the selected ranking proves the call strictly
smaller. Once that edge is checked under a well-founded relation, the callee's
instantiated `ensures` is the inductive hypothesis. `terminates by` therefore
does more than promise that computation stops; in a recursive proof it licenses
the logical assumption that makes induction sound.

The readable body is not the complete derivation. Elaboration records the
implicit computation, constructor rules, branch facts, inductive applications,
and licensed normalization in a kernel-checkable certificate. A deterministic
review synopsis is rendered from that certificate and names its fingerprint,
implicit closure rules, exact cited laws, and trust closure. It is never rebuilt
by a second analysis of the source: a plausible explanation of a different
proof would be worse than no explanation.

Automation does not erase authority or provenance. A normalization step names
the selected conformance and exact laws it consumes. If any cited law or
well-foundedness theorem is admitted, every dependent conclusion remains
admission-dependent. Total procedures may be replayed during checking; partial
search may not be trusted merely because it found an answer and must emit
checkable evidence.

The checker should automatically solve common cases:

- arithmetic normalization,
- equality reflexivity,
- range implications,
- branch facts,
- disjoint field facts,
- simple generic const facts.

When automation fails, library authors can provide helper machines. When a fact
cannot be proven from machine code, contracts, or boundary foundations, it must
cross an explicit boundary.
