# Delta v1 provisional feature ledger

This ledger guides discovery while the complete `omega-bootstrap` source is
being written. It is not a Delta specification or an admission list. D0, the
sample corpus, and the Rust producer establish implementation evidence only.

A construct may enter Delta v1 when a concrete bridge requirement or an explicit
language-coherence, robustness, safety, or maintainability argument shows that
retaining it lowers whole-bootstrap cost. Its entry must identify that reason,
exact semantics, lower-rung meaning, positive coverage, and a negative gate for
the nearest excluded form. Accidental producer/corpus behavior is removed before
the v1 freeze; Delta is not reduced to a whitelist of tokens used by one source
revision.

This ledger is not the `Ωself` profile. `Ωself` records which ordinary Omega
features the production compiler source uses and `omega-bootstrap` must accept;
this file records which Delta facilities are justified by the implementation of
that bridge. A feature excluded from one surface is not thereby excluded from,
or admitted to, the other.

No third bridge feature ledger is needed. `omega-bootstrap`'s implementation
features are Delta-v1 entries here; its accepted Omega features are `Ωself`
entries in the separate product-source profile. The full Omega specification
already governs what the resulting production compiler implements.

The objective is a small, robust compiler-host language, not the smallest token
census. A modest facility may remain without many textual occurrences when it
makes the bridge materially safer, clearer, more modular, or easier to assure.
Conversely, similarity to Omega is a consistency benefit rather than a subset
requirement.

The design floor is ordinary C-like compiler power with specified behavior:
structured control, predictable data, explicit memory/resource handling, and
sealed byte I/O. Delta may exceed the literal token census of one bridge
revision when a modest companion feature makes that floor coherent. It need not
inherit Omega's proof surface, dependent types, production allocation model, or
general host abstractions merely to look more like the product language.

## Fixed constraints

- deterministic specified behavior, with no undefined behavior;
- no ambient host authority; every failure is a checked result, static rejection,
  or defined trap rather than truncation or undefined behavior;
- lower-rung meaning for every admitted construct;
- Omega spelling, grammar, precedence, and ordinary meaning when Delta retains
  the same construct and that choice is not materially more expensive; and
- explicit rejection of unsupported source rather than producer-shaped
  acceptance.

## Current candidates

| Candidate | Current evidence | Simpler form to test first | Status |
| --- | --- | --- | --- |
| machines, states, transitions, loops, and calls | `lowermachine` self-host and O0/O1 bridge slices | remove recursion or other forms not used by the complete bridge | demonstrated, not frozen |
| integer arithmetic | D0 and the Rust producer accept several overflow policies and disagree at some edges | use Exact throughout; add only a narrow modular operation if artifact encoding requires it | unresolved |
| records, fixed arrays, slices, and payload sums | checkpoint 000001's standalone `compiler/psi/source/source.omg` passes a general Delta-written record/array/attached-machine parser and checker with exhaustive native, representative self-built, and Rust-free meaning evidence; private `CKIR1` plus its direct conservative backend now close exact native/self bytes, canonical-Gamma 0/251/252 observations, exhaustive resource/relation teeth, product behavior, and independent lower-rooted source→CKIR1→limited-ELF reconstruction for the finite, acyclic, returning tranche without widening Terminal Psi; payload sums and slices remain separate later needs | compare total bridge/assurance cost with product-source refactors as later checkpoints arrive; use explicit tags where later payload-sum machinery does not pay for itself | first artifact tranche and its lower-rooted assurance demonstrated; final retain/refactor disposition unresolved |
| runtime-sized reservation from fixed backing and integer-offset arenas | storage canary and `lowermachine` tables; the CKIR backend uses three statically partitioned fixed arenas below the lower-rung persistent-array capacity, with explicit 251/252 behavior and no general allocation dependency | keep statically partitioned fixed arrays or ordinary Delta library code while they suffice; add only deterministic bump/paged reservation, specified exhaustion, and bulk reset when the complete bridge demonstrates that need | fixed partitioning demonstrated; runtime reservation remains unresolved and unpresumed |
| host boundary | current source declares `boundary trait Console` with partly hardwired operations | use one sealed interface for source bytes, artifact bytes, diagnostics, and termination | general boundary traits not presumed |
| source units and modular organization | the canonical length-delimited bridge bundle preserves labels and exact bytes; the real Delta frontend retains bounded descriptors/labels/spans and independently scans every unit; the private compilation envelope separates labels from package/module authority and its Delta checker validates bounded wire/tables/graph/string roles/nested bundle/EOF/resources; a separate Delta resolver now independently lexes every envelope-owned source, enforces authored-module agreement, exact direct requester-local aliases and same-package paths, visibility, duplicates, deterministic declaration order, static bindings, normalized types, and the exact selected root, then publishes canonical `OMGRSW1` through exhaustive native/self-built evidence and representative Gamma `0`/`251`/`252` observations; it does not accept a resolver receipt, compare SHA-256, lower bodies, or join CKIR/ELF | keep package/module semantics in ordinary Omega and pass the bridge one reconciled, independently committed graph; do not add package semantics to Delta merely because its bridge implementation decodes that graph | structural graph transport and normalized multi-unit resolution demonstrated; authority and resolved-source artifact join open |
| contracts, refinements, and proof-oriented syntax | experimental producer corpus | runtime/static checks plus externally checked emitted certificates | not presumed |
| mixed field-plus-case data and other producer experiments | Rust-producer acceptance or planned slices | separate records and sums | not presumed |

## Freeze gate

Before Delta v1 is named complete:

1. Publish the complete deterministic `omega-bootstrap` source manifest.
2. Classify every retained feature as required by that closure or justified by
   an explicit coherence, robustness, safety, or maintainability argument, and
   record why the simpler alternative was rejected.
3. Remove accidental parser/backend behavior and experimental corpus features.
4. Publish normative grammar and semantic edge tables independent of the source
   files that motivated them.
5. Prove the complete closure valid and run native, self-hosted, and lower-rung
   differentials plus phase-isolated negative gates.

This ledger and the `Ωself` inventory answer different questions. Delta may
retain a facility the product compiler source never uses when it materially
simplifies implementation of the bridge, and the product compiler may use an
Omega feature Delta does not have when `omega-bootstrap` can implement that
feature directly. There is no requirement that either inventory be a subset of
the other.
