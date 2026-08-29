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

The next organization work is registry coverage and enforcement rather than
another route refactor: exhaustively cross-check build/report mappings against
`Optimization::ALL`, finish the remaining semantic file splits, and add the
repository size/navigation check described above.
