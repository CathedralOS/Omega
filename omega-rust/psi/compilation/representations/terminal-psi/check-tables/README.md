# Check tables

These are the tables the lowered-form checker reads and writes, and the
contract every session adding a construct family builds against. They
record what is known about one machine's unsealed Terminal Psi module as rows
over dense identities, so no checking step needs a name, a printed expression
or a path string to find a fact. The crate is types only: no elaboration,
checking or prover. It serves the decision to
[check on the lowered form](../../../../../../wiki/drafts/designs/check_on_lowered_form.md).

[`program`](src/program.rs) owns the complete unsealed module and its checking
rows. [`compilation`](src/compilation.rs) groups that module with every demanded
generic form and conformance witness, using kept identities instead of calling
back into source preparation. [`names`](src/names.rs) and source spans exist
only to map diagnostics back to source. Typed source records and Build operands
stay with the producer/driver publication envelope, outside this source-free
crate.

## What each table holds

Every table is an `IdTable`: a flat `Vec` indexed by identity. Elaboration
creates each identity with `IdTable::push`, so identities are dense from 1 and a
lookup is one index.

| Table | Indexed by | Holds |
| --- | --- | --- |
| `values` | `ValueId` | One evaluated scalar: its type and its single origin. A write never changes it. |
| `knowledge` | `KnowledgeId` | A proposition over value identities, the point it is available from, its one justification and its premises. It never becomes false. |
| `propositions` | `PropositionId` | The proposition each knowledge row states. Machine-local, minted by `push`: Terminal Psi keeps contract propositions inline and has no proposition table. |
| `places` | `ExactPlaceId` | A root place, or a parent place plus one typed segment (field, case, fixed index, runtime index named by its captured selector value, descriptor extent metadata), with its sorted children and the writes to it. |
| `roots` | `PlaceId` | The `ExactPlaceId` of each root place. |
| `writes` | `WriteId` | The forward write log: a value, unknown contents, or a hole. |
| `obligations` | `ObligationId` | Claims, loans, windows and debts (conserved across edges) and proof questions (discharged at one site). |
| `edges` | `EdgeId` | Source, destination, and the fate of every conserved obligation crossing the edge. |
| `invocations` | `InvocationId` | The one entry operation of ordinary and terminal calls: callee, ordered actuals, one requirement obligation per callee clause. |

`ValueId`, `EdgeId`, `ObligationId`, `MachineId`, `BlockId`, `OperationId` and
root `PlaceId` are the lowered form's own identities, so the checker and the
emitted product name the same things. `ExactPlaceId`, `KnowledgeId`, `WriteId`
and `InvocationId` belong to this crate because the lowered form has no identity
for those concepts (its `PlaceId` names roots only). `PropositionId` is reused from
the vocabulary, but here it is machine-local and indexes
`MachineTables::propositions`, not the module's named proposition declarations.

## Rules every lane relies on

A lane is one piece of work that teaches the checker a construct family.

- **Identities never cross `MachineTables`.** They are machine-local integers
  with no machine tag. `row()` refuses an identity past the end of a
  table, but an in-range identity from another machine silently finds the wrong
  row. So every checking API takes exactly one `MachineTables` and only
  identities read from it. Crossing machines goes through an
  `InvocationRow`, which holds a `MachineId` and clause positions.
- **Equal place identities mean the same place; unequal ones do not mean
  separate places.** A root overlaps every projection beneath it, and
  `FixedIndex(3)` and `RuntimeIndex { selector }` with a selector known to be 3
  are two rows for one storage cell. The checker decides overlap from parent
  links and what it knows about selector values. Nothing may treat unequal
  identities as disjoint.
- **Lookups are derived columns, not maps.** A root by `PlaceId`
  (`MachineTables::roots`), a child by segment (`PlaceRow::children`, sorted,
  binary search), and the writes to a place (`PlaceRow::writes`, in program
  order). Elaboration fills each as it pushes rows. A lane that needs
  another lookup stops and reports it, as it would for a new table; it never
  adds a map.

## Descriptor extent observations

`Segment::Extent` projects the current `u64` extent (length) beneath an exact
borrowed byte descriptor. It uses the existing place interning and reaching-write
walk; replacing the descriptor or an overlapping mutable call invalidates its
prior contents. A count already read is an immutable
scalar snapshot.
[Byte views](../../../../../../wiki/spec/terminal-psi/byte_views.md#values-and-observations)
requires an observation of that exact descriptor, and input requirements name
its `ViewExtent`. An observed formal's extent uses `EntryContent` like a scalar
field. A fresh read equals that entry snapshot only while the actual write
history still supplies it. The projection stays within the descriptor's
ownership and adds no physical field, backing or ownership obligation.

## What a lane may add

A lane adds proof questions through the existing obligation kinds:

- **An obligation kind:** a variant of `obligations::ObligationKind`, whose doc
  comment quotes the specification sentence that creates it, and a row in the
  table below.

Closed refusal reasons and typed metadata projections extend their existing
enumerations with the same specification citation, as `Extent` does
above.

A justification (`values::Justification`) is neither. A lane does not add one,
or reuse one for a second meaning: a new reason a knowledge row can hold changes
this contract and needs its own pull request, whose variant quotes the
specification sentence it stands for, as the existing ones do.

A lane never adds a table, a second identity for an existing concept, a map
keyed by an identity, or a field holding a name, a printed expression, a path
string or a formatted handle. Text appears only in `Refusal::detail`. A lane
that thinks it needs one stops and reports why.

Every checking step returns `Checked<T>`: a row, or a `Refusal` carrying a
closed `RefusalReason`, the machine and the lowered `Construct` it concerns.
A step cannot return "nothing". A construct the lane does not serve is refused
as `Unsupported`, never accepted by default.

Four reasons are verdicts about the program and the rest are about the
checker. `Unproved` means an obligation's evidence did not check.
`Unestablished` means a place is read, copied, passed or returned on a path
where nothing established it. A local declared without an initializer is
zero-constructed (`local-without-initializer`), so for plain storage this means
the zeroed storage of a gated type
([dependent values](../../../../../../wiki/spec/language/dependent_values.md#default-domains-and-zero-initialization):
"Otherwise the type is gated: construction or checked qualification must
establish the domain before observation."). It is a structural judgment over the
write log and the graph, with no obligation row and no certificate; the places
foundation lane added it as a reason, not a table. That lane's two structural
judgments over a sum use the same reason: a case payload named where its case
is not established on every path
(structural_access.md: "Each `Case` step requires source admission to
establish the selected case for the same root and prefix at the read's
observation epoch"), and a dispatch with no edge for a declared case
([patterns](../../../../../../wiki/spec/language/patterns.md): "Missing
runtime coverage is a compile error, not an implicit trap or fallthrough").
`Conflict` means two live loans cannot both exist (overlapping places with an
exclusive access, or an exclusive reborrow of storage held shared): a resource
judgment over the lowered form, with no certificate. `Unsupported` means the
checker does not serve the construct; it says nothing about the program.

## What the tables cannot say yet

A lane met each, reported it, and read the fact from the lowered form instead.

- **Where a copy's contents came from.** `Content` cannot say "what that other
  place held". A whole copy logs `Overwritten`, and the checker reads the
  source from the operation at that row.
- **Which case a construction established.** No `Content` form names a case,
  so a case literal logs `Overwritten` on its root, and the checker reads
  `result_case` from the establishing operation at that row.
- **That a sum holds a case on an edge.** A `StructuralCase` terminator has no
  Boolean value, and knowledge names only values, so the case selected on one
  of its edges is not a knowledge row. It lives in the structural payload
  judgment of `terminal-psi-to-terminal-psi`. What such an edge *binds* is about
  values (each payload parameter equals the value its payload leaf holds), so it
  is a knowledge row justified as `Justification::DispatchBinding`.
- **A domain debt on a data type's `requires`.** `ObligationKind::DomainDebt`
  names its domain by a `DomainSemanticId`; a data `requires` clause has no such
  identity (its clauses are stored in `Elaboration::domains`, by type and
  position). An invariant window on one is therefore the opening write's
  `DataDomain` row, which `terminal-psi-to-terminal-psi` follows to its
  consumption points, not a conserved row here.

- **What a call supplies for an erased formal.** `Actual` is a value or a
  place. The argument for an `[erased]` scalar formal is a term that is never
  evaluated
  ([proof contracts](../../../../../../wiki/spec/proofs/contracts.md#explicit-erased-bindings):
  "Construction supplies the erased term"), so `InvocationRow::actuals` lists
  only the runtime actuals, and the checker reads the erased terms from the
  call's `erased_arguments`, by position against the callee's
  `erased_scalar_formals`.

## Obligation kinds

| Kind | Family | Created by |
| --- | --- | --- |
| `Claim` | conserved | [ownership](../../../../../../wiki/spec/terminal-psi/ownership.md): "Structural ownership is an exact live set of roots, projected claims, and remaining subtrees" |
| `Loan` | conserved | [loans](../../../../../../wiki/spec/terminal-psi/loans.md): "Loan resource \| Exact occurrence, owner lineage, captured place, access, parent lifetime, and restoration obligation." The lender is the owner, a parent loan of the machine, or the loan the invocation passed in at a borrowed formal (`Lender::Incoming`: "A direct reborrow names its immediate parent resource"). |
| `RestorationWindow` | conserved | [language ownership](../../../../../../wiki/spec/language/ownership.md#borrowed-storage-invariant-windows): "records a restoration obligation on the original place and loan" |
| `DomainDebt` | conserved | [dependent values](../../../../../../wiki/spec/language/dependent_values.md#invariant-windows): "The domain must be proved again at the next consumption point" |
| `EstablishmentDebt` | conserved | [dependent values](../../../../../../wiki/spec/language/dependent_values.md#invariant-windows): "may open initial target-establishment debt before a valid target value exists" |
| `CallRequirement` | question | [fact validity](../../../../../../wiki/spec/terminal-psi/fact_validity.md): "one exact obligation for every callee `requires` clause" |
| `IntegerWidthPositive` | question | [width-indexed integers](../../../../../../wiki/spec/language/numeric_values.md#width-indexed-integers): "Generic formation proves positivity under the declaration's static requirements." One per actual static width input used by the owning machine's declared carriers, at its entry, including unread borrowed storage and zero-length array element types. |
| `OperationSafety` | question | [control flow](../../../../../../wiki/spec/terminal-psi/control_flow.md): "Each partial operation still proves its own safety" |
| `DataDomain` | question | [data and literals](../../../../../../wiki/spec/language/data_and_literals.md): "Construction and every restoring write prove the owning default-domain clauses." One per (operation, clause of a data type's `requires`), at a construction, a zero establishment and a store into a field of that type or of a type containing it; [structural access](../../../../../../wiki/spec/terminal-psi/structural_access.md#complete-record-construction): "The obligation binds the initializer or replacement SSA value, never the old field contents." |
| `ArrivalInvariant` | question | control flow: "one obligation for every actual incoming edge" |
| `NormalGuarantee` | question | control flow: "All normal returns ... must establish the published guarantees." One per (normal return edge, clause); names its `PublishedGuarantee`. |
| `PublishedGuarantee` | question | the same sentence: the per-clause identity in `ContractClause`, holding when every return edge's `NormalGuarantee` for the clause holds (vacuously with no return). |
| `CrashCoverage` | question | [calls and outcomes](../../../../../../wiki/spec/terminal-psi/calls_and_outcomes.md#crash): "proves every site guard ... and checks same-cause coverage". Its `CrashSite` is a block with a no-successor terminator ("closed causes on distinct no-successor terminators") or a call operation whose callee may return and crash ("Surviving crash routes are derived from the pinned callee contract using exact actuals") |
| `WellFoundedness` | question | control flow: "a shared well-foundedness question" |
| `RankComparison` | question | control flow: "comparison obligations for every internal edge" |

## The four numbers every later PR reports

1. **Obligations with kernel-checked evidence:** of the obligations the lane
   rebuilds, how many are discharged by kernel-checked evidence, as a count
   with its denominator, per obligation kind.
2. **Decisions keyed on text:** branches or lookups in the lane's code that
   compare a name, spelling, printed expression or path string. The target is
   zero; the 2026-11-10 atlas counted 2,004 in the current compiler.
3. **Silent drops:** places where the lane's code discards an input, an
   obligation or a failed lookup without a refusal (`unwrap_or_default`,
   `filter_map` over failures, `_ =>` arms that accept). The target is zero;
   the atlas counted 930.
4. **Routes per obligation kind:** how many separate code paths create or
   discharge each kind. The target is one; more than one is the duplication
   this design exists to remove.

## Open

- **Text inside propositions.** A knowledge row points into its machine's
  `propositions` table (`PropositionId`). `semantic_vocabulary::Proposition` is
  reused unchanged, and its `ContentConservation` variant still reaches text
  keys: `ContentAlgebra::parameter`, `ContentPlaceSegment::{Case, Field}` and
  `ContentProjectionScalar` hold `String`s. Route one (scalars) uses none of
  them. The lane that adds content knowledge first replaces them with typed
  identities; this crate does not wrap or copy them.
- **A second place form inside propositions: decided, values only.**
  `Proposition`'s structural variants (`StructuralIdentity`,
  `StructuralCaseMembership`, IEEE and byte-sequence fields) name storage as
  `PlaceId` plus `Vec<CanonicalStructuralPathSegment>`, beside this crate's
  `ExactPlaceId`. The places foundation lane chose one form
  (`wiki/drafts/designs/places_foundation_lane.md`, "One route, no second
  carrier"): knowledge never names a place. A storage observation enters as the
  value its read produced, and what the read equals is one more equation between
  values, derived from the stores the lowered operations make
  (`terminal-psi-to-terminal-psi`, `places.rs`), which the write log must
  restate. No lane uses a structural `Proposition` variant in a knowledge row; a
  contract clause that can only be stated as one is refused.
- **Contract propositions have no table.** Terminal Psi keeps a machine's
  `requires` and `ensures` inline in its `MachineContract`; its only
  `PropositionId` table holds named proposition declarations. A callee clause
  is identified by its machine and position. Knowledge rows index this crate's
  own machine-local `propositions` table.
