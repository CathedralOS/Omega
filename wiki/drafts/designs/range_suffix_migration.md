# Scalar range contract migration

Companion recipe for **CANONICALIZE-SCALAR-RANGE-CONTRACTS**. Commit
`669925b8b9` originally declared the suffix revoked without owner authority; the
owner has since selected the no-suffix design explicitly. The decision is now
valid, but prior migrations remain unaudited because many generated nominal
domains where a contextual proposition was intended.

The removed spelling is `T [min..=max]` / `T [min..max]` on scalar type
positions. One proposition surface replaces it: data/common fields and guarded case-payload relationships use data
`requires`, machine parameters use `requires`, results use `ensures`, and locals retain facts established by their
initializer, guards, and calls. `T in Domain` is not generic range sugar; use it
only when that exact named domain identity is independently part of the API.

Delete this recipe once every historical migration is audited, all source uses
the canonical clauses, and the parser/representation path is removed.

## Decision table by position

| Old spelling | New spelling | When |
|---|---|---|
| `-> T [lo..=hi]` | `-> T` + `ensures result >= lo && result <= hi` | Result bounds are published postconditions. Use `-> T in D` only when the exact named domain is independently required. |
| returned conversion `value as T [lo..=hi]` | bare Exact cast plus the owning result's `ensures` | Preserve the demanded output predicate separately from the cast's independent representability and denotation obligations. |
| machine parameter `x: T [lo..=hi]` | `x: T` + `requires x >= lo && x <= hi` | Callers prove callable preconditions; ranges are not parameter type identity. |
| common field `f: T [lo..=hi]` | `f: T` plus data `requires f >= lo && f <= hi` | The relation is part of the containing value's default domain. |
| case payload `case X(v: T [lo..=hi])` | `case X(v: T)` plus a guarded data `requires` predicate | The relation exists only for the active case. |
| local `let v: T [lo..=hi] = source` | `let v: T = source` | The initializer must already establish the facts. Preserve a needed bound through the producer's contract or a checked guard; the annotation cannot assert it. |
| `T [lo..hi]` (exclusive) | `value >= lo && value < hi` | Use the owning `requires`/`ensures` clause. |
| parameter `x: T [0..=self.n]` | `x: T` + `requires x <= self.n` | Runtime-dependent endpoints are ordinary call propositions. |
| generic range shell used to infer `N` | explicit binder/equation, or intentional indexed domain | Anonymous interval propositions are not structural type identity. Supply `N` explicitly when no exact declaration structure determines it. |

## Positions that reject instead of migrating

Layout-determining uses — array extents, `const` positions, specialization keys —
require ordinary static values. A runtime range proposition does not make its
endpoint static and does not authorize maximum-sized storage.

## Procedure per file

1. Grep the file for `T [` … `..=` / `..` brackets inside type positions
   (`param:`, `->`, `field:`, `local:` declarations). Do not touch `for` /
   loop ranges or slice indices — those are expressions, not the revoked
   suffix.
2. Classify the owning position: common field, case payload, parameter, result,
   local, generic shell, or static/layout use. Apply the decision table exactly.
3. Remove migration-generated interval domains unless their nominal identity or
   establishment route has an independent customer. Equal predicates do not
   justify keeping a named domain.
4. Re-check the package: `mbx run -p omega -- --check <root.omg>`. For corpus
   fixtures, run `python3 tools/corpus_gate.py --filter <group>/<name>` and
   the matching-host execution leg for authored runtime oracles.
5. The compiler-side deletion (parse path, `TypeConstraintNode::Range`,
   range-shell generic matching, range-recommending diagnostics) is the last
   step — do not pre-delete it while source still spells the suffix.

## Scale and ordering

The original inventory at `7a9a7b8287` measured 1,598 occurrences across 544 `.omg` files —
456 under `tests/omega`, 48 `samples/cli`, 14 `source/library`, 11
`tests/native-differential`, 7 `omega-rust/psi`, 4 `samples/gui` — plus 41
Rust files naming a range type-constraint. The corpus migration dominates;
batch the audit by customer so each batch lands with its own scoped witness.
The Epsilon-written Omega parser and its fixtures migrate on the same recipe;
Epsilon's own surface is unchanged.

Do not use later reduced occurrence counts as completion evidence: they include
unaudited rewrites. Completion requires the history-derived inventory to record
each original declaration and its canonical replacement.

The four-use census at `abda3e427bc` is historical, not the current frontier.
The provider-dependent contract endpoints now use ordinary `requires`; their
constant-recipe work roots and publication route are covered by
[`provider_contract_endpoint`](../../../samples/cli/facts/provider_contract_endpoint/README.md).
Do not rebuild that repair or replace those clauses with nominal domains.
The mathematical constrained-formal negative still keeps its suffix until
parser deletion can turn it into a parse-refusal pin.

At `cf0881cf9aa0e7a1c98e6d7eabe41f1010223ca6`, a primitive-scalar census found
566 suffix occurrences in 159 files. This is a discovery count, not a full
historical audit or evidence that named/generic suffixes are absent. The
runtime-index cohort below accounts for 536 occurrences: 268 in authored
samples and 268 in their exact corpus pins. Other source migrations, the
historical audit and the shared range parser/representation deletion remain
open under the parent task.

Historical audit sections below record the clauses used by their original
landings, including then-current `where` syntax. They do not authorize that
retired spelling for new source. The canonical replacement is data `requires`;
`REQUIRES-UNIFICATION` owns the remaining source and grammar retirement.

## Audited runtime-index application productions

Migration base: `cf0881cf9aa0e7a1c98e6d7eabe41f1010223ca6`. Paths below are
under [`samples/cli/facts/runtime_index_application`](../../../samples/cli/facts/runtime_index_application/README.md).
Each listed declaration occurs in both `<place>/main.omg` and its
`<place>/invalid/main.omg` twin. Corpus pins are byte-identical under
`tests/omega/{pass,fail}/facts/runtime_index_application_<production>_through_<place>_{exit,rejected}`.
The twin changes only constructed payload 8 to 10; it retains every bound,
producer operation and mutable flag observation.

Machine inputs keep their scalar carriers and move their complete intervals to
`requires`. Result bounds move to `ensures result`; receiving locals rely on
the initializer's checked facts. `Bounds.total` moves to data `requires`.
Named `Minimum`/`Maximum` endpoints remain named in `const_name`. Existing
postconditions such as `result == total + 1` remain alongside the migrated
interval. The intentional `AtMost<Max>` indexed domain in `qualified_computed`
is retained: its exact domain identity and explicit weakening are independent
of the anonymous interval removed here.

The inventory names every old declaration below. `probe` in `target_body`
denotes each of its four original hosted bodies; the replacement is one
boundary requirement plus four explicit named implementations selected by
Build. These pure implementations introduce no runtime service or authority.
Counts include positives and reject twins, and exclude matching corpus copies.

### `computed_result`

| Place | Historical declarations and suffixes | Positions, including twin |
| --- | --- | --- |
| `local` | `probe.input: input:u32[8..=9]`; `construct.total: total:u32[8..=9]`; `produce.total: total:u32[8..=9]`; `total: let total:u32[8..=9]` | 8 |
| `field` | `probe.input: input:u32[8..=9]`; `construct.total: total:u32[8..=9]`; `produce.total: total:u32[8..=9]`; `Bounds.total: data Bounds [copy] {total:u32[8..=9];}` | 8 |
| `parameter` | `probe.total: total:u32[8..=9]`; `construct.total: total:u32[8..=9]`; `produce.total: total:u32[8..=9]` | 6 |
| `call_result` | `probe.input: input:u32[8..=9]`; `retain.input: input:u32[8..=9]`; `retain.result: ->u32[8..=9]`; `construct.total: total:u32[8..=9]`; `produce.total: total:u32[8..=9]` | 10 |
| `match_join` | `probe.first: first:u32[8..=9]`; `probe.second: second:u32[8..=9]`; `construct.total: total:u32[8..=9]`; `produce.total: total:u32[8..=9]`; `total: let total:u32[8..=9]` | 10 |
| `const_name` | `probe.total: total:u32[Minimum..=Maximum]`; `construct.total: total:u32[8..=9]`; `produce.total: total:u32[8..=9]` | 6 |
| `other_state` | `next.total: total:u32[8..=9]`; `probe.input: input:u32[8..=9]`; `construct.total: total:u32[8..=9]`; `produce.total: total:u32[8..=9]` | 8 |
| `other_machine` | `probe.input: input:u32[8..=9]`; `forward.total: total:u32[8..=9]`; `construct.total: total:u32[8..=9]`; `produce.total: total:u32[8..=9]` | 8 |
| `target_body` | `probe.total: total:u32[8..=9]`; `construct.total: total:u32[8..=9]`; `produce.total: total:u32[8..=9]` | 12 |

### `mutating_operator`

| Place | Historical declarations and suffixes | Positions, including twin |
| --- | --- | --- |
| `local` | `probe.input: input:u32[8..=9]`; `construct.total: total:u32[8..=9]`; `total: let total:u32[8..=9]` | 6 |
| `field` | `probe.input: input:u32[8..=9]`; `construct.total: total:u32[8..=9]`; `Bounds.total: data Bounds [copy] {total:u32[8..=9];}` | 6 |
| `parameter` | `probe.total: total:u32[8..=9]`; `construct.total: total:u32[8..=9]` | 4 |
| `call_result` | `probe.input: input:u32[8..=9]`; `retain.input: input:u32[8..=9]`; `retain.result: ->u32[8..=9]`; `construct.total: total:u32[8..=9]` | 8 |
| `match_join` | `probe.first: first:u32[8..=9]`; `probe.second: second:u32[8..=9]`; `construct.total: total:u32[8..=9]`; `total: let total:u32[8..=9]` | 8 |
| `const_name` | `probe.total: total:u32[Minimum..=Maximum]`; `construct.total: total:u32[8..=9]` | 4 |
| `other_state` | `next.total: total:u32[8..=9]`; `probe.input: input:u32[8..=9]`; `construct.total: total:u32[8..=9]` | 6 |
| `other_machine` | `probe.input: input:u32[8..=9]`; `forward.total: total:u32[8..=9]`; `construct.total: total:u32[8..=9]` | 6 |
| `target_body` | `probe.total: total:u32[8..=9]`; `construct.total: total:u32[8..=9]` | 10 |

### `ordinary_call`

| Place | Historical declarations and suffixes | Positions, including twin |
| --- | --- | --- |
| `local` | `probe.input: input:u32[8..=9]`; `construct.total: total:u32[8..=9]`; `count.total: total:u32[8..=9]`; `total: let total:u32[8..=9]` | 8 |
| `field` | `probe.input: input:u32[8..=9]`; `construct.total: total:u32[8..=9]`; `count.total: total:u32[8..=9]`; `Bounds.total: data Bounds [copy] {total:u32[8..=9];}` | 8 |
| `parameter` | `probe.total: total:u32[8..=9]`; `construct.total: total:u32[8..=9]`; `count.total: total:u32[8..=9]` | 6 |
| `call_result` | `probe.input: input:u32[8..=9]`; `retain.input: input:u32[8..=9]`; `retain.result: ->u32[8..=9]`; `construct.total: total:u32[8..=9]`; `count.total: total:u32[8..=9]` | 10 |
| `match_join` | `probe.first: first:u32[8..=9]`; `probe.second: second:u32[8..=9]`; `construct.total: total:u32[8..=9]`; `count.total: total:u32[8..=9]`; `total: let total:u32[8..=9]` | 10 |
| `const_name` | `probe.total: total:u32[Minimum..=Maximum]`; `construct.total: total:u32[8..=9]`; `count.total: total:u32[8..=9]` | 6 |
| `other_state` | `next.total: total:u32[8..=9]`; `probe.input: input:u32[8..=9]`; `construct.total: total:u32[8..=9]`; `count.total: total:u32[8..=9]` | 8 |
| `other_machine` | `probe.input: input:u32[8..=9]`; `forward.total: total:u32[8..=9]`; `construct.total: total:u32[8..=9]`; `count.total: total:u32[8..=9]` | 8 |
| `target_body` | `probe.total: total:u32[8..=9]`; `construct.total: total:u32[8..=9]`; `count.total: total:u32[8..=9]` | 12 |

### `qualified_computed`

| Place | Historical declarations and suffixes | Positions, including twin |
| --- | --- | --- |
| `local` | `probe.input: input:u64[8..=9]`; `construct.total: total:u64[8..=9]`; `total: let total:u64[8..=9]` | 6 |
| `field` | `probe.input: input:u64[8..=9]`; `construct.total: total:u64[8..=9]`; `Bounds.total: data Bounds [copy] {total:u64[8..=9];}` | 6 |
| `parameter` | `probe.total: total:u64[8..=9]`; `construct.total: total:u64[8..=9]` | 4 |
| `call_result` | `probe.input: input:u64[8..=9]`; `retain.input: input:u64[8..=9]`; `retain.result: ->u64[8..=9]`; `construct.total: total:u64[8..=9]` | 8 |
| `match_join` | `probe.first: first:u64[8..=9]`; `probe.second: second:u64[8..=9]`; `construct.total: total:u64[8..=9]`; `total: let total:u64[8..=9]` | 8 |
| `const_name` | `probe.total: total:u64[Minimum..=Maximum]`; `construct.total: total:u64[8..=9]` | 4 |
| `other_state` | `next.total: total:u64[8..=9]`; `probe.input: input:u64[8..=9]`; `construct.total: total:u64[8..=9]` | 6 |
| `other_machine` | `probe.input: input:u64[8..=9]`; `forward.total: total:u64[8..=9]`; `construct.total: total:u64[8..=9]` | 6 |
| `target_body` | `probe.total: total:u64[8..=9]`; `construct.total: total:u64[8..=9]` | 10 |

## Audited whole-slice customer

The five scalar positions in `samples/cli/probes/slice_view_whole` at
`25b435334dd` retain their i32 carriers and original calculation 31. The
[position inventory](../../../samples/cli/probes/slice_view_whole/README.md#historical-scalar-positions)
maps each suffix to its collection predicate, callable obligation or local
facts. Selected-cell citations follow from the predicate's body; mutable
handback proves the predicate for the functional store result. Neither route
assumes a fixed slice extent or an arbitrary predicate's preservation.

## Audited bounded reference carriers

The following six customers retain 19 historical scalar positions. Every
`Cell.value: i32 [0..=100]` becomes plain i32 with data
`where value >= 0, value <= 100`; every `Main.index: u64 [0..=1]` becomes
plain u64 with data `where index <= 1`. The unsigned carrier supplies the
zero floor. Observed locals retain their original initializer and consume its
bounds through `require_cell_value`, rather than asserting them with a local
annotation. State parameters retain both bounds as ordinary `requires`.

Paths below are under `tests/omega/pass/borrows/`.

| Customer and original snapshot | Historical positions | Count |
| --- | --- | --- |
| `runtime_nested_reference_array_bounded_read_exit`, `fcce1da2785` | `Cell.value`, `Main.index`, `observed` | 3 |
| `runtime_nested_carrier_bounded_reference_read_exit`, `40f16ba2097` | `Cell.value`, `Main.index`, `observed` | 3 |
| `runtime_stored_reference_array_constructor_transfer_exit`, `3723809d259` | `Cell.value`, `observed`, `other` | 3 |
| `runtime_stored_reference_leaf_constructor_transfer_exit`, `6b52c3a60e2` | `Cell.value`, `observed` | 2 |
| `runtime_shared_reference_array_cohort_restoration_exit`, `60ac0370941` | `Cell.value`, `Main.index`, `first`, `observed` | 4 |
| `runtime_stored_reference_case_residual_retirement_exit`, `0a1b53bab78` | `Cell.value`, `observed`, `selected.observed`, `selected.second` | 4 |

The original reference-array construction, nested record reads, constructor
transfers, descriptor retirement, case payloads and writes remain unchanged.
Each authored exit is 70 in native and Terminal execution. The paired
`fail/borrows/reference_record_{field,index}_predicate_invalid_store` controls
reject writes outside the field predicates;
`fail/borrows/reference_array_field_requires_declared_predicate` prevents a
same-named unbounded field from supplying the borrowed-read guarantee.

## Audited scalar field loans and selector ordering

These customers retain eight historical scalar positions without changing
their carriers, reference operations or authored results.

| Customer and original snapshot | Historical positions | Canonical clauses | Count |
| --- | --- | --- | --- |
| `pass/borrows/runtime_bounded_and_shared_field_loans_exit`, `cb389d2a167` | `Main.position`, `reset.position`, `replace.position` | data `where position <= 1`; both formals use `requires position <= 1` and `ensures position <= 1` | 3 |
| `fail/borrow/bounded_reference_store_preserves_range`, `cb389d2a167` | the same three declarations | the same clauses; the original reset-to-2 operation still rejects | 3 |
| `pass/borrows/runtime_reference_initializer_selector_before_later_write_exit`, `61726859ba2` | `Cursor.row`, `Cursor::advance.result` | data `where row < 2`; `ensures result < 4` | 2 |

The first program still observes position 1 and writes -17 to array slot 1
before reset changes position to 0; native and Terminal exit 0. The matrix
program still selects row 1 before the column-selector call changes the row
to 0; native and Terminal exit 70. The negative reset preserves its assignment
of 2 and now refuses the normal-return guarantee rather than a retired
store-enforced parameter type.

An arrival requirement alone cannot preserve a mutable field at handback.
The checked selected callee's return guarantee must imply the field's
standing bound. Local loans retain the same original field identity, and
normal-return guarantees cannot reuse facts invalidated by a nested write.
Disjoint sibling predicates do not prohibit a write-only field projection;
facts that mention the selected field, including predicate-call arguments,
remain fences. The runtime local-loan control temporarily writes 2 then
restores 0, distinguishing handback from per-store parameter restrictions.

## Audited call selectors and mutable arrival observations

Five call customers retain six historical field bounds as data predicates.
Paths below are under `tests/omega/pass/calls/`. The unsigned carriers retain
their natural zero floor; exclusive upper bounds remain exclusive.

| Customer and original snapshot | Historical fields | Canonical data predicates | Count |
| --- | --- | --- | --- |
| `runtime_mutable_call_before_transition_args_exit`, `3e1cd000697` | `Loader.cursor: u64 [0..=16383]` | `Loader where cursor <= 16383` | 1 |
| `runtime_mutable_call_empty_input_exit`, `20d09cf7cc0` | the same cursor declaration | the same predicate | 1 |
| `runtime_mutable_call_short_input_exit`, `20d09cf7cc0` | the same cursor declaration | the same predicate | 1 |
| `nested_indexed_scalar_arguments_exit`, `980c103a473` | `Main.position: u64[0..4]` | `Main where position < 4` | 1 |
| `ordered_field_selector_calls_exit`, `07969428a79` | `Cursor.row: u64[0..2]`, `Cursor.position: u64[0..4]` | `Cursor where row < 2, position < 4` | 2 |

The loaders keep their original arrays, input bytes, policy conversions and
mutable helper followed by state-argument observations. Complete input still
exits 70; empty and short input exit 72. Nested free/attached scalar calls
and the two selector evaluation orders still exit 70. Native and Terminal
execution agree on macOS ARM64. `fail/calls/selector_field_predicate_upper_bound`
rejects nonliteral row and column writes whose arrival premises include the
one-past value; an inclusive premise cannot establish an exclusive field bound.

After ordinary package preparation, the public command is
`omega run --both tests/omega/pass/calls/<customer>/main.omg`. Supply the
customer's `input.txt` for each loader; use empty stdin for the other programs.
The corpus equivalent is `python3 tools/corpus_gate.py --both --filter
calls/runtime_mutable_call_,calls/nested_indexed_scalar_arguments_exit,calls/ordered_field_selector_calls_exit`.

## Audited recast selectors and aggregate leaves

Seven runtime customers retain ten historical field positions. Paths below
are under `tests/omega/pass/recast/`; snapshots retain the original suffixes.

| Customer and original snapshot | Original fields | Canonical data predicates | Count |
| --- | --- | --- | --- |
| `runtime_inline_shared_slice_record_read_exit`, `feec5b2ac33` | `Main.k: u32 [0..=4]` | `Main where k <= 4` | 1 |
| `runtime_inline_slice_view_mutable_write_exit`, `feec5b2ac33` | `Main.k: u32 [0..=4]` | `Main where k <= 4` | 1 |
| `runtime_interior_slice_view_mutable_write_exit`, `7bba92b19f0` | `Main.k: u32 [0..=4]` | `Main where k <= 4` | 1 |
| `runtime_inline_slice_recast_call_exit`, `feec5b2ac33` | `Main.offset: u32 [0..=2]` | `Main where offset <= 2` | 1 |
| `runtime_offset_byte_recast_mutable_write_exit`, `83045e063d2` | `Main.k: u32 [0..=4]` | `Main where k <= 4` | 1 |
| `runtime_record_array_view_mutable_write_exit`, `83045e063d2` | `Main.k: u32 [0..=4]`, `Main.i: u32 [0..=1]` | `Main where k <= 4 && i <= 1` | 2 |
| `runtime_aggregate_slice_representation_recast_exit`, `83045e063d2` | `Narrow.signed_value: i16 [0..=100]`, `Wide.unsigned_value: u16 [0..=200]`, `Exact.unsigned_value: u16 [0..=100]` | `Narrow where signed_value >= 0 && signed_value <= 100`; `Wide where unsigned_value <= 200`; `Exact where unsigned_value <= 100` | 3 |

All carriers remain unchanged and unsigned fields retain their carrier's zero
floor. The original reference formations, state transfers, selector mutation,
little-endian bytes, nested indexed stores, shared weakening and mutable
representation equivalence remain in place. Each program's authored exit is
70 in both native and Terminal execution. Reproduce after ordinary package
review with `omega run --both <customer>/main.omg`.

The aggregate customer retains its padded element stride and sign-changing
leaf views. `fail/recast/recast_slice_where_strengthening` rejects a shared
slice whose target demands a tighter leaf predicate; a fitting current zero
does not establish representation compatibility. The paired
`recast_slice_where_invalid_indexed_store` rejects an out-of-range write at
a runtime selector despite valid geometry and equivalent mutable leaf sets.
Existing invalid offsets, non-tiling suffixes, relational predicates and
unequal mutable bounds retain their independent refusals.

## Audited core layout fields

The 13 common-field suffixes in `source/library/core/layout.omg` at
`c1303f89d4` retain their predicates on their containing data. All carriers
remain `u64`; its nonnegative carrier supplies each old zero lower bound.
No interval-only domain or replacement nominal identity is introduced.

| Declaration and fields | Original suffix | Canonical predicate |
|---|---|---|
| `SchemaField.size` | `[0..=4096]` | `size <= 4096` |
| `SchemaField.align` | `[1..=16]` | `align >= 1, align <= 16` |
| `SchemaCase.payload_field_count`, `retired_payload_identity_count` | `[0..=32]` | each count `<= 32` |
| `Schema.field_count`, `retired_field_identity_count`, `case_count`, `retired_case_identity_count` | `[0..=32]` | each count `<= 32` |
| `AccessPlan.field_count`, `BoundaryReach.service_count`, `ResourceProfile.region_count` | `[0..=32]` | each count `<= 32` |
| `TransferRules.rule_count`, `AtomicTransferRules.rule_count` | `[0..=16]` | each count `<= 16` |

Compiler reflection must establish these values before policy execution,
including inactive field slots whose alignment is one, not zero. The
`layouts/schema_field_at_declared_size` and
`layouts/schema_field_exceeds_declared_size` controls bound the reflected
extent; `schema_identity_payload_predicate` checks a different active payload
predicate against a smaller authored schema roster. Existing native plan-laid
array writes and placed-view policies constrain field-key and padding transport.

The nine parameter suffixes in the same revision belong to the finite
`AccessPlan::with_bounded` search, not to the identity of an access plan.
Their `u64` carriers remain unchanged and each bound is now an arrival
requirement. Zero lower bounds are inherent in the carrier.

| Machine/state and parameters | Original suffix | Canonical requirement |
|---|---|---|
| `AccessPlan::with_bounded.fuel` | `[1..=128]` | `fuel >= 1 && fuel <= 128` |
| `seek.index`, `seek.fuel` | `[0..=32]`, `[1..=128]` | `index <= 32 && fuel >= 1 && fuel <= 128` |
| `inspect.index`, `inspect.fuel` | `[0..=31]`, `[1..=128]` | `index <= 31 && fuel >= 1 && fuel <= 128` |
| `replace.index` | `[0..=31]` | `index <= 31` |
| `advance.index`, `advance.fuel` | `[0..=31]`, `[1..=128]` | `index <= 31 && fuel >= 1 && fuel <= 128` |
| `grow_invalid.field_count` | `[0..=31]` | `field_count <= 31` |

The initial call supplies fuel 128 and index zero. Search guards establish
the next state's bounds before subtracting fuel or advancing an index;
`grow_invalid` receives a count only on the `< 32` arm. The invalid-key route
still retains an extra or overwriting decision for policy rejection, rather
than silently ignoring it. `layouts/access_plan_inaccessible_seed` and
`layouts/placed_view_source_access_compile` exercise the public construction
route and repeated replacement; they do not claim a hosted runtime entry.

## Audited filesystem parameter history

The original 16 positions at `7a9a7b8287` are four `name_len` parameters in
each `std/targets/{linux_arm64,linux_x86_64,macos_arm64,macos_x86_64}/filesystem_impl.omg`.
Commit `b3bc01f500a7` replaces every `u64[0..=255]` with bare `u64` and
`requires name_len <= 255`; the unsigned carrier retains the zero lower bound.
Provider consolidation (`6d0b8da07bdd`, `323fbc4788cd`) moves their operations
to the shared [POSIX adapter](../../../source/library/std/posix_filesystem_adapter.omg).

| Original declaration in each of four targets | Current shared declaration | Preserved requirement |
| --- | --- | --- |
| `rds_rm_child_go.name_len` | `posix_filesystem_rda_step::rds_rm_child_go.name_len` | `name_len <= 255` |
| `rds_kind.name_len` | `posix_filesystem_rda_step::rds_kind.name_len` | `name_len <= 255` |
| `rds_unlink.name_len` | `posix_filesystem_rda_step::rds_unlink.name_len` | `name_len <= 255` |
| `rds_descend.name_len` | `posix_filesystem_rda_step::rds_descend.name_len` | `name_len <= 255` |

Commit `e95da6c81fa8` canonicalizes a later, different 16-position filesystem
cohort: ten shared-adapter parameters (five name-length and five release-stack
parameters), plus six directory-offset results across Linux ARM64, Linux x86-64
and macOS ARM64. Those results retain `18 <= result <= 20` or
`19 <= result <= 21` as `ensures`. Its count is not the original 16-parameter
inventory; its additional callback parameter is outside both filesystem counts.

This reconciles source positions, not complete filesystem execution acceptance.
Matching-host native/Terminal service and occurrence-bound release obligations
remain under **FILESYSTEM-PLATFORM-SERVICES**. Historical compile/native records
do not establish all of them or authorize parser retirement; the parent still
requires the remaining customer audit and applicable runtime evidence.

## Audited IEEE conversion contracts

The 29 historical scalar positions in the three IEEE conversion customers and
their paired refusals retain their authored operations and values. Twenty
positive positions become ordinary `requires`/`ensures` or bare receiving
locals; eight invalid declarations and one invalid cast target retain the
independent Exact or strict-output obligation. Their complete inventories are
beside the [exact-format](../../../tests/omega/pass/float/runtime_exact_format_narrowing_exit/README.md#historical-scalar-positions),
[integer](../../../tests/omega/pass/float/runtime_exact_integer_coercion_exit/README.md#historical-scalar-positions)
and [rounding](../../../tests/omega/pass/float/runtime_format_conversion_result_bounds_exit/README.md#historical-scalar-positions)
witnesses, at revisions `1d445c0646f`, `615a7f1dd29` and `42fbfe08b15`.
No interval-only nominal domains replace these clauses. The binary32 endpoint
`0.1f32` widens explicitly in its binary64 requirement, preserving its landed
value rather than substituting binary64 decimal 0.1.

Public result predicates travel through captured values and exact normal
returns. `float/runtime_result_predicate_custody_exit` covers local/state,
constructor-field and saved-copy transport. Stale values, unrelated operands,
parameter-only guarantees and unbounded tail results cannot grant bounds.

The nine additional positions at `f2714a1d179` are reconciled in the
[refined-conversion inventory](../../../tests/omega/pass/float/runtime_refined_conversion_target_exit/README.md#historical-scalar-positions).
The returned casts keep their original operations while their demanded bounds
move to ordinary result postconditions. False output bounds and fractional or
lossy input conversions have separate refusal witnesses, because source
validation can stop before exit checking. The original runtime oracle is kept;
local-capture and later-state variations use the same conversion judgments.

## Audited time fields and initializer facts

All 16 scalar suffixes in `source/library/std/time.omg` at `c1303f89d4`
are reconciled in the [declaration inventory beside the runtime witness](../../../tests/omega/pass/time/runtime_duration_units_and_scaling_exit/README.md#historical-declaration-inventory).
Three are the `subsecond_nanoseconds` fields of `Duration`, `Instant` and
`SystemTime`; their containing data retains `<= 999_999_999`. Thirteen are
locals whose exact upper bounds follow from their unchanged remainder,
remainder-and-scale or clamp initializers. Their unsigned carriers retain
zero lower bounds. No nominal interval domain substitutes for a proposition.

Pure Duration construction, multiplication and division, and Instant/SystemTime
carry, borrow and overflow execute in both engines. Invalid normalized constants
and a mutation leaving invalid timestamp storage reject. Clock-dependent
`Time::{now,system_time_now,elapsed_since,sleep_for}` additionally need the
hosted consumers named in that witness; a pure arithmetic run alone does not
qualify their host bindings.

## Audited ranked borrowed-field customers

The six `[0..=8]` declarations in
`termination/ranked_callee_projected_receiver_compile` at `8f93c866a3`
are reconciled in its [runtime witness inventory](../../../tests/omega/pass/termination/ranked_callee_projected_receiver_compile/README.md#historical-scalar-declarations).
The full caller observes the callee's write through the nested cursor and
exits 70 in both engines. The bound lookup now follows the common readable
declaration path instead of requiring a one-field source spelling. The
absent-predicate control refuses; matching names supply no interval authority.

The two declarations recovered from
`c60792d7bdd^:omega-rust/omega/compiler/compiler/tests/rank_endpoint_borrows.rs`
live in `termination/rank_endpoint_borrows_recovery`: `Wrap.remaining`
retains `u64 [4..=500]` as data `where remaining >= 4, remaining <= 500`,
and `walk.n` retains `u64 [0..=4]` as `requires n <= 4`. The complete program
carries an exclusive reference in an owned record across ranked recursive
calls and exits zero in both engines. The rank range expression is unchanged;
it is not a scalar type suffix or a storage capacity.

The `limit: u64 [0..=10]` declaration in
`termination/rank_range_subject_intervening_write` at `8894bb375de` now
contributes `limit <= 10` to the machine's existing `requires`. Its subject
write still invalidates the rank premise and rejects. A mutable parameter's
arrival requirement is not store enforcement: the separately canonical
`rank_range_mutable_bounded_endpoint` preserves its premise because its
prefix does not write the bound; `rank_range_mutable_endpoint_overflow_after_write`
rejects Exact endpoint arithmetic after replacing the bound with `u64::MAX`.

## Audited ranked record arrivals and snapshots

All 13 suffix declarations below use `u32 [0..=5]` in their original revisions.
The carriers remain `u32`, supplying the zero floor. Fields retain `<= 5` on
their containing data; callable and state parameters retain `<= 5` as their
own arrival requirements. No interval-only domain is introduced.

| Fixture and original revision | Original positions | Canonical obligations |
|---|---|---|
| `termination/rank_range_record_snapshot_compile`, `568c41c7a7f` | `Pair.left`, `walk.remaining`, `s.copy`, `finish.result` | data `where left <= 5`; three parameter `requires` |
| `termination/rank_arrival_captured_record_fields_exit`, `bd68c42a6ef` | `Pair.left`, `Pair.right`, `walk.remaining` | data `where left <= 5, right <= 5`; entry `requires remaining <= 5` |
| `termination/rank_arrival_same_carrier_fields_exit`, `bd68c42a6ef` | the same three positions | the same clauses on those declarations |
| `termination/rank_arrival_unrelated_field_step`, `bd68c42a6ef` | the same three positions | the same clauses; strict rank decrease still rejects |

The [snapshot witness](../../../tests/omega/pass/termination/rank_range_record_snapshot_compile/README.md)
now executes the existing walk, returning the initial five after the live field
reaches zero. The other two valid programs return zero in both engines, with
and without an immutable record capture. Equal field bounds supply no equation
between fields and no permission to decrease the wrong coordinate; the invalid
sibling retains its original refusal. The rank range expressions remain
unchanged. They neither constrain storage identity nor make an endpoint static.

## Audited result-field and fresh-owner rank arrivals

The [result-field witness inventory](../../../tests/omega/pass/termination/runtime_public_result_field_rank_arrivals_exit/README.md#historical-declaration-inventory)
reconciles eight `Countdown.remaining: u64 [0..=5]` declarations: three at
`fba85426683` and five at `bd68c42a6ef`. Each record retains data
`where remaining <= 5`, its plain field and unsigned zero floor. No result
suffix or interval-only domain is introduced. The original measures,
recurrent calls, record constructors and identity guarantees stay unchanged.

The exact result-field equation still carries the countdown through ordinary
calls and state arrivals. The fresh-owner walk now executes its original five
input and returns zero through the guarded replacement and nested recurrent
carrier. Both engines observe the authored results. Field predicates provide
bounds, not the selected subject's identity or evidence of strict decrease:
the six missing-equation, wrong-subject and opaque-completion controls retain
their original refusals.

## Audited field-selected endpoints and unrelated arrays

The [endpoint group inventory](../../../tests/omega/pass/termination/measure_field_rank_endpoint/README.md#historical-scalar-declarations)
reconciles nine declarations at `a16c1ae91f0`, `fba85426683`,
`c3300162d8b` and `95171450b11`: eight bounded `u64` fields keep their
containing-data upper bounds, and `walk.ceiling` retains both five and ten
as ordinary arrival requirements. Unsigned carriers retain field zero floors.
The computed-limit control's bare `limit` was never a bounded declaration.

The original field-selected walk and the original walk carrying the unrelated
65,536-byte array now have full runtime observations. Their measures and
recursive bodies remain unchanged. The matching invalid programs still refuse
broken callee premises, replaced endpoints and overflowing computed endpoints.
The projected-array local-borrow regression closes the encountered carrier
join through actual first/last byte reads, not a source-shaped shortcut.

## Audited mutual call and computed-copy arrivals

Eleven scalar suffixes retain ordinary arrival propositions and unsigned zero
floors. The recursive calls, computed-copy equations and rank windows remain
unchanged; no interval-only domain substitutes for a bound.

| Fixture and original revision | Original positions | Canonical obligations |
|---|---|---|
| `termination/rank_range_call_component_prefix_write`, `3e87e79438f` | `scan_a.index`, `scan_a.limit`, `scan_b.index`, `scan_b.limit`: `u64 [0..=4]` | each machine requires both `<= 4`, retaining `index <= limit` |
| `termination/runtime_joint_rank_computed_copy_exit`, original `rank_range_call_computed_copy_unproven` at `88d4c1e4f77` | `count.remaining`, `pair.left`, `pair.right`, `step.n`: `u32 [0..=9]` | each parameter's own arrival requires `<= 9` |
| `termination/rank_range_declared_step_unbounded`, `c788455baa2` | `walk.remaining`, `pair.live`, `pair.saved`: `u32 [0..=9]` | each parameter's own arrival requires `<= 9`; stride remains unconstrained |

The [reordered-endpoint witness](../../../tests/omega/pass/termination/rank_range_call_component/README.md)
now executes the existing mutual countdown and observes two. Its prefix-reset
control re-establishes the recursive call's scalar bounds with a post-write
guard, but still refuses entry-relative ranking. A guard grants no descent.
The [computed-copy witness](../../../tests/omega/pass/termination/runtime_joint_rank_computed_copy_exit/README.md)
still returns zero through its equal-copy weak edge and strict decrement.
The unbounded-step control permits zero and retains the strict-decrease
refusal, correcting an already-stale diagnostic pin rather than its algorithm.

## Audited mutable dependent state arrivals

The [disjoint-write witness inventory](../../../tests/omega/pass/termination/runtime_rank_state_constraint_disjoint_write_exit/README.md)
reconciles eight suffixes at `bd68c42a6ef`: four each in
`termination/runtime_rank_state_constraint_disjoint_write_exit` and
`termination/rank_state_dependent_bound_prefix_write`. The unsigned carriers
retain their zero floors. Entry `remaining <= 9` and state
`current <= 9 && ceiling <= 4 && scratch <= ceiling` replace the annotations.

The dependent inequality must remain attached to its arrival observations,
not become an assumption about a replacement ceiling. The shared rank-premise
coordinate query now includes authored requirements as well as declared type
constraints; named-state and call-component prefix consumers protect the same
exact paths. The separate whole-root requirement scan is removed.
The original disjoint store still returns 70 and executes with exit zero in
both engines. The ceiling-write control rejects even though zero satisfies the
independent ceiling upper bound. The field-scoped call-component witness also
retains sibling writes and their caller-visible effect.

## Audited calling vocabulary counts

The 11 scalar suffixes in `source/library/std/calling.omg` at `c1303f89d4`
retain their original bounds as ordinary data and active-case predicates.
The carriers remain `u64`, so zero lower bounds need no additional predicate.
These counts describe authored values; they do not authorize an ABI placement.

| Declaration and fields | Original suffix | Canonical predicate |
|---|---|---|
| `NativePlace::Field.field_path_count` | `[1..=16]` | active case count `>= 1, <= 16` |
| `BoundarySignature.shape_count`, `field_count` | `[0..=256]` | each count `<= 256` |
| `BoundarySignature.parameter_count`, `native_parameter_count`, `callback_binder_count`, `callback_demand_count` | `[0..=32]` | each count `<= 32` |
| `ValuePlacement.location_count` | `[0..=16]` | `location_count <= 16` |
| `RegisterSet.register_count` | `[0..=64]` | `register_count <= 64` |
| `CallPlan.parameter_count`, `callback_materialization_count` | `[0..=32]` | each count `<= 32` |

`calling/calling_count_boundaries_exit` returns complete signature and call-plan
aggregates and observes the exact upper bounds, including nested placements.
`calling/native_field_path_empty` rejects the zero active payload count;
`calling/call_plan_count_invalid_handback` rejects a mutable write above the
declared count. Existing mutable-descriptor and nested case/array host programs
constrain ordinary ABI transport of this vocabulary.

## Audited numeric widening results

All 12 result suffixes in `source/library/core/numeric_conversion.omg` at
`c1303f89d4` now publish their original bounds through `ensures`, with unchanged
result carriers and exact conversions. No replacement nominal domain remains.
The additional value equality on `widen_u32_to_u64` preserves its mathematical
input; it does not grant const-index eligibility or change result type identity.

| Source carrier and declarations | Original result suffix | Canonical result guarantee |
|---|---|---|
| `widen_u8_to_{u16,u32,u64,i16,i32,i64}` | `[0..=255]` | `result >= 0 && result <= 255` |
| `widen_u16_to_{u32,u64,i32,i64}` | `[0..=65535]` | `result >= 0 && result <= 65535` |
| `widen_u32_to_{u64,i64}` | `[0..=4294967295]` | `result >= 0 && result <= 4294967295` |

`arithmetic/runtime_widened_result_predicates_exit` uses runtime byte input,
constructs a 12-field record from the published guarantees, and carries the
record through a helper return and state before observing each exact endpoint.
The stricter-bound and invalid-write controls distinguish published upper
bounds from unjustified narrowing, and nonnegative signed results from their
wider signed carriers. Existing `core/numeric_widening_published_ranges` and
`core/numeric_widening_preserves_values` also constrain Exact arithmetic and
sign/zero extension.

## Audited console and entry-helper bounds

The two console suffixes at `c1303f89d4` retain their original `i32` carriers:
`ByteRead::Byte.value [0..=255]` is the active case's
`where value >= 0 && value <= 255`; the bounded reader's `store` state has
the same relationship in `requires`. No interval-only domain is introduced.
The active payload supplies the exact conversion premise for storing a `u8`.
LF, full-buffer, partial-EOF and reused-middle-window runtime witnesses under
`host/runtime_console_bounded_line_*_exit` retain their authored bytes, counts,
stop reasons and next unread byte. The negative and above-byte constructor
controls both reject. `LineReadResult.count` has no scalar suffix in that
revision: do not invent a declaration-wide capacity bound from those programs.
`host/runtime_console_byte_payload_bounds_exit` also admits both 0 and 255,
transports extracted payload facts into a requires-based state, and distinguishes
Byte(0) from the zero-initialized Eof case.

The five hosted-entry `extent_shape.field_range.first_field` parameters at
`c1303f89d4` had `u64 [0..=254]`. Their current carrier remains `u64`; zero
is inherent and the upper bound is an ordinary arrival requirement.

| Original owner | Current requirement owner |
|---|---|
| `MacosArm64`, `LinuxX86_64`, `LinuxArm64`, `WindowsX86_64` target-entry helpers | shared `HostedEntryShapes::extent_shape.field_range`, `requires first_field <= 254` |
| `MacosX64` target-entry helper | retained local `MacosX64::extent_shape.field_range`, `requires first_field <= 254` |

Each helper reaches that state only after `first_field < 255`. Its
`first_field + 1` therefore fits and addresses at most slot 255; the separate
`second_field < signature.field_count` guard supplies the actual roster bound.
The shared extraction does not alter target ABI choices or turn a flow bound
into shape identity. The console witnesses exercise the selected macOS ARM64
entry policy as well; this is not a claim of matching-host execution for the
other entry products.

## Audited brightness command bounds

The [controller's declaration inventory](../../../samples/cli/basics/brightness_control/README.md#historical-declaration-inventory)
reconciles four declarations at `c8eaaebde25`: two active-case payload bounds,
the containing brightness field predicate, and the apply-state requirement.
The related invalid constructor at `97ebeb57d89` keeps its fifth declaration's
case bound and the deliberately invalid literal 9999. No nominal interval
domain is introduced. The three dispatch arms and authored controller output
are unchanged; native and Terminal execution each observe 65.

First-case zero provisioning uses the constructor's predicate judgment for
the exact zero payload, rather than rejecting every case clause or granting
an unchecked default. A later empty case cannot establish an invalid first
case. The regression also separates resolved field identities from same-named
module constants and carries an applied generic command through parameters,
results and ordinary local storage. Mathematical contract arithmetic never
saturates to fit the checker's finite interval endpoints.

## Remaining scalar customers after the runtime-index cohort

Audit base: `82512cd792cbdab6abd3da32ba6987362e0b1f50`. Its primitive-scalar
census found 30 positions in 15 files. This batch accounts for 29 positions;
`fail/proofs/mathematical_constrained_scalar_formal` remains a suffix customer
until parser retirement. The count is not the history-derived audit or parent
acceptance.

| Customer and original declaration | Canonical replacement |
| --- | --- |
| `static_callback_reach/field` positive/twin and their exact corpus pins: `Carrier.value: u64[0..=37]` (4 positions) | Bare field; data `requires value >= 0 && value <= 37`. The selected callback must establish the construction bound. Extra reach remains the twin's only difference. |
| `pass/build/native_component_case_cycle/producer`: `Decision::Ready.index`, `observe.index`, `poll.index`, `step.index`, `waited.index` (5) | Guarded complete-data `requires` for Ready; identical 1..16 machine requirements. |
| `pass/build/native_component_case_loan_cycle/producer`: `Decision::Ready.index`, `observe.index`, `poll.index`, `forwarded.index/result`, `step.index`, `advance.index`, `waited.index` (8) | Same guarded data and input requirements; `forwarded` publishes 1..16 with `ensures`. Independent component verification, payload loan, pause and 16-iteration oracles remain. |
| `fail/borrows/returned_reference_exposed_integer_local`: `install_one.value`, `choose.position`, `selected_position` (3) | Bare carriers, `install_one` requires value <= 1, existing `choose` position < 2, and initializer facts. The returned loan must still reject the conflicting write. |
| `pass/generics/runtime_computed_direct_result_exit`: `create.total`, `probe.total` (2) | Ordinary 0..100 input requirements; retain `Index<(total+1)>` and observed result 4. |
| `pass/generics/runtime_value_recipe_bounded_declaration`: `Contract::unused.total` (1) | Bare input and complete 0..100 requirement; retain computed result declaration. |
| `fail/generics/runtime_value_recipe_range_after_write`: `invalid.input` (1) | 0..100 input requirement. Overwriting with the same maximum, now `0xffff_ffff`, must still make exact +1 reject. |
| `target_layout_derived_constant/controls/qualified` and `fail/constants/target_layout_derived_qualified_reject`: `BAD: u64[0..=3]` (2) | Bare constant through `bounded_layout(value)` requiring value <= 3. The selected geometry still produces 5 or 9 and rejects that actual false bound. A bound of 9 supplies the valid counterpart; no nominal interval domain is introduced. |
| `fail/generics/runtime_value_recipe_refined_carrier`: `Index.Count` (1) | Retired test. Mentioning unused, unconstructed `Index<0>` has no canonical carrier-mismatch obligation. Keep actual wrong-carrier and establishment controls. |
| `fail/generics/runtime_value_recipe_range_self_cast`: local and cast target (2) | Retired range-shell test. Keep the intentional named-domain `qualified_unproved`, exact `unused_overflow`, and stale `range_after_write` controls. |

The two retired obligations exist only in the removed scalar suffix surface
([declaration and membership](../../spec/language/domains.md#declaration-and-membership)).
Their fixture files and any recorded outcome blocks are removed together;
existing independent controls retain the surviving obligations. Neither an
unsupported qualified constant nor an arbitrary nominal interval replaces them.

The canonical component customers expose the same general fact transports:
formed conjunctions retain their independently justified case guard; closed
callbacks read their retained invocation contract scope; completed shared
bindings retain the exact current observation after an earlier write. Later
writes still retire observations. These are repairs of existing owners, not
new range-specific inference routes. The existing nine-place
`case_where_equality/data_predicate` row also carries an equivalent `u64`
`lo >= 0` conjunct before its equality call, retaining the same equality and
`hi - lo` operation without creating another row.

Qualification producer: `ba4c5b471bb80e396b6084495a267aa4e6194951`, macOS
ARM64, optimized development profile. All nine callback places and nine
case-equality places, both component customers, both generic customers, and
four existing contract/snapshot controls execute with native/Terminal agreement
(26 programs; the snapshot retains exit 70, the others exit 0). The focused
corpus records those 26 runtime oracles and 38 intended rejection diagnostics.
The unrelated closed-local-types callback control still refuses its static
binder on the pre-repair producer too; it is retained as nonaccepted baseline
evidence, not a passing control. This does not close the parent migration or
claim execution on another host.

## Audited copied scalar field and restricted peer

Commit `7516f0970de014e3c13009bf67d9c69f087d4671` introduced three later
suffix positions in the copied-field regression pair. These positions are
additional to the earlier 30-position census above.

| Original declaration | Canonical replacement |
| --- | --- |
| `pass/borrows/copied_scalar_field_with_restricted_peer`: `Limits.value: u64 [0..=5]` | Bare `u64` field with complete-data `requires value <= 5`. |
| Same positive: `bounded.value: &u64 [0..=5]` | Bare `&u64` parameter with machine `requires value <= 5`. |
| `fail/borrows/copied_scalar_field_without_restriction`: `bounded.value: &u64 [0..=5]` | The same borrowed parameter and invocation requirement; `Limits` stays unconstrained. |

The unsigned carrier retains the zero floor. No nominal scalar domain replaces
the anonymous interval. The copied `Frame` still contains 42 and sibling 99;
the case selection, observed-value guard and exit-0 oracle are unchanged.
The positive executes with native/Terminal agreement. Its twin now rejects the
missing `limits.value <= 5` invocation fact rather than a range-type mismatch.
Its later equality guard cannot establish the earlier call's requirement.

Producer: optimized `867738eb9535b2fba0b8300eeaccd64d2c2e6474` on macOS
ARM64. The original positive also ran 0/0 and the original negative rejected
before migration. This closes only these three source positions, not the
historical audit, parser retirement, a new fact row or the parent task.
