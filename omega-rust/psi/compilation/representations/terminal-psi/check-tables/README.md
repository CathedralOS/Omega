# Check tables

These are the tables the checker on the lowered form reads and writes, and the
contract every construct-family session builds against. They hold what is
known about one machine's unsealed Terminal Psi module as rows over dense
identities, so no checking step ever needs a name, a printed expression or a
path string to find a fact. The crate is types only: no elaboration, no
checking, no prover. The decision it serves is
[check on the lowered form](../../../../../../wiki/drafts/designs/check_on_lowered_form.md).

## What each table holds

All tables are `IdTable`s: flat `Vec`s indexed by identity. Elaboration mints
every identity with `IdTable::push`, so identities are dense from 1 and a
lookup is one index.

| Table | Indexed by | Holds |
| --- | --- | --- |
| `values` | `ValueId` | One evaluated scalar: its type, its single origin, and for a callable selector the machines it may select. A write never changes it. |
| `knowledge` | `KnowledgeId` | A proposition over value identities, the point it is available from, its one justification and its premises. It never becomes false. |
| `propositions` | `PropositionId` | The proposition each knowledge row states. Machine-local, minted by `push`: Terminal Psi keeps contract propositions inline and has no proposition table. |
| `places` | `ExactPlaceId` | A root place, or a parent place plus one typed segment (field, case, fixed index, runtime index named by its captured selector value), with its sorted children and the writes to it. |
| `roots` | `PlaceId` | The `ExactPlaceId` of each root place. |
| `writes` | `WriteId` | The forward write log: a value, an installed reference, unknown contents, or a hole. |
| `referent_sets`, `selection_sets` | `ReferentSetId`, `SelectionSetId` | May-sets: the places an installed reference may denote, the machines a selector may select. Join is union; `Unknown` means everything. |
| `obligations` | `ObligationId` | Claims, loans, windows and debts (conserved across edges) and proof questions (discharged at one site). |
| `edges` | `EdgeId` | Source, destination, and the fate of every conserved obligation crossing the edge. |
| `invocations` | `InvocationId` | The one entry operation of ordinary and terminal calls: callee, ordered actuals, one requirement obligation per callee clause. |
| `summaries` (module) | `MachineId` | A callee's published contract, one term per axis: reach, invokes, suspension, blocking, crash routes, termination. Its `requires` and `ensures` are the callee's `MachineContract` in the lowered module, by clause position. |

`ValueId`, `EdgeId`, `ObligationId`, `MachineId`, `BlockId`, `OperationId` and
root `PlaceId` are the lowered form's own identities, so the checker and the
emitted product name the same things. `ExactPlaceId`, `KnowledgeId`,
`WriteId`, `ReferentSetId`, `SelectionSetId` and `InvocationId` belong to this
crate because the lowered form has no identity for those concepts (its
`PlaceId` names roots only). `PropositionId` is reused from the vocabulary, but
here it is machine-local and indexes `MachineTables::propositions`, not the
module's named proposition declarations.

References and callables are keyed differently because the lowered form holds
them differently. A reference is a structural place, so the places it may
denote are installed by the write that stores it (`Content::Reference`). A
callable selector is a scalar value, so what it may select is a column of its
value row.

## Rules every lane relies on

- **Identities never cross `MachineTables`.** They are machine-local integers
  with no machine tag. `row()` refuses an identity past the end of a table, but
  an in-range identity from another machine resolves to the wrong row with no
  error. So every checking API takes exactly one `MachineTables` and only
  identities read from it. Crossing machines goes through an `InvocationRow`
  and the callee's `Summary`, which hold `MachineId`s and clause positions.
- **Equal place identities mean the same place; unequal ones do not mean
  disjoint places.** A root overlaps every projection beneath it, and
  `FixedIndex(3)` and `RuntimeIndex { selector }` with a selector known to be 3
  are two rows for one storage cell. Overlap is a question the checker answers
  from parent links and knowledge about selector values. Nothing may treat
  unequal identities as disjoint.
- **Lookups are derived columns, not maps.** A root by `PlaceId`
  (`MachineTables::roots`), a child by segment (`PlaceRow::children`, sorted,
  binary search), the writes to a place (`PlaceRow::writes`, in program
  order), and a selector's selection set (`ValueRow::selects`). Elaboration
  fills each as it pushes rows. A lane that needs another lookup stops and
  reports it, as it would for a new table; it never adds a map.

## What a lane may add

A construct-family lane changes this crate in exactly two places:

- **An obligation kind:** a variant of `obligations::ObligationKind`, whose doc
  comment quotes the specification sentence that creates it, and a row in the
  table below.
- **A summary term:** a field of `edges::Summary`, cited the same way.

A lane never adds a table, a second identity for an existing concept, a map
keyed by an identity, or a field holding a name, a printed expression, a path
string or a formatted handle. Text appears only in `Refusal::detail`. If a
lane believes it needs any of those, it stops and reports why instead.

Every checking step returns `Checked<T>`: a row, or a `Refusal` carrying a
closed `RefusalReason`, the machine and the lowered `Construct` it concerns.
There is no way to return "nothing". A construct the lane does not serve is
refused as `Unsupported`, never accepted by default.

## Obligation kinds

| Kind | Family | Created by |
| --- | --- | --- |
| `Claim` | conserved | [ownership](../../../../../../wiki/spec/terminal-psi/ownership.md): "Structural ownership is an exact live set of roots, projected claims, and remaining subtrees" |
| `Loan` | conserved | [loans](../../../../../../wiki/spec/terminal-psi/loans.md): "Loan resource \| Exact occurrence, owner lineage, captured place, access, parent lifetime, and restoration obligation." |
| `RestorationWindow` | conserved | [language ownership](../../../../../../wiki/spec/language/ownership.md#borrowed-storage-invariant-windows): "records a restoration obligation on the original place and loan" |
| `DomainDebt` | conserved | [dependent values](../../../../../../wiki/spec/language/dependent_values.md#invariant-windows): "The domain must be proved again at the next consumption point" |
| `EstablishmentDebt` | conserved | [dependent values](../../../../../../wiki/spec/language/dependent_values.md#invariant-windows): "may open initial target-establishment debt before a valid target value exists" |
| `CallRequirement` | question | [fact validity](../../../../../../wiki/spec/terminal-psi/fact_validity.md): "one exact obligation for every callee `requires` clause" |
| `OperationSafety` | question | [control flow](../../../../../../wiki/spec/terminal-psi/control_flow.md): "Each partial operation still proves its own safety" |
| `ArrivalInvariant` | question | control flow: "one obligation for every actual incoming edge" |
| `NormalGuarantee` | question | control flow: "All normal returns ... must establish the published guarantees." One per (normal return edge, clause); names its `PublishedGuarantee`. |
| `PublishedGuarantee` | question | the same sentence: the per-clause identity in `ContractClause`, holding when every return edge's `NormalGuarantee` for the clause holds (vacuously with no return). |
| `CrashCoverage` | question | [calls and outcomes](../../../../../../wiki/spec/terminal-psi/calls_and_outcomes.md#crash): "proves every site guard ... and checks same-cause coverage". Its `CrashSite` is a block with a no-successor terminator ("closed causes on distinct no-successor terminators") or a call operation whose callee may return and crash ("Surviving crash routes are derived from the pinned callee contract using exact actuals") |
| `WellFoundedness` | question | control flow: "a shared well-foundedness question" |
| `RankComparison` | question | control flow: "comparison obligations for every internal edge" |

## The four numbers every later PR reports

1. **Obligations with kernel-checked evidence:** of the obligations the lane
   reconstructs, how many are discharged by evidence the proof kernel checks,
   as a count with its denominator, per obligation kind.
2. **Decisions keyed on text:** branches or lookups in the lane's code that
   compare a name, spelling, printed expression or path string. The target is
   zero; the 2026-11-10 atlas counted 2,004 in the current compiler.
3. **Silent drops:** places where the lane's code discards an input, an
   obligation or a lookup failure without a refusal (`unwrap_or_default`,
   `filter_map` over failures, `_ =>` arms that accept). The target is zero;
   the atlas counted 930.
4. **Routes per obligation kind:** how many distinct code paths create or
   discharge each kind. The target is one; more than one is the duplication
   this design exists to remove.

## Open

- **Text inside propositions.** A knowledge row points at its machine's
  `propositions` table (`PropositionId`). `semantic_vocabulary::Proposition` is
  reused unchanged, and its `ContentConservation` variant still reaches text
  keys: `ContentAlgebra::parameter`, `ContentPlaceSegment::{Case, Field}` and
  `ContentProjectionScalar` hold `String`s. Route one (scalars) uses none of
  them. The lane that adds content knowledge replaces them with typed
  identities first; this crate does not wrap or copy them.
- **A second place form inside propositions.** `Proposition`'s structural
  variants (`StructuralIdentity`, `StructuralCaseMembership`, IEEE and
  byte-sequence fields) name storage as `PlaceId` plus
  `Vec<CanonicalStructuralPathSegment>`, beside this crate's `ExactPlaceId`.
  Knowledge here is over values, and a storage observation enters as the value
  a read produced, so route one needs neither. The records-and-fields lane
  must choose one form before it adds knowledge about places; it must not keep
  both.
- **Contract propositions have no table.** Terminal Psi keeps a machine's
  `requires` and `ensures` inline in its `MachineContract`; its only
  `PropositionId` table is named proposition declarations. `Summary`
  therefore names no proposition: a callee clause is its machine and
  position. Knowledge rows index this crate's own machine-local
  `propositions` table. `CrashRoute::guards` still assumes a module
  proposition table.
  A body-derived result relation has no `MachineContract` slot; the lane
  that publishes `SummaryBasis::BodyDerived` decides where it lives.
- **No carry term in `Summary`.** The design record lists carry as a summary
  axis, but the specification defines carry on values, claims and suspension
  crossings, not as a callee's published term. The sentences found:
  [carry](../../../../../../wiki/spec/resources/carry.md) "Ordinary data
  composes its live fields and explicit type-wide policy using the most
  restrictive demand on each axis" and "A machine that may retain
  CPU/thread-restricted values requires its runtime to establish the
  corresponding preservation"; [calls and outcomes](../../../../../../wiki/spec/terminal-psi/calls_and_outcomes.md#suspension)
  "Terminal retains provider-independent carry demand" and a suspension plan
  "retains each value/place, type, storage role, structural access, exact live
  claims, and carry requirements". None says what a callee publishes. The
  suspension lane adds the term with its citation, or asks the owner.
