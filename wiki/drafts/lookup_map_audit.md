# Lookup-map justification audit (linux_x86_64, ff782bdf21)

Policy under audit — `omega-rust/pipeline.md` (pipeline ownership):

> Scoped symbol-tree lookup is the baseline; extra lookup maps require a
> measured reason.

The baseline is `SymbolTable`'s `SymbolLookup` surface
(`psi/foundation/symbols`, consumed through
`syntax-trees-to-symbol-resolved-trees/src/symbols/lookup.rs` and
`domain_name_reaches`/`prefer_module_local_domain`): resolution answers
name/spelling questions by walking the scoped tree, not a side index.

## Scope

Counted as "extra lookup maps": maps keyed on `SymbolHandle` (or an
`(SymbolHandle, …)` tuple) that a pipeline stage builds beside the scoped
tree to accelerate symbol→position/identity resolution. Domain payload
tables (`BTreeMap<String, …>` unit-type plans, const-value bindings,
proof-recursion field tables) map user-authored spellings/values, not
resolution candidates — excluded. Test-only helpers excluded.

## Inventory

| Site | Map | Role | Measured reason |
| --- | --- | --- | --- |
| `typed-trees-to-checked-trees/src/flow/context.rs` | `machine_index`, `state_index_in_machine`, `state_location`, plus two query caches | "Program shape lookups indexed once per context … rebuilds lazily" — positional index over the typed tree | structural rationale only; no measurement cited |
| `typed-trees-to-checked-trees/src/proof/mathematical_signature.rs` | `carriers`, `authored_positions`, `authored_index` | lazily interned carrier positions for signature-order declarations | structural rationale only; no measurement cited |
| `typed-trees-to-checked-trees/src/product_pruning/dependencies.rs` | `state_machine`, `edges` (SymbolHandle→Vec) | retain/prune dependency graph over checked symbols | structural rationale only; no measurement cited |
| `syntax-trees-to-symbol-resolved-trees/src/lowering/machine/token_bindings.rs` | `ordinals` | binder symbol → first-occurrence position during the single rendering pass | structural rationale only; no measurement cited |
| `syntax-trees-to-symbol-resolved-trees/src/preparation/trait_defaults.rs` | `traits` (`HashMap<SymbolHandle, TraitDefaultsInput>`), `substitution`/`EffectiveDefaults` (String-keyed) | trait-default pooling by owner symbol | structural rationale only; no measurement cited |
| `semantics/validation/src/machine_calls/calls/write_frames/demand.rs` | 9+ `Mutex<HashMap<…>>` demand caches (`stable_expression_bindings`, `assignment_frames`, `inferred_state_frames`, …) | "Retain them across resolver queries" — memoized write-frame answers | documented as caching, not as measured acceleration |

## Finding

Every extra symbol-keyed lookup map carries a structural comment
(index-once, lazy intern, retain-across-queries) but **no site records a
measured reason** — no benchmark row, timing comparison, or
`predicted_cost_delta`-style evidence accompanies any of them. The policy
is therefore enforced nowhere today.

## Disposition

This is an audit, not a removal: the maps are plausible accelerators and
unmeasured removal would be the same policy violation in reverse. The
bounded follow-up is the twin stub LOOKUP-MAP-MEASUREMENT-AUDIT — pick the
two hottest (the `flow/context` shape index and the `write_frames` demand
caches, both on the checking hot path), measure against the scoped-tree
baseline via `tools/benchmark`, and record either the measured reason or a
removal diff per site.

## Commands run

```sh
grep -rn "HashMap<SymbolHandle\|BTreeMap<SymbolHandle" omega-rust/ -r \
  --include="*.rs" | grep -v test   # 6 non-test files, ~33 sites
# per-site justification scan: no 'measured'/'benchmark'/'profile' cite
# at any site
```
