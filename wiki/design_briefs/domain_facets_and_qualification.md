# Design Brief: Domains And Qualification

Current design as of 2026-07-25. Chapter 8 carries the language-guide
surface. This brief owns domain meaning, establishment, `as`, semantic roles,
normalization, and the staged units model.

## One surface, independent internal aspects

A **domain** is a zero-cost static theory attached to an unchanged runtime
carrier. One domain may contribute any compatible combination of:

- a predicate body, discharged by the prover;
- semantic declarations, such as denotation, dimension, operators, or
  conversions;
- owner-authorized establishment routes for facts that are not derivable from
  the carrier; and
- transparent aliases over compatible domain atoms.

These aspects share one source declaration because they compose. `Utf8` has a
predicate body, `Km` contributes denotation and unit operations,
`Wrapping` contributes an arithmetic policy, `Percent` may contribute both a
range predicate and unit meaning, and `Reservation::Issued` is a bodyless
historical fact.

They remain separate compiler records and algebras:

```text
DomainTheory {
    carrier,
    optional_predicate_body,
    semantic_contributions_by_role,
    establishment_routes,
    alias_expansion,
}
```

The prover consumes predicate bodies. Static operator resolution consumes
semantic contributions. Establishment checks consume routes and receipts.
Multiplicity governs copying and outstanding obligations. Carry governs where
the resulting value or claim may travel. A domain declaration does not fuse
those systems.

Domains add no runtime tag, wrapper, hidden storage, or second object model.
Qualification changes the static theory carried by a value, not its runtime
representation.

## Domain bodies

A nonempty body states propositions about the classified value:

```omega
domain [u8]::Path {
    no_nul(self);
}
```

Membership may be established only when those propositions are proved. The
owner is subject to the same obligation as every consumer. A checked validator
may perform runtime work and guarantee the domain in its successful result; an
admitted boundary may assert the fact under a receipt. Neither route turns the
predicate into an unchecked cast.

A bodyless declaration names a qualification that cannot be derived from the
carrier:

```omega
pub domain Reservation::Issued;
```

The braced empty form has the same meaning. It is not an always-true predicate.
An explicitly universal predicate writes `true` in its body.

Bodyless membership is established only by:

- an owner-authorized checked machine;
- propagation from an already-qualified value;
- a checked transformation of existing evidence or resource provenance; or
- an admitted boundary receipt authorized by the owner-authored requirement
  being satisfied.

A qualified result or `ensures` clause is an obligation on a checked
implementation, not evidence by itself.

## `as`: qualification without value change

The governing rule is:

> **`as` changes static qualification, never the runtime value.**

Every accepted domain-qualification `as` preserves all three independent
runtime axes:

| Axis | Requirement |
|---|---|
| carrier type and layout | unchanged |
| runtime payload value | unchanged |
| runtime work and control | none |

For a domain with a predicate body, `as` asks the prover to discharge that
body at the exact use site. A literal, dominating guard, prior guarantee, or
validator result may provide the proof. `as` never runs a validator.

For a bodyless domain, `as` is available only when the domain owner publishes
a canonical representation-qualification conformance. The conformance is
compile-time evidence and emits no call. It is not an alternative route for a
domain with a predicate body: a body must always be proved.

Consequences:

- `bytes as [u8] in Path` succeeds only when `no_nul(bytes)` is known;
- `5 as i32 in Km` may use `Km`'s canonical open qualification route;
- `reservation as Reservation in Issued` fails when issuance requires
  `BoxOffice` state;
- `extent as Extent in Granted` fails when authority must originate from an
  admitted receipt or conserved predecessor; and
- converting kilometres to metres is an ordinary named conversion because it
  changes the numeric payload.

### The core qualification requirement

Core publishes one blessed trait relationship between a carrier `Self` and a
qualified type `Q`. A satisfying machine is an evidence witness, not a runtime
implementation selected by `as`.

Conformance validation requires:

- erasing `Q` yields `Self`;
- `Q` adds exactly one normalized bodyless qualification;
- the returned value retains the input value's dataflow identity: aliases and
  proof statements may intervene, but transformation or reconstruction may
  not;
- the complete machine contract has no service reach, suspension, blocking,
  mutation, trap, failure, abort, or other abnormal outcome;
- termination is guaranteed; and
- the satisfier is declared by the domain-owning package or by an explicitly
  delegated owner-authorized package.

The trap/control check is independent of the effect row: terminal outcomes are
not operational effects.

One visible home satisfier permits the `as` shorthand. If several are visible,
implicit selection rejects and lists them; the program calls the intended
named satisfier directly. Satisfier names are ordinary library names and have
no compiler significance. Both the shorthand and a direct call through this
blessed conformance erase to the unchanged input value.

The trait's final public symbol and exact generic spelling are selected with
the core declaration. Its semantics and validation contract are fixed by the
rules above.

## Establishment, propagation, and conservation

All membership feeds one qualification judgment while retaining its evidence
source:

| Evidence source | What it establishes |
|---|---|
| prover | a nonempty predicate body |
| canonical qualification conformance | an open bodyless qualification |
| owner checked machine | a bodyless fact under that machine's contracts |
| checked transformation | inherited or conserved evidence |
| admitted receipt | an explicitly accepted external assertion or root |

An admitted membership assertion is valid only on the bare result of a
boundary requirement whose result carrier matches the domain target. A direct
accepted-machine membership guarantee is not authorization. Checked proof
facts retain the boundary trait and exact requirement signature, and the
artifact records that signature, the public origin class, and the selected
provider receipt where applicable. Private proof steps and implementation
witnesses remain private evidence.

Reconstructing equal carrier fields does not reproduce qualification. Existing
qualified values retain their facts through ordinary assignment, move, and
permitted copy. Mutation invalidates a bodyless subject-bound fact unless the
operation explicitly preserves or re-establishes it.

Multiplicity, not the domain declaration, governs duplication and debt:

- unrestricted carriers may copy;
- affine carriers may move or abandon but may not duplicate;
- linear carriers must move and eventually discharge.

A fact participating in a conserved resource transformation requires a
non-copyable carrier. A must-discharge obligation requires a linear carrier or
an independent linear token. A reusable historical fact such as
`Artifact::Admitted` may live on a reusable carrier.

Multiplicity does not imply divisibility. A content-bearing qualified claim
separately publishes one normalized projection into a compiler-owned partial
composition algebra. That projection governs establishment backing, authorized
access, n-ary decomposition/recomposition, and retirement accounting. The
initial resource vocabulary is `Indivisible | Interval<Scalar>`; a tracked
resource claim with no decomposable projection is indivisible. Ordinary
qualifications that carry no resource content acquire no content entry at all.
Domain facets retain their predicate/semantic meaning, while permission
attenuation, content, lineage, and carry remain independent claim axes.

Carry is independent. Mobility demands attach to the established value or
resource provenance and survive qualification forgetting until the underlying
claim is discharged.

## Semantic roles and operator coherence

Semantic contributions are keyed by a small compiler-known role vocabulary.
Compatible contributions in different roles compose into one operator
meaning; competing contributions to the same role reject. Cross-role
composition is not permission to run two unrelated operator implementations
in an arbitrary order: the selected contracts must determine one checked
meaning.
The initial roles are:

| Role | Examples | Consumer |
|---|---|---|
| predicate knowledge | `NonZero`, `Utf8`, ranges | prover |
| denotation/dimension | `Km`, `Metres`, `Degrees` | normalizer and operator result theory |
| arithmetic policy | `Exact`, `Wrapping`, `Saturating`, `Trapping` | primitive arithmetic lowering |

Later compiler releases may add roles such as rounding or comparison policy
when a real customer requires them. Role identity is closed and
compiler-owned; packages author theories within the admitted roles.

This distinction is load-bearing. `Km` and `Wrapping` both affect `+`, but in
different ways: `Km` determines dimensional meaning while `Wrapping`
determines overflow behavior. They compose. `Wrapping & Trapping` contributes
two arithmetic policies and rejects.

Predicate obligations compose independently. A standing range combined with
an arithmetic policy is legal only where every permitted operation preserves
the range; flow facts may instead be invalidated and later re-proved.

A domain predicate does not synthesize its operators. A domain-owned operator
still publishes a signature and relational contract, and its checked
definition or selected satisfier must discharge that contract. For a
normalized degree domain, returning a value in `[0, 360)` is necessary but not
sufficient: the contract must also relate the result to the operands modulo
360, or an implementation that always returned zero would pass.

The same example shows how roles compose without competing overloads. Two
normalized degree operands lie in `[0, 359]`, so their unreduced sum lies in
`[0, 718]`. The degree-addition realization can therefore prove that its
carrier addition is Exact, then reduce modulo 360. If the bindings also select
`Wrapping`, machine-width overflow remains unreachable and the two arithmetic
policies are observationally identical for that operation. This is a local
proof of policy independence, not a claim that Wrapping and Exact are globally
equivalent.

Operator resolution reads static binding qualifications, never incidental
flow facts. Resolution is compile-time, unambiguous, and recorded in the
checked artifact. Adding an unrelated import cannot inject a competing
meaning.

## Arithmetic policies

`Wrapping`, `Saturating`, and `Trapping` are the closed core arithmetic-policy
vocabulary. Qualifying a value with one of them performs no runtime work.
Subsequent operations use the selected behavior:

- `Wrapping` reduces at the declared machine width;
- `Saturating` clamps overflow;
- `Trapping` emits a runtime overflow check and terminal trap.

The later operation may therefore cost work or terminate abnormally even
though qualification itself cannot.

Mixed arithmetic policies reject. Arithmetic-policy removal or replacement
changes only future operator selection; it does not reinterpret an already
stored payload.

An arithmetic-policy qualification may weaken to the unqualified carrier,
whose arithmetic is Exact by default. The current payload is preserved and
every later Exact operation must discharge its ordinary safety obligations.
This does not reinterpret earlier wrapping arithmetic as exact mathematics.
Selecting Wrapping, Saturating, or Trapping from an Exact binding remains an
explicit choice because it changes future operation behavior.

Core's arithmetic-policy qualifications satisfy the same canonical
representation-qualification relationship as an authored unit such as `Km`.
Their primitive lowering is special; their establishment and `as` behavior is
not a second qualification mechanism.

## The operation taxonomy

Carrier identity, payload identity, and runtime work are independent:

| Operation | Carrier | Payload | Runtime work |
|---|---|---|---|
| qualify with `as` | same | same | none |
| forget qualification | same | same | none |
| representation recast | changes | same bits under its validated plan | none |
| validation | same | same | yes |
| conversion | same or different | may change | ordinary contracted work |

Numeric width conversion and unit conversion are conversions, not domain
qualification. Narrowing additionally chooses a trapping, saturating,
wrapping, or checked-result policy explicitly. The current numeric `as`
spelling is a compatibility surface to migrate after named numeric conversion
operations are fixed.

## Weakening and forgetting

A semantic qualification may weaken implicitly to its carrier only when:

1. the identity map preserves denotation; and
2. every default operation agrees with the qualified operation throughout the
   default operation's accepted region.

Certified arithmetic policies can satisfy this law because exact arithmetic
reinstates its proof obligations after weakening. Units fail the denotation
condition: forgetting `1 Km` yields raw `1`, while converting it to metres
yields `1000`. Unit qualification therefore never disappears silently.

Accepted weakening certificates seal the overlapping theory. Later extensions
must re-prove agreement rather than changing the meaning of an existing
program.

## Transparent aliases

A public alias names a nonempty conjunction over compatible subjects:

```omega
pub domain Socket::Usable =
    Socket::Connected & Socket::Authenticated;
```

Expansion precedes normalization and identity hashing, so the alias and its
atoms have one normalized identity. Alias edits change every published
contract that expands them. Diagnostics report missing atoms rather than
stopping at the alias name.

Compiler-owned atoms may participate. `Carry::Portable` expands to the four
positive carry permissions; packages may publish their own aliases over that
closed vocabulary.

## Normalization is not entailment

The deterministic normalizer owns what a domain expression *is*: sorted and
deduplicated conjunctions, canonical dimension vectors, scale products, kind
tags, semantic roles, and alias expansion. Type identity, semantic interface
identity, and monomorphization keys depend on this normalized form.

The entailment engine proves propositions about that identity. Stronger future
proof automation may accept more programs but may not change normalized
identity or operator meaning.

Physical ABI remains the carrier's ABI. Semantic interface identity includes
the normalized domain theory.

## Units as the semantic stress test

`Quantity = structural dimension × nominal kind × rational scale ×
presentation`:

- dimension composes structurally through multiplication and division;
- kind distinguishes equal-dimension meanings such as Energy and Torque;
- scale is a rational factor and changes only through explicit conversion;
- presentation is a non-semantic display alias removed before identity.

Mixed-scale addition requires explicit conversion. A quantity may combine its
denotation role with an arithmetic policy such as `Wrapping`; the roles
compose rather than competing for the `+` spelling.

Required tests:

1. `Km + Metre` rejects without conversion.
2. `Km / Metre` preserves the scale factor.
3. Energy and Torque remain distinct despite equal dimensions.
4. Generic identity preserves unit qualification.
5. Passing a unit-qualified value to its carrier requires certified weakening,
   explicit forgetting, or conversion as appropriate.
6. `Km & Wrapping` composes while `Wrapping & Trapping` rejects.
7. `(5 as Km) as Metres` rejects because metres require a payload-changing
   conversion.

## Implementation staging

The compiler currently carries an explicit predicate/semantic facet pair and
special arithmetic-domain paths. Semicolon and empty-braced declarations now
both normalize to an explicit bodyless predicate-body record in syntax,
symbol-resolved, and typed trees; an explicit `{ true; }` body remains
predicate-bearing. This removes fact-count inference for body presence.
Checked facts now also retain a normalized establishment origin separately
from their program-point origin. Carrier-owner checked machines can establish
their own bodyless result facts, while bodyful and unrelated-owner results
still require ordinary proof; call-result and statement transfer preserve the
evidence. The checked artifact publishes origin/source/receipt rows, and a
granted selected provider plan supplies the normalized receipt identity for
matching admitted facts. This is the first P1a tranche: exact admitted-subject
authorization, canonical `as`, aliases, and replacement of the compatibility
facet pair remain.
Migration should:

1. replace the facet pair with the independent domain-theory records above;
2. add bodyless-domain establishment and evidence-source identity;
3. publish and validate the core representation-qualification trait;
4. add role-keyed semantic contribution and collision checking;
5. preserve normalized qualification through generics, contracts, artifacts,
   and separate compilation; and
6. migrate numeric width conversions away from the qualification spelling once
   their named operations are fixed.

General open operator-family linking, external unit-kind equations, authored
weakening-certificate syntax, and richer unit families remain separate
customers. They do not change the qualification model.

## Cross-references

Chapter 8 owns the guide surface; chapter 5 owns primitive arithmetic;
chapter 10 owns proof machines; chapter 14 owns traits and named satisfiers;
chapter 16 owns terminal failure; and
`authority_values_and_boundary_evidence.md` owns authority provenance and
receipts.
