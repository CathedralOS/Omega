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

This table records decisions still to make, not the history of every gate that
produced the evidence. Exact formats, fixture shapes, resource counts, and
negative matrices belong in the linked bridge contracts and beside their gates.

| Candidate | Current evidence | Simpler form to test first | Status |
| --- | --- | --- | --- |
| machines, states, transitions, loops, and calls | `lowermachine` self-host, historical O0/O1 canaries, and the CKIR2 exact-root tranche demonstrate typed finite acyclic attached-machine calls across sources in one logical module; producer, Rust-free meaning, lower-rooted reconstruction, and one-frame composition are closed | keep the finite static-call form; add recursion, general member receivers, or broader module/package calls only when the complete bridge requires them | finite static calls demonstrated; broader forms and final disposition unresolved |
| integer arithmetic | D0 and the Rust producer accept several overflow policies and disagree at some edges | use Exact throughout; add only a narrow modular operation if artifact encoding requires it | unresolved |
| records, fixed arrays, and recursively constant aggregates | the checkpoint-000001 frontend probe and selected one-/two-package checked-IR paths establish records, arrays, nominal identity, conservative lowering, and lower-rooted reconstruction for bounded runtime source tranches; focused CKIR3 native/self gates lower the exact generated Unicode tables and general controls through a typed interned constant DAG, while all native/self/mixed producer-backend pairs compose to independently evaluated result 70 and an independently reconstructed read-only image/ELF | retain the typed constant graph/pool and aggregate copy unless the remaining Rust-free meaning/refinement cost reverses the comparison with hand-expanded initialization or positional compiler data | bounded runtime records/arrays and focused constant-aggregate composition demonstrated; Rust-free meaning, lower-rooted assurance, general coverage, and final disposition unresolved |
| slices and runtime views | checkpoint 000001 uses source, decoded-byte, token, and spelling views, but no complete bridge artifact path exists yet | compare a regular slice facility with explicit backing-plus-span records; retain slices when the latter duplicates checking or obscures ownership | observed in product source; bridge cost and final disposition unresolved |
| payload-bearing sum data | checkpoint 000001 uses token, numeric-base, diagnostic, and console-result sums, but the current checked-IR tranches do not lower them | compare general tagged data with explicit tag-plus-payload records; do not force the split when it increases invalid states or duplicated dispatch | observed in product source; bridge cost and final disposition unresolved |
| runtime-sized reservation from fixed backing and integer-offset arenas | storage canaries and current compiler tables demonstrate fixed partitioning, checked exhaustion, and bulk reset without a general heap | keep fixed arrays or library arenas while sufficient; add deterministic bump/paged reservation only when the complete bridge needs it | fixed partitioning demonstrated; runtime reservation unpresumed |
| host boundary | current source declares `boundary trait Console` with partly hardwired operations | use one sealed interface for source bytes, artifact bytes, diagnostics, and termination | general boundary traits not presumed |
| source units and modular organization | the bundle, compilation envelope, resolver witness, checked-IR lowering, and independent reconstruction close bounded public multi-unit custody and nominal identity; accepted-lock authority and private cross-module semantics remain separate | keep package/module semantics in Omega and pass Delta one reconciled graph; decoding that graph does not make package semantics a Delta feature | bounded public multi-unit path demonstrated; authority and general closure open |
| contracts, refinements, and proof-oriented syntax | experimental producer corpus | runtime/static checks plus externally checked emitted certificates | not presumed |
| mixed field-plus-case data and other producer experiments | Rust-producer acceptance or planned slices | separate records and sums | not presumed |

Evidence owners for the longer rows are
[`SOURCE_CUSTODY_FRONTEND_PROBE.md`](../../omega-bootstrap/compiler/SOURCE_CUSTODY_FRONTEND_PROBE.md),
[`OMEGA_BOOTSTRAP_COMPILATION.md`](../../omega-bootstrap/compiler/OMEGA_BOOTSTRAP_COMPILATION.md),
[`OMEGA_BOOTSTRAP_RESOLUTION.md`](../../omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION.md),
[`OMEGA_BOOTSTRAP_CHECKED_IR.md`](../../omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR.md),
[`OMEGA_BOOTSTRAP_CHECKED_IR_V2.md`](../../omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V2.md),
[`OMEGA_BOOTSTRAP_CHECKED_IR_V3.md`](../../omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V3.md),
and the lower-rooted refinement contracts
[`OMGRFN2`](../../assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS.md)
and
[`OMGRFN3`](../../assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V3.md).

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
