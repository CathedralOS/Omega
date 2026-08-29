# Optimizer Source Organization

This brief is a navigability contract. The architecture entrance is
[optimizer_architecture.md](../optimizer_architecture.md).

## Entrance rule

A human entering any optimizer stage must find a small `lib.rs` or `mod.rs`
that explains the stage and exposes its real coordination point. It must name
the next semantic rungs and own the stage catalog or compute-to-validation
join. It must not contain rule mechanics, codecs, broad fixtures, or hundreds
of accessors. A pure re-export wall is also insufficient.

Preferred entrance size is below 100 lines. Crossing 200 lines requires a
specific reason in review. Production files above 1,500 lines fail the
organization gate; lower thresholds are preferred for mixed-responsibility
files.

## Taxonomy

Use folders for semantic responsibilities, then exact families:

```text
analyses/<fact>/{mod,model,compute,identity,validate,tests}.rs
planning/<plan>/{mod,model,compute,identity,validate,codec,tests}.rs
rules/<target>/<exact-rule>/{mod,model,compute,identity,validate,codec,tests}.rs
stages/<stage>/{mod,catalog,model,compute,validate,tests}.rs
```

Not every leaf needs every file. Do not create empty layers or one crate per
rule. Shared code belongs at the nearest semantic ancestor only when two or
more leaves use the same concept and the concept has one contract.

## Catalog rule

There is one visibly named catalog per stage. It owns:

- canonical rule order;
- exact source-visible selection mapping;
- rule descriptor construction; and
- coverage tests proving every declared rule is either scheduled or rejected
  for a stated target/phase reason.

No second match table may silently become an alternate registry. Build
vocabulary, reports, and codecs derive from or exhaustively test against the
closed `Optimization::ALL` vocabulary.

For Psi, `rules/mod.rs` is the selection/application entrance,
`rules/catalog.rs` is the complete ordered pass table, and every
`rules/passes/<exact-pass>/catalog.rs` owns only that pass's exact rule order.
Enabling or disabling a Psi pass therefore changes one visible descriptor
table; changing a rule's within-pass order changes one local catalog.

## Squalr pattern carried forward

The clearest concrete reference is Squalr's
`registries/scan_rules/pointer_scan_rule_registry.rs`: one short registry
visibly lists the built-in planning rules. The `rule_map_search_kernel.rs` leaf
owns the SIMD-linear/scalar-linear/scalar-binary choice, while
`pointer_scans/pointer_scan_dispatcher.rs` has one obvious application loop.
The element-scan registry follows the same shape for its parameter and filter
rule families.

Omega keeps that navigational shape: catalog, named leaves, one application
loop. Omega strengthens it with deterministic catalog order, exact typed
selection names, immutable candidate plans, independent validation, and
identity-bound receipts. It deliberately does not copy the global unsafe
singleton, string-keyed scheduling, or unordered `HashMap` iteration.

## Test placement

Focused tests live beside the responsibility they verify. Integration tests
mirror production taxonomy under `tests/coordination`, `tests/stages`, and
`tests/fixtures`. Large fixture catalogs are split by typed artifact family.

Every rule has positive, negative, boundary, disabled-selection, budget, and
corruption tests. Every entrance has a catalog-coverage test.

## Current reference slices

- `omega-psi-optimizer`: analyses, pass manager, ordered rules, and pass
  families.
- `omega-optimization-validation`: candidate validators and complete-unit
  validators remain independent of producers.
- `omega-regalloc`: analyses, allocation decisions, and exact recovery rules.
- `omega-machine-optimizer`: analyses, post-allocation planning, and rules
  grouped by ISA and exact transformation.
- `omega-optimization-pipeline`: coordination separated from ordered custody
  stages and tests that mirror those stages.

## Refactor trigger

Refactor before adding a rule when any of these are true:

- the stage entrance needs rule-specific mechanics;
- enabling a rule requires edits to several unrelated route enums;
- a new rule copies an existing owning carrier across encoding/layout/
  realization;
- a file mixes model, compute, validation, codec, and broad tests; or
- the only way to locate built-ins is repository-wide search.

The x86 XOR-zero milestone exercised this trigger across the full physical
conveyor. The symbolic-machine stage now has one catalog and typed result;
encoding, layout, whole-function exit, and realization consume that result;
and fragment publication has one generic post-allocation source. Adding
XOR-zero did not copy the former MOVN route, and the named CBNZ/MOVN owning
complete routes were removed.

The organization gate is executable for the catalog-driven reference slices.
`omega-optimization-core` declares
each exact name, stable tag, build case, build counter, phase, and canonical
order once; its descriptor generates both `Optimization` and
`Optimization::ALL`. Both injected build preludes are parsed against those
generated views and every exact name is evaluated independently through both
preludes, so swapped name-to-counter mappings fail.

Psi, selected-lowering, allocation-recovery, post-allocation, and
function-relative-layout stages expose ordered catalogs with phase coverage
tests. Validator candidates, semantic analyses, optimization-unit identity and
rewrite machinery, projection tests, and physical custody tests descend through
small named entrances rather than monoliths. The repository architecture test
enforces the 1,500-line file ceiling and the entrance exception contract over
the governed optimizer roots. It additionally names the coordination marker
that must remain in each migrated executable-stage entrance and requires one
local rule catalog for every Psi pass; a small re-export wall no longer passes
that check. The optimized ordinary-callable-entry stage is a physical example:
its `mod.rs` owns build/replay, with records in `model.rs`, semantic
reconstruction in `reconstruction.rs`, and wire format in `codec.rs`. The
selected-lowering literal-fold stage follows the same rule: its entrance owns
phase projection plus catalog dispatch, then descends through `model`,
`execution`, and `accounting`. Register-allocation rule folders use the same
shape; pressure rematerialization keeps its production computation and broad
fixtures in separate leaves below its real compute/validate entrance. The
optimized object-artifact boundary likewise exposes one build/replay entrance
above separate model, reconstruction, and codec leaves. The preceding
relocation-free object-container boundary mirrors it and keeps codec tests out
of production leaves. Target register-environment custody exposes one
build/validation entrance above the exact target catalog, validated custody
model, validation mechanics, and tests; the target/ABI matrix is therefore
visible without burying the stage join in that catalog. Allocation-legality
staging puts each exact availability policy in one visible leaf and keeps
analysis, independent replay, custody projection, and the retained model
separate; its entrance owns policy selection plus the replay-gated stage join.
Register-home staging preserves baseline-legality and post-copy-reanalysis as
explicit source families while sharing construction, independent validation,
custody projection, and model leaves below one replay-gated entrance.
Post-fold home staging applies the same shape to one-step literal-fold chains
and complete selected-lowering runs, with manifest projection separated from
construction and replay.
Post-allocation machine analysis likewise separates source-route construction
from replay validation and the sealed model, while its entrance owns the
common effects-plus-machine custody join. Active-resident rematerialization
keeps producer computation and independent replay validation in separate
leaves; its entrance alone grants stage custody after that reconstruction.
Pre-allocation machine-effect staging keeps its exact ISA catalog, analysis,
source-route construction, independent replay, custody projection, and model
in named leaves; its entrance replay-gates every supported selected-source
lineage.
Liveness computation and pre-allocation machine-effect encoding also keep
their broad fixtures in sibling test leaves, so production file size measures
production responsibility.

When fixtures legitimately need a production module's private helpers, keep
the logical child module and use an explicit sibling `#[path]`; do not retain
hundreds of test lines at the bottom of `compute.rs` or `codec.rs` merely for
privacy.

Migration is not complete merely because every file is under the hard ceiling.
Remaining flat executable-stage leaves are tracked in `TASKS_OPTIMIZER.md` and
must move behind semantic folder entrances before those areas gain new rules.
