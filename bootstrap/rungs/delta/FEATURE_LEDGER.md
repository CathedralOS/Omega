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
| records, fixed arrays, slices, and payload sums | compiler corpus and current bridge slices | use records plus explicit tags where payload-sum machinery does not pay for itself | unresolved |
| runtime-sized reservation from fixed backing and integer-offset arenas | storage canary and `lowermachine` tables | use statically partitioned fixed arrays or ordinary Delta library code; otherwise retain only deterministic bump/paged reservation, specified exhaustion, and bulk reset actually needed by the bridge | unresolved |
| host boundary | current source declares `boundary trait Console` with partly hardwired operations | use one sealed interface for source bytes, artifact bytes, diagnostics, and termination | general boundary traits not presumed |
| source units and modular organization | the canonical length-delimited bridge bundle preserves labels and exact bytes; the completed bridge must publish a transitive multi-source closure | keep a bundle-wide namespace if it supports maintainable separate source units; add native package semantics only for a demonstrated bridge requirement | bundle demonstrated; language model unresolved |
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
