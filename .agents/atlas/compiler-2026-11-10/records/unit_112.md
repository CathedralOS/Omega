# Unit 112 notes

Files in unit (all under `omega-rust/psi/interpreter/terminal-interpreter/src/`):
- `engine_custody.rs` (1-2815) READ IN FULL
- `engine_execution.rs` (1-3900) NOT READ
- `engine_structural.rs` (1-2321) READ IN FULL
- `engine_terminators.rs` (1-2610) NOT READ
- `lib.rs` (1-169) READ IN FULL
- `storage_bytes_and_views.rs` (1-5598) NOT READ

Only 5,305 of 17,413 lines were read (custody, structural, lib). The other three files are unread; nothing is said about them below except grep counts of callers of custody functions found in them.

---

## lib.rs (1-169)

1. Purpose. Crate root of the Terminal Psi interpreter. Declares modules, re-exports a public surface, and defines `TerminalExecution::start_artifact`.
2. Shape. 14 private modules (`lib.rs:34-48`). Three `pub(crate)` alias modules `boundary` (`:50-57`), `engine` (`:59-64`), `storage` (`:66-75`) that rename the flat `boundary_*`, `engine_*`, `storage_*` files into paths like `crate::engine::custody` (= `engine_custody`, `:61`). One `impl TerminalExecution` with `start_artifact(semantic_bytes, scalar_arguments, structural_inputs)` (`:157-169`), which loads a `TerminalProgram` and calls `start_execution(..., None)`.
3. Inputs/outputs. Pure re-exports plus one constructor that delegates to `embedding::program::TerminalProgram::load` (`:163`).
4. Decisions. None.
5. Cases by shape. None.
6. Same job elsewhere. `lib.rs:151` re-exports `terminal_installation::{...}` but there is no `mod terminal_installation;` in the module list at `:34-48` (read). So `terminal_installation` must be an external crate name (inferred, not checked; I did not grep the Cargo.toml).
7. Unused/one-caller. Not checked.
8. Stated limits. Header says "Work-bounded reference interpreter" (`:2`); the module-doc at `:12-19` says the engine "depends only on verified Terminal Psi representations".
9. Fail-open. none seen.

## engine_custody.rs (1-2815)

1. Purpose. Holds the rules for who owns which part of a structural value during execution. It tracks "live claims" (linear-ownership claims at a place+path), an "affine frontier" (a set of place+path regions that must still be discarded or consumed), loans completed after calls, nominal cleanup, call-argument binding and aliasing checks, claim transfer between caller and callee, and path/type resolution helpers. Comment at `:1-7`: "single-move affine custody, split and merge claim frontiers across projections, and verify loan provenance".
2. Shape. File is a set of three inline modules followed by free functions.
   - `mod complete_values` (`:9-48`): `TerminalExecution::validate_complete_owned_value_claims` (`:16`) delegates to `terminal_semantics::owned_structure::validate_complete_claim_frontier`.
   - `mod field_stores` (`:49-180`): `restore_store_frontier` (`:61`) re-adds the destination region to the affine frontier after a field store, then merges sibling entries upward into the parent when all children are present (`:122-176`).
   - `mod windows` (`:181-466`): `loan_completions_for_operation` (`:195`), `complete_loan_custody` (`:207`), `owned_window_projection` (`:294`), `stage_window_move_custody` (`:325`), `stage_window_store_claims` (`:397`).
   - Free functions: `bind_result_claim` (`:491`), `place_is_claimed` (`:509`), `require_owned_value_claims` (`:513`, free fn plus `impl` method at `:557`), `whole_path_discard` (`:607`), `has_live_loan` (`:618`), `commit_cleanup_action` (`:628`, 12 mutable table arguments, matches `TerminalAffineCleanupAction` variants `DiscardRoot`, `DiscardResidual`, `RetireHome`, `InvokeNominal`), `begin_nominal_cleanup` (`:778`), `validate_nominal_cleanup_action` (`:844`), `complete_nominal_cleanup` (`:918`), `retire_cleanup_home` (`:951`), `bind_arguments` (`:980`, scalars), `bind_boundary_arguments` (`:1012`), `bind_active_structural_arguments` (`:1027`), `StructuralArgumentProvenance` (`:1049`), `StructuralArgumentSources` (`:1054`), `bind_active_retained_arguments` (`:1088`), `bind_structural_arguments` (`:1142`), `retains_source_lender` (`:1150`), `bind_existing_structural_places` (`:1263`), `bind_structural_primitive_values` (`:1356`), `bind_affine_frontier` / `_types` (`:1450`, `:1463`), `remove_affine_root` (`:1496`), `whole_value_frontier` (`:1510`), `exclude_linear_regions` (`:1538`), `projected_ownership_claims` (`:1576`), `consume_projected_ownership` (`:1596`), `consume_projected_affine_complement` (`:1630`), `consume_affine_discard` (`:1675`), `consume_affine_projection` (`:1712`), `cleanup_field_type` (`:1763`), `split_affine_frontier_at_projection` (`:1788`, recursive), `literal_fixed_array` (`:1914`), `fixed_index_step` (`:1928`), `resolve_structural_path_type` / `_shape` / `_with_leaf` (`:1943`, `:1960`, `:1984`), `inline_structural_field` (`:2028`), `bind_entry_claims` (`:2039`), `resolve_structural_arguments` (`:2080`), `verified_structural_place` (`:2097`), `project_structural_value` (+ `_shared_structural_observation`, `_with_leaf`; `:2122`, `:2139`, `:2156`), `direct_scalar_field_type` (`:2187`), `transfer_claims` (`:2205`), `rebind_structural_result_claims` (`:2366`), `validate_boundary_requirements` (`:2414`), `complete_claims` (`:2439`). Test module `:2488-2815` (7 unit tests on claim transfer, aliasing and mutable-lender retention).
3. Inputs/outputs. Takes `ProgramDeclarations<StructuralTypeDeclaration>`, `RuntimeRows<PlaceId, TerminalStructuralValue>`, `BTreeMap<ClaimId, LiveClaim>` (claims), `BTreeSet<StructuralAffineDiscard>` (frontier), and `structural_cases` maps. Mutates the frontier/claims passed in, or returns new maps (e.g. `transfer_claims` returns `(remaining caller claims, callee claims)` `:2221`). Delegates semantics checks to `terminal_semantics::*` (`owned_structure`, `nominal_cleanup::target`, `structural_paths_may_overlap`, `projection_stays_in_inline_storage`, `runtime_structural_path_tip`, etc.).
4. How it decides.
   - Overlap of two projections is decided by path prefix (`starts_with`) or `terminal_semantics::structural_paths_may_overlap` (`:375`, `:624`, `:876`, `:1326`). Keyed on place identity plus path of segments, not names.
   - Argument aliasing (`bind_existing_structural_places`, `:1310-1347`): two arguments with the same `opaque_identity`, overlapping paths, and either custody (non-Unrestricted multiplicity) or an exclusive borrow, are refused with `StructuralArgumentAliasing`, unless `retains_source_lender` says one is a lender of the other.
   - `retains_source_lender` (`:1150-1261`) decides by access mode and multiplicity pairs of the parameter and its source argument (three arms: unrestricted MutableBorrow, affine Owned, SharedBorrow forwarded from MutableBorrow/affine Owned parameter), then requires path prefix and `access_covers`, then one of: same source place, resolved reference projection, or forwarded parameter.
   - Frontier split (`split_affine_frontier_at_projection`, `:1788`) walks type shape: Record, literal FixedArray, Sum/Mixed (uses `case_membership` closure to find active case; `:1853`).
   - Nominal cleanup validation (`:844-916`) checks claim list equality with the cleanup's `consumed_claims`, multiplicity Linear, and frontier overlap.
5. Cases by shape.
   - `split_affine_frontier_at_projection` handles only Record, literal-length FixedArray, Sum, Mixed. Everything else (`PrimitiveScalar`, `AtomicCell`, `Reference`, `ByteSequence`, `ElementView`, non-literal `FixedArray`) falls to `None` then `AffineProjectionNotRepresentable` (`:1890-1904`).
   - `restore_store_frontier` merges children upward only for Record and literal `FixedArray` whose length `<= frontier.len()` (`:147-150`); any other shape does `break` (`:163`).
   - `retains_source_lender`: three hardcoded access/multiplicity pairings (`:1169-1232`); `_ => continue`.
   - `cleanup_field_type` (`:1763-1786`): `BoundedOwned` byte carriers and non-structural, non-byte fields yield `Ok(None)` (skipped in frontier splits).
6. Same job elsewhere.
   - "split frontier at projection" is done by `split_affine_frontier_at_projection` (`:1788`) and re-entered from `consume_affine_projection` (`:1754`), `exclude_linear_regions` (`:1565`), `complete_loan_custody` (`:262`), `stage_window_store_claims` (`:439`). One implementation, several callers.
   - "merge frontier children into parent" appears only in `restore_store_frontier` (`:122-176`); I did not see the inverse merge elsewhere in the read ranges.
   - Overlap-of-claim checks are open-coded in `place_is_claimed` (`:509`), `has_live_loan` (`:618`), `projected_ownership_claims` (`:1576`), `require_owned_value_claims` (`:534-537`), `consume_affine_discard` (`:1692`), `transfer_claims` (`:2348`): same prefix-either-way idea written out several times.
   - `require_owned_value_claims` exists as free fn (`:513`) and as an `impl` method (`:557`) that calls it.
   - `complete_values::validate_complete_owned_value_claims` (`:16`) and `require_owned_value_claims` both call into `terminal_semantics::owned_structure` (`validate_complete_claim_frontier` vs `validate_owned_claim_frontier`).
7. Unused/one-caller (Grep count mode, counts include the definition line where the regex matched it; `\bNAME\(` over `omega-rust`):
   - `complete_claims(`: 8 hits (def 1; `storage_providers.rs` 5, `engine_calls.rs` 2). Its fourth parameter `_boundary_parameters` is unused (`:2443`, underscore name).
   - `bind_boundary_arguments(`: 3 hits (def 1; `engine_terminators.rs` 1, `engine_calls.rs` 1).
   - `whole_value_frontier(`: 3 hits, all in `engine_terminators.rs` (definition has `<'a>` so the regex did not match it); 3 call sites.
   - `consume_projected_ownership(`: 3 hits (def 1, `engine_terminators.rs` 2); 2 call sites.
   - `resolve_structural_path_shape(`: 1 hit, in `storage_bytes_and_views.rs` (definition has a lifetime generic so not matched); 1 call site.
   - `direct_scalar_field_type(`: 2 hits (def 1, `boundary_inputs.rs` 1); 1 call site.
   - `resolve_single_structural_argument(` or `bind_active_structural_arguments(` (one combined grep): 19 hits across 9 files: `storage_records_and_arrays.rs` 2, `storage_providers.rs` 1, `storage_primitives.rs` 4, `storage_bytes_and_views.rs` 1, `engine_terminators.rs` 3, `engine_custody.rs` 2 (the two definitions), `engine_calls.rs` 4, `boundary_inputs.rs` 1, `boundary_effects.rs` 1. So about 17 call sites between the two names; not split per name.
   - `validate_complete_owned_value_claims(` or `loan_completions_for_operation(` (one combined grep): 12 hits across `storage_references.rs` 2, `storage_providers.rs` 5, `engine_terminators.rs` 1, `engine_custody.rs` 2, `engine_calls.rs` 2 (not split per name).
   - `StructuralArgumentSources` struct (`:1054`): not grepped.
8. Stated limits. `engine_custody.rs:1-7` header only. Errors named `AffineProjectionNotRepresentable` (`:1778`, `:1898`, `:1978`, `:2008`, `:2019`) mark shapes it cannot split or resolve.
9. Fail-open risks.
   - `require_owned_value_claims` (impl, `:556-604`) returns `Ok(())` early in three cases: access is not Owned (`:561`), the parameter at that place has non-Owned access, or `projection_stays_in_inline_storage` is false (`:577-587`). A wrong program where an owned operand whose path leaves inline storage (crosses a reference/pointer) is used without claims would skip the custody check here. Status: traced only to the extent that the early-return conditions are explicit; I did not trace callers. Classification: open.
   - `restore_store_frontier` returns `Ok(())` without touching the frontier when the path contains `Referent` (`:69`), when the destination is not owned-affine (`:97`), or when `owned_window_projection` returns `None` (`:101-103`, which itself returns `None` for Referent paths, element-view roots, and MutableBorrow parameters, `:302-308`). A store through a mutable-borrow parameter therefore adds no restoration obligation. Classification: open (no comment states the reason).
   - `exclude_linear_regions` `continue`s when a claim's place is not an affine root in the frontier (`:1556`). So a claim on a non-affine root adds no frontier split. Classification: open.
   - `bind_active_retained_arguments` / `bind_structural_arguments` with `types = None` makes `retains_source_lender` `continue` in the MutableBorrow arm (`:1174`), i.e. it refuses (not accepts) when types are absent. Fail-closed.
   - `transfer_claims` skips entry claims whose case projection is inactive (`:2244-2252`) and skips transfers whose projection is inactive (`:2277-2282`), requiring the caller NOT to hold that claim in the second case. Intended accept. Classification: traced within the function (inactive-case entries must have no caller claim).
   - `bind_entry_claims` content entry claims use `.or_insert(...)` with `place: parameter.map(...)` so a content claim with no matching parameter is accepted with `place: None` (`:2067-2076`; similar `:2356-2362`). Classification: open. What gets through: a content-entry claim naming an input that is not a parameter is recorded as an unplaced claim rather than refused.
   - `place_is_claimed` and `has_live_loan` return false when no entry matches; used as guards in `retire_cleanup_home` (`:958`) and nominal cleanup (`:731`); nothing was seen that treats `false` as preserved beyond the intended meaning.

## engine_structural.rs (1-2321)

1. Purpose. Execution of the structural operations of Terminal Psi: building records and sum/mixed cases, zeroed values, locals, scalar and structural field stores, moving values and fields, leaf copies, reading scalar/boolean fields, case membership, and byte-sequence read/length. Header comment `:1-8`.
2. Shape. `struct ResolvedStructuralWindow { backing, structural_type, path, aggregate_view }` (`:43`). One big `impl TerminalExecution` with:
   - `execute_establish_scalar_case` (`:51`), `publish_scalar_aggregate` (`:83`), `publish_host_reference_result` (`:114`), `stage_host_referent` (`:155`), `host_referent_field_reference` (`:240`), `referent_reference_paths` (`:264`), `track_affine_result` (`:293`).
   - `execute_establish_structural_case` (`:300`), `establish_structural_case` (`:312`).
   - `prepare_subtree_retirement` (`:448`), `prune_subtree` (`:462`, prunes 12 row tables).
   - `execute_establish_byte_sequence_literal` (`:496`), `result_place_in_use` (`:549`), `execute_establish_zeroed_structural` (`:556`), `materialize_zeroed_structural_storage` (`:587`) and `_with_established_fields` (`:594`), `materialize_zeroed_leaves` (`:610`, one arm per `ZeroedStructuralLeaf` variant: RecordField, ByteSequenceField, CaseField, EmptyScalarArray, ByteArray, ArrayScalar, SumSelection).
   - `execute_establish_structural_local` (`:744`), `execute_establish_trivial_affine_local` (`:772`), `execute_establish_record` (`:802`; the `establish_record` body is not in this file).
   - `execute_structural_scalar_field_store` (`:811`), `execute_move_structural_value` (`:960`, also serves `TransformContent`), `execute_move_structural_field` (`:1066`), `execute_store_structural_field` (`:1134`, about 300 lines), `structural_window_requirements` (`:1438`), `structural_window_hole` (`:1489`), `transplant_subtree` (`:1623`).
   - `execute_structural_leaf_copy` (`:1644`, handles both `StructuralLeafCopy` and `StructuralCaseLeafCopy`), `copy_scalar_array_payload` (`:1965`), `byte_carrier_leaf_copy` (`:1985`), `execute_structural_case_membership` (`:2058`), `observed_field_cell` / `_in` (`:2067`, `:2077`), `execute_boolean_structural_field` (`:2126`), `execute_scalar_structural_field` (`:2136`), `read_observed_field` (`:2146`), `execute_byte_sequence_read` (`:2240`), `execute_byte_sequence_length` (`:2282`).
3. Inputs/outputs. Reads the `terminal_psi::Operation` and live tables on `TerminalExecution` (`structural_values`, `linear_homes`, `live_claims`, `live_affine_frontier`, `structural_scalar_fields`, `structural_cases`, `reference_referents`, `byte_sequence_values`, `element_view_values`, `scalar_array_values`, etc.). Mutates those tables and returns `OperationFlow::Advance`. Stages writes in `ConstructorStorage` and publishes at the end (`:430-445`, `:1396`), so most failures happen before any table is changed.
4. How it decides.
   - Verifies the operation against the declaration in the verified machine by identity: `structural_window_hole` re-reads the declared op by `operation.id` and compares `declared_root`, `declared_field`, and `declared_path.len()` (`:1498-1516`).
   - Field matching in `establish_structural_case` is by field `id` and position in a roster (`:362-365`), roster length must equal `fields.len()` (`:349`).
   - Store disposition `Vacant` / `ReplaceDiscardable` etc. comes from the operation (`:1243`, `:1269`).
   - Copy decisions are keyed on type shape (`ByteSequence BoundedOwned`, `Reference`, `ElementView`, scalar array via `scalar_array_leaf_shape`) (`:1649-1658`, `:1711-1721`, `:1860-1866`, `:1898-1930`).
   - Origin check for stores through references (`structural_window_hole`, `:1530-1585`): after the last `Referent` segment, the base must come from a borrowed non-Linear parameter, an owned non-Linear parameter or non-Linear no-claim producer, or an element-view lender; otherwise `malformed()`.
5. Cases by shape.
   - `establish_structural_case` only for `Sum` or `Mixed` (`:321-330`).
   - `execute_byte_sequence_read` requires result type u8 and index/length operands of u64 (`:2248-2264`); `execute_byte_sequence_length` requires u64 result (`:2291`).
   - `byte_carrier_leaf_copy` supports only `Boolean` and `Integer` leaves in a byte-array layout; other scalar types return `malformed()` (`:2008-2012`, `:2038`). It also requires the leaf path to end in `FixedIndex` (`:1990-1993`).
   - `execute_structural_leaf_copy` accepts result multiplicity Unrestricted, or Affine only when `is_copyable_carrier` (`:1702-1703`).
   - Aggregate storage views: `execute_move_structural_field` refuses with `UnsupportedSemanticVariant("moving an aggregate storage interpretation")` (`:1084-1088`); `execute_store_structural_field` allows an aggregate-view window only for `ReplaceDiscardable`, a copy source, and a scalar-array leaf type, else `UnsupportedSemanticVariant("non-copy replacement of an aggregate storage interpretation")` (`:1242-1250`).
   - `execute_structural_scalar_field_store` rejects destinations whose access is not Owned/MutableBorrow/WriteOnlyBorrow (`:847-851`).
6. Same job elsewhere.
   - Zero values: `materialize_zeroed_leaves` (`:610`) makes zero scalars via a local closure `zero` (`:615-629`) covering Boolean, Integer, Binary32, Binary64. I did not read other zero-value code.
   - "Place already holds a value" tests: `result_place_in_use` (`:549`) checks four tables; other sites check one table with `contains_key` (`:59`, `:318`, `:1072`, `:1667`, `:1706`, `:1974`) rather than calling it. Same job, different breadth.
   - Moving a subtree appears in `execute_move_structural_field` (via `transplant_subtree`, `:1127`) and `execute_store_structural_field` (open-coded stage/prune/publish, `:1379-1396`), and `transplant_subtree` (`:1623-1642`) does the same sequence. Both prune destination then source.
   - Scalar-array copy has three sites inside `execute_structural_leaf_copy`: `:1745`, `:1756-1760`, and `copy_scalar_array_payload` (`:1850`, `:1965`).
7. Unused/one-caller. Not separately grepped for this file's methods. `execute_*` methods are dispatched from `engine_execution.rs` (not read; inferred from `dispatched_operation!` macro use and imports at `:18-20`).
8. Stated limits. `UnsupportedSemanticVariant` strings above (`:205`, `:604`, `:1085`, `:1247`). Error text "an affine case payload requires its owned frontier" at `:1699` (leaf copy of an affine, non-copyable case payload is refused).
9. Fail-open risks.
   - `execute_store_structural_field`, membership repair loop: `if window.aggregate_view { continue; }` (`:1322-1324`) skips the enclosing-membership preservation for aggregate-view windows. Classification: open (no comment). What passes: a replacement into an aggregate view does not update owner qualifications; the earlier guard at `:1242-1250` limits this to scalar-array leaf copies.
   - `execute_store_structural_field`, when `window.aggregate_view` and `writes` is `None`, the check is `scalar_array_leaf_shape(...).is_some_and(|(_, count)| count == 0)` else `malformed()` (`:1372-1377`). So only empty arrays take that branch; otherwise refuses. Fail-closed.
   - `restore_store_frontier` (engine_custody) is called here with the `hole` (`:1264`); see custody notes for its silent returns.
   - `execute_establish_byte_sequence_literal`: if the destination already has a value, it is accepted when identity, type, empty path/qualifications, and byte equality all match (`:517-527`), making a repeat of the same establish idempotent. Classification: traced inside function (the condition is checked; any mismatch gives `malformed()`).
   - `execute_structural_leaf_copy` quotient adaptation: `leaf.structural_type != result.structural_type` is tolerated when `quotient_adaptation` is true or `shared_descriptor_copy_matches` (`:1815-1845`), otherwise routes to `byte_carrier_leaf_copy`, which refuses unless layout matches. Classification: open for the quotient arm (decided by `terminal_semantics::quotients::quotient_copy_adaptation`, not read).
   - Others: none seen. The file uses `malformed()` heavily for unmatched cases.

---

## engine_execution.rs, engine_terminators.rs, storage_bytes_and_views.rs

NOT READ. No claims are made about their contents. The only things known about them come from greps of names defined in `engine_custody.rs`: `whole_value_frontier`, `consume_projected_ownership`, `bind_boundary_arguments` are called in `engine_terminators.rs`; `resolve_structural_path_shape` is called in `storage_bytes_and_views.rs`.

---

## Coverage

- Read in full: `lib.rs` 1-169 (169 lines); `engine_custody.rs` 1-2815 (2,815 lines); `engine_structural.rs` 1-2321 (2,321 lines). Total 5,305.
- Skimmed: none.
- Not read: `engine_execution.rs` 1-3900 (3,900); `engine_terminators.rs` 1-2610 (2,610); `storage_bytes_and_views.rs` 1-5598 (5,598). Total 12,108.

## Five things a maintainer most needs to know about the read part of this unit

1. Custody is tracked in two parallel structures: `live_claims` (linear claims, place+path) and `live_affine_frontier` (set of still-to-discard regions). Nearly every function in `engine_custody.rs` keeps them in step; splits go through the single recursive `split_affine_frontier_at_projection` (`engine_custody.rs:1788`).
2. Several store/move paths silently do nothing to the frontier for mutable-borrow parameters, element-view roots and `Referent` paths (`engine_custody.rs:69`, `:97`, `:302-308`). Nothing at those sites says why.
3. `execute_store_structural_field` (`engine_structural.rs:1134-1436`) is one function that handles vacant-local initialization, replacement, aggregate views, membership repair across all call frames, and retirement of the source home. It builds new state first and commits at the end (`:1398-1435`).
4. Staged writes (`ConstructorStorage`, `identity_cursor`) are used so errors leave tables unchanged: `establish_structural_case` publishes at `:442-444`; `execute_move_structural_value` commits operands then publishes at `:1058-1062`.
5. Structural checks are by identity and path, not by names; the only string-keyed text in the read ranges is error `context`/`detail` text built with `format!("{:?}")` (`engine_custody.rs:1779`, `:1899`, `:1979`; `engine_structural.rs:1294`, `:1339`), used for reporting only.

## What I am unsure about

- Whether `terminal_installation` (re-exported at `lib.rs:151`) is an external crate (no `mod` for it in `lib.rs`; not checked in Cargo.toml).
- Per-name caller counts for `resolve_single_structural_argument` and `bind_active_structural_arguments` (combined grep), and for `validate_complete_owned_value_claims`, `loan_completions_for_operation` (greps were combined).
- Where `establish_record` (called at `engine_structural.rs:807`) is defined.
- All behavior in the three unread files.
