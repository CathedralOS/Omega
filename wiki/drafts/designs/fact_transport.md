# Fact transport

Proposal, not ratified. Evidence measured on `main` at `d10c04d34d6`
(2026-11-09). No compiler change or fixture outcome is claimed here.

The Psi checker keeps forgetting what it already proved when a value moves:
through a call, out of a field, past a write, into a case arm, around a loop or
into a Build-selected implementation. Since 2026-11-01, 202 `psi:` PRs each
restored one kind of fact at one such place. They are not 202 bugs. They are
one missing mechanism: every fact kind has its own store, its own path
vocabulary and its own transport code at every place, and nothing moves "all
facts" across a place. This design gives the checker one place type, one fact
environment and one transport operation per place, so a new fact kind gets
every place for free and a new place carries every fact kind.

## The pattern in the PRs

Classifying the 202 "preserve / keep / carry / retain" titles by fact kind and
place (keyword heuristic over titles; individual cells are approximate):

| Fact kind | Return | Args | Write | Field | Case | Loop | Nested / pause | Build / generic | Total |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Reference origins, views, backing | 10 | 5 | 10 | 6 | 1 | 2 | 5 | 5 | 52 |
| Predicates, contracts, domains | 2 | 5 | 3 | – | 1 | 1 | 2 | 8 | 32 |
| Bounds, ranges, guards | 3 | 3 | 3 | 1 | 2 | 2 | 3 | 5 | 31 |
| Loans, borrows, lifetimes | 1 | 7 | 4 | 6 | 1 | 1 | 4 | 2 | 30 |
| Case and generic identity | – | 1 | – | 1 | 3 | – | – | 16 | 22 |
| Rank, termination, crash | – | 2 | – | 1 | – | – | 3 | 3 | 13 |

Totals include titles whose place the heuristic could not name. Some PRs say
the shape outright: "preserve guarded byte roundtrips in the remaining four
places" (#1454), "…across three transports" (#1451), "…through fields and
parameters" (#1317) followed by "…through remaining transports" (#1329).

The cost runs both ways. Missing transport rejects valid programs; transport
that keeps a fact it should kill accepts invalid ones: "stop a rebound
reference from keeping its old referent's bounds" (#1717), "retire facts read
through a reference when its referent is stored" (#1734).

## What the code does today

Paths are under `omega-rust/psi/compilation/`. `P5` is
`pipeline/05_typed-trees-to-checked-trees/src`, `V` is `validation/src`.

### Stores and places

- About 16 published fact stores in `CheckFacts`
  (`representations/checked-trees/src/facts.rs:23`, about 50 fields) and about
  6 transient query stores in validation. No trait or struct holds
  heterogeneous facts behind rebase / substitute / invalidate.
- One canonical place exists: `facts::Place { root, segments }`
  (`representations/checked-trees/facts/src/contexts_and_places.rs:1975`), with
  an owned twin `V/places.rs:5319` `CanonicalPlace`. Around it sit about 70
  `*Place*` / `*Path*` / `*Projection*` types, including
  `BorrowLoanOwnerSegment`, `CheckedUnitStructuralPathSegment`,
  `CapturedStorageSegment`, `CheckedStructuralPredicatePathSegment` and two
  local `PlaceSegment` enums.
- `ValueEnvironment` (`V/arithmetic_domains.rs:25765`) keys 13 of its 17 fact
  collections by display `String` and rebinds them by string prefix.
- AGENTS.md rule 3 (facts at exact projected place) holds for the ownership,
  borrow and invalidation rows. It does not hold for the `FactTable` index
  (by program point), `AssignedFactRef` (root only),
  `StructuralJudge.invalidated_storage` (root only) or the string-keyed numeric
  environments.

### Overlap

"Does a write to A affect a fact at B" has about ten implementations over two
path representations: `V/places.rs:517` `canonical_place_segments_may_overlap`
(about 50 uses), `V/calls.rs:16017` `frame_paths_overlap` on strings (27 uses),
independent re-implementations in `V/domains_and_predicates.rs:10038`,
`P5/checks_borrows_overlap.rs:2688`, `P5/checks_multiplicity.rs:4435`,
`P5/flow_mutation.rs:651`, `P5/borrow.rs:2172` and
`P5/checks_ranges_facts.rs:4098`, five identical wrappers and about 19 inline
`root == root && …` copies.

### Writes, fields, cases, loops

Stage 05 already has one shared carrier: `matching_mutation_for_fact_place` and
`context_survives_place_mutations` (`P5/flow.rs:6901`, `:7016`), which its
comment calls "one law", covering the `FactPayload` variants
(`representations/checked-trees/facts/src/evidence.rs:886`). Every other store
has its own keep / kill code: the validation numeric environment
(`V/arithmetic_domains.rs:26147`, one hand-written `retain` per map), range
facts (`P5/checks_ranges_facts.rs:3717`), loans (`P5/flow.rs:6046`,
`P5/borrow.rs:9081`), custody, the structural judge's storage graph
(`V/contract_entailment.rs:71799`) and termination. Case narrowing has at least
six routines; #1843 added the same "unique compatible Boolean arm" rule to five
of them. One case-field fact (#2078) touched about 60 files across stages 01–06.

### Calls

At least seven separate normal-return modules carry results back to the caller,
split by fact kind (`V/contract_entailment.rs:57365`, `V/arithmetic_domains.rs:9854`,
`V/reference_result_custody.rs:568`, `P5/flow.rs:910`, `P5/flow_transfers.rs:1882`,
`P5/borrow.rs:3655`, `P5/checks_multiplicity.rs:5743`, …). The formal-to-actual
rewrite has no shared function; near-identical copies live in
`P5/checks_ranges.rs:831` (bounds), `P5/checks_termination.rs:1078` (ranks),
`P5/checks_contracts_calls.rs:574` (contracts), `P5/published_facts.rs`
`rebase_place`, and `V/calls.rs` `remap_place` / `rebase_invocation_home`.
About 200 functions named for substitution, rebasing or formal/actual mapping
live in 44 files. #2209 patched four carriers to return one borrowed case view.

### Build selection and specialization

Specialization runs in pass 04, before checking: it clones the template body
and runs one substitution pass per argument kind
(`pipeline/04_typed-trees-to-typed-trees/src/body_cloning_and_rewriting.rs`,
`:668` onward). Checking then re-derives every fact on the clone. Facts reach a
selected or private body only through the per-state arrival join `StateValues`
(`P5/flow_state_values.rs:2703`), a struct with one field and one meet per
fact kind. So the Build column is not facts being stripped. It is each fact
kind needing its own entry-join plumbing. About 20 distinct substitution
mechanisms exist across typed-trees, 04, 05, validation and Terminal Psi.

### Into lowering

`CheckFacts` is one nominal channel, but lowering still re-derives: stage 06
makes 158 `validation::` calls. Each "carry into lowering" PR (#2032, #1143)
moves one re-derivation into one new `CheckFacts` field.

### A probable live instance

`ValueEnvironment::rebind` (`V/arithmetic_domains.rs:25971`) builds a fresh
environment and copies only the string-keyed collections. `integer_ranges`,
`boolean_values`, `ordered_values`, `float_elements` and
`numeric_observations` do not survive a rebind. Whether this is intended is
unverified; it is exactly the loss the PR stream keeps repairing.

## Design

1. **One place key.** `facts::Place` is the only path key for facts. String
   paths and the rival segment vocabularies convert at their boundary and
   then retire.
2. **One overlap relation.** A single function returns how two places relate:
   `Disjoint`, `Equal`, `Prefix`, `Extends` or `MayOverlap`. Every keep / kill
   decision uses it.
3. **One fact environment.** A data-oriented environment keyed by `Place`,
   with dense per-kind row tables. Each fact kind supplies only what is
   genuinely its own:
   - its response to a write at a related place (keep, kill, or narrow; the
     default for `MayOverlap` is kill, so a kind cannot be over-carried by
     omission);
   - its meet at a join;
   - substitution of any terms it embeds beyond its key place (the default
     rebases the key place only).
4. **One transport operation per place**, written once over the environment:

   | Place | Operation |
   | --- | --- |
   | Call entry | Build one formal-to-actual `PlaceMap`; rebase every fact through it. |
   | Call return | Invert the map, kill the callee's write frame, admit the callee's postconditions. |
   | Write | Apply the overlap relation to every fact. |
   | Field projection | Restrict to the field's subtree and rebase. |
   | Case match | Narrow through the case segment. |
   | Loop / back-edge, state arrival | One generic join (replaces `StateValues`' per-field meets). |
   | Nested / pausing call | Compose entry and return; kill what suspension may change. |
   | Build / generic selection | Same arrival join; selection contributes a `PlaceMap`, not new plumbing. |
   | Into lowering | Stage 06 reads environment rows instead of calling validation. |

The stage 05 `FactPlan` law is the seed: extend it, do not replace it.

## Migration

Each step is a shippable PR that deletes the code paths it supersedes in the
same commit ([compositional lowering](../../../omega-rust/pipeline.md#compositional-lowering)).

1. **Overlap.** One relation; migrate the ten implementations and inline copies.
2. **`PlaceMap`.** One formal-to-actual mapper; migrate the six rebasers.
3. **Numeric environment keys.** Move `ValueEnvironment` and the entailment
   `Engine` from `String` to `Place`; settle the `rebind` question above.
4. **Generic arrival join.** Make `StateValues` generic over the environment.
   This addresses the Build column (41 of 202 PRs).
5. **Call return.** Collapse the normal-return modules onto the one operation.
6. **Lowering.** Replace stage 06 `validation::` re-derivation with environment reads.
7. **Check generic bodies once.** Check the template against its contracts and
   instantiate its published facts by substitution at each application, as
   the spec requires (see below); keep per-specialization target and layout
   checks. This retires the Build column.

Add a ratchet beside `tools/cache_state.txt` counting overlap functions,
formal/actual rebasers and place/path types; counts may only fall. While the
migration runs, fix a new "preserve X through Y" defect by moving place Y onto
the shared operation, not by adding a carrier.

Success is measurable: the rate of preserve / carry PRs falls, and a new fact
kind lands with no place-specific code.

## Risks

- **Soundness.** Uniform transport can over-carry. Kill-on-`MayOverlap` by
  default and per-kind opt-in for keeps keep #1717 / #1734-class errors out.
- **Performance.** The checker is under active speed work (43 PRs since
  2026-11-05). Dense per-kind tables keyed by place keep lookups flat; measure
  each migration step against the existing timing gates.
- **Size.** Every step touches large modules (`V/contract_entailment.rs` is
  89k lines). Sequence by column, one place per PR.

## Open questions

1. Validation is the independent verifier at trust boundaries. Should it share
   the place, overlap and `PlaceMap` layer with stage 05 (transport, not
   judgment), or keep its own for independence?
2. Is the `ValueEnvironment::rebind` loss intended?

## Generic bodies: the spec already decides

The specification requires a generic body to be valid from its declared
contracts, independent of any one application:
[generics](../../spec/language/generics.md) ("An unspecialized body checks every
admitted alternative; one consumer cannot authorize an otherwise invalid generic
body"; "Generic bodies use the abstract requirement envelope. Callers substitute
selected published contracts into those dependencies"; generic axioms are
admitted on the template, not per instance) and
[dependent values](../../spec/language/dependent_values.md) (generic bodies
"cannot rely on a later specialization to justify an otherwise unchecked
observation"). Per-specialization obligations are limited to target, layout,
instruction-width and static-only `const` operations; monomorphization remains
the code-generation baseline.

So the Build column needs no new decision. Check each generic body once against
its contracts, publish its facts in terms of its binders, and substitute
arguments into those facts at each application; keep per-specialization target
and layout checks. Pass 04 currently clones bodies before checking and checks
each clone. That repeats work and may let a concrete instance prove what the
template cannot, which the spec forbids. Verify which happens before migrating.

## Method

PR titles from `git log --since=2026-11-01` filtered to `psi:` subjects matching
preserve / retain / carry / keep / returned / borrowed / through, classified by
keyword. Code locations come from tracing those PRs' diffs into the current
tree after the 2026-11-08 consolidation (#2152–#2161), with the cited
signatures spot-checked by hand. `git log --grep='(#N)' -1` returns the wrong
commit for several numbers (#1957, #1610, #1611, #1596, #2068); match the
squash subject suffix exactly.
