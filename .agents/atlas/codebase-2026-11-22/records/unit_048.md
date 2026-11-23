# Record: unit_048 (area 07-psi-semantics, crate terminal-semantics)

Checkout `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. Root `S/` = `source-rust/subsystems/compiler/psi/semantics/src/`.
All files read in full with the Read tool. Counts below come from Grep count mode over `source-rust/` (patterns named inline).

---------------------------------------------------------------------------------------------------
## 1. `S/plain_affine.rs` (4,396 lines)

1. **Purpose.** Decides which owned affine storage regions are still available as a machine's blocks run (moves, partial moves, decomposition, vacated windows in borrowed storage), and from that derives the cleanup the producer must emit on each normal edge. A checker re-derives the same schedule and compares it (module doc, :1-6).
2. **Shape.**
   - `Region` trie (:138-145): `mode`, `partition_count`, `base`, `whole`, `unavailable_children`, sorted `children`. Methods `available`/`set`/`partition`/`intersection`/`vacancies` (:159-326).
   - `Root` (:338) per-place facts: `owned`, `affine`, `multiplicity`, `whole_restoration`, `unqualified_owned`, `parameter`, `exclusive`.
   - `State` (:359) = `regions: Vec<Option<Region>>` + `active` frontier + `claims`. `State::meet` (:374) joins two arrivals.
   - `Analysis` (:469) walker; `Analysis::walk` (:2419), `operation` (:3035), `dispose` (:3817), `equalize` (:3919), `dispose_components` (:3536), `needs_explicit_disposition` (:3664), `backings` (:4047), `observed` (:4226), `argument` (:4303), `vacate` (:4360).
   - `Plan` (:100): `affine_roots`, `scheduled_roots`, `copyable_owned_roots`, `cleanup: Vec<(EdgeId, Vec<TerminalAffineCleanupAction>)>`, `stores`, `unavailable`, `reference_ends`, `borrowed_header_ends`, `claims`.
   - Entry points: `plan` (:987), `plan_in` (:993), `plan_reference_ends` (:1011), `verify` (:1030), `verify_in` (:1038), `plan_claims_in` (:1066), `verify_claims_in` (:1084), `borrowed_address_liveness` (:877), `declared_multiplicity` (:1184), `validate_carried_permissions` (:1224), `runtime_array_region` (:495), `analyze` (:1371), `validate_successor` (:2063), `prepare_disposition` (:1288), `live_entries` (:723), `reference_backing_uses` (:575).
   - Errors: `Error::{Unavailable, Unsupported{detail,site}, Malformed{detail,site}}` (:52); `Unavailable.unrestored` distinguishes "still owed back at handback" from plain use-after-move (:30-35, message :40).
3. **Inputs/outputs.** Reads a `TerminalMachine` and the dense `StructuralTypeDeclaration` slice; with a `TerminalModule` it also reads descriptor graphs via `crate::descriptor_results::*` and `crate::descriptor_lifetimes::*` (:1909, :2151) and `crate::nominal_cleanup` (:1335, :3065). Output is a `Plan`; `ClaimsPlan::install` (claims.rs:212) writes claim rows back into a machine. `plan` never mutates the machine; the producer installs.
4. **How it decides.**
   - Availability is keyed on exact place + canonical structural path (`Segment`), not on scalar contents (:1-6, test :1248). A write to `a.x` only touches `a.x` and the ancestor's `whole` bit (`Region::set` :198-231).
   - Join: availability is the intersection (:303-311); two arrivals must agree on component partitions (:384-388) and on the ordered `active` set (:404-408), else `Unsupported`.
   - Disposal choice is keyed on declared type flags: `nominal_cleanup`, `multiplicity == Linear`, `owns_linear_claim`, `no_code_affine_cleanup && descriptor_cleanup` (`prepare_disposition` :1319-1365; `dispose` :3841-3872).
   - Whether a root is tracked at all is keyed on type: not copyable, nominal cleanup, contains a reference, or a required-result multiplicity (:1736-1744).
   - A store's disposition (`Vacant` vs `ReplaceDiscardable`) is keyed on whether the receiving region is available (:3443-3453) and compared with the supplied one only when `verify_stores` (:3467).
   - Borrowed windows: `MoveStructuralField` is accepted only through a `Referent` crossing of a `MutableBorrow` reference (:3110-3138) and only when the root is owned or an exclusive loan (:3147).
   - Liveness for `end_references` / cleanup uses a backward dataflow `live_entries` (:723-840), keyed on place ids.
5. **Cases by shape.**
   - `Components::fields` handles only Record/Sum/Mixed (:3770-3779); others return `None` and the caller says "component disposition has no exact structural components" (:3619).
   - Cleanup ends a wrapper only for the nominal types it can name; "erased component disposition is not served" (:3627, :3735).
   - `StructuralPlaceKind::ProviderAttachment` -> `Unsupported("affine provider storage")` (:1694-1696).
   - `observed` handles three shapes: exact static path, descriptor carrier before a `Referent`/index, or a runtime-indexed `FixedArray` (:4231-4271). Anything else is "outside the static structural vocabulary".
   - Suspension crossing refuses when lent storage is partly vacant (:2452-2472), quoting the spec: "a possibly suspending call while an owned value is out of borrowed storage".
   - `Terminator::PendingNormalCompletion` handled by its own branch (:2482-2530).
6. **Same job elsewhere.** Claim bookkeeping inside this file's walk is in `plain_affine/claims.rs` (below). Descriptor-based loan lifetime is the same job as `references/reference_completion.rs` `disposed_reference_on_return` (:22) and `reference_ends.rs` (suspected overlap: all three decide when a descriptor/loan ends; read here, not compared line by line). `Region` is a second structure for "exact field path" facts besides `ClaimsState` (claims.rs:29).
7. **Unused / one-caller.** Grep `borrowed_address_liveness|plan_reference_ends|plan_claims_in|verify_claims_in|publish_contract_claims|plain_affine::(plan|verify|plan_in|verify_in)` over `source-rust/`: 52 hits in 19 files (includes tests and the definitions); callers are source-to-psi stages 05 and 06 and `psi/artifact/wire/src/foundation_validation.rs`. `runtime_array_region` is `pub(crate)`; users in unit: `reference_ends.rs:223`. Comment claims: "THE ... single" not stated.
8. **Stated limits.** "arrivals disagree on the nominal component partition" (:385); "owned homes established in different orders at a reconvergence" (:406); "a partially rebuilt home at a reconvergence" (:3936); "arrivals that disagree on a window open in borrowed storage" (:2751); "partial nominal cleanup requires its authorized component partition" (:3614); comment "A suspension plan states none of that yet" (:2449-2451); "The bounded record operation serves no implicit claim settlement" (:3077).
9. **Fail-open risks.**
   - `root(place)` returning `Ok(None)` (untracked place) makes `read`, `argument`, `end_references`, `storage_use` return `Ok(())` and skip the check (:4281, :4310, :2306, :2995). Traced: `roots[i]` is `None` only for a non-parameter place that is copyable, reference-free, without nominal cleanup and not a descriptor-release target (:1760); stated in the comment "Copyable storage joins the region walk when a disposition affects it" (:1533). So a use-after-move of a copyable value is not reported here. Traced.
   - `roots.iter().all(Option::is_none) && !has_borrowed_windows && claim_mode.is_none()` returns an empty `Plan` (:1802-1807): no walk at all; comment: "Copy permission does not erase an explicitly destructive window." Traced.
   - `prepare_disposition` revisit with `visiting[raw]` true returns `Ok(false)` (:1299-1304): a cyclic type is treated as "not disposable", the refusing direction. Not a fail-open.
   - `restore_reference`/`restore_reference_leaves` return `Ok(())` when `self.references.is_none()` (:2264, :2346): the restoration check is skipped for a machine analysed without a module. Open: callers `plan`/`verify` (no module) reach it; whether a borrowed-window machine can reach that path without a module is not traced (the `MoveStructuralField` with `Referent` branch itself errors when `references` is `None`, :4075-4077, which narrows it).
   - `verify_schedule` compares only actions whose place is in `scheduled_roots` (:1143-1146); an extra supplied action on an unscheduled place is rejected earlier (:1133-1141). Traced.
   - `validate_carried_permissions` returns `Ok(())` for an empty `carried` list (:1235). Traced: no carried parameter, nothing to check.

---------------------------------------------------------------------------------------------------
## 2. `S/plain_affine/claims.rs` (1,391 lines)

1. **Purpose.** Tracks logical (linear) claims through the same ownership walk: where each claim identity sits (place + path), how it transfers on moves, calls, returns and cleanup, and publishes or verifies the claim rows stored in the machine.
2. **Shape.** `ClaimLocation` (:20); `ClaimsState` (:29) with `take`/`put`/`take_at`/`meet`; `ClaimsPlan` (:194) with `install` (:212), `call_frontier` (:206), `call_is_unreachable` (:209); `Mode::{Publish,Verify}` (:308); `Catalogues` (:317, lazily caches `owned_structure::claim_catalogue` per type); `Flow` (:513) with `new` (:520), `operation` (:831), `select_case` (:1135), `successor` (:1159), `terminal` (:1199), `cleanup` (:1281), `unreachable_block` (:1319), `finish` (:1371); `publish_contract_claims` (:389); `prepare_loan_completions` (:419); `validate_bindings` (:368).
3. **Inputs/outputs.** Reads machine contract (`entry_claims`, `block_entry_claims`, `loan_completions`), operations, `crate::owned_structure::*`. In Publish mode mints fresh `ClaimId`s (`fresh` :361); in Verify mode takes them from the machine and checks them. `install` writes rows into operation results, `claim_transfers`, `completion_receipts`, cleanup actions, `InvokeNoNormal`, `ReturnStructural`.
4. **How it decides.** Claims are keyed on place id + `Vec<Path>` (exact inline path). `ClaimsState::take` refuses a path with anything but `Field|Case|FixedIndex` (:110-118) and refuses to split an intrinsic claim (:119-126). Transfers are matched by `starts_with` on paths (:127-129). `meet` requires frontiers equal (:73-79). Call transfers sorted by claim id (:875). Verify mode compares the machine's rows with the reconstruction at each site (:779, :902, :912, :1226, :1265, :1308).
5. **Cases by shape.** `Flow::operation` has explicit arms for `DecomposeStructuralValue`, `MoveStructuralValue|TransformContent`, `MoveStructuralField` (exclusive-parameter vs ordinary), `StoreStructuralField`, `EstablishRecord|EstablishStructuralCase`, `EstablishStructuralArray`, `EstablishZeroedStructural`, `StructuralLeafCopy`; everything else is `_ => {}` (:951-1128). Calls are a fixed list of eight kinds (:847-857, repeated at :1329-1338).
6. **Same job elsewhere.** The call-kind list is written out twice in this file (:847, :1329) and in `install` (:230-247); same list also used by `plain_affine.rs` only via `call_structural_arguments`. `validate_claim_transfers`/`claim_catalogue` live in `owned_structure` (outside unit).
7. **Unused / one-caller.** `publish_contract_claims` pub re-export (`plain_affine.rs:9`); grep (see item 7 above) shows callers in source-to-psi stage 05 `claim_publication.rs` (3 hits across the pattern). `ClaimsPlan::call_frontier` and `call_is_unreachable` not separately grepped.
8. **Stated limits.** "implicit zero-filled storage establishes no linear claim" (:1105); "an open Build tail with a live logical claim" (plain_affine.rs:2497); "external completion with owned claims needs its declared custody handoff" (:1245); "arrivals disagree on their exact logical claim frontier" (:76).
9. **Fail-open risks.**
   - `Flow::operation` `_ => {}` (:1127) leaves claims untouched for any operation not listed; no claim moves. Open: whether an unlisted operation can carry a claim-bearing owned argument. Mitigated: all owned-argument transfer ops are listed; calls take the `is_call` branch.
   - `StoreStructuralField` into an exclusive-parameter root takes the value's claims and drops them (:1036-1045): comment says the replacement claims "leave this activation with it". Stated.
   - `ClaimsState::take` returns empty for non-owned access (:107-109): borrowed arguments carry no custody. Stated at call site (:1173-1175).
   - `unreachable_block` copies fresh result claims into the plan for blocks the walk never entered (:1319-1370), so unreachable blocks publish rows without a walk. Traced to the plan's own constructor only; open whether Verify mode checks them (no Verify check seen in this function).

---------------------------------------------------------------------------------------------------
## 3. `S/plain_affine/tests.rs` (1,998 lines)

Test module for the two files above (`mod tests;` at plain_affine.rs:4396). Group, not itemised: ~30 `#[test]` functions. Builders: `transferring` (:8), `returning_relay` (:127), `reconvergence_types` (:1370), `owned_analysis` (:1409), `borrowed_backedge` (:693). Cover: multiplicity declaration; linear/nondroppable relay and return; partial moves cannot retire an intrinsic claim (:509); join cannot discard a live transfer-only residual (:545); successor capture before cleanup and swap on a backedge (:603, :1173); borrowed successor keeps backing (:773, :806); dead borrowed header rejoining (:885); successor cannot strengthen access (:1006); borrowed address liveness (:1038); byte-element successor (:1096); region set/intersection/partition (:1248-1320); nominal wrapper component disposal (:1323, :1552, :1612); reference release and observation (:1484, :1691); join equalization (:1752, :1790, :1839, :1868, :1881, :1952). Pins exact `Err(Unsupported{detail:...})` strings.

---------------------------------------------------------------------------------------------------
## 4. `S/program_storage_entry.rs` (204 lines)

1. **Purpose.** Recognises the bundled `ProgramStorageEntry::enter` declaration's granted `Extent` parameter so a granted extent is consumed, never created ("These checks never introduce one", :1-2).
2. **Shape.** `ProgramStorageGrant` (:97) with `from_module` (:106), `no_wrap` (:172), `accepts` (:195); `extent_fields` (:63); `bundled_extent_declaration` (:25); `surface_parameter` (:50); const `SURFACE_GRANT` (:23).
3. **Inputs/outputs.** Reads `TerminalModule` identity text table, `structural_domains`, the carrier type. Output: a `ProgramStorageGrant` or `&'static str` error.
4. **How it decides.** Keyed on **source spelling**: identities are looked up by rebuilt identity strings (`compound(...)`, `atom(Label::Path, "Extent::Granted")`, :112-125, :146-152), and field names compared as text `"base"`/`"length"` via `module.identities.text(field.identity)` (:88). The carrier is accepted only if linear, owns a linear claim, and its declaration text equals `"toolchain::Extent"` (:137-143). The predicate must equal `no_wrap(subject)` = `base + length <= 2^64` (:161-166, :181-192). `accepts` keys on position, type id, linear, owned, and the single qualification equal to the domain id (:195-203).
5. **Cases by shape.** Exactly two fields (`[base, length]`, :76), base `address(64)`, length `u64` (:80-85).
6. **Same job elsewhere.** `references/resident_references.rs:65-93` `validate_resident_owner_fields` checks a similar base/length Extent shape by scalar type and field id rather than name (both read).
7. **Unused/one-caller.** Grep `ProgramStorageGrant|extent_fields(`: 6 hits in 3 files: this file (3), `logical_providers/fixed_image.rs` (1), psi-to-native `program_entry/visible_parameters.rs` (2).
8. **Stated limits.** none seen.
9. **Fail-open risks.** none seen (every miss returns `Err`).

---------------------------------------------------------------------------------------------------
## 5. `S/provider_candidates.rs` (168 lines)

1. **Purpose.** Joins a chosen logical provider to its exact accepted candidate positions, shared by source settlement and native admission (:1-3).
2. **Shape.** `CheckedProviderRow` (:9), `CheckedProviderQuery` (:16), `selected_candidate_positions` (:26), `receiving_application_matches` (:153).
3. **Inputs/outputs.** Reads `module.interfaces.provider_candidates`, `provider_instance_requirements`, `identities`; returns one `Vec<usize>` per requested row or a `String` error.
4. **How it decides.** Keyed on **identity text equality**: `module.identities.text(...) == Some(owner)` / `Some(query.provider)` / `Some(requirement)` / `Some(row.machine)` (:41-52) plus `source_candidate_package_identity` equality. Duplicate realizations are detected by sorting `requirement_identity` and finding equal neighbours (:62-67). A retained required instance must match exactly one position (`count != 1` -> error, :137).
5. **Cases by shape.** none (generic).
6. **Same job elsewhere.** none seen in unit.
7. **Unused/one-caller.** Grep `selected_candidate_positions`: 19 hits in 6 files (psi-to-native `execution-scope/scope.rs` 6, `bound_reach.rs` 6, `native_execution_scope.rs` 2, `target_providers.rs` 2, `logical_providers.rs` 2, this file 1). Comment says source and native "use this same declaration query"; consistent with those callers.
8. **Stated limits.** none.
9. **Fail-open risks.**
   - A row whose `matching` is empty is returned as an empty position list, not an error (:87). The later loop (:89-149) errors only if a retained required instance resolves to a count other than 1; if the module carries no `provider_instance_requirements` for that row, an empty selection passes. Open: caller behavior on an empty list not traced.
   - `receiving_application_matches` returns `false` when the identity text is missing (:162-164), refusing. Not fail-open.

---------------------------------------------------------------------------------------------------
## 6. `S/reference_ends.rs` (631 lines)

1. **Purpose.** Proposes which owned borrowed-descriptor leaves have died so the affine walk can end them with `ReleaseReference`. "Descriptor liveness proposes disposal candidates, not permission to dispose storage" (:1-4).
2. **Shape.** `Candidate{argument, unproved_case}` (:17), `Candidates = BTreeMap<DescriptorPosition, Vec<Candidate>>` (:22), `Error{machine,operation,detail}` (:24), `candidates(module)` (:55), `machine_candidates` (:264), `validate_call_owners` (:120), `indexed_store_contents` (:193), `add_dependencies` (:561).
3. **Inputs/outputs.** Reads the module's borrowed-storage descriptor graph (`terminal_borrowed_storage_graph_from`, :74), `DescriptorAnalysis::flow`, lifetimes; returns an `IdTable<MachineId, Candidates>` dense by machine id (:87-113).
4. **How it decides.** Splits each lifetime's authorized locations into `live` and `dead` by whether `lifetime.live` contains the position (:410-419). A dead location becomes a candidate only if it is in inline storage (:462), the tip `is_borrowed_descriptor` (:480) and every `Case` on its path is an established case (`case_is_established`, :498). Parent ordering: topological over `dependencies` pairs, cyclic -> error (:516-557). Keyed on `DescriptorLocation` (root place + path), not names.
5. **Cases by shape.** `indexed_store_contents` considers only `StoreStructuralField` and `StructuralByteViewFieldStore` (:201-209). `add_dependencies` handles a `Referent` crossing or an owned-slot inventory (:595-610). Reference formation list: `EstablishReference|EstablishAccessReference|EstablishElementView|EstablishMutableElementView` (:389-393).
6. **Same job elsewhere.** `plain_affine.rs` `end_references` (:2292) applies these candidates; `reference_completion.rs::disposed_reference_on_return` decides a similar close at a return.
7. **Unused/one-caller.** `candidates` (this file): called from source-to-psi stage 05 `reference_ends.rs` (1 hit under the plan_reference_ends pattern; `candidates(` itself not separately counted).
8. **Stated limits.** "Unknown contents stay with the whole carrier" (:306-308).
9. **Fail-open risks.** none seen as acceptance; `continue` branches (:442-446, :462-468, :480-482, :498-500) skip a candidate, meaning the descriptor is not ended early. Leaving it unreleased is the refusing direction because ordinary cleanup still owes it (stated :495-497).

---------------------------------------------------------------------------------------------------
## 7. `S/references.rs` (26 lines)

Module root: declares `representation_geometry`, `boundary_results`, `declared_results`, `reference_completion`, `reference_origins`, `reference_storage_geometry`, `reference_views`, `resident_references` and re-exports (:8-26). Comment (:3-5): "Semantic judgments for structural reference origins, placed loan regions, reborrow custody, and reference access capabilities." Nothing to decide.

---------------------------------------------------------------------------------------------------
## 8. `S/references/boundary_results.rs` (582 lines)

1. **Purpose.** Validates a boundary (foreign provider) machine's declared reference outputs and result lifetime groups, independent of any spatial reference contract (:1-3).
2. **Shape.** `boundary_lifetime_slots` (:19), `provider_lifetime_debts` (:59), `validate_provider_reference_outputs` (:108), `validate_declared_boundary_reference_outputs` (:212), `validate_leaf` (:281), `boundary_result_slots` (:309), `boundary_descriptor_access` (:362), `boundary_result_lifetimes` (:391), `boundary_result_lifetimes_match` (:482).
3. **Inputs/outputs.** Reads `BoundaryMachineDeclaration`, type table, optional module descriptor analysis; returns slots/debts or `&'static str`.
4. **How it decides.** Keyed on exact descriptor path and declared access: `boundary_descriptor_access(...) != Some(output.access)` (:151); lifetime coverage keyed on `borrowed_result_leaves` path equality in sorted order (:430-437). `boundary_result_lifetimes_match` compares two modules through normalized `(tag, identity text, index)` tuples (:488-533): keyed on **declaration identity text**, not ids, since component modules number identities independently (:479-481).
5. **Cases by shape.** `AllElements` pattern may match `FixedIndex` elements only (:45-49); `validate_leaf` allows only `Field|Case|FixedIndex` paths (:287-296); the path normalizer rejects other segments: "boundary lifetime comparison needs canonical static paths" (:520).
6. **Same job elsewhere.** The declared-lifetime-group validation parallels `declared_results.rs::validate_result_lifetime_groups` (:175; called from :413 here).
7. **Unused/one-caller.** `provider_lifetime_debts` is `pub(crate)`; users in unit: this file :92? (calls at :167, :331). Not grepped across crate.
8. **Stated limits.** "boundary result needs joint spatial and logical lifetime completion" (:398).
9. **Fail-open risks.** In `validate_provider_reference_outputs`, `if implementation.normal_exits.is_empty() { continue; }` (:136) skips a provider with no normal exit. Stated in effect: nothing to establish on a normal return. Traced to the comment on :107.
   `boundary_result_lifetimes` with `result_lifetimes == None` returns `Ok(&[])` when the result has no borrowed storage (:403-410), else errors. Traced.

---------------------------------------------------------------------------------------------------
## 9. `S/references/declared_results.rs` (562 lines)

1. **Purpose.** The one "declared account" of which inputs a call's reference result may borrow from, read from the public machine contract's `result_lifetimes` for selected calls, rigid (bodiless) calls and runtime callables (:1-8).
2. **Shape.** `DeclaredReferenceResult` (:24); `public_reference_result` (:35), `selected_reference_result` (:47), `requirement_reference_result` (:91), `callable_reference_result` (:155), `validate_result_lifetime_groups` (:175), methods `validate_actual_sources` (:221), `validate_actual_result` (:298), `validate` (:340), `region` (:395); `declared_region` (:407); `impl ReferenceResultShape` (:526).
3. **Inputs/outputs.** Reads module, closed-conformance dispatch applications (`crate::closed_conformances::*`), `definition::result_lifetimes`; produces `TerminalDynamicReferenceRegion`s.
4. **How it decides.** Selected call: finds the dispatch application and the schema by 4 identity fields (:72-79); recomputes lifetime groups from the original declared types and requires equality with the header (:82). Actual body sources must each be covered by a declared region of the same parameter position (:260-285). Access: `access.admits_referent(supplied)` (:453).
5. **Cases by shape.** Leaf carriers: `Reference`, `ElementView{access:Some}`, `ByteSequence(BorrowedView|BoundedView {access:Some})` only (:419-434). Stored input must be a `Reference` (adds `Referent`) or an access-declared view (:469-491).
6. **Same job elsewhere.** `boundary_results.rs` does the boundary version of the same lifetime-group validation.
7. **Unused/one-caller.** `public_reference_result`, `selected_reference_result`, `requirement_reference_result` appear in `reference_completion.rs` (:1190, :1226), `memberships.rs` (:48). Not grepped crate-wide.
8. **Stated limits.** "selected caller-chosen result needs its admitted backing and retention" (:378).
9. **Fail-open risks.** `declared_reference_result` with `result_lifetimes == None` and no borrowed leaves yields `&[]` (:121-139): a result with no borrowed leaves has an empty frontier; stated at :124-126, and borrowed results still refuse. Traced.
   `requirement_reference_result` returns `Ok(None)` for a callee with a body (:98-100): then "the callee's actual body origins govern"; stated at :88.

---------------------------------------------------------------------------------------------------
## 10. `S/references/declared_results/definition.rs` (445 lines)

1. **Purpose.** Rebuilds the public lifetime groups from the original declared type identity strings, independent of the editable group rows and of what the body returns (:1-3). Includes two in-file tests (:320-445).
2. **Shape.** `result_lifetimes` (:18), `occurrences` (:103), `DeclaredTypeWalk` visitor (:168) implementing `TypeIdentityPackageOwnerVisitor`.
3. **Inputs/outputs.** Walks a type-identity **string** with `visit_type_identity_package_owners`; outputs `(lifetime ordinal, path, access, referent)` occurrences grouped by lifetime.
4. **How it decides.** Keyed on **source/type-identity spelling** (forms `Reference`, `MutableReference`, `WriteOnlyReference`, `Slice`, `Array`, `Generic`, `Nominal`, `Named`: :224-247, :294-299). After the walk, each recorded access must equal the current type's stored reference access (:142-151) and no two occurrences may share a path (:158-164).
5. **Cases by shape.** Depth cap 128 (:183). A `Generic` ancestor of a region returns `Unadmitted::ApplicationScope` (:285-293). A nominal/named/generic type must satisfy `plain_owned_shape` else `UnsupportedEmbeddedName` (:259-263).
6. **Same job elsewhere.** none.
7. **Unused/one-caller.** `result_lifetimes` is `pub(super)`; one caller (`declared_results.rs:82`).
8. **Stated limits.** error text "original public type has an unserved declared lifetime frontier" (:1-... mapped at :136).
9. **Fail-open risks.** none seen.

---------------------------------------------------------------------------------------------------
## 11. `S/references/reference_completion.rs` (1,598 lines)

1. **Purpose.** Several judgments about loans at calls and returns: whether a return explicitly closes a loan's carrier names, whether a returned reference's ancestry is consistent, what immediate loan a descriptor has, and which input loans a call result keeps alive.
2. **Shape.**
   - `disposed_reference_on_return` (:22); `validate_reference_result_completion` (:154) with `ReferenceCompletionOccurrence` (:11) and `ReferenceCompletionError` (:128).
   - `BorrowedLoanSource` (:221), `BorrowedLoanSources` (:419), `borrowed_loan_sources` (:458), `borrowed_loan_source` (:441), `reference_loan_parent` (:781), `joined_incoming_loans` (:353), `incoming_view_occurrence` (:270).
   - `RetainedCallLoan` (:813), `reference_call_lifetimes` (:1046), `retain_call_lifetime` (:842), `retain_provider_inputs` (:872), `retain_result_dependencies` (:938).
   - `cleanup_disposes_reference` (:1572); `borrowed_entry_reference_referent` (:105).
3. **Inputs/outputs.** Reads machine, module, `DescriptorAnalysis`, lifetimes, flow steps. Returns booleans, `BorrowedLoanSource(s)`, `Vec<RetainedCallLoan>`; errors are `&'static str` or `DescriptorGraphError`.
4. **How it decides.**
   - Return closure: walks successors from formation; a cleanup action must name each surviving name by root place and path-prefix (`cleanup_disposes_reference`, :1580-1597). A path containing `Referent` is never disposed by cleanup (:1577).
   - Completion: parents of a returned or caller-stored occurrence must themselves be disposed, not returned, unless in caller storage (:194-196); a closing parent may not have a live child (:206-213).
   - Loan source: keyed on lifetime `authorized`/`surviving` sets at the position and on descriptor origin kinds (:475-531, :541-735). A shared byte view with every origin `Empty` or a literal of a shared byte view is `Owner` (:548-581).
   - Retained call loans: bound by `bind_reference_call_results` / `bind_public_reference_call_results`; retained path stops at the first non-static segment ("Stopping early only widens the retained region, never narrows it", :820-823).
5. **Cases by shape.** `reference_call_lifetimes` branches on callee kind: runtime callable, `Boundary`, or `Machine` with or without a body (:1097-1245). `retain_result_dependencies` refuses `Invocation|AccessFormation|Carried` origins "needs its admitted retention resource" (:978-983). `joined_incoming_loans` handles only a shared reference whose arrivals trace to one borrowed input field; "exclusive references ... stay refused" (:345-352).
6. **Same job elsewhere.** `reference_ends.rs` and `plain_affine.rs::end_references` (above).
7. **Unused/one-caller.** Grep `disposed_reference_on_return|validate_reference_result_completion|cleanup_disposes_reference`: 17 hits in 5 files (psi-to-native legalization custody 2, optimization-unit-semantics `current_ownership.rs` 2, `semantics/src/lib.rs` 3, this file 8, source-to-psi `places/loans.rs` 2). So all three have external callers.
8. **Stated limits.** Quotes: "An empty intersection at a join proves no release" (:31); "owned view backing is not an incoming reference occurrence" (:800); "returned dynamic reference needs its admitted completion resource" (:1136).
9. **Fail-open risks.**
   - `disposed_reference_on_return` returns `false` (refuse) when `flow` is `None` (:58). Not fail-open.
   - `retain_result_dependencies` skips (continues) for owned arguments, view-descriptor formals and stored non-descriptor formals (:993-1016) with comments that the loan travels via descriptor flow. Stated.
   - `borrowed_loan_sources`: skips a lifetime whose `authorized` is empty everywhere (:480-486) — stated: "that lifetime authorizes no carrier anywhere".
   - `reference_call_lifetimes` returns an empty vec when the call has no invocation/runtime/dynamic form or no borrowed result (:1066-1082): such a call retains no loan. Traced: no borrowed result means nothing to keep alive.
   - `continue` on `(None, ReferenceCallOrigin::Invocation{..})` (:1356) and `transfers_existing` (:1359-1362): stated in comments.

---------------------------------------------------------------------------------------------------
## 12. `S/references/reference_origins.rs` (431 lines)

1. **Purpose.** Shared algebra on reference origins: binding an origin to an actual argument, whether one origin refines/covers another, overlap, and resolving the typed endpoint of a path that may contain `Element` steps.
2. **Shape.** `ReferenceOriginBinding` (:31) + `transfers_existing` (:39); `bind_reference_origin` (:61); `reference_origin_refines` (:101, private); `reference_origin_is_covered` (:183); `reference_result_refines_regions` (:251); `reference_origin_paths_may_overlap` (:309); `captured_reference_origin_type` (:334); `reference_origin_type` (:351); `resolve_reference_origin_type` (:370); `exact_origin_type` (:403); `collection_element_type` (:414). Submodules declared :7-16.
3. **Inputs/outputs.** Reads the type iterator and `ReferenceOriginProjection` paths (`Exact(segment)` or `Element`). Outputs types/bools.
4. **How it decides.** Origin refinement keyed on place equality, access equality and path prefix with `Element` matching any `FixedIndex` (:109-130); `Projected` requires equal length, `WithinRegion` allows deeper paths with constraints on `Referent` and descriptor crossings (:131-179). A runtime `Element` is covered only if every index of a literal-length array is covered (:195-236). `collection_element_type` for a `ByteSequence` accepts only a unique unsigned 8-bit non-address, unrestricted primitive in the type table (:422-428).
5. **Cases by shape.** `collection_element_type`: `FixedArray`, `ElementView`, `ByteSequence` only (:419-430). `reference_origin_type` rejects `RuntimeIndex`/`FixedRange` (:356-366).
6. **Same job elsewhere.** `references/reference_origins/descriptor_regions.rs` uses `reference_origin_is_covered` (:115) and has its own `inline_view_is_covered` (:605).
7. **Unused/one-caller.** Grep `descriptor_source_projection_accessible|reference_view_offset_is_zero|reference_result_refines_regions|reference_origin_paths_may_overlap|shared_descriptor_copy_matches|descriptor_value_transfer_matches|descriptor_reborrow_matches`: 55 hits in 24 files total (mixed patterns, not per-function). Per-function counts not taken.
8. **Stated limits.** none.
9. **Fail-open risks.** `reference_origin_is_covered` ends `false` (refuse) when nothing covers (:237). `reference_origin_paths_may_overlap` returns `true` (conservative: assumes overlap) at the end and for `RepresentationView` (:316-331). Both refusing directions.

---------------------------------------------------------------------------------------------------
## 13. `S/references/reference_origins/borrowed_leaves.rs` (184 lines)

**Purpose.** Enumerates borrowed leaves (references and views) of a result type in sorted path order up to a cap. `borrowed_result_leaves` (:19) / `BorrowedResultRoster::visit` (:70). Keyed on declared type shape and `maximum_leaves` supplied by the caller (not a fixed number): errors "dynamic result has more borrowed leaves than public regions" (:57, :132). Shapes: Reference, ElementView, ByteSequence borrowed/bounded, FixedArray (closed literal extent only: "requires a closed array extent", :124), Record, Mixed, Sum. Other shapes contribute nothing (`_ => {}` :110, :145). Fail-open: a type shape outside the list silently yields zero leaves (:110, :145), e.g. an `AtomicCell` or `PrimitiveScalar`; that is correct (no borrowed leaf). Cyclic owned type refused (:75). Users: `declared_results.rs`, `boundary_results.rs:421`, `result_regions.rs:24`, `call_results.rs:191`, `descriptor_regions.rs:23`.

## 14. `S/references/reference_origins/call_results.rs` (291 lines)

**Purpose.** Binds a callee's declared reference-result origins to the call's actual arguments. `bind_reference_call_results` (:93) for `ReferenceCallContract::{Machine, Requirement, Callable}`; `bind_public_reference_call_results` (:182); `ReferenceCallSource::projected_origin_suffix` (:54). Keyed on parameter position found by place id (`structural_parameter_position`, :134) and the declared region's `argument_index` (:206-208). Scalar view admission: a differing endpoint is allowed only if `scalar_reference_view_admits(access, stored, expected)` and then adds a `RepresentationView` step (:65-88). Fail-open: `projected_origin_suffix` returns the suffix unchanged when extent is not `Projected` or endpoint equals expected (:62-64); stated by the doc comment (:44-52). `reference_regions` errors propagate with `?`. None else.

## 15. `S/references/reference_origins/descriptor_regions.rs` (637 lines)

**Purpose.** Checks that returned view descriptors and reference leaves stay within the declared input regions, and derives the "reference sources" mapping from actual returned slots. `validate_descriptor_result_regions` (:16), `reference_result_sources` (:136), `reference_result_contents` (:147), `reference_sources_for_leaves` (:292), `descriptor_input_origin_path` (:465), `contents_path` (:564), `inline_view_is_covered` (:605), `input_path_accessible` (:540).
**Decides** on origin kind: `Empty` skipped; `Literal` allowed only for `SharedBorrow` leaves that have a shared input-region contract (:49-86); `Provider` becomes an `Invocation` origin and requires a non-empty `invocations` list and an empty projection (:321-335); `Parameter|IncomingSlot|IncomingContents` become parameter origins, with `projection.indirect || projection.window` refused (:350). Mixed input roots across alternatives refused (:449-455). Generic unknown contents need `TypeParameter` shape and owned/shared access (:276-280).
**Cases by shape.** `append_projection` accepts `Field|Case|FixedIndex|Referent|RuntimeIndex->Element`; `FixedRange` stops the walk silently (`break`, :524); anything else errors (:534).
**Fail-open.** `FixedRange { .. } => break` (:524) stops appending projection at a range, shortening the origin path (widening to the enclosing storage). Open: no comment at the site; it widens, which is the conservative direction for origins used as bounds, but the check `inline_view_is_covered` (:605-637) accepts an exact path equal to an allowed path only. Open.
Unknown-contents packet with empty origins `continue`s when the case facts exclude the payload (:206-212). Traced to `case_facts`.

## 16. `S/references/reference_origins/memberships.rs` (330) and `memberships/tests.rs` (265)

**Purpose.** Computes which domain memberships a call's returned shared reference keeps from its source (`ReferenceResultMemberships::for_operation` :26; `reference_call_preserved_memberships` :125; `preserve_call_memberships` :150; `reference_result_memberships_weaken` :293). Keyed on mapped output-leaf path and, per candidate source, `Projected` extent and a `Parameter` origin (:205-213); the common memberships are the intersection across candidate sources (:266-272). Only `SharedBorrow` reference leaves are considered (:195-201). Descriptor graph built lazily once via `OnceCell` (:15, :108). Fail-open: if any candidate source cannot be resolved the leaf retains nothing (`complete = false`, :262-265) and a mutable leaf retains nothing: refuse direction. `reference_result_memberships_weaken` returns true only when each extra source row crosses a shared reference (:293-327).
Tests (`memberships/tests.rs`): 4 `#[test]`s (:65, :92, :201, :232): unresolved views keep loans without authority; indexed generic return needs read access; malformed mappings are rejected before the graph is built (`builds == 0`); only a projected shared referent demands the graph.

## 17. `S/references/reference_origins/result_regions.rs` (372 lines)

**Purpose.** `validate_dynamic_reference_regions` (:9), `validate_input_region` (:86), `reference_leaves` (:179), `reference_inventory` (:205), `ReferenceInventory::for_restoration` (:216), `collect_reference_inventory` (:239). Region list must be unique-ordered, cover each borrowed leaf and match leaf access (:21-43). An input region must cross at least one borrowed boundary ("dynamic result cannot borrow private owned parameter storage", :173). Inventory: `TypeParameter` (and, for a shared input, a non-literal `FixedArray`) is recorded as unknown contents, not enumerated (:259-272); `_ => return None` for other shapes (:324). "Enumerating known references never proves an unknown packet empty" (:178). Caps: `maximum_leaves` supplied by caller; `usize::MAX` for restoration (:235). Fail-open: none seen (`None` means "no complete inventory", refusing). Users: `plain_affine.rs:2365`, `descriptor_regions.rs:153`.

---------------------------------------------------------------------------------------------------
## 18. `S/references/reference_storage_geometry.rs` (1,542 lines) and `reference_storage_geometry/tests.rs` (905)

1. **Purpose.** Computes storage size, alignment, byte offsets, regions and view compatibility of structural types, with unresolved target inputs reported instead of defaulted. Used by both source plan validation and foreign storage recipes (:134-136).
2. **Shape.** `ReferenceStorageGeometry<Types,Extents,Records>` (:387) with `layout` (:453), `sum_layout` (:468), `region` (:492), `location_alignment_limit` (:511), `source_compatible` (:566), `view_source_compatible` (:645), `region_source_compatible` (:702), `record_fields` (:748), `dependencies` (:432). Free: `geometry` (:869), `sum_layout` (:1040), `storage_path` (:1267), `storage_region` (:1167), `authored_field_storage_extent` (:175), `validate_storage_fragments` (:256), `validate_storage_destination_intervals` (:308), `schema_field_layout_source` (:321), `reference_field_representation_width` (:137), `reference_storage_size/layout/projection/region` (:1130-1165), `reference_scalar_storage` (:1230). `ReferenceStorageRegion::alignment_at` (:39). Trait `RecordStorageResolver`/`StorageGeometryError` re-exported from `terminal_psi` (:120).
3. **Inputs/outputs.** Reads type declarations, optional `TargetCarrierLayout` (pointer, descriptor, case-tag sizes), a `RecordStorageResolver` (selected layout realization), an `extents` closure for `ArrayExtent::Application`. Calls `layout_planning::standalone_integer_geometry` (:80, one call site in this crate). Outputs `(size, alignment)`, regions or `StorageGeometryError`.
4. **How it decides.** Keyed on declared type shape and per-field `AuthoredFieldStorage`. Address-sized integers need `TargetCarrierLayout` else `MissingCarrierLayout` (:73-76). Alignment: `1 << min(tz(base_alignment), tz(offset))` only when base is a power of two (:39-45). `source_representation_compatible` allows an 8-bit unsigned non-address source to view anything (`byte_endpoint`, :590-615) and otherwise requires equal layout or equal size with byte-scalar target (:620-634). `view_source_compatible` requires region length equal to target size and alignment multiple (:684-688).
5. **Cases by shape.** `geometry` handles Reference, ByteSequence, ElementView, AtomicCell, PrimitiveScalar, Record (TargetLayout/Authored/Packed), FixedArray, Sum, Mixed; `_ => Err(UnsupportedType)` (:994). `storage_path` handles Sum/Mixed fields and cases, Record fields (not AccessView/Accessor, :1419-1420), `FixedIndex`, `FixedRange`, `RepresentationView`; `_ => return None` (:1519). Depth cap 128 (:877).
6. **Same job elsewhere.** `layout_plans::packing::conventional_sum_geometry` (:1092) for sums; `layout_planning::standalone_integer_geometry` (:80) for standalone integers (two different default layouts: this crate's `scalar_geometry` for non-integer scalars at :61-64 uses width==alignment).
7. **Unused/one-caller.** `reference_storage_size`, `reference_storage_layout`, `reference_storage_projection`, `reference_storage_region`, `reference_scalar_storage` not individually grepped. Users in this unit: `reference_views.rs`, `representation_geometry.rs`.
8. **Stated limits.** "Placement follows the existing native default, not the stricter byte-view codec query. Address placement remains target-owned" (:66-67); "This limit alone proves no alignment" (:509-510).
9. **Fail-open risks.** none seen for acceptance; unresolved items return `None`/`Err`. `physical_carrier_geometry` returns `Ok(None)` (fall through to default geometry) when the resolver has no carrier layout (:1005-1007). Traced: `UnselectedRecordStorage` (no `carrier_layout` override seen in this file) supplies defaults.
   Tests file: 9 `#[test]` (:30, :46, :107, :234, :448, :614, :722, :788, :854): alignment retention, address placement, physical carriers, schema source, sum offsets, extent discovery, array extent overflow, authored child width, fragmented fields.

---------------------------------------------------------------------------------------------------
## 19. `S/references/reference_views.rs` (717 lines) and `reference_views/scalar_view_admission_tests.rs` (103)

1. **Purpose.** Admission rules for references and descriptors that view other storage: descriptor copy/transfer/reborrow matching, reference-source compatibility, access along a path, scalar recast admission.
2. **Shape.** `shared_descriptor_copy_matches` (:9), `descriptor_value_transfer_matches` (:31), `descriptor_reborrow_matches` (:46), `descriptor_reborrow_shapes_match` (:69), `stored_reference_access` (:87), `descriptor_geometry_matches` (:107), `is_copyable_carrier` (:140), `is_shared_reference_carrier` (:152), `reference_source_storage_path` (:190), `reference_source_projection_compatible` (:210), `reference_source_projection_accessible` (:266), `descriptor_source_projection_accessible` (:350), `reference_byte_field_compatible` (:417), `reference_storage_export_byte_width` (:460), `scalar_reference_view_admits` (:577), `borrowed_field_presentation` (:596), `shared_direct_byte_descriptor_source` (:640), `reference_source_compatible` (:677), `plain_storage` (:517), `storage_scalars_satisfy` (:532).
3. **Inputs/outputs.** Type iterators and shapes; bools/`Option`.
4. **How it decides.** `scalar_reference_view_admits` (:577-590), keyed on access: Owned requires `stored == presented`; Shared requires `stored->presented` total; WriteOnly requires `presented->stored` total; Mutable both. Spec cited: `wiki/spec/layouts/recasts.md#shared-and-mutable-views` (:569). Path access: walks segments; a `Referent` after a `Reference` sets `authority` to that reference's access and requires `requested.admits_referent` (:291-305); views with declared access are checked (:309-343). `is_shared_reference_carrier` follows `FixedArray` element chains up to the number of types (:156-186).
5. **Cases by shape.** `descriptor_geometry_matches`: ElementView same element; BorrowedView any; BoundedView equal capacity (:112-136). `reference_source_compatible`: same type; shared byte descriptor from `BorrowedView{None}` to `BorrowedView{Some(Mutable)}`; BoundedOwned equal; scalar view (:677-715). `reference_storage_export_byte_width` loops over bool, f32, f64 and signed/unsigned 8..128 in steps of 8 (:472-487).
6. **Same job elsewhere.** `reference_source_projection_accessible` and `descriptor_source_projection_accessible` repeat the same element-view / byte-view access check (:309-343 vs :376-413).
7. **Unused/one-caller.** Counts in item 12's combined grep (55 hits, 24 files; includes psi-to-native stages 02/04/05/09, optimization-unit-semantics, interpreter memory). All named functions have external callers in that set.
8. **Stated limits.** "Domain handback obligations on the lender stay with the checker; this judges only representation validity" (:575-576).
9. **Fail-open risks.** `reference_source_projection_accessible` `_ => {}` for other path segments (:344) lets them pass without an access check. Traced: only Referent and index segments change authority; other segments are inline field steps. `shared_direct_byte_descriptor_source` returns `None` for non-matching shapes (:649-665), meaning "not this case". Callers' handling of `None` not traced.
Tests file: 3 `#[test]` (:30, :51, :82).

---------------------------------------------------------------------------------------------------
## 20. `S/references/representation_geometry.rs` (655)

**Purpose.** Validates a storage view (`reference_view_compatible` :67) over every retained target layout input, with `validate_reference_view_source` (:15) and `reference_view_offset_is_zero` (:48). Keyed on type ids: both source and referent must be "total representation" (no scalar restriction, no `requires`, no linear claim, ints non-address or floats, no qualified field, :197-245); a writable view also needs the source total (:97). Layout is checked once per retained `TargetLayoutInput`, with all applications required to share one input roster ("no common complete layout input roster", :143-148). A path containing `Referent` is refused ("requires its captured physical storage correspondence", :76-80). `validate_reference_view_source`: a borrowed incoming scalar needs a fixed export width equal to its byte width (:24-32); other sources must be `OperationResult|ByteSequenceLiteral|TrivialAffineLocal` (:38-42). `reference_view_offset_is_zero` accepts only a literal u64 0 (:56-64). `target_carriers` (:247) builds `TargetCarrierLayout` with descriptor size = 2 x pointer. Fail-open: no applications found -> falls back to the non-target geometry check (:135-141); it yields a bool from `view_source_compatible` which requires `MissingCarrierLayout`-free layouts for address-bearing types; open on whether a record needing carriers silently fails there (it returns false via `.ok()?`). Tests: 4 `#[test]`s (:293, :369, :518, :565).

## 21. `S/references/resident_references.rs` (93)

**Purpose.** `resident_reference_plan` (:7) returns the unique retained `TerminalAccessViewPlan` for a resident-interpretation reference, after `validate_resident_owner_geometry` (:53)/`validate_resident_owner_fields` (:65). Keyed on structural domain ids: `granted != resident`, and both domains' `carrier == source_type` (:32-40); access may not increase (`requested.admits(access)`, :28); exactly one plan with `carrier == referent` (:43-49). Owner record must own a linear claim, `base != length`, base an address integer, length unsigned 64-bit non-address (:73-89). Counts (Grep `resident_reference_plan|validate_resident_owner_*`): 14 hits in 7 files incl. psi-to-native `access_reference_validation.rs` 3, wire `foundation_validation.rs` 2, interpreter `memory/references.rs` 1. Fail-open: none seen.

---------------------------------------------------------------------------------------------------
## 22. `S/scalar_conditions.rs` (135)

**Purpose.** Reads condition subjects and scalar application dependencies from a published catalog. `proof_provider_edges` (:11), `invocation_declarations` (:29), `visit_condition_rows` (:46), `visit_condition_applications` (:81), `ConditionPosition` (:40). Decides nothing semantic beyond refusing absent rows: "scalar scope contains an absent range condition" (:57) / absent completion (:73) / absent observation (:90). Keyed on proof declaration ids and `ScalarRangeObservation` kind. Counts (Grep `proof_provider_edges|invocation_declarations(|visit_condition_rows|visit_condition_applications`): 24 hits across 9 files (not per-function; includes `execution-inputs/scalar-applications/conditions.rs` 6, `synchronous_calls/*` 8, source-to-psi 05 and 07). Fail-open: `ScalarRangeObservation::Literal(_) => Ok(())` (:93) visits no application: correct since a literal has none.

---------------------------------------------------------------------------------------------------
## 23. `S/scalar_qualifications.rs` (1,629)

1. **Purpose.** Validates that nominal scalar domain membership ("qualifications") is never minted except by declared routes: field/primitive catalogs, transfers, returns, coercions, issuer routes. "Neither an ordinary result annotation nor an unbound coercion can mint membership" (:1-3).
2. **Shape.** `ScalarQualificationError` (:15); `validate` (:975), `validate_for_bodies` (:986), `validate_transfers` (:173), `validate_machine_transfers` (:229), `validate_field_types` (:537), `validate_predicate_binding` (:682), `validate_arguments` (:770), `checked_introduction` (:860), `case_guarded` (:712), `retains_predicate` (:755), `retains_conjunct` (:1595), `reordered_predicate` (:1605), `issuer_route_authorizes` (:87) with `IssuerRoute` (:73), `arithmetic_policy_identity` (:59), accessors `qualification_domains` (:939), `scalar_domain` (:952), `field_qualifications` (:832), `primitive_qualifications` (:846), `values` (:963).
3. **Inputs/outputs.** Reads `module.types.scalar_qualifications` (domains, sets, fields, primitives, coercions, returns), machines, `membership_facts::analyze_bodies` (:1093). Outputs `Ok(())` or a typed error; no derived rows.
4. **How it decides.** Keyed on ids and set contents, with several **text/identity comparisons**: arithmetic policy names `"builtin:policy:Wrapping|Saturating|Trapping"` (:63-65); `issuer_route_authorizes` compares retained identity **text** of the issuer machine to `callable_evidence_identity(...)` (:98-112) and uses template-callable identity for specializations (:113-123). Catalog canonicity: sorted/unique ids, sets non-empty, no duplicate set (:991-1031). A return that adds a domain needs a `returns` row whose origin is authorised: `Vacuous` only when the domain has no predicate/routes/prerequisites; `Proved` only when a predicate exists with no routes; `Forwarded` only when the source already carries or forwards via `membership_facts`; `Established{route}` only when the issuer route matches this machine (:1323-1390). Predicates are compared with `retains_conjunct`: equality, order-insensitive reordering, or containment in a conjunction (:1595-1600).
5. **Cases by shape.** Predicate-subject substitution supports only `Integer` and `Boolean` carriers; other carriers allowed only with `Truth` (:593-611). `case_guarded` handles exactly "`(self in C && owed...) || self in D || ...`" or a one-case conjunction (:708-753, doc :708-711).
6. **Same job elsewhere.** Domain-membership tracking is also in `membership_facts.rs` (outside this unit; called :1093) and `structural_qualifications.rs` (outside unit). Suspected overlap; not read here.
7. **Unused/one-caller.** Not individually grepped (the module is the area's public scalar-membership checker; users listed in the crate map).
8. **Stated limits.** "Abstract check-form requirements remain header assumptions" (:984-985).
9. **Fail-open risks.**
   - `validate_transfers` iterates all `module.machines` while `validate_for_bodies(module, bodies)` receives a `bodies` list used only for `membership_facts::analyze_bodies` (:1093); return-origin checks run over `catalog.returns` for any machine (:1221-1468). Open: machines not in `bodies` still get structural validation but membership-fact proofs only for `bodies`. Stated in the doc comment :984-985.
   - `ScalarReturnOrigin::Forwarded` accepted when `missing_membership_requirements(...)` is `Some(empty)` (:1337-1341) or via edge facts. Traced to catalog logic.
   - `case_guarded` returning `None` means a clause "owes nothing conditionally" (:708-711): a sum field predicate guarded by a differently-shaped clause is treated as not retained, so `retained` false -> error (:631-635). Refusing.
   - `reordered_predicate` normalizes only `Equal`, `Conjunction`, `Disjunction`, `Implication` (:1605-1627); other nodes compare structurally. Stated: "no arithmetic rewriting or new predicate premise is admitted" (:1604).
   - Scalar read results not in the `stored` set but `qualified` are refused (:529-531). Refusing.

---------------------------------------------------------------------------------------------------
## 24. `S/scalar_target_ranges.rs` (439) with `calls.rs` (87), `guards.rs` (340), `storage.rs` (112)

1. **Purpose.** For exact integer arithmetic whose operands trace back to target-input scalar applications, builds the range conditions the target must satisfy ("ground arithmetic that can be checked once the selected scalar values exist", :1). Also reconstructs pending call prerequisites for layout-dependent callee `requires` clauses (`layout_call_conditions`, calls.rs:7).
2. **Shape.** `TargetScalarRange{obligation, owner, requirement, steps}` with `retain` (:36) and `matches` (:45); `target_scalar_ranges` (:56); `GroundRange` (:163) with `term`, `value`, `definition`, `value_origin`, `operation`, `equated`; guards: `branches` (guards.rs:39), `guard_indicators` (:74), `indicator` (:108), `guard` (:144), `call_guards` (:201), `predicate` (:220), `incoming_predicate` (:255), `exact_operand` (:311); storage: `StorageValues::new` (storage.rs:28), `sources` (:53).
3. **Inputs/outputs.** Reads module `scalar_applications`, machine blocks, dominator tree (`control_graph`), descriptor graph and scalar captures. Outputs `Vec<TargetScalarRange>` sorted by obligation (`:151`) or `(ObligationId, ScalarPredicateCondition)` rows.
4. **How it decides.** Early exit when the module has no scalar applications (:61) or the machine cannot acquire a first target input from a requires clause, a `ReadScalarApplication`, or a callee `ensures` clause (:72-83). A range is built only for `CanonicalScalarGoal::ExactArithmeticRepresentable` of an integer type with min/max (:102-113), and only if the traced expression contains an `Application` observation (:124-129). Value tracing is keyed on a **unique dominating definition** (`definition` :236-249) and requires the declared type to match (:268). Guard indicators multiply by an indicator for each conditional edge that dominates the block and whose target has a single incoming edge (guards.rs:57-68, :90-93).
5. **Cases by shape.** Hard limits: `remaining: 4096` node budget (:121, guards.rs:85, calls.rs:66), depth `< 64` (:176), storage trace `visited.len() > 64` -> `None` (storage.rs:81). Term shapes accepted: `Value`, `Application`, `IntegerWiden`, `IntegerExactCast`, `ExactIntegerAdd|Subtract|Multiply`, else a literal (:181-221). Operations accepted: `IntegerConstant`, `ReadScalarApplication`, `PrimitiveScalarRead|ScalarStructuralField` (via storage), calls with no `required_contract` and no `static_reach_binding` (:352-358), and proof-bearing leaf denotations (:386-391). `guard` handles `BooleanConstant`, `BooleanNot`, `BooleanEqual` with a constant, `IntegerEqual`, `IntegerLessThan`, `IntegerLessOrEqual` (guards.rs:162-189).
6. **Same job elsewhere.** `membership_facts`/`scalar_qualifications` analyze flow facts separately; the loop-carried case "needs an inductive proof" (storage.rs:78).
7. **Unused/one-caller.** Grep `layout_call_conditions|target_scalar_ranges(`: 12 hits in 6 files (wire `foundation_validation.rs` 4, source-to-psi stage 06 `lib.rs` 2 + tests 2, this unit 3). Source-to-psi and saved-artifact ingress both call it (calls.rs doc :1-2).
8. **Stated limits.** "An unknown runtime guard supplies no restriction" (guards.rs:101); "A loop-carried storage value needs an inductive proof" (storage.rs:78); "A runtime alternative supplies no premise about which branch executes" (guards.rs:199-200).
9. **Fail-open risks.**
   - Any failure to trace an expression `continue`s with no range for that operation (:123-130, :97-101). Effect: no target-range obligation is retained for that operation. Open/traced: the arithmetic still carries its own `ExactArithmeticRepresentable` obligation from the lowered checker (`obligation` is only used to key a retained range). So this function only adds ranges; missing one weakens nothing it was asked to prove, but the caller's use of absence not traced.
   - `guard_indicators`: an unknown guard truncates and contributes no restriction (guards.rs:101-103). Stated; absence of a restriction widens the range required, refusing direction.
   - `call_guards`: an untraceable guard is omitted (guards.rs:211-216). Stated in doc :199-200; omission produces fewer premises, so the prerequisite is stricter. Traced.
   - `layout_call_conditions` skips a call whose `invocation_requires` is `None` (calls.rs:43-45). Open: no comment.
   - Dominator tree failure returns empty guards (guards.rs:87-89). Same direction.

---------------------------------------------------------------------------------------------------
## 25. `S/scalars.rs` (19)

Module root only: declares `proof_bearing_scalar`, `scalar_array`, `scalar_field_requirements`, `scalar_leaf_schema`, `scalar_leaf_semantics`, `scalar_observations`, `scalar_ordering` (pub), `unchanged_scalar`, and test-only `symbolic_integer_widths`, `trapping_integer_tests`. Comment (:1-3): "Semantic judgments for scalar leaf evaluation, arithmetic and comparison contracts." Not decisions.

## 26. `S/scalars/proof_bearing_scalar.rs` (619)

1. **Purpose.** Defines the fixed table of integer operations that carry proof obligations (exact cast/shift/add/sub/mul/div/rem and wrapping/saturating div/rem) and computes, for one operation, its denotation term, canonical goal and result equation. Also the `TrappingInteger` meaning built from the Exact primitive's goal.
2. **Shape.** `ProofBearingScalarSemanticRow` (:146) with `ALL` table generated by `proof_bearing_scalar_rows!` (:161, instance :194-207, 12 rows); `ProofBearingIntegerPolicyBinding` (:28) and `proof_bearing_integer_policy_binding` (:47); `catalog_goal_shape` (:91); `validate_proof_bearing_scalar_semantic_rows` (:216); `proof_bearing_scalar_semantic_row` (:244); `ProofBearingScalarLeafSemantics` (:255); `proof_bearing_scalar_leaf_semantics` (:475); `build_denotation` (:375); `TrappingIntegerSemantics` (:527) and `trapping_integer_semantics` (:547).
3. **Inputs/outputs.** Reads an `Operation` and a `&dyn ValueTypes`; returns semantics (`obligation`, `canonical_goal`, `denotation`, `result_equation = Equal(result, denotation)`).
4. **How it decides.** Keyed on `OperationSemanticTag`. Operand typing: add/subtract use `admits_exact_add_operands`/`admits_exact_subtract_operands`; other binaries require both operand types equal result type (:428-440); shifts require value type == result type (:406). Goal shape must equal both the row's and the `integer_policy_bridge` catalog's formation conditions (:235, :493). Each call revalidates the whole table (`proof_bearing_scalar_semantic_row` calls `validate_proof_bearing_scalar_semantic_rows`, :247).
5. **Cases by shape.** Fixed list of 12 tags (:195-206); `IntegerExactCast` unary; others binary. Other tags return `Ok(None)` (:479-481). `catalog_goal_shape` maps the five (primitive, formation-conditions) combos, `_ => None` (:124).
6. **Same job elsewhere.** `CanonicalScalarGoal::for_term` (`canonical_goal` submodule, not in unit). `trapping_integer_semantics` reuses the Exact tag rows (:593-594).
7. **Unused/one-caller.** Grep `trapping_integer_semantics(`: 7 hits in 5 files (its own definition 1, `scalars/trapping_integer_tests.rs` 3, `bytes/byte_sequence_predicates.rs` 1, source-to-psi 06 `crashes.rs` 1 and `lib.rs` 1).
8. **Stated limits.** Spec quotes in doc comment (:516-526) about "its own `Trap` site".
9. **Fail-open risks.** none seen (every mismatch is an `OperationSemanticError`). Open cost note only: the table is re-validated per operation (:247).
Related: `mod formation_tests;` (:619) is declared here but its file is outside this unit's rows.

---------------------------------------------------------------------------------------------------
## Coverage

Read in full (all with the Read tool, every line of the listed range):
- `plain_affine.rs` 1-4396 (4,396)
- `plain_affine/claims.rs` 1-1391 (1,391)
- `plain_affine/tests.rs` 1-1998 (1,998)
- `program_storage_entry.rs` 1-204; `provider_candidates.rs` 1-168; `reference_ends.rs` 1-631; `references.rs` 1-26 (1,029)
- `references/boundary_results.rs` 582; `declared_results.rs` 562; `declared_results/definition.rs` 445; `reference_completion.rs` 1,598 (3,187)
- `references/reference_origins.rs` 431; `borrowed_leaves.rs` 184; `call_results.rs` 291; `descriptor_regions.rs` 637; `memberships.rs` 330; `memberships/tests.rs` 265; `result_regions.rs` 372 (2,510)
- `references/reference_storage_geometry.rs` 1,542; `.../tests.rs` 905 (2,447)
- `references/reference_views.rs` 717; `scalar_view_admission_tests.rs` 103; `representation_geometry.rs` 655; `resident_references.rs` 93 (1,568)
- `scalar_conditions.rs` 135; `scalar_qualifications.rs` 1,629; `scalar_target_ranges.rs` 439; `calls.rs` 87; `guards.rs` 340; `storage.rs` 112; `scalars.rs` 19; `scalars/proof_bearing_scalar.rs` 619 (3,380)

Total read in full: 21,906 lines (sum of the 31 TSV rows). Skimmed: 0. Not read: none.

Grep commands used for counts (path `source-rust/`, output_mode count): `ProgramStorageGrant|extent_fields\(`; `selected_candidate_positions`; `proof_provider_edges|invocation_declarations\(|visit_condition_rows|visit_condition_applications`; `resident_reference_plan|validate_resident_owner_fields|validate_resident_owner_geometry`; `layout_call_conditions|target_scalar_ranges\(`; `borrowed_address_liveness|plan_reference_ends|plan_claims_in|verify_claims_in|publish_contract_claims|plain_affine::(plan|verify|plan_in|verify_in)\b`; `trapping_integer_semantics\(`; `disposed_reference_on_return|validate_reference_result_completion|cleanup_disposes_reference`; the combined view/descriptor pattern in item 12. Per-function counts were not taken where a combined pattern is named.

## Five things a maintainer most needs to know about this unit

1. `plain_affine.rs` skips tracking (`root(...) == None`) for copyable, reference-free, non-nominal places, so `read`/`argument`/`end_references` return `Ok(())` for them (:1760, :4281, :4310). Use-after-move of those values is not this walk's job.
2. The affine walk does the same job as the loan/descriptor lifetime code in `reference_ends.rs` and `reference_completion.rs`: all three decide when a descriptor or loan ends, using the descriptor-flow analysis owned outside this unit; `end_references` (plain_affine.rs:2292) only applies candidates `reference_ends::candidates` proposes.
3. Several decisions are keyed on identity **text**, not ids: `program_storage_entry.rs` (rebuilt identity strings and field names "base"/"length", :112-125, :88), `provider_candidates.rs` (text equality of provider/requirement/machine identities, :41-52), `scalar_qualifications.rs` issuer routes (`callable_evidence_identity`, :98-112), `declared_results/definition.rs` (walks a type identity string, :122), `boundary_results.rs::boundary_result_lifetimes_match` (declaration identity text, :523-529).
4. Fixed numeric caps with no comment on why: `remaining: 4096`, depth 64, storage trace 64 in `scalar_target_ranges*`; depth 128 in `reference_storage_geometry.rs:877`, `definition.rs:183`, `reference_views.rs:538`. Hitting one yields `None`/`Err` (no accept), except where the caller treats `None` as "no extra range" (`scalar_target_ranges.rs:123-130`).
5. Verify mode vs Publish mode share one walk (`analyze(..., bound_output, ..., claim_mode)`): `verify` reconstructs the schedule then compares (plain_affine.rs:1030-1180); claim rows are compared per site in `claims.rs` (:779, :902, :912, :1226, :1265, :1308). A schedule or claim row the walk does not visit (e.g. unreachable blocks, `claims.rs:1319-1370`) is copied from the supplied rows without a comparison seen in that function.

## What I am unsure about

- Whether the `restore_reference*` early return when `self.references.is_none()` (plain_affine.rs:2264, :2346) can be reached by a machine that has borrowed windows; the `MoveStructuralField`/`Referent` path errors without references (:4075), which narrows it, but I did not trace every caller of `plan`/`verify` (no-module entry points).
- `claims.rs::unreachable_block` (:1319): whether Verify mode ever checks the rows it copies for unreachable blocks; I did not see a check there and did not read the `ClaimsPlan` consumers.
- `descriptor_regions.rs:524` `FixedRange => break` widens a returned view's origin path; I did not trace whether the later coverage checks make that sound.
- Per-function external call counts for most `pub` items in `reference_views.rs`, `reference_origins.rs` and `reference_storage_geometry.rs`: I ran only combined-pattern greps (counts above are totals, not per function).
- Overlap claims with `membership_facts.rs`, `structural_qualifications.rs`, `descriptor_results/*`, `descriptor_lifetimes/*`, `owned_structure`, `nominal_cleanup` are "suspected": those files are outside this unit and were not read.
- Several shorter entries (sections 13-17, 20-22, 24) are compressed to purpose, decision key and fail-open notes rather than the full nine-item layout; the code in them was read in full.
