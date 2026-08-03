# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Last pruned: 2026-08-02.

## Q1 — Who owns entry-provisioned program-storage root identities?

The typed process/firmware handoff must introduce a small number of authority
roots for the loaded image and initial stack/storage, after which the compiler
derives sections and statics as subextents. Which semantic shape is canonical?

- one core-owned stable entry requirement with core-owned image/storage Extent
  domains, inherited by `UefiApplication::entry` and other target entry traits;
  or
- target-owned entry requirements and root domains, with some separate generic
  relationship by which Omega recognizes their image/storage roles?

Core `Extent::Granted` cannot directly cite a Cathedral/UEFI-specific route,
and recognizing friendly target domain names would violate the ownership
firewall. This decision fixes provider-schema identity, route ownership, the
typed handoff shape, and the generic derivation key used for sections/statics.

## Q2 — What is the final artifact-footprint certificate boundary?

Final machine-state admission must independently validate the exact placed
bytes of compiler functions, format-owned thunks, relaxation products,
veneers, generated stubs, and admitted leaves against the selected
`StatePlan`. Which evidence boundary is canonical for both statically linked
and dynamically loaded admitted artifacts?

- a self-describing, versioned instruction/region certificate whose normalized
  semantic rows are replayed against exact final bytes by the admission
  checker; or
- an independent target decoder which derives the complete footprint directly
  from final executable regions, with admitted leaves joined through a
  separate receipt vocabulary?

The choice fixes the certificate format, the trusted decoder surface, how
relocation and generated-region identities bind to decoded instructions, and
where admitted leaf claims enter transitive composition. The current exact
relocation envelope, checked-assembly validators, import-thunk validators, and
complete executable-region inventory are sound precursors, but none may claim
complete final-footprint validation until this boundary is settled.

## Q3 — Where are width-varying foreign record fields normalized?

The portable filesystem metadata surface needs one semantic record, but the
native `struct stat` fields are not representation-identical. Linux x86-64
uses 64-bit `st_nlink` and `st_blksize`, while the AArch64 asm-generic ABI uses
32-bit fields; Darwin differs again. The current `FieldPlan::At` only relocates
one representation-identical field, and `Bits` requires complete source-bit
tiling, so neither can honestly project and extend these target-sized integers.
Which boundary is canonical?

- extend the closed layout-plan vocabulary with a checked integer placement
  that names stored width and signed/zero extension into the semantic carrier,
  including explicit rules for whether and how mutable views may write it; or
- keep layout plans representation-preserving and require target-owned checked
  adapter machines to decode raw foreign bytes into one canonical semantic
  metadata record before portable code observes it?

The choice determines whether width conversion is layout semantics or ordinary
target-policy computation, whether direct foreign-record views remain the
filesystem mechanism, and what read/write guarantees a future width-adapting
placement would carry. Correct Linux `StatLayout`, path metadata, and decoded
descriptor metadata must not claim completion until this is settled.

## Q4 — What is the canonical terminal-Psi conditional edge?

The live terminal-Psi vocabulary has only total unconditional jumps and
returns. Its architecture requires future guards and branch-created blocks to
be explicit, but it does not yet choose the semantic shape that makes one
conditional transition independently verifiable, executable, serializable,
meterable, and lowerable. Which edge form is canonical?

- one conditional edge with an exact Boolean guard and ordered true/false
  successors, each carrying its own typed block-parameter bindings and edge
  actions; or
- separate guarded edges whose mutual exclusivity and exhaustiveness are
  reconstructed as a block-level obligation?

The choice fixes successor ordering and identity, whether the guard is an
already-defined Boolean value or may contain a closed predicate form, where
branch exclusivity/exhaustiveness is proven, which selected edge is charged,
and how exact-path and safe-point fuel certificates identify the untaken arm.
Do not freeze semantic v13, extend the codec, or publish branch certificates
until this shape is settled as one reviewed vertical slice.

## Q5 — What is the source surface for proposition families and aliases?

The law-bearing-relation ruling requires nominal proposition families over
representative values, typed proof-static index telescopes, and transparent
proposition aliases backed by carrierless selected-conformance evidence. It
does not settle how any of those declarations are written. The live grammar
has executable `bool` machines and transparent domain aliases, but neither is
a proposition family: running a decider is not evidence, and a domain
classifies one carrier rather than relating independently indexed values.
Which source model is canonical?

- add a dedicated proposition-family declaration with explicit static-index
  and representative-value binders, plus a distinct transparent proposition-
  alias declaration whose right side names carrierless evidence; or
- define proposition families entirely as a normalized projection of an
  ordinary proof trait/conformance, with a separate alias form naming that
  projection and no new nominal declaration kind?

The choice fixes proposition symbol identity, generic/index binder syntax,
how proposition application appears in `requires`/`ensures` and proof bodies,
how aliases bind hidden evidence terms, and which declaration selected
`Reflexive`/`Symmetric`/`Transitive` conformances name. Do not migrate `%` from
its executable-`bool` pilot or add a parser-only `Prop` spelling until this
surface is settled end to end.

## Q6 — How does terminal Psi bind partition input claims without asserting equality?

A checked direct partition wrapper retains exact entry claim identities and a
mechanically replayable partition substitution, but it correctly has no
one-to-one identity reshuffle: aggregate conservation does not prove that any
input equals one particular output. Terminal semantic v12 currently lets a
partition composition name an input claim only through a
`ContentIdentityReshuffle` row, which would assert that stronger and potentially
false equality. Which versioned representation is canonical?

- add an independent entry-claim binding row that records the dense claim ID,
  projection, algebra, and entry structural place without naming an output; or
- make each partition-composition row carry its complete input-claim bindings
  directly, keeping those bindings scoped to the derived theorem rather than a
  reusable module-level table?

The choice fixes claim-ID canonicalization, duplicate/overlap validation,
whether other future content axioms may reference entry claims independently,
and the next semantic-version allocation alongside Q4's conditional-edge
slice. The checked-to-terminal producer must remain fail-closed, and no
content-bearing executable source canary may claim completion, until the
verifier, codec, migration, proof bundle, and producer consume one reviewed
shape end to end.

## Q7 — How does a named whole-trait conformance bind its requirement satisfiers?

Omega promises that `Type satisfies Trait as Name` selects one coherent,
complete requirement surface and that one type may provide several such
surfaces. The live representation gives the standalone whole-trait edge a
stable `Type::Name` symbol, but validation currently chooses attached states by
requirement name without consulting `Name`. Separately, a machine may spell
`satisfies Trait::requirement as Name`; the guide's one-requirement proxy
example treats that spelling as enough to cast to `dyn Type::Name`, even though
no standalone whole-trait edge is declared. Which binding model is canonical?

- make the standalone whole-trait declaration own an explicit, complete
  requirement-to-satisfier map (including defaults and laws), with machine
  aliases serving only as references used by that map; or
- define a conformance as the coherent group of attached satisfiers carrying
  the same `as Name`, with a precise rule for whether a standalone declaration
  is required and whether unique unaliased/default satisfiers may fill missing
  members.

The choice fixes completeness checking, default-member inclusion, overload
selection, whether two named conformances may deliberately share a satisfier,
third-party conformance coherence, and the exact per-requirement adapter rows
stored in a local dynamic table. Checked selection may retain the stable edge
identity, but Psi and Omega must not emit requirement adapters or a table by
guessing satisfiers from matching state names until this association is
settled.
