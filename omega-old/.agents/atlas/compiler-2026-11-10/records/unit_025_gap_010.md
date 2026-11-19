Second pass for unit 25

Ranges covered (all in `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/current_ownership.rs`):
- 7101-9965 (2865 lines), read in full.

No contradiction with the earlier notes: they say current_ownership.rs lines 1101-9965 were not read, and nothing here conflicts with the part they did read.

Enclosing context: line 7101 is the tail of `combine_alternative_origins` (defined at 7061) inside `pub(crate) mod references` (opens 3780, closes at 7412). Found by grepping `^    fn ` / `^    pub... fn ` and module headers. Line 9965 is the last line of the file.

Module map for 7101-9965 (grepped `mod` headers, read bodies):
- `references` tail: 7101-7412
- `mod replay`: 7413-8839
- `mod residuals`: 8840-9096
- `mod storage_regions`: 9097-9241 (re-export at 9242)
- `mod structural`: 9243-9443 (re-export at 9831-9834)
- `mod value_custody`: 9444-9830
- top-level items: 9836-9965

## 1. `references` tail (7101-7412)

Purpose: finishes the reference-origin bookkeeping for return paths. It checks that a returned structural value's reference leaves come from the sources the function signature declares, and that a normal return restores every borrowed reference the caller lent.

Shape:
- `combine_alternative_origins` tail (7101-7105): builds `ReferenceOrigin::Alternatives { place, origins }` and returns it as both root and `ReferenceParent::Root`. Its error path is `Err(invalid())` at 7101, and the guard before it is above the range.
- `transfer_return` (7107, `pub(super)`): returns `Ok(false)` if `structural_source_contract(function, source)` is `None` (7116) or the type contains no reference (7119). Otherwise:
  - It requires the count of live references carried at `source` (owned-resident carrier path) to equal `result.reference_sources.len()` (7126-7141).
  - For each declared mapping it finds the live reference by `(carrier, carrier_path)`.
  - An `ReferenceOrigin::Invocation` root must appear in `mapping.sources`, have `ReferenceParent::Root(root)` equal to its own root, and have no child reference (7183-7200). Otherwise it errors with "invocation reference return differs from completed call custody".
  - Every other root goes through `return_origin_family` (7207). Each resulting source must be covered by a parameter-supplied origin (`terminal_semantics::reference_origin_is_covered`, 7236). The parent must be its own root and there must be no children (7222-7249).
  - It then calls `terminal_semantics::validate_reference_result_completion` on `ReferenceCompletionOccurrence` rows (7258-7288), and `live.retain` drops the returned carriers (7289).
  - It pushes `StructuralReferenceResultSource` rows into `observed`.
- `return_origin_family` (7295, private): maps a `ReferenceOrigin` to `ReferenceOriginSource` rows.
  - `Invocation` returns `None` (7310).
  - `Exact(RuntimeIndex)` path segments are rewritten to `Element` (7321-7324).
  - For a `Projected` extent whose endpoint type is not the expected type, the endpoint must be a `Reference { referent == expected, access covers }` shape, else `None` (7328-7336). It then appends `Exact(Referent)`.
  - The result is sorted and deduped.
- `validate_return_coverage` (7351, `pub(super)`): for each declared `reference_sources` mapping, every `Projected`-extent source must appear among the observed return origins for that path (7364-7374). Only `Projected` extents are checked.
- `validate_normal_return` (7379, `pub(super)`): rebuilds `entry_references`, keeps those whose carrier is a non-`Owned` structural parameter (7387-7391), asks `effects.remaining_at_return(...)` for the live set, sorts both by identity and requires element equality (7396-7410). Error text: "normal completion does not restore borrowed reference custody".

Inputs/outputs: reads `PsiOptimizationFunction`, the structural type map, and `LiveReference` rows. `transfer_return` mutates `live` and `observed`. The rest return `Result`.

How it decides: keyed on `PlaceId` carrier and `carrier_path` (7130, 7146), `ReferenceOrigin` variant (7183, 7309), structural `access_covers` (7228, 7333), and `ReferenceIdentity` equality for the parent/child check (7198, 7241).

Callers (grepped):
- `transfer_return`: one site, 8593.
- `validate_return_coverage`: one site, 8833.
- `validate_normal_return`: one site, 3183.
All are in this file.

Fail-open: none seen at 7101-7412. All mismatches return `Err` or `None`. `transfer_return` returning `Ok(false)` when the source has no signature or no reference type (7116-7121) just means "nothing to transfer". The caller at 8593 ignores the bool. Traced (read): `ReturnStructural` goes on to the owned-place and claim checks regardless.

Stated limits: none quoted.

## 2. `mod replay` (7413-8839)

Purpose: the forward replay of a function's CFG that rebuilds the live ownership frontier (claims, owned places, partially-moved paths, live references) block by block. It rejects any operation or edge that violates it, and it records a snapshot at every block entry, operation entry/exit and edge entry/exit. The result is the `OwnershipFrontierFact` list that `candidates.rs` (6858-6872, 7477-7490) later compares across merges and threads.

Shape:
- Helpers:
  - `is_borrowed_member_move` (7438): non-empty path and the source contract is `MutableBorrow`.
  - `validate_atomic_access` (7447): for `O::AtomicEvent` with a location, requires an arrival parameter at `location.root`, an atomic cell type at the path, no qualifications, no resident-observe on a `WriteOnlyBorrow`, and, when `Owned`, a live owned place with no partial custody (7467-7491).
  - `claims_sit_at_owned_arguments` (7495): each claim pair must sit at an `Owned` argument whose place equals `live.input`, whose path is a prefix, and be unique via `seen.insert`.
  - `claim_path_is_inactive` (7522): true if any `Case` segment of the path is not a member on every path (`membership_on_every_path(..., false)`, 7541-7552).
- `validate_current_ownership_cfg` (7560, `pub(super)`): the worklist. It uses `ready: BTreeSet<BlockId>` and `pop_first` (7587, 7593), plus `incoming: BTreeMap<BlockId, CurrentOwnership>` (7588). Per node it does the following, in order:
  1. Partial-custody check for `MoveStructuralField` / `StoreStructuralField` using `references::window_hole` (7641-7683).
  2. `value_use.observations`, then `references::apply_operation`, then `validate_atomic_access` (7698-7718).
  3. `StructuralScalarFieldStore` to an `OwnedResult` / `OwnedBlockParameter` requires a live owned place if multiplicity is not `Unrestricted` (7720-7743).
  4. `StructuralCase`, `StructuralCaseMembership` and `StructuralScalarFieldRead` require liveness, and no overlapping partial custody, for an `Owned` non-`Unrestricted` source (7745-7792).
  5. `EstablishTrivialAffineLocal` and `ReturnStructural.trivial_affine_locals` insert owned `Affine` results (7794-7819).
  6. Structural arguments and multiplicities are collected per operation shape (7821-7910). Owned arguments call `value_use.moving`, others `value_use.observing`.
  7. `consumed_places` is computed (7918-7936), then `pins.check_operation`, then the whole-place-partially-moved check (7937-7961).
  8. Invocation claim-transfer check (`CallStructural`, `InvokeNoNormal`) compares expected claim paths per argument with the live claims (7963-8100).
  9. `BoundaryCall` completion-receipts check (8101-8144).
  10. `TransformContent` derivation/parent claim accounting (8145-8241).
  11. `ReleaseActivationClaim` must release exactly one claim at exactly the source (8242-8259).
  12. Transferred claims are removed (8260-8288).
  13. Consumed and moved owned places are updated: `partial_custody_paths` is extended, `projected_root_is_fully_consumed` removes the owned place, and `ReleaseReference` with a non-empty path is handled (8289-8416).
  14. `install_store` for `StoreStructuralField` (8418-8435).
  15. Result establishment, with claim transfer for `MoveStructuralValue` / `TransformContent` (8437-8550).
  16. Terminator-specific checks (8567-8692):
     - `Return` and `ReturnUnit` run `cleanup_frontier`; `Return` also runs `reject_live_linear_claim`.
     - `ReturnStructural` does a long sequence: moving check, `transfer_return`, no partial custody, owned-place removal unless the result is `Unrestricted`, `validate_complete_claim_frontier`, `returned_claims == expected`, cleanup, and no remaining claims.
     - `Crash` requires `frontier_lower_bound == live claim keys` (8678-8690).
  Then per outgoing edge (8701-8823): snapshot, `pins.check_edge`, filter claims by selected `StructuralCase` arm (8721-8754), `consume_edge_sources`, `apply_edge_cleanup_actions`, `establish_edge_parameters`, snapshot, then join with any existing `incoming[target]`.
- After the loop: any block without an incoming frontier gives `UnreachableBlock` (8826-8831). Then `validate_return_coverage`, facts are extended from `snapshots`, and `pins.descriptor_requirements()` is returned (8833-8837).

Inputs/outputs: reads the unit (`identities`, `dynamic_dispatch`, `psi`, `structural_types`), the function, block and predecessor maps, other functions, and boundary machines. It mutates `facts` and `call_accesses`. It returns descriptor compatibility requirements.

How it decides (keyed on):
- `PlaceId` and `ClaimId` identity for claims and owned places.
- Structural path prefix and overlap (`starts_with`, `structural_paths_may_overlap`, 7674, 8064, 8326).
- `psi_edge` equality to select the case arm (8736, 8766).
- `membership_on_every_path` for case membership: whether a case is excluded on every path (7541, 7626, 8212, 8637, 8770).
- Join comparison at 8801-8818 is whole-struct equality: `claims` equality gives `CurrentClaimJoinMismatch`; equality of `owned_places`, `partial_custody_paths` and `live_references` gives `CurrentOwnedPlaceJoinMismatch`. A block's frontier is fixed by its first arrival; later arrivals must be equal.

Cases by shape (fixed lists):
- Structural-argument extraction match (7831-7842): `establish_record_or_case_pattern`, `TransformContent`, `EstablishStructuralArray`, `call_structural_or_boundary_call_pattern`, `StoreStructuralField`, `ReleaseActivationClaim`, `MoveStructuralValue`; everything else is `&[]`.
- Parameter-multiplicity match (7843-7910): the same families, plus `CallStructural`/`InvokeNoNormal` to `Machine`, `CallStructural` to `Dynamic`, and `BoundaryCall`; `_ => Vec::new()`. Note `ReleaseReference` (7929) and `ReturnStructural` are not in the argument list and are handled separately.
- Terminator match (8567-8692): only `Return`, `ReturnUnit`, `ReturnStructural`, and `Crash` (guarded); `_ => {}`.
- `StructuralCase` outgoing claim filter (8721-8754) applies only if the block's last node is a `StructuralCase`.
- `invocation` handling (7964-7982) covers `CallStructural` and `InvokeNoNormal` only. Boundary calls use the receipts check instead.

Same job elsewhere:
- Partial-move bookkeeping (insert path, reject overlap, check "fully consumed", remove owned place) is written inline three times in `validate_current_ownership_cfg`:
  - 8361-8383 (structural arguments with a path);
  - 8386-8416 (`ReleaseReference` with a path);
  - the same overlap test at 8321-8338 (pre-check).
  It is also done in `cleanup` and `block_parameters` (earlier in this file, not in my range).
- "Is the call's structural contract for this place missing" is written as `structural_source_contract(...).ok_or(StructuralCallContractMismatch ...)` at 7849-7857 and 7862-7868.
- Claim-path expected-vs-actual comparisons: 8039-8086 (call) vs 8107-8137 (boundary call) vs 8156-8237 (transform). Three similar loops with different keys.
- Suspected: `psi/compilation/validation/src/value_custody_core.rs` (1 hit on `uninstalled_home|…` family in my grep? No: the hit list for `partial_affine_residuals|selected_affine_residuals` includes `psi/.../validation/src/value_custody_core.rs:1`, `terminal-psi/semantics/src/structural.rs:1`, and `06_checked-trees-to-lowered-psi/src/emission_cleanup.rs:1`). These are in other crates, not read, so a parallel Psi-side ownership/residual computation is only suspected.

Unused/one-caller:
- `validate_current_ownership_cfg`: one call site, 9857 (grepped; the only other mention is the definition).
- `validate_atomic_access`: one call site, 7711.
- `claim_path_is_inactive`: used at 8050 and 8467 (read).

Stated limits: none quoted in this module.

Fail-open risks:
- 8826-8831 and 8801-8822: a block reached by several edges is checked only against the FIRST arrival's frontier by equality. This is fail-closed (mismatch errors).
- 7458-7460 `validate_atomic_access` returns `Ok(())` when `event.location()` is `None`: an atomic event with no storage location gets no authority check. Reachable by any `AtomicEvent` whose `location()` is `None`; I did not read `AtomicEvent::location` or its callers. "open".
- 7116-7121 (`transfer_return` returning `Ok(false)`) discussed above: traced, not an accept path.
- `observations` and `observing` (in `value_custody`, section 5) return `Ok(())` for many shapes; see there.
- `expect(...)` panics on 7596, 7608, 7623, 8704 are invariants of earlier validation (message: "reachable block has a current ownership frontier", "validated block is nonempty"). Panics not accepts.
- Other than the above, none seen.

## 3. `mod residuals` (8840-9096)

Purpose: for a partly-moved affine structural value, compute the list of unmoved sub-places ("residuals") that must still be disposed of, or return `None` if the moved set is not a shape it can describe.

Shape:
- `partial_affine_residuals` (8848, `pub(super)`): `selected_affine_residuals` with a `selected` callback that is always false (8859).
- `selected_affine_residuals` (8863, `pub(super)`): returns `None` if `moved_paths` is empty, any path is empty, any path does not resolve (`resolve_structural_path`, 8873), or one moved path is a prefix of another (8880-8885).
- `collect_partial_affine_residuals` (8903): matches the type shape:
  - `Record`: errors with `None` if there are no fields (8914).
  - `FixedArray` with a literal non-zero length: refuses if `length > remaining + moved_paths.len()` (8932), then recurses per index in reverse (8935).
  - `Sum` and `Mixed`: picks the case from the moved paths, else from the unique `selected(prefix, case.id)` match (8949-8959). It rejects any moved path that names another case, or a `Field` when there are no common fields (8965-8971). It recurses on payload fields and common fields.
  - `_ => return None` (9009). That covers `PrimitiveScalar`, `AtomicCell`, `ByteSequence`, `Reference`, `ElementView`, and arrays with a non-literal extent.
- `collect_field_residuals` (9014): walks fields in reverse; any erased field returns `None` (9024). Scalar fields are skipped. A byte-sequence field with `BoundedOwned` carrier is skipped, and other carriers (inline) recurse via `inline_carrier_declaration` (9030-9038). Structural fields recurse.
- `collect_child_residuals` (9057): if no moved path descends into the child, the child itself is a residual (limit `max_residuals`, else `None`, 9076). If all descendants are non-empty, recurse. If a descendant is empty and `descendants.len() != 1`, `None` (9090).

How it decides: keyed on `StructuralTypeShape` variant, `StructuralPathSegment` (`Case`, `Field`, `FixedIndex`), and counts bounded by `max_residuals`. `max_residuals` is a limit passed by the caller (3310 passes `limit`; 9359 passes 0, meaning "drained iff no residuals").

Cases by shape (fixed list): Record, literal-length FixedArray, Sum/Mixed only. Quote of the refusal: `_ => return None` (9009). A moved path through any other type shape makes the whole computation `None`.

Callers (grepped): `partial_affine_residuals` called from `structural::partial_affine_root_is_drained` (9359); `selected_affine_residuals` called from `cleanup` (3310) and from 8854. So the `None` result is consumed by two sites (read 9359 here; 3310 not read in my range).

Fail-open: the one place a `None` could be mistaken for success is 9358-9361: `partial_affine_residuals(..., 0).is_some_and(|r| r.is_empty())`. `None` yields `false` ("not drained"), which is fail-closed. Traced (read). Other: none seen.

Stated limits: none quoted.

## 4. `mod storage_regions` (9097-9241)

Purpose: turns an operation's structural path into the storage region it touches, and decides whether a type can hold on to a loan.

Shape:
- `projected` (9111, `pub(super)`): resolves constant runtime indexes in the path using all operations in the function (9118-9124). If no source contract, the result is `StorageRegion { path: [], indirect: true }` (9126-9129). Otherwise it calls `terminal_semantics::projected_storage_region`, and when the region is not indirect and the path has a `RuntimeIndex`, the region path is truncated to include the first runtime-index segment (9137-9143).
- `canonical_field` (9149): the same for a canonical field path; with no source contract it returns an empty path (9156).
- `written_or_exclusive_regions` (9166): maps `operation.storage_mutations()` to `projected` regions.
- `storage_cannot_retain_loan` (9183, `pub(crate)`): memoised recursion over the structural type graph. It inserts `false` before recursing, so a cycle counts as "can retain" (9191). Verdicts by shape (9195-9216):
  - `PrimitiveScalar` / `AtomicCell`: true.
  - `ByteSequence`: `!carrier.is_borrowed()`.
  - `Record` / `Sum` / `Mixed`: all fields recursively true.
  - `FixedArray`: element type.
  - `Reference` / `ElementView`: false.
  - A type id not in the map returns `false` (9192-9194).
- `fields_cannot_retain_loan` (9221): scalar, atomic, bounded and refined integers, IEEE floats are true; `Erased` is true only if the field relevance is erased.

Callers (grepped, whole `omega-rust`): `storage_cannot_retain_loan` has external callers: `unit_validation.rs:3371` (1 site), plus `cleanup`-side use at current_ownership.rs:3101/3106 and `call_accesses` import at 2816. `projected` / `canonical_field` are used inside `value_custody` (9647, 9662, 9698, 9744) and elsewhere in the file earlier (not recounted). `written_or_exclusive_regions` is imported at 2816 (call site not in my range).

How it decides: type shape and `carrier.is_borrowed()`. Memo is a `BTreeMap<StructuralTypeId, bool>` passed by the caller (not cached across calls).

Fail-open: 9192-9194 unknown type id yields `false`, which means "can retain a loan" (the cautious answer), so fail-closed for the loan check. Cycle seed `false` (9191) is likewise cautious. 9126-9129: no source contract yields an indirect, empty-path region, which overlaps everything (cautious). None seen that accept a wrong program.

Stated limits: none.

## 5. `mod structural` (9243-9443)

Purpose: structural-type lookups used by the replay: what type a place has, which places are partial-affine roots, and whether a set of moved paths consumes the whole root.

Shape:
- `place_structural_type` (9254): looks the place up in `structural_parameters` + block parameters, then the result, then `structural_places` by `kind`. `OperationResult`, `ByteSequenceLiteral` and `TrivialAffineLocal` give their type, `ProviderAttachment` gives `attachment`; `Parameter`, `BlockParameter` and `Result` give `None` (9292-9294). Caller: `cleanup` import at 3113.
- `partial_affine_root_type` (9299, `pub(crate)`): `None` if the place has an entry claim or content-entry claim (9303). For an arrival parameter it returns the type iff `is_unqualified_owned_affine()` (9306-9308). Otherwise the place must be `OperationResult` and a producing node must match (`psi_operation == producer`, same place and type, `Affine`, no qualifications, no projected qualifications, no claims; 9319-9347). The producing node families are `CallStructural` with a structural result, `BoundaryCall` with a structural result, `EstablishStructuralArray`, and the record/case establish pattern.
- `partial_affine_root_is_drained` (9352): `partial_affine_residuals(.., 0)` is `Some(empty)`.
- `projected_root_is_fully_consumed` (9364): false if any live claim names the place or the function declares a content-entry claim; false if there is no partial-custody entry; else `projected_paths_are_fully_consumed`.
- `projected_paths_are_fully_consumed` (9384, `pub(crate)`): false for a content-entry claim; for a partial-affine root, the drained test; else the place must be a structural parameter with `Linear` multiplicity and a `FixedArray` type of literal length, and `moved` must contain exactly `length` entries, each `[FixedIndex(i)]` for `0..length` (9399-9419).
- `constructor_arguments` (9422, `pub(crate)`): structural arguments of record/case fields (`RecordFieldValue::Structural`) and array elements.

Cases by shape: `projected_paths_are_fully_consumed` handles only (a) partial-affine roots and (b) a `Linear` parameter whose type is a literal-length `FixedArray` with every element moved as `[FixedIndex(i)]`. Everything else returns `false` (9396-9413). A comment-free fixed list.

Callers (grepped over `omega-rust`):
- `partial_affine_root_type`: `unit_validation.rs:11138`, plus this file's 9358, 9393.
- `partial_affine_root_is_drained`: `unit_validation.rs:872`, plus 9394.
- `projected_paths_are_fully_consumed`: `unit_validation.rs:715`, plus 9381.
- `constructor_arguments`: `unit_validation.rs:4140`, plus 7822. Note `terminal-psi/src/control_flow.rs:1585` defines its own method of the same name on a different type, used at `bytes.rs:540` and `emission_cleanup.rs:1018`. Same job on the Psi side (suspected: I did not read those bodies).
- `place_structural_type`: one use at 3113 import, call at 3223 area not read.
So the three `pub(crate)` re-exports at 9831-9834 are not dead; each has at least one external site.

Fail-open: `projected_root_is_fully_consumed` returning true removes the owned place from the frontier (8382, 8414). It is true only when `partial_affine_residuals` says the moved set leaves nothing, or all `FixedIndex` entries are moved on a `Linear` fixed array. A moved set the residual computation cannot describe gives `None` and so false (place stays owned and must still be disposed). Traced (read). None seen.

Stated limits: none.

## 6. `mod value_custody` (9444-9830)

Purpose: value-level custody checks on owned structural values: that an owned value being moved or observed is still there, and that any live claim frontier in its path is complete. It also installs a stored value into a destination window and relocates claims.

Shape:
- `ValueUse<'a>` (9458): `function`, `types: &[StructuralTypeDeclaration]`, `block`, `node`, `admitted_case: &dyn Fn(PlaceId, &[StructuralPathSegment], StructuralCaseId) -> bool`.
- `uninstalled_home` (9466, `pub(super)`): `Linear` owned place whose only missing path is the empty path.
- `refusal` / `refusal_at` (9477, 9481): build `CurrentOwnedValueUnavailable` with up to four path segments.
- `frontier` (9496): any live claim overlapping the path with multiplicity other than `Linear` errors (9507-9514). Otherwise `terminal_semantics::owned_structure::validate_owned_claim_frontier` with `admitted_case`.
- `moving` (9525): `Ok(())` if the root is not `Owned` or the path leaves inline storage (9534-9542). Otherwise it errors if any `partial_custody_paths` entry overlaps the path (9543-9557), then checks `frontier` at the runtime path tip.
- `observing` (9569): `Ok(())` if root is not `Owned` (9578). Errors if a non-`Unrestricted` root is not in `owned_places` with matching multiplicity and not fully consumed, or if absent paths overlap (9581-9603). Then walks each path prefix (skipping prefixes that end in a `Case` segment); the first prefix that leaves inline storage or has no tip returns `Ok(())`; the first tip that `owns_linear_claim` or is the full path checks `frontier` and returns (9604-9627).
- `observations` (9630): a match on operation shapes that observe owned storage: `EstablishReference`, `EstablishAccessReference`, `EstablishElementView`, `ElementViewLength`, `ElementViewSubslice`, `ByteSequenceLength`, `ByteSequenceSubslice`, `GlobalDescriptorTable`, `primitive_storage_observation_pattern`, `StructuralScalarFieldRead`, `StructuralCase`, `StructuralLeafCopy` with `Ordinary` path, `MoveStructuralField`, `byte_len_or_field_read_pattern`, `ElementViewRead`, `ByteSequenceRead`, `EstablishByteSequenceConcatenation`, `ByteSequenceEqual`, `ByteSequencePredicate`, `ReloadSegmentDescriptors`, `ActivateTaskRegister`. Final arm `_ => Ok(())` (9730).
- `byte_operand` (9734): resolves `ByteSequenceOperand::{Whole, Field}` to a region and calls `observing`.
- `install_store` (9754): computes the receiving window `hole` via `references::window_hole`, else `refusal` "store has no exact receiving window" (9763-9774). A destination is "owned" when it is not a parameter and the hole stays in inline storage (9775-9780). It rejects overwriting a live owned claim at an overlapping path (9787-9797). It moves the value's claims onto the destination with the path `hole + claim.path` (9798-9817) and, for an owned empty hole, clears the empty path from `partial_custody_paths` (9818-9826).

How it decides: keyed on `PlaceId`, structural path prefix/overlap, `StructuralAccess::Owned`, `StructuralMultiplicity`, and the type tip's `owns_linear_claim` flag. Case admission is delegated to the `admitted_case` callback, which the caller builds from `membership_at(..., false)` (7625).

Cases by shape: `observations` is a closed list (above); any operation not listed is not checked for observation of owned storage (`_ => Ok(())`, 9730). `StructuralLeafCopy` is only checked for the `Ordinary` path variant (9672-9676); other copy path variants are not checked here.

Fail-open (each one `Ok(())` accept path):
- 9530-9542 `moving`: `Ok(())` for any non-`Owned` root, or a path that leaves inline storage. Wrong program: moving out of a borrowed root or through an indirection is not checked here. It is presumably covered by the access checks (`check_root_access` etc., not in my range). "open".
- 9578-9580 `observing`: same for a non-`Owned` root. "open" (the borrowed-access checks are in `references`, outside my range).
- 9609-9622 `observing`: returns `Ok(())` as soon as a prefix leaves inline storage or has no tip, without checking deeper claims. A claim frontier below an indirection is not checked by this function. "open".
- 9730 `observations` `_ => Ok(())`: an operation that reads an owned value but is not in the list gets no moved/partial check here. Which ones exist was not enumerated. "open". One related fact: `StructuralLeafCopy` with a non-`Ordinary` path falls into this arm (9672-9676 only matches `Ordinary`).
- 9799-9816 `install_store`: when `owned` is false (the destination is a parameter, or the hole leaves inline storage), moved claims are removed from the frontier and not reinserted (`current.claims.remove` then `if owned`). The claims are dropped. Wrong program this could let through: a stored value's claims vanish when the destination is a borrowed parameter. "open"; I did not trace whether the call at 8427 only reaches this for destinations where that is intended.

Callers (grepped): `uninstalled_home` called at 3388 and 7673 (2 sites). `ValueUse` constructed once, at 7628. `install_store` called once, at 8427. All in this file.

Stated limits: none.

## 7. Top-level items (9836-9965)

- Re-exports (9831-9834, 9836, 9965): `constructor_arguments`, `partial_affine_root_is_drained`, `partial_affine_root_type`, `projected_paths_are_fully_consumed`, `parameter_establishment_order`, and `validate_descriptor_compatibility` (alias of `address_joins::validate_compatibility`). The earlier re-export of `storage_cannot_retain_loan` is at 9242. External-use counts are in section 5.
- `validate_current_ownership_frontier_collecting` (9839, `pub(super)`): builds `reconstruct_entry_ownership` and calls `replay::validate_current_ownership_cfg`. One caller: `unit_validation.rs:9660` (grepped).
- `LiveClaim` (9872): `input: Option<PlaceId>`, `path`, `multiplicity: Option<StructuralMultiplicity>`.
- `CurrentOwnership` (9879): `claims: BTreeMap<ClaimId, LiveClaim>`, `owned_places: BTreeMap<PlaceId, StructuralMultiplicity>`, `partial_custody_paths: BTreeMap<PlaceId, BTreeSet<Vec<StructuralPathSegment>>>`, `live_references: Vec<LiveReference>`. `snapshot()` (9887) copies the first three into an `OwnershipFrontierSnapshot` and leaves `live_references` OUT (9888-9915). So the recorded facts do not carry live references. Note the join comparison at 8810 does compare `live_references`.
- `reconstruct_entry_ownership` (9919): claims from `entry_claim_declarations` (path empty gives the parameter's multiplicity, else `Linear`, 9933-9937) and `content_entry_claims` (`or_insert`, so a content claim never overrides a declared claim, 9943). `.expect("structural signature validation precedes current ownership replay")` at 9927 panics if a declared claim names a non-parameter. In a content-entry claim a non-parameter root yields `input: None` and `multiplicity: None` (9944-9946). Owned places are non-`Unrestricted` `Owned` parameters (9951-9959).

How it decides: entry claims and owned places are keyed on `PlaceId` and `access == Owned` and non-`Unrestricted` multiplicity.

Fail-open: a content-entry claim whose root is not a structural parameter is inserted with `input: None` and `multiplicity: None` (9943-9947) rather than erroring. Wrong program: a content claim naming a non-parameter root is recorded as a claim with no owner place, and later checks that filter by `claim.input == Some(place)` (e.g. 8063, 8456, 8650) will not see it, while 8680-8682 (`Crash`) compares the whole claim key set. Whether such a unit can pass earlier validation was not traced. "open".

## Same job elsewhere (cross-range)

- The "expected claim paths vs live claim paths" reconstruction at 8039-8086 duplicates in shape the receipt check at 8107-8137 (same file, same function).
- `candidates.rs:7461` `reconstruct_linear_thread_ownership_is_identity` and `candidates.rs:6847` consume the snapshots this file produces (`OwnershipFrontierFact`); the snapshot omits `live_references` (9888-9915), so those identity tests cannot see reference-loan changes (inferred from the earlier notes' description, not re-read).

## Unused or one-caller code (greps run)

Command: Grep, count/content mode, over `omega-rust`, pattern `storage_cannot_retain_loan|partial_affine_root_is_drained|partial_affine_root_type|projected_paths_are_fully_consumed|constructor_arguments|validate_descriptor_compatibility|parameter_establishment_order`: 32 occurrences in 5 files (this file 23, `unit_validation.rs` 6, three Psi-side files 1 each; the Psi hits are a different `constructor_arguments` method). Every `pub(crate)` item in 9242-9965 has at least one call site outside this file's definitions, in `unit_validation.rs` (lines 715, 872, 1250, 3371, 4140, 11138). `parameter_establishment_order` has one in-file caller (3223) and no caller outside the file (its only other occurrences are the definition, the import at 3114, and the re-export at 9836).

Command: Grep, content mode, pattern `validate_current_ownership_frontier_collecting|reconstruct_entry_ownership|uninstalled_home|validate_return_coverage|validate_normal_return|transfer_return|selected_affine_residuals|validate_atomic_access`: one external caller for the entry point (`unit_validation.rs:9660`); every other function has 1-2 in-file call sites (listed per section above).

Comment claims of "single/shared/canonical": none in my range.

## Stated limits (whole range)

No comments in 7101-9965 say "unsupported", "remains", "TODO" or "migration debt" (read all lines). Limits are encoded as refusals only. The one comment in the range is `#[allow(clippy::too_many_arguments)]` at 7521 and 7559.

## Coverage

- Read in full: `current_ownership.rs` 7101-9965 = 2865 lines.
- Skimmed: none.
- Not read: none.
- Total 2865 = 2865 read + 0 not read.
- Extra lines read outside the range only via grep output of `fn`/`mod`/`use` header lines (to find enclosing modules); no body outside the range was read.

## Five things a maintainer most needs to know about this range

1. `replay::validate_current_ownership_cfg` (7560-8838) is the only place that walks the CFG forward. Each block is entered once, with the frontier from its first predecessor, and later predecessors must equal it exactly (8801-8818), so any join with differing ownership state is an error, not a merge.
2. Snapshots (`CurrentOwnership::snapshot`, 9887) record claims, owned places and partial custody but not `live_references`, even though joins compare `live_references` (8810). The recorded `OwnershipFrontierFact` rows used by `candidates.rs` therefore carry no reference state.
3. Which operations are checked is decided by closed `match` lists, not a table: structural-argument and multiplicity extraction (7831-7910), the terminator match (8567-8692), and `ValueUse::observations` (9635-9730, `_ => Ok(())`). A new operation that touches owned storage is unchecked unless added to these lists by hand.
4. Partial-move accounting only works for three shapes: `Record`, literal-length `FixedArray`, `Sum`/`Mixed` (`residuals`, 8903-9012), plus the literal-length `Linear` fixed-array parameter special case (9402-9419). Any other moved-path shape makes the residual computation return `None`, which the callers treat as "not drained / not fully consumed".
5. `storage_cannot_retain_loan` (9183) seeds `false` before recursing (9191), so recursive types and unknown ids count as "can retain a loan". It is used outside this file at `unit_validation.rs:3371`.

## What I am unsure about

- Whether `AtomicEvent::location()` returning `None` (7458-7460) is reachable from a validated unit; I did not read that function.
- Whether `install_store` dropping moved claims when `owned` is false (9798-9817) is the intended behaviour for borrowed-parameter destinations; I did not read the callers' earlier checks (`window_hole`, `check_root_access`).
- Whether `moving`/`observing` returning `Ok(())` for non-`Owned` roots (9534, 9578) is backed by access checks in `references` (outside my range).
- The full list of operations that reach `observations`' `_ => Ok(())` arm (9730).
- Whether the Psi-side `constructor_arguments` and residual helpers (control_flow.rs:1585, value_custody_core.rs, terminal-psi semantics structural.rs) are true duplicates; only grep hit counts were seen.
<!-- END -->
