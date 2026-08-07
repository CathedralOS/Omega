# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Last pruned: 2026-08-06.

## Q1 — How does a `Respects` proof name a normalized argument record and domain?

The settled quotient model normalizes every operation's attached receiver and
parameters into one argument record, derives the representative-dependent
semantic precondition `P` from the operation, and requires `Respects<F, RA,
RR>` to prove domain invariance plus result congruence. It does not settle the
source-visible identities by which a proof machine names that synthesized
record, its field projections, or `P`. Which surface is canonical?

- synthesize a proof-only structural record and domain proposition for each
  callable, with stable receiver/parameter projections made available inside
  the selected `Respects` conformance; or
- require an author-declared nominal argument-record adapter and an explicit
  proposition for the representative-dependent domain, with validation tying
  both exactly to the normalized callable signature and preconditions?

The choice fixes whether parameter renames affect the record, how attached and
free machines share one proof form, how `RA` is typed over heterogeneous
carrier indices, what a partial-operation proof writes, callable and
conformance identity, terminal-Psi vocabulary, and diagnostics for omitted or
ambient-only preconditions. Until it is settled, the compiler must not bless
the legacy flattened pair-of-calls scan as a `Respects` conformance or infer
domain invariance merely because a result-congruence-shaped machine exists.

## Q2 — How does Build select a target entry schema and its implementation?

Core now defines `ProgramStorageEntry::enter`, and target packages may inherit
that stable semantic requirement while refining only target policy and ABI.
The Build model, however, still leaves its final entry schema and exact
entry-discovery/default behavior open. Cathedral can therefore declare
`UefiApplication: ProgramStorageEntry`, but the compiler cannot yet select it
and generate the UEFI-to-semantic geometry bridge without guessing. Which
selection model is canonical?

- make entry an explicit ordinary `Build` slot that names one target-entry
  schema and one satisfying implementation/conformance, with no discovery or
  implicit default; or
- make the registered target profile name its required entry schema and have
  Build resolve one eligible satisfying implementation under a specified
  uniqueness/default rule?

The choice fixes workspace composition, dependency visibility, multiple-entry
packages, target-profile defaults, lock/trust identity, diagnostics for zero or
multiple candidates, and the input to generated ABI stubs. Until it is settled,
the compiler must not recognize `Main::run`, `main`, or any other export by
name, and Cathedral's raw UEFI callable remains transitional.

## Q3 — How does a witness-bearing proposition declare its evidence contract?

A nominal witness-bearing proposition owns one canonical carrierless evidence
interface. That interface determines what every establishing conformance must
supply and what proof-only elimination may recover. The proposition symbol
keeps its nominal identity when the evidence packaging is revised, but the
normalized interface is fingerprinted semantic content: changing it is a
breaking proof-interface revision.

The declaration category, binders, and evidence semantics are settled. The
remaining choice is the dedicated source position that publishes the
interface:

- use an explicit clause after the proposition signature, with a clause word
  reserved for the one evidence interface; or
- retain the domain-like brace form whose sole entry names that interface.

The interface must not appear among ordinary `where` constraints: bounds
restrict which applications are well formed, while this interface is
identity-bearing public proof content. It also must not use `=`. A transparent
`=` proposition expands logically, creates no nominal identity, and changes
meaning when its right-hand side changes; a witness-bearing proposition is
nominal and exposes an opaque introduction/elimination contract.

The choice fixes parsing and diagnostics, canonical source rendering, the
normalized proposition-declaration row, and how API review distinguishes an
evidence contract from ordinary generic bounds. Until it is settled, the
current brace-shaped syntax node is provisional and must not be treated as the
permanent language spelling.

## Q4 — Where does a content namespace declare its origin policy?

Every `IntervalSet<CoordinateSpace>` coordinate space and
`CountedQuantity<Unit>` unit has one closed origin policy. `ProgramLocal`
permits owner-authorized declared capacity; `ProviderBacked` permits fresh
roots only through selected admitted issuance. The compiler currently retains
only the normalized identity of the generic argument. An ordinary declaration
such as `data PhysicalMemory {}` says neither which policy applies nor which
package owns that decision. Which source surface is canonical?

- introduce a dedicated sealed content-namespace declaration that names the
  coordinate-space or unit identity and its origin policy; or
- attach a content-origin clause to the owner-controlled nominal data
  declaration used as the algebra parameter, with the clause becoming active
  when that type is selected by `IntervalSet` or `CountedQuantity`.

The choice fixes declaration ownership, imports and visibility, duplicate or
conflicting policy diagnostics, normalized identity and fingerprints, artifact
provenance, and where declared `ProgramLocal` capacity is authorized. Omission
must reject once a type is used as a content namespace; the compiler must not
infer the policy from a name, constructor, content projection, or provider use.
Relating a program-local namespace to external reality remains a separate
admitted correspondence and must not be implied by either spelling.
