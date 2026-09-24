# Lookup-map justification audit

Mechanical sweep at `c2ccb2a202` of every name-keyed map beside the scoped
symbol tree, under the pipeline ownership rule: "Scoped symbol-tree lookup is
the baseline; extra lookup maps require a measured reason"
([pipeline.md](../../../omega-rust/pipeline.md)). A name-keyed lookup map here
means a `HashMap`/`BTreeMap` keyed by a `str`/`String`/identifier that resolves
program declarations — the role the scoped `SymbolTable` already owns. Maps
keyed by handles, source coordinates, canonical identities, or runtime binder
names are data-plane storage, not declaration lookup, and are enumerated only
where they could be mistaken for the covered kind. Delete this audit once
`tests/architecture/scoped_lookup_maps.rs` and its key-domain catalog carry
every justification recorded here.

## Method

- `rg '(HashMap|BTreeMap)<[^>]*(str|String|SymbolName|InternedName|Identifier)[^>]*>'`
  over `omega-rust/`, `source/`, and `tools/`; tests excluded.
- `rg '(HashMap|BTreeMap)<[^>]*,[^>]*(Handle|Id|Index|Symbol)'` to catch
  handle-valued maps; each hit classified by key meaning.
- `psi/foundation/symbols` inspected directly: `SymbolTable` stores
  `HierarchyArena<Symbol>` + `Arena<SymbolName>` +
  `Vec<SourceScopedTopLevelBinding>` and resolves via `child_handles` /
  `find_top_level_by_name_and_kinds_from_source` scans — it contains no
  name index itself, so every external name-keyed map is a candidate extra
  lookup map.

## Findings

**Zero name-keyed declaration-lookup maps exist beside the scoped symbol
tree.** Declaration resolution already routes through `SymbolTable`
(`symbols/lookup.rs`, `symbols/scoped_paths.rs`, `symbols/symbol_table/`).
Every retained name-keyed map carries one of the reasons below; none is a
faster path around scoped lookup, so the policy currently holds with nothing
to remove.

### Justified classes observed

- **Generic-application substitution environments**
  (`02_syntax-trees-to-symbol-resolved-trees/src/preparation/generic_data/{substitution,synthesis,eligibility,trait_defaults,const_evaluation/*}.rs`,
  `preparation/type_equations.rs`):
  `HashMap<String, TypeReferenceHandle>` /
  `HashMap<String, ClosedArgumentIdentity>`. Key space is one declaration's
  own `const`/`type` parameter names; lifetime is one bound application. The
  symbol tree cannot express "parameter name → chosen argument" because a
  substitution is not a declaration, so these maps are the scoped environment
  for that application, not a lookup bypass.

- **Evaluation environments**
  (`checked-interpreter/src/interpreter/evaluator{,/execution}.rs`,
  `validation/src/proof_contracts/**/value_environment.rs`,
  `contract_entailment/{arithmetic,inductive}_judgment.rs`,
  `build-time-evaluation/src/layouts/**`):
  `BTreeMap<String, Cell>`/`BTreeMap<String, Interval>` binder-value frames.
  These are runtime data keyed by binder spelling inside one activated
  frame/domain — declaration resolution has already happened before they
  exist.

- **Canonical-identity rejoins and package registries**
  (`compiler/terminal-artifact/src/terminal_artifact/behavior_exclusions.rs`,
  `package-compilation/src/package_compilation.rs`,
  `component-description/src/component_verification.rs`):
  `BTreeMap<String, SymbolHandle>` /
  `BTreeMap<String, PackageKeyIdentity>`. Keys are normalized overload
  identities or package names emitted by external catalogs/specs — not
  authored scoped spellings — so the scoped tree cannot serve them. Each map
  is built once per module/package for a bounded rejoin, then dropped.

- **Module/diagnostic metadata maps**
  (`selection/domain_operator_homes.rs`: `HashMap<SourceId, String>` /
  `HashMap<SourceId, Vec<String>>`): source→module-name records for
  diagnostics; keyed by source coordinate, not a declaration name.

### Not name-keyed (handle/identity-keyed, compliant by construction)

`04_typed-trees-to-checked-trees/src/product_pruning/dependencies.rs`
(`HashMap<SymbolHandle, Vec<SymbolHandle>>` edges),
`02_abstract-operations-to-target-operations/.../dynamic.rs`
(`BTreeMap<MachineId|PlaceId|StructuralTypeId, _>`),
`07_selected-instructions-to-selected-instructions/.../liveness/validate/replay.rs`,
`abstract-operations/src/abstract_operations/atomic.rs`,
`behavior_exclusions.rs` virtual-filesystem tables
(`BTreeMap<u32|handle, _>`), and similar — keyed by arena/machine handles, so
they are durable-child indexing, the sanctioned default.

## Residual risk and trigger

The census at `c2ccb2a202` is now repeatable:
`tests/architecture/scoped_lookup_maps.rs` re-enumerates every production
`HashMap`/`BTreeMap` keyed by an authored-spelling token and fails when a
file carries one without a cataloged key-domain justification — or when a
catalog entry outlives its map. A new name-keyed declaration map violates
the policy only if it resolves authored spellings the scoped tree already
owns; when a genuinely new key domain appears (e.g. another
canonical-identity space), the measured-reason clause is satisfied by
recording the key domain and why scoped lookup cannot serve it — as the
classes above do.

The gate sees declarations in source text; a map whose key hides behind a
type alias, or a `Map` constructed only through a generic parameter, can
still slip through — as can a second unjustified map added to a file that
already cataloged one. Both are review-visible in the same file as the
recorded row, so the tripwire's cost stays proportional to the policy's
value.
