Second pass for unit 26

Ranges covered (paths relative to `<checkout>/`):

1. `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs` lines 5751-11750 (6000 lines).

Short prefix: `UV` = that file. Every `file:line` below is in `UV` unless another path is given.

The range starts inside `function_structure::control_flow::validate_total_cfg` (header at `UV:5745`, body from 5751). It ends inside `operation_contracts::scalar_types::element_views` (module opens `UV:11748`; the `use` lines at 11749-11752 are the last lines of the range, and `types_match` starts at 11754, outside it). I read to 11764 for context; those extra lines are not counted.

Contradiction with the earlier notes (`unit_026.md`): the earlier notes say `validate_total_cfg` was cut off after its signature (`unit_026.md` Coverage). Its body is `UV:5751-5804` and is recorded here. The earlier module map (`activation_storage` ... `function_structure` 2740, `identity_indexes` 9675, `no_normal_calls` 9732, `operation_contracts` 9825, `validate_function` 9546) matches what I saw. The earlier notes say `function_structure` runs "to about 9675"; it ends at `UV:9674`.

No judgment in this range is keyed on `display_name`, labels, string compare or `format!` of a debug value. Every decision is keyed on ids (`PlaceId`, `BlockId`, `MachineId`, `OperationId`, `StructuralTypeId`, `ValueId`, `ClaimId`), on positions (block order, node index, parameter position, argument index) or on type-declaration shapes. The only strings are `&'static str` reason texts in `InvalidOwnedValueTransfer` (`UV:8104-8143`) and `InvalidNoNormalInvocation` (`UV:9751, 9768, 9772, 9786, 9817`).

---

## A. `function_structure::control_flow` tail (`UV:5751-5823`)

**Purpose.** Finishes `validate_total_cfg`: every block is reachable from entry, and (unless cycles are admitted) the block graph is acyclic. Adds `block_reaches`, a reachability query.

**Shape.** `validate_total_cfg` (`UV:5745`, `pub(crate)`) and `block_reaches` (`UV:5806`, `pub(crate)`).

**Inputs/outputs.** Reads `function.entry`, the block map and a successor map. Returns `Ok(())` or `UnreachableBlock{machine,block}` (`UV:5764`) or `ControlCycle{machine,block}` (`UV:5798`). `block_reaches` returns `bool`.

**How it decides.** Reachability is a DFS from `function.entry` (`UV:5751-5757`); unequal counts is an error. Cycle test is Kahn's algorithm over in-degrees (`UV:5770-5792`); `visited != blocks.len() && !admit_cycles` is the error (`UV:5793`). Keyed on `BlockId`.

**Cases by shape.** None.

**Same job elsewhere.** Successor-reachability DFS is written in `block_reaches` (`UV:5806-5822`) and again inline in `validate_total_cfg` (`UV:5751-5757`). Same job as `dominators(...)` use at `UV:6718` is different (dominance, not reachability).

**Unused/one-caller.** `grep -c "\bvalidate_total_cfg\b"` over `omega-rust`: 2 occurrences, both in `UV` (definition plus the call at `UV:9595`). `grep -c "\bblock_reaches\b"`: 6 occurrences, all in `UV` (definition, import at `UV:6411`, uses at `UV:6563, 6564, 6582, 6583`).

**Stated limits.** None.

**Fail-open.** `admit_cycles == true` skips the cycle error entirely (`UV:5793`). It comes from `cycle_policy.admits(function.machine)` at `UV:9599`; the policy set is `ControlCyclePolicy.admitted_machines` (`UV:5614-5626`, read in first pass). Which machines are admitted is decided by the entry points at `UV:18229/18260`, not read: **open**. `.expect(...)` at 5763, 5776, 5786, 5797 are panics, not accepts.

---

## B. `fact_index` (`UV:5824-5992`)

**Purpose.** Rebuilds the list of "facts" a function should have (obligation references and boolean/integer constants) from its operations and requires the stored list to equal it.

**Shape.** `validate_fact_index` (`UV:5828`) compares `reconstruct_fact_index(function)` (`UV:5840`, `pub(crate)`, re-exported `UV:9544`) to `function.facts` with `!=` (`UV:5831-5836`). Error `FactIndexMismatch(machine)`.

**How it decides.** Pure recomputation. One closed list of operations gives `OperationObligationReference{obligation, support: psi_operation}` (`UV:5848-5944`): scalar field store, byte-view field store, record/case field initializers with a `range_obligation`, byte concatenation capacity, byte subslice/write/store pair/field byte store/read/field read, element view subslice, element view regions `RepresentationBytes`/`WholeRepresentation`, element read/write pair, `IntegerExactCast`, `IntegerShift` with `Exact` policy, `IntegerArithmetic` with `arithmetic.obligation()`. Then every operation with a psi operation adds one fact per `runtime_indices()` entry (`UV:5959-5966`). Then `BooleanConstant` and `IntegerConstant` add constant facts (`UV:5967-5988`). Both match blocks end `_ => {}` (`UV:5957, 5987`). Order of output is block order, then operation order; equality is order-sensitive.

**Cases by shape.** Fixed lists as above; operations outside them contribute nothing unless they have `runtime_indices()`.

**Same job elsewhere.** The set of obligation-carrying operations is the same family that `catalogs::validate_structural_field_range_authority` (read in first pass, `unit_026.md` 4.4) walks to demand accepted facts. Two lists of "operations with obligations" therefore exist (`UV:5848-5944` here; first-pass `UV:1482-2117`). Whether they agree: not compared.

**Unused/one-caller.** `reconstruct_fact_index` is `pub(crate)` and re-exported (`UV:9544`); a `grep -c` of `reconstruct_fact_index|constructible_record|boundary_requirements_match|boundary_completion_matches` over `omega-rust` gave 11 occurrences, all in `UV` (the four names combined; I did not split per name). `validate_fact_index` is called once at `UV:9646`.

**Stated limits.** None.

**Fail-open.** Not an accept path: any operation not listed is not expected to have a fact, and an extra stored fact fails equality. None seen.

---

## C. `mutable_views` (`UV:5993-6340`)

**Purpose.** Tracks which mutable (exclusive) borrow places are still available at each block's arrival, and rejects operations or edge bindings that use a mutable place that has been retired.

**Shape.**
- `exclusive_access` (`UV:6002`): `matches!(access, terminal_psi::exclusive_borrow_pair_pattern!())` (macro defined elsewhere).
- `CurrentMutableBorrows { mutable, arrivals, possible_arrivals }` (`UV:6006`), `may_be_live(block, place)` (`UV:6013`) reads `possible_arrivals`.
- `MutableBorrowTransfer { source, target, retired, generated }` (`UV:6020`), `prepare_transfers` (`UV:6027`).
- `current_mutable_borrows` (`UV:6092`, `pub(crate)`, re-exported `UV:6340`): fixpoint loop (`UV:6198-6250`).
- `validate` (`UV:6258`, `pub(super)`).

**Inputs/outputs.** Reads the function's structural parameters, block parameters, node results and edge `structural_bindings`, plus the type table. Produces the two arrival maps. `validate` returns `StructuralPlaceNotAvailable(site, place)`.

**How it decides.**
- "Mutable" places: a parameter or block parameter with exclusive access whose type is a shared-view declaration or an address join (`UV:6120-6137`), plus results of operations whose result contract is exclusive and whose type is a shared view (`UV:6096-6119`). Keyed on `PlaceId` and `StructuralTypeId` declaration shape.
- Per edge, a mutable argument is retired if `argument.path.is_empty()` and (it is a "descriptor view" or an attenuated shared block name, or has exclusive access) (`UV:6062-6069`). Everything produced in the source block and not retired is "generated" into the target (`UV:6071-6080`).
- Two arrival sets are iterated together: `arrivals` (must-arrive, intersection style: starts at all-`mutable` for non-entry blocks and shrinks, `UV:6176-6181, 6229-6233`) and `possible_arrivals` (may-arrive, union style, `UV:6235-6243`). Loop stops when neither changes (`UV:6245`).
- `validate`: for each node, each place in `operation_place_accesses(op)` that is mutable must be in `available` (`UV:6277-6282`); two bindings of the same mutable place on one edge conflict unless both shared or the paths cannot overlap (`UV:6283-6308`, uses `terminal_semantics::structural_paths_may_overlap`); a node's mutable result place becomes available (`UV:6309-6313`). `ByteSequenceWrite` must be preceded in the same block by a `ByteSequenceLength` on the same destination and value, with no intervening operation that has place inputs other than `ByteSequenceLength`/`ByteSequenceWrite` (`UV:6314-6334`).

**Cases by shape.** `ByteSequenceWrite` length-observation rule is local to one block (`rposition` in `block.nodes[..position]`, `UV:6320`). A mutable place is only tracked when `shared_view_declaration` or `is_address_join` holds (`UV:6126-6133`).

**Same job elsewhere.** The `ByteSequenceLength`-before-write requirement is also enforced in `byte_views::validate_source` (first pass `unit_026.md` 4.6, `UV:4852-4858`), which only checks existence of a length node anywhere in the function. This copy (`UV:6320-6333`) is the position-sensitive check. Two checks of the same fact with different strength.

**Unused/one-caller.** `current_mutable_borrows`: `grep -c` over `omega-rust` gave 3 occurrences in `UV` (definition, re-export `UV:6340`, call at `UV:6264`) and 1 in `current_ownership.rs` (an outside use).

**Stated limits.** None.

**Fail-open.**
- `UV:6045-6047`: `prepare_transfers` skips an edge whose target block is missing (`continue`). Traced: `control_flow::validate_nodes_and_edges` (first pass, `UV:5708-5719`) already rejects unknown targets and `validate_function` runs it first (`UV:9594`), so unreachable with a bad edge.
- `UV:6265-6267`: when no mutable places exist the whole check returns `Ok`. Intended (nothing to track), traced to `UV:6138-6144`.
- `UV:6286-6288`: bindings of non-mutable places are not checked here. Not an accept of a bad mutable use.
- `block_names` etc. use `produced[&source.id]` (`UV:6071`) and `arrivals[&block.id]` (`UV:6269`) as indexing, which would panic, not accept.

---

## D. `parameters`, `provenance`, `results` (`UV:6341-6638`)

**`parameters`** (`UV:6341-6404`). `validate_entry_claim_index` (`UV:6346`): the set of claims in `entry_claim_declarations` must be duplicate-free and equal `function.entry_claims` (`UV:6349-6356`). `validate_parameter_metadata` (`UV:6364`): each function parameter's `site` equals `FunctionParameter(position)`, each block parameter's `site` equals `BlockParameter{block,position}` (`UV:6374, 6389`). Position overflow of `u32` is an error, not an accept. Fail-open: none seen.

**`provenance`** (`UV:6405-6596`), `validate_provenance_work_effects` (`UV:6413`).
- Purpose: every node/edge carries provenance sites, a work settlement of exactly 1 unit per site, and a linear effect-chain number; sites may repeat only in limited cases.
- How it decides: a node with empty provenance and no successors is `IncompleteProvenance` (`UV:6423`); duplicate sites inside a node are `DuplicateProvenance` (`UV:6428-6437`); `node.work` must list exactly the node's provenance sites in order with `units == 1` (`UV:6461-6474`); each edge's provenance must start with `PsiProvenance::Edge(edge.psi_edge)` and contain only `Edge` entries (`UV:6480-6490`), `edge.work` must mirror it (`UV:6504-6518`). Effect chain: `node.effect.input == expected_effect && output == expected_effect + 1`, counter increases by one per node over blocks in stored order (`UV:6526-6535`). After the scan: a node-provenance site occurring in two or more nodes is allowed only if it is an `Edge` site and every occurrence is an "exact terminal" (a return/return-unit/return-structural/crash with no successors, `UV:6438-6443`), and no two occurrences are co-executable (same block, or one block reaching the other, `UV:6551-6574`). Edge-provenance sites repeated across edges are rejected if one edge's target reaches the other edge's owner block (`UV:6575-6593`).
- Cases by shape: the "exact terminal" list (`UV:6438-6443`, via `return_or_return_unit_pattern!` and `return_structural_or_crash_pattern!`).
- Dead code: `for settlement in &node.work { let _ = settlement; }` (`UV:6475-6477`) does nothing.
- Same job elsewhere: first-pass `derived_metadata::provenance_matches_operation` (`unit_026.md` 4.5) checks the same node provenance against a recomputed expectation, and accepts extra trailing provenance (`starts_with`). This function accepts only a limited extra: duplicates across nodes only for terminal `Edge` sites. They check different sides.
- Fail-open: `UV:6552-6554` `occurrences.len() < 2` skips the co-executability check for sites occurring once; intended (nothing to compare). `UV:6411` `block_reaches` over `successors` built at `UV:6538-6550`. None seen.

**`results`** (`UV:6597-6638`), `validate_function_results` (`UV:6601`). For each `Return`, `ReturnUnit`, `ReturnStructural` the function's declared `result` kind must match (`UV:6605-6627`). For `Return`, the value id and scalar type must equal the declared signature (`UV:6613`). For structural results only the kind is compared here (`UV:6618-6621`); the structural contract is checked in `structural_roots` at `UV:8367`. Other operations `continue` (`UV:6628`). Fail-open: none seen.

---

## E. `structural_roots` (`UV:6639-8961`)

**Purpose.** The largest per-function validator for structural (place-based) operations: checks that each operation that reads/writes/moves structural storage has an exactly matching source contract, result shape, memberships (qualification domains) and place kind.

**Shape.**
- Submodules: `availability` (`UV:6671`), `primitive_locals` (`UV:7000`).
- Free functions: `place_kinds_operation_result` (`UV:7140`), `ReferenceMembershipValidation` (`UV:7154`, with `graph` and `analysis`), `validate_structural_root_uniqueness` (`UV:7173`), `validate_structural_root_operations` (`UV:7190-8511`, one 1300-line function with one `match &node.operation`), then helpers `observed_scalar_field` (`UV:8513`), `relevant_scalar_field` (`UV:8545`), `readable_field_type` (`UV:8579`), `borrowed_window_root` (`UV:8629`), `root_window_loan` (`UV:8698`), `window_field_type` (`UV:8708`), `window_memberships` (`UV:8731`), `reference_result_memberships_match` (`UV:8754`), `view_memberships_match` (`UV:8822`), `copy_memberships_match` (`UV:8896`).

### E1. `availability` (`UV:6671-6999`)

- `validate_structural_place_availability` (`UV:6681`): builds `producers: PlaceId -> (block, Option<node>)` from block structural parameters and from each node's structural result contract (plus `byte_literal_establish_pair_pattern!` places) (`UV:6686-6717`). Then for every node, each place in `operation_place_inputs` that has a producer must be available: same block and earlier node (or the producer is a block parameter, `producer_node.is_none_or(...)`), or the producer block dominates the use block (`UV:6728-6741`). Keyed on `PlaceId`, `BlockId`, dominator sets.
- `operation_place_accesses` (`UV:6748`, exhaustive `match` over all `AbstractOperation` variants, no wildcard; the last arm lists place-free operations `UV:6878-6913`), `operation_place_inputs` (`UV:6917`) = accesses plus cleanup-roster action places (`UV:6919-6921`). Re-exported at `UV:7138` and `UV:8962-8964`. Two unit tests at `UV:6925-6998`.
- Fail-open: `UV:6724-6727`: a place with no recorded producer is `continue`d (skipped). Wrong program that gets through: an operation reading a place that nothing produces (not a parameter, block parameter or operation result). Traced partially: `producers` is built only from structural results and byte-literal places; parameters of the function entry (`function.structural_parameters`) are not inserted into `producers` (only block structural parameters, `UV:6688`), so entry parameters are skipped on purpose; whether an undeclared place is caught elsewhere is by `derived_metadata::validate_operation_places` (first pass `UV:2468`, closed list) and `validate_structural_root_uniqueness`; not traced to completion: **open**.

### E2. `primitive_locals::validate` (`UV:7012-7113`)

Checks `EstablishPrimitiveLocal`, `PrimitiveLocalStore`, `PrimitiveScalarRead`. A local must be `Unrestricted`, claim-free and of a primitive scalar type matching the value (`UV:7029-7036`); a store needs the local to have no qualifications and no scalar restriction (`UV:7059-7065`); a read is allowed from a local with empty path, from a projected local with a path, or from a reading-borrow / owned-with-path parameter (`UV:7066-7102`). Error `InvalidPrimitiveLocal`. `local(place)` finds the first `EstablishPrimitiveLocal` for that place anywhere in the function (`UV:7037-7046`; function-wide, not flow-sensitive). `_ => true` at `UV:7104` for every other operation. Helper `projected_local` (`UV:7115`).

### E3. `validate_structural_root_operations` (`UV:7190-8511`)

**Inputs.** Unit, function, type and domain maps, predecessors, an optional mutable `ReferenceMembershipValidation` (descriptor analysis), and an optional descriptor graph `view_graph`. Output `Result<(), Error>`; most failures are the generic `catalog_mismatch(function)` (`UV:7315` and 30+ other sites).

**Preamble decisions (`UV:7202-7241`).**
- `shared_descriptor_result`: function returns an `Unrestricted` structural result whose type is a shared descriptor.
- `aggregate_origins_valid`: `None` slots (no structural result) is true; `Some(None)` (could not compute slots) is false; `Some(Some(slots))` is true if empty, else depends on `owned_aggregate_descriptor_returns_with_graph` (`UV:7216-7226`).
- `descriptor_origins_valid` requires a graph and `reconstruct_result_origins(...).is_some()` when the result is a shared descriptor (`UV:7227-7236`).
- `exact_store_destination` / `exact_scalar_store_destination` (`UV:7242-7297`): destination parameter must be the function's own arrival parameter (`function.arrival_parameter(place) == Some(destination)`), a block parameter only with Mutable/WriteOnly access; `OwnedResult` must match both the place-kind table and an actual producing node; `OwnedBlockParameter` must not be in the entry block and must equal the block's declared parameter at that position.

**Per-node decisions (`match &node.operation`, `UV:7324-8507`).** Before the match, each node must pass `view_memberships_match`, `reference_result_memberships_match` (else `catalog_mismatch`, `UV:7301-7316`) and `primitive_locals::validate` (`UV:7317`). Then the arms:

| Operation | What it requires (citation) |
|---|---|
| `EstablishActivationClaim` | rebuilds a `terminal_psi::Operation` and calls `activation_claim_shape_in` (`UV:7334-7351`); place kind must be this op's result (`UV:7352`) |
| `StructuralByteSequenceFieldByteStore` | `byte_field_store_capacity` is `Some` (`UV:7356-7366`) |
| byte store pair | `validate_byte_view_source` and capacity (`UV:7367-7386`) |
| `StructuralByteViewFieldStore` | `byte_view_field_store_is_valid` (`UV:7387-7396`) |
| byte length / field read pattern | geometry via `record_field_destination` for `Referent` paths else `byte_field_root`; destination not `WriteOnlyBorrow` unless it is a field-length op; static scalar-store path; the carrier is a record field, not erased, and `sequence_field_allows_observation` (`UV:7397-7449`) |
| `StructuralCaseMembership` | result is Boolean, source not `WriteOnlyBorrow`, resolved path endpoint is a `Sum`/`Mixed` containing the case (`UV:7451-7484`); error `InvalidStructuralCaseDispatch` |
| `EstablishElementView` `WholeRepresentation` | `valid_representation_count` (`UV:7485-7503`) |
| byte/element subslice and length | `descriptor_sources::valid` (`UV:7504-7516`) |
| `ByteSequenceEqual`, `ByteSequencePredicate` | `Whole` operands pass `valid_whole`; `Field` operands are not examined here (`UV:7517-7550`) |
| `ByteSequenceRead`/`Write` | `validate_byte_view_source` (`UV:7551-7572`) |
| `WriteOnlyPrimitiveStore` | exact destination, `primitive_store_restriction` is `Some`, multiplicity Unrestricted or (non-empty path and Affine), destination not entry-claim declared, projected scalar type equals value type (`UV:7573-7609`); `InvalidWriteOnlyPrimitiveStore` |
| `StructuralScalarFieldStore` | exact destination, `scalar_field_store_preserves_domain_content`, `scalar_field_store_preserves_content`, field declaration scalar type equals value type, `scalar_field_requires_evidence(field_type) == range_obligation.is_some()` (`UV:7610-7677`); `InvalidStructuralScalarFieldStore` |
| `StructuralScalarFieldRead` | result is Boolean or Integer and `observed_scalar_field(...) == Some(result.scalar_type)` (`UV:7678-7723`); `InvalidStructuralScalarFieldRead` |
| `EstablishAccessReference` | not a `BorrowedExtent` backing; exactly one access-view plan per carrier (`plans.next()`, `plans.next()` is `None`); `validate_backing` ok; result affine-plain; place kind; source access Shared/Mutable; source type compatible; leaf access within (`UV:7724-7777`) |
| `EstablishReference` | source ancestors preserved, byte-field compatibility or captured memberships cover the result's projected qualifications, result Affine, no qualifications, no claims, source access Shared/Mutable, storage-source compatible (`UV:7778-7941`) |
| `ReleaseReference` | empty path or an `Owned`/`Affine`/unqualified source, path segments only `Field`/`FixedIndex`, endpoint is a `Reference` type (`UV:7942-7975`) |
| `MoveStructuralField` | `borrowed_window_root` with MutableBorrow, `window_field_type` equals result type, `root_window_loan`, window memberships equal the result's, place kind (`UV:7976-8019`) |
| `StoreStructuralField` | window root with the disposition's required access, either whole-owned store or loan, value `Owned` with empty path, field type equals the value's source type, `discardable_store_value` unless `Vacant`, memberships supplied (`UV:8020-8092`) |
| `MoveStructuralValue` / `TransformContent` | see below |
| `StructuralLeafCopy` | see below |
| `ReturnStructural` | see below |
| `_ => {}` | `UV:8506`: all other operations are not checked by this function |

**`MoveStructuralValue` / `TransformContent` (`UV:8093-8282`).** Source and signature access must both be `Owned`; the source place must differ from the result; the canonical static projection of the source path must have the result's type; result multiplicity must not be `Unrestricted` and neither must the source's; for a whole move (empty path) multiplicities must be equal; no establishment bindings on the result (`UV:8118-8144`). Requires `view_graph` (`UV:8145`), else error "descriptor flow graph is malformed". The selected payload contract (`current_selected_payload_contract`) must have the result's carrier and every source qualification must be present on the result (`UV:8147-8174`). For `TransformContent`: `validate_contract`, `validate_memberships`, and per-parent `validate_parent_memberships` with a closure that calls `membership_on_every_path` (`UV:8175-8245`), then `continue` (`UV:8246`) so the generic membership-addition test below is skipped for transforms. For a plain move: every membership the result adds must have issuer requirements supplied (`missing_membership_requirements` is `Some` and no missing node has establishment routes) (`UV:8248-8281`).

**`StructuralLeafCopy` (`UV:8283-8366`).** Source contract must exist and not be `WriteOnlyBorrow`; result has no establishment bindings. `Ordinary(path)` with empty path and same type: multiplicities equal and the source has no result evidence; other `Ordinary` paths are accepted by this clause (`UV:8321`) and gated by the second `match` (`resolve_leaf_copy_path == expected`, or `shared_descriptor_copy_matches`, or `licensed_copy` for empty path, or `byte_carrier_leaf_window`). `Case(path)`: result has no qualifications and `canonical_case_leaf_type` equals the result type. Plus multiplicity must be `Unrestricted`, or an owned case copy (affine, non-reference result), or a shared-descriptor copy (`is_copyable_carrier`, affine), plus `copy_memberships_match`, no claims, place kind (`UV:8347-8362`).

**`ReturnStructural` (`UV:8367-8505`).** The function must declare a structural result. The source's contract is chosen from four places in order: a function parameter whose place kind is `Parameter{position}` and equals the parameter; else a block parameter (owned access, or any access when `shared_descriptor_result`); else a byte-sequence literal place (Unrestricted, no qualifications); else an operation-result place whose producing node matches and which is `Owned` (unless `shared_descriptor_result`) (`UV:8375-8468`). Then type and multiplicity must equal the declared signature; if the declared result is a bounded owned byte sequence, qualifications must equal exactly and none may be minted, otherwise every non-minted qualification must be in the signature and projected qualifications must `reference_result_memberships_weaken` into the signature's (`UV:8469-8497`). `descriptor_origins_valid` must hold (`UV:8474`).

**Cases by shape.** Closed `match` with `_ => {}` (`UV:8506`); closed lists inside `record_field_destination` calls (`Referent` first segment, `UV:7404`). `exact_scalar_store_destination` handles exactly three destination kinds (`UV:7255-7296`). `ReleaseReference` allows only `Field`/`FixedIndex` path segments (`UV:7951-7957`). `window_memberships` etc. are by type shape.

**Same job elsewhere.**
- `relevant_scalar_field` (`UV:8545`) and the `Record` field lookup in the byte-length arm (`UV:7432-7445`) are two field-existence-and-not-erased lookups. A third is in `byte_field_freshness::field_carrier` / `byte_concatenation` (first pass, `UV:2828-2857`).
- `ReturnStructural` source-contract selection (`UV:8375-8468`) duplicates the structural-source-contract resolution that `operation_contracts::structural_source_contract` (`lib.rs:99`, defined in `UV` after 11750, not read) performs for every place. This function does not call it for `ReturnStructural`, it re-derives the contract from `place_kinds`.
- `exact_scalar_store_destination` repeats the arrival-parameter/place-kind check used in `structural_transfers` (`UV:8996-9033`, `function.structural_places.iter().any(|place| place.id == parameter.place && place.kind == BlockParameter{...})`).
- `reference_result_memberships_match` (`UV:8754-8820`) and `view_memberships_match` (`UV:8822-8894`) and `copy_memberships_match` (`UV:8896-8960`) are three membership-subset checks with the same final shape (result's qualifications and projected rows must be contained in an evidence set computed via `terminal_semantics::descriptor_results::*`); each builds its own `DescriptorAnalysis`.

**Unused/one-caller.** `current_selected_payload_contract` and many `terminal_semantics::*` functions are outside the unit. In `UV`: `validate_structural_root_operations` is called once (`UV:9610`); `validate_structural_root_uniqueness` once (`UV:9588`); `place_kinds_operation_result` is `pub(crate)` with many in-file uses. `operation_place_accesses`/`operation_place_inputs`: not grepped for external callers (the first-pass notes cite `lib.rs:89, 95` wrappers).

**Stated limits.** `UV:8142` error text only: "transfer requires the same owned carrier, multiplicity and fresh result without issuance". No migration-debt comments in this range's code (the only comments are the `//` on test names and `UV:6320` inline).

**Fail-open risks (traced where possible).**
1. `UV:8506` `_ => {}`: operations not listed are not checked here. These are covered by other phases or have no structural contract; not traced for each variant: **open**.
2. `UV:7517-7550`: `ByteSequenceEqual`/`ByteSequencePredicate` operands of kind `Field` are skipped (`if let ... Whole {..} = operand && !valid_whole(...)`). Wrong program that could pass: a `Field` operand whose field/length does not exist or is erased. Possibly checked in `byte_field_freshness::validate` (first pass, `UV:2942-2975`, only for `StructuralByteSequenceFieldByteStore`/`FieldRead`), not for equal/predicate operands: **open**. I did not find a check for `Field` operand validity of `ByteSequenceEqual`/`ByteSequencePredicate` in the ranges I read; `byte_concatenation` (first pass) does check `Field` operands, but only for concatenation.
3. `UV:8246` `continue` after `TransformContent` skips the generic added-membership requirement check (`UV:8248-8281`). It runs `validate_memberships` and `validate_parent_memberships` instead; the replacement is a different check. Stated by structure only: **open**.
4. `UV:8770-8772`: `reference_result_memberships_match` returns `true` when the call's callee is not a function in this unit (`callee.machine().and_then(|m| unit.function(m))` is `None`, includes `Dynamic` callees). A call to an external or dynamic callee whose result carries projected qualifications is accepted by this check. Traced: only this function reads `callee` here; whether another phase (`operation_structural_call_contract_matches`, after line 11750, not read) rejects it: **open**.
5. `UV:8776-8782`: also `true` when every result projected row is contained in the callee's signature; intended.
6. `UV:8835-8837` `view_memberships_match` returns `true` when the view result has no projected qualifications; intended (nothing to prove). `UV:8868-8870` `Empty` evidence returns `true`: the element payload carries no memberships, so the result has to carry none either; but note it returns `true` even though `result.projected_qualifications` is non-empty (the early `return true` at 8836 is for empty only). So a view result that claims projected qualifications while the analysis finds `Empty` payload evidence is accepted. Open: the `Empty` variant's definition lives in `terminal_semantics`, not read.
7. `UV:7840-7844`, `UV:7845`: for `EstablishReference` the `expected` referent lookup falls back to a `Reference` shape's referent; only checked as `is_some()`, the referent is not compared to anything: `expected.is_some()` (`UV:7845`). The `Some` value is never used. A reference result type whose referent type does not match the source is caught by `reference_source_compatible`; stated by the code, not by a comment.
8. `UV:7871-7880`: when no `reference_memberships` context is supplied, `captured` stays the declared-memberships set only; for a `SharedBorrow` source that then needs the graph, `UV:7884` returns `false`. Fail-closed.
9. `UV:6724-6727` (see E1).
10. `UV:7104` `_ => true` in `primitive_locals::validate`: operations other than the three are not checked there.

---

## F. `structural_transfers` (`UV:8965-9542`)

**Purpose.** Checks block structural parameters and every edge's structural bindings: that each binding's argument can legally supply the destination block's parameter (access, multiplicity, type, qualifications).

**Shape.** `validate` (`UV:8981`, `pub(super)`) and predicates `qualified_owned_arrival` (`UV:9289`), `byte_leaf_predicates` (`UV:9321`), `address_join` (`UV:9367`), `owner_loan` (`UV:9377`), `shared_view_carrier` (`UV:9387`), `qualified_owned_bytes` (`UV:9398`), `qualified_shared_view` (`UV:9409`), `qualified_shared_view_source` (`UV:9420`), `plain` (`UV:9461`), `source_contract` (`UV:9478`, returns `(type, access, multiplicity)`). Error for nearly everything: `VerifiedOptimizationUnitProjectionMismatch` (`UV:8986-8987`) and `UnknownSuccessor` (`UV:9038`).

**How it decides.**
- Parameter side (`UV:8993-9035`): entry block has no structural parameters; positions equal indices; place unique across the function; the parameter must be one of nine accepted forms (`plain`, `qualified_owned_arrival`, `address_join`, unqualified view-descriptor, `owner_loan`, `qualified_owned_bytes`, `qualified_shared_view`, `shared_view_carrier`; `UV:9000-9008`) and its access must be Owned or one of a few borrow/view shapes (`UV:9009-9020`); place kind must equal `BlockParameter{block,position}`; not an entry-claim place.
- Edge side (`UV:9036-9285`): binding count equals destination parameter count; for non-owned descriptor parameters the source's `descriptor_source_projection_accessible` must hold (`UV:9054-9075`); then one branch per parameter kind selects the `exact_source` test: qualified owned arrival (`UV:9077-9099`), qualified shared view (`UV:9100`), owner loan or qualified owned bytes (forwarded, copied from a `StructuralLeafCopy`, projected from a parameter field, or an owned-bounded owner loan, `UV:9102-9173`), borrowed address join (`UV:9174-9213`), shared view carrier (`UV:9214-9219`), view-descriptor parameter with non-empty path (`UV:9220-9230`), non-empty path with Owned parameter (`UV:9231-9252`), else the plain case: Owned requires same type/Owned access/same multiplicity, borrowed requires a `borrowed_referent_presentation` and a suppliable access (`UV:9253-9270`). Finally `binding.parameter == parameter.place`, `binding.argument.access == parameter.access`, `exact_source`, and not (`claim_hit && parameter.access != Owned && !borrowed_address`) (`UV:9272-9282`).
- All keyed on parameter/place ids and type shapes.

**Cases by shape.** The ordered `else if` chain (`UV:9077-9270`) is a closed list of nine parameter shapes; the first matching branch decides and later ones are not tried.

**Same job elsewhere.** `plain` + `source_contract` (`UV:9478-9496`) re-derive the structural source contract of a place that `operation_contracts::structural_source_contract` also gives (used inside this function at `UV:9078, 9161, 9177, 9208, 9245`). Two different contract views of the same place are used in one function: `source_contract` (3-tuple) and `structural_source_contract` (full struct). `qualified_shared_view_source` (`UV:9420-9459`) and the `forwarded` branch in the owner-loan arm (`UV:9105-9117`) are the same forwarded-parameter test written twice. `byte_leaf_predicates` (`UV:9321`) relates to first pass `byte_predicates` (`unit_026.md`, `UV:4188-4219`) but is independent.

**Unused/one-caller.** `structural_transfers::validate` called once at `UV:9593`.

**Stated limits.** None.

**Fail-open.** None seen that accept a wrong binding silently. Notes: `source_contract` (`UV:9478-9541`) returns `None` for a parameter that is neither `plain` nor an unqualified view descriptor (`UV:9489-9496`), and the later branches require `source.is_some_and(...)`. `UV:9111, 9136`: `forwarded` accepts a parameter whose `projected_qualifications` is empty while comparing `qualifications`; a block parameter with non-empty `projected_qualifications` is never the "forwarded" source (conservative). The wildcard-like last `else` (`UV:9253-9270`) handles every remaining case with explicit access matching; the `match parameter.access` covers all four access variants listed (Owned, Shared, Mutable, WriteOnly), so no `_ =>` arm.

---

## G. `validate_function` (`UV:9546-9673`)

**Purpose.** The per-function driver. Runs the checks in a fixed order and returns the first error.

**Order (read).** nominal cleanup subject and attachment (`UV:9565-9575`: the subject must be the function's `attachment`, be a known type, and be the subject of exactly one function in the unit); published service ceiling (`UV:9576`); structural catalog and witness lists (`UV:9581`); provider attachment specialization (`UV:9587`); structural root uniqueness (`UV:9588`); entry claim index (`UV:9590`); block index (`UV:9591`); parameter metadata (`UV:9592`); `structural_transfers` (`UV:9593`); nodes and edges (`UV:9594`); total CFG (`UV:9595`); function results (`UV:9601`); byte-sequence literal and trivial-affine-local witnesses (`UV:9603-9604`); structural place availability (`UV:9605`); structural root operations (`UV:9610`); references (`UV:9619`); byte concatenation, byte-field freshness, whole-descriptor freshness, case arguments, indexed byte stores (`UV:9625-9643`); mutable views (`UV:9644`); provenance, work and effects (`UV:9645`); fact index (`UV:9646`); values and bindings (`UV:9647`); places and claims (`UV:9659`); then ownership frontier collection (`UV:9660-9671`).

**Inputs/outputs.** Takes mutable accumulators `call_accesses`, `descriptor_requirements`, `ownership_frontiers` and an optional `reference_memberships`; appends the ownership requirements (`UV:9671`). Returns `Result<(), Error>`.

**Same job elsewhere.** `validate_function` is the one place the phase order lives for per-function checks. It reaches `validate_values_and_bindings` and `validate_function_structural_catalog` (defined after line 11750, not read).

**Fail-open.** None in the driver. The `nominal_cleanup_subject` check uses `functions.values().filter(...).count() != 1` (`UV:9568-9572`) so a missing or duplicate subject is an error. `validate_function` is `pub(crate)`; called from `catalogs` (first pass, `UV:1233`).

---

## H. `identity_indexes` (`UV:9675-9731`)

`validate_identity_and_fact_indexes` (`UV:9684`, `pub(super)`): recomputes the unit identity and compares (`ContentIdentityMismatch`, `UV:9687-9693`); `unit.work_schedule == WorkSchedule::CURRENT.identity()` else `WrongWorkSchedule` (`UV:9694`); accepted obligation facts must have `fact.psi == unit.psi`, canonical identity, and be strictly ascending by `(machine, owner, obligation)` (`UV:9697-9707`; error `AcceptedObligationFactIndexMismatch`); proof questions must have `terminal_psi == unit.psi`, canonical identity, unique `identity` and unique `(owner, obligation)` (`UV:9708-9717`); ownership frontier facts must have `psi == unit.psi`, canonical identity, `canonical_ownership_frontier_snapshot`, and strictly ascending `(machine, site)` (`UV:9718-9728`). Keyed on identities and orderings. Fail-open: none seen. Same family as first-pass `catalogs` order checks (`unit_026.md` 4.4).

---

## I. `no_normal_calls` (`UV:9732-9824`)

**Purpose.** Every `InvokeNoNormal` must call a machine in the unit that cannot complete normally.

**Shape.** `CurrentCompletionGraph { functions }` (`UV:9740`) implements `terminal_semantics::control_graph::NormalCompletionGraph` (`UV:9744-9791`): `entry(machine)` returns the function entry (`UV:9747`), `disposition(machine, block)` classifies the block's last node (`UV:9754`). `validate` (`UV:9793`).

**How it decides.** Disposition by last node: `Return`/`ReturnUnit`/`ReturnStructural` is `Return`; `Crash`/`ExternalCompletion` is `Stop`; `InvokeNoNormal` is `Invoke(callee)`; `Jump`/`Conditional`/`StructuralCase` is `Successors(targets)` (`UV:9773-9783`); any other last node is an error "unclassified current control node" (`UV:9784-9788`). Missing function or block is an error (`UV:9751, 9768`). `validate` runs `normal_completion_possible_in` per distinct callee (`UV:9812`); `true` is `InvalidNoNormalInvocation` "current module callee may complete normally" (`UV:9817`). `admitted` is a `Vec` with `contains` (`UV:9798, 9804`), so a linear scan.

**Cases by shape.** Closed list of terminator kinds (`UV:9773-9783`).

**Fail-open.** Callees not in the unit give an error (`UV:9751`), so not accepted. Traced; none seen.

---

## J. `operation_contracts` (starts `UV:9825`, range covers `UV:9825-11750`)

`pub(crate) mod operation_contracts` starts at `UV:9825` and continues well past my range. Submodules seen in range: `affine_calls`, `atomic_coherence`, `boundaries`, `claim_transfers`, `node_contracts`, `payloadless_cases`, `plain_calls`, `records`, `scalar_arrays`, start of `scalar_types` (with `byte_views`, start of `element_views`). Modules after (`structural_access`, `service_calls`, etc.) are outside the range.

### J1. `affine_calls` (`UV:9834-9962`)

`exact_plain_affine_structural_call` (`UV:9841`, `pub(crate)`): true only when the call is a plain structural call (`plain_structural_call`, J6) to a callee with exactly one structural parameter, one block, one node, which is `ReturnStructural` of that parameter with no returned claims, trivial affine locals or cleanup; the parameter is position 0, Owned, Affine, same type as the signature, no qualifications; argument Owned, empty path; call result equals signature type and multiplicity; no selected evidence, no entry claims, no outcome-specific ensures, empty published service ceiling; and the type is a "finite owned shape" (`UV:9846-9902`). This is an "identity function" shape recogniser. `finite_owned_shape` (`UV:9905`) is a DFS with `active`/`complete` sets: Record fields, `FixedArray` with literal length > 0, `Mixed` fields and case fields; `_ => false`; fields must be non-erased and Structural/Scalar/Atomic/AtomicBoundedInteger/IeeeFloat/BoundedOwned byte sequence, anything else false (`UV:9941-9961`). Cases by shape: exactly one parameter, one block, one node (`UV:9849-9860`). `grep -c`: `exact_plain_affine_structural_call` 3 occurrences, all in `UV` (definition and two callers after the range).

### J2. `atomic_coherence` (`UV:9963-10068`)

`validate_function_atomic_coherence` (`UV:9971`): builds per-block operation sequences and calls `abstract_operations::happens_before_atomic_coherence_violation` with closures for dominance (`UV:9993-9997`), edge mapping of a root back to the predecessor's argument and path (`UV:9999-10029`, returns `None` if a parameter is bound zero or several times, or if two parallel edges map differently), aliasing (`atomic_projections_may_alias`, `UV:10030-10045`) and field path completion (`UV:10046-10057`). Violation gives `AtomicEventCoherenceMismatch(site, violation)`. The actual coherence rule is in `abstract_operations` (outside unit). Fail-open: a `None` from the edge-mapping closure is returned to the kernel, which decides; not read: **open**.

### J3. `boundaries` (`UV:10069-10148`)

`boundary_requirements_match` (`UV:10075`): boundary `requires` strictly ascending; each requirement's domain exists, its `argument_index` refers to an argument, and the caller's source contract carries the qualification at `argument.path + requirement.path` (`UV:10081-10095`). `boundary_completion_matches` (`UV:10098`): rebuilds the expected `CompletionClaimSource` list from the caller's entry claims and content entry claims (merging by claim id), sorts, and requires equality with the stored `sources`; receipts strictly ascending, unique by claim, and each receipt's argument is `Owned` (`UV:10104-10146`). Receipts' `argument_index` out of range gives `false`. Fail-open: none seen. Grep: the combined count above (11) included these; both are called after line 11750.

### J4. `claim_transfers` (`UV:10149-10408`)

- `function_entry_claim_paths` (`UV:10154`): returns `None` if the function has content entry claims (`UV:10157`); otherwise one `TerminalDynamicClaimPath{argument_index, path}` per entry claim, sorted, `None` on duplicates. `grep -c`: 3 occurrences in `UV` (definition, re-export `UV:10409`, one use).
- `validate_slot_claim_transfers` (`UV:10178`): transfers strictly ascending, claim ids unique, `(argument_index, path)` unique, each argument Owned and listed in the slot's `entry_claim_paths`; every expected slot path must be transferred unless the path contains a `Case` segment (`UV:10206-10211`). Fail-open: the exemption for paths with a `Case` segment (`UV:10208-10210`) means a dynamic slot's claim path through a case variant does not have to be transferred. No comment says why: **open**.
- `validate_internal_claim_transfers` (`UV:10214`): for non-owned arguments the caller's and callee's entry claim paths below the argument must be equal (`UV:10221-10241`); a projected argument cannot touch content claims (`UV:10242-10247`); content claims must match per argument (`UV:10248-10264`); transfer list size, ordering, uniqueness (`UV:10277-10287`); each transfer's argument and parameter exist and the callee declares the claim at that path, or empty path for a content-claimed parameter (`UV:10288-10308`); every callee claim's parameter appears in some transfer (`UV:10309-10316`). `let _ = argument;` at `UV:10307` is dead. `grep -c "validate_internal_claim_transfers"`: 4 occurrences in `UV`. Note `arguments.iter().zip(&callee.structural_parameters)` (`UV:10220`) stops at the shorter list: a callee with more parameters than arguments leaves extra parameters unchecked in that loop; the final loop `UV:10309-10316` still requires each claim-bearing parameter to appear in a transfer, and `arguments.get(index)` at `UV:10289` returns `false` on a missing argument. Traced within the function; argument-count equality is presumably checked by the call-contract code after line 11750: **open**.
- `proposition_structural_roots` (`UV:10319`): collects the `PlaceId` roots mentioned by a `Proposition` (scalar terms, integer math, IEEE float comparison, byte equality, case membership, structural identity, content conservation, and/or/implication; `Truth`, `Falsehood`, `Atom` have none). Closed match over `Proposition` variants.

### J5. `node_contracts` (`UV:10410-10501`)

`validate_node_operation_contracts` (`UV:10423`): runs three checks in order and maps each to its own error with the site: `operation_scalar_types_match` -> `ScalarOperationContractMismatch`; `operation_structural_call_contract_matches` -> `StructuralCallContractMismatch`; `operation_service_contract_matches` -> `OperationServiceContractMismatch` (`UV:10443-10498`). The three called functions are defined after line 11750 (`scalar_types`, `service_calls`), not read.

### J6. `payloadless_cases` (`UV:10502-10822`) and `plain_calls` (`UV:10823-10880`)

- `plain_calls::plain_structural_call` (`UV:10838`): returns a `PlainStructuralCall` view of a `CallStructural` whose result has no qualifications, projected qualifications or claims, no claim transfers, no requirement obligations, `crash_continuations == contract.crash_routes`, and whose callee has no entry claims, content claims or evidence lanes, and a `verified_contract` with empty `requires` and `ensures` (`UV:10860-10870`). `None` otherwise. This is the common gate used by `exact_plain_affine_structural_call`, `plain_scalar_sum_call`, `exact_payloadless_structural_call`, `plain_record_call`, `plain_scalar_array_call`.
- `payloadless_cases`: `plain_scalar_sum_call` (`UV:10514`): plain call, Affine or Unrestricted result, all callee parameters are Shared/Mutable borrows, Unrestricted, unqualified; result type is a `Sum` whose every case field is non-erased and scalar (`UV:10519-10552`). `case_establishment_matches` (`UV:10555`): `EstablishCase` result placed exactly, Unrestricted or Affine, constructor qualifications match, no claims, no establishment bindings, `owned_shared_byte_descriptor_shape` holds, and some case with `id == result_case` has matching field initializers (`UV:10561-10618`). `exact_payloadless_case_return_exits` (`UV:10621`): callee result unqualified and Unrestricted, no calls inside (`UV:10632-10638`), and every block that ends in `ReturnStructural` returns the result of an `EstablishCase` with no fields, whose place and type match; at least one exit (`UV:10642-10697`; blocks not ending in `ReturnStructural` are skipped with `continue` at `UV:10653`). `exact_payloadless_structural_call` (`UV:10700`): plain call, callee parameters Owned/Unrestricted/unqualified without borrowed storage, call result equals callee result, outcome-specific ensures mention only the result place (`UV:10727-10731`), and the exits check. `payloadless_selected_evidence_surface_matches` (`UV:10735`): `true` for non-`CallStructural`, and when `selected_evidence` is empty; otherwise requires `exact_payloadless_structural_call` and the callee's contract to have outcome-specific ensures. `validate_structural_call_result` (`UV:10759`): call result type, multiplicity and qualifications equal the callee signature, the signature's projected rows are all in the result, result lists sorted, `validate_result_claim_catalog` ok; `exact_claim_free` returns `true` early (`UV:10803`); otherwise Linear requires exactly one root claim, Affine requires every claim to have a non-empty path, Unrestricted requires none (`UV:10812-10820`).
- Cases by shape: "payloadless case" and "scalar sum" and "plain record", "scalar array", "affine identity" are five fixed shapes of callee, each with its own recogniser.
- Same job elsewhere: five `plain_*_call` recognisers (`exact_plain_affine_structural_call` `UV:9841`, `plain_scalar_sum_call` `UV:10514`, `exact_payloadless_structural_call` `UV:10700`, `plain_record_call` `UV:10906`, `plain_scalar_array_call` `UV:11582`) each repeat the checks "multiplicity", "no qualifications", "no projected qualifications", "no selected evidence", "no outcome-specific ensures" on `call.signature`. Same gate in five bodies.
- Unused/one-caller: `grep -c` over `omega-rust` gave 3 occurrences each, all in `UV` (definition + 2 uses), for `plain_scalar_sum_call`, `plain_record_call`, `plain_scalar_array_call`; 3 for `payloadless_selected_evidence_surface_matches`; 3 for `exact_plain_affine_structural_call`. No outside crate uses them.
- Fail-open: `UV:10653` skips blocks that do not end in `ReturnStructural` when counting exits; a block ending in `Crash` or a jump is not an exit, which is intended (they do not return), and `exits != 0` is required. `UV:10749` `payloadless_selected_evidence_surface_matches` returns `true` for any operation that is not a `CallStructural` with a structural result; the caller is presumably only reached for calls: **open**. `UV:10803` `exact_claim_free` true returns `true` skipping the claim-shape check; claim-freedom is decided by the caller: **open**.

### J7. `records` (`UV:10881-11486`)

**Purpose.** Constructors: does an `EstablishRecord`, `EstablishCase` or `EstablishZeroedStructural` match its declared type and its field initializers, and does it carry exactly the qualifications its fields supply.

**Shape.** `plain_record_call` (`UV:10906`), `zeroed_establishment_matches` (`UV:10932`), `record_establishment_matches` (`UV:10962`), `constructor_qualifications_match` (`UV:11046`, `pub(super)`), `constructor_argument_contract` (`UV:11119`), `field_initializers_match` (`UV:11144`), `constructed_field_predicates` (`UV:11242`), `source_has_claims` (`UV:11273`), `plain_record` (`UV:11299`), `constructible_record` (`UV:11306`), `shared_view_member` (`UV:11313`), `record_type` (`UV:11322`), `constructible` (`UV:11409`), `structural_initializer_matches` (`UV:11465`).

**How it decides.**
- `record_establishment_matches`: result placed exactly; custody: Linear needs exactly one claim with empty path, non-linear needs none (`UV:10977-10982`); constructor qualifications; `constructible_record(types, result_type)`; field initializers match; then the projected qualifications are recomputed from the Structural field arguments and must equal `result.projected_qualifications` exactly after sort and dedup (`UV:11013-11043`).
- `field_initializers_match` (`UV:11144-11240`): non-erased declared fields and initializers must have equal counts and equal ids in order (`UV:11153-11160`); `Structural` field with a `Structural` initializer goes to `structural_initializer_matches`; a byte-sequence field with a `Structural` initializer needs an Owned argument with empty path, a different place from the result, a matching carrier (bounded-owned: Owned/Unrestricted source of an equal `ByteSequence` type; shared readable view: Owned or Shared source of a shared readable view), declared byte predicates, no claims on the source, non-linear source, and available predicates must imply the field's (`UV:11170-11215`); a scalar initializer needs a definition of that value id with the declared scalar type among function/block parameters and block definitions, and `scalar_field_requires_evidence(field_type) == range_obligation.is_some()` (`UV:11216-11236`); anything else false.
- `structural_initializer_matches` (`UV:11465-11485`): argument Owned; source Owned or (SharedBorrow when the expected type is a shared view member); source type equals expected; no claims on source; argument place not the result place; source non-linear; result multiplicity stricter only if the source is Unrestricted when the result is Unrestricted.
- `constructor_qualifications_match` (`UV:11046-11117`): result has no whole qualifications or establishment bindings and its projected rows must be well-formed for the type; each projected row path must start with `Field(id)` (record or mixed) or `Case(case), Field(id)` when `selected_case == Some(case)`; the field's initializer must be a Structural argument whose source carries that qualification at `argument.path + suffix` (`carries_qualification`). Any other path shape is `false`.
- `record_type` (`UV:11322`) and `constructible` (`UV:11409`): iterative/recursive type walks with active and complete sets (cycle is `false`). `record_type(types, root, references, shared_view_members)` allows Record/Sum/Mixed/FixedArray/PrimitiveScalar, `Reference` types with a referent if `references`, shared views if `shared_view_members`; fields must be scalar-like, structural, or bounded-owned byte sequences; borrowed byte-view fields only if `references || shared_view_members`; erased fields are skipped only if their type is `Erased` (`UV:11374-11379`). `constructible` is a stricter variant: erased fields are never allowed (`UV:11445-11447`) and byte sequences are not allowed (`UV:11456`).

**Cases by shape.** `field_initializers_match` is a closed three-way match on (field type, initializer) (`UV:11163-11237`). `constructor_qualifications_match` handles only `Field` and `Case,Field` leading segments (`UV:11071-11095`). `record_type` supports a closed list of shapes and field types.

**Same job elsewhere.** `record_type` (`UV:11322`), `constructible` (`UV:11409`) and `finite_owned_shape` (`UV:9905`) are three recursive walks over the type graph with active/complete cycle tracking and a closed list of allowed field types. `record_type` with `(true, true)` vs `constructible` differ in byte-sequence and erased-field handling. `grep -n "constructible("` over the optimization-unit directory shows `constructible` has callers at `UV:12777, 12787, 15291, 15309, 15340, 15345` (outside my range) plus the definition at `UV:11409`; `constructible_record` and `plain_record_call` are called inside `UV`. `source_has_claims` (`UV:11273`) repeats the "result with non-empty claims by place" scan that `readable_field_type` also does (`UV:8603-8622`) with slightly different operation lists (here includes `EstablishScalarArray`, `EstablishStructuralArray`, `EstablishZeroedStructural`, `EstablishAccessReference`; there `EstablishRecord`/`EstablishZeroedStructural`/`EstablishScalarArray`/array-or-case/`EstablishReference`/`MoveStructuralField`/calls). Two near-identical lists with different members (`UV:8604-8618` vs `UV:11277-11294`).

**Fail-open.** `UV:11269` `(whole, captured) => whole.or(captured)`: if only one of the two predicate sources is `Some`, that one is used alone, and the result `Some(_)` is accepted if it implies the field's predicates (`UV:11205-11214`). Missing predicate evidence yields `None` and then the `is_some_and` is false: fail-closed. `UV:11204`: `field_type.byte_predicates().is_empty()` skips predicate proof when the field requires none; intended. `UV:11125-11136` `constructor_argument_contract` with a non-empty path requires Field/FixedIndex only and an Owned argument; partial roots use `partial_affine_root_type` (outside unit). None seen as accept of a wrong program, beyond the open items stated.

### J8. `scalar_arrays` (`UV:11487-11667`)

`scalar_array_establishment_matches` (`UV:11494`): result type has a scalar-array leaf shape (`scalar_array_leaf_shape` gives scalar type and count), Unrestricted, no establishment bindings, `scalar_array_qualifications_hold` with a closure that resolves each element value to a literal integer constant, or (for a non-entry block parameter) to the arguments of all incoming edges (`UV:11525-11561`); no projected qualifications, no claims, element count equals the leaf count, place placed exactly, and every element value has a definition with the leaf scalar type (`UV:11517-11579`). `plain_scalar_array_call` (`UV:11582`): plain call, Unrestricted result and parameters, scalar-array leaf shape. `structural_array_establishment_matches` (`UV:11607`): result is a `FixedArray` with literal length; projected qualifications recomputed from each element argument with `FixedIndex(position)` and compared exactly; Unrestricted or Affine; no qualifications, no claims; element count equals length; every element passes `structural_initializer_matches` (`UV:11612-11666`). Fail-open: in the closure at `UV:11544-11552`, a block parameter in the entry block returns `None` (unknown origin) and a missing binding makes `collect::<Option<Vec<_>>>()` `None` (`UV:11558`), both leading to "not constant"; the kernel decides what `None` means for qualification proof: **open** (closes if the kernel treats `None` as non-constant).

### J9. `scalar_types` start (`UV:11668-11750`)

Imports and the `byte_views` submodule (`UV:11681-11747`): `types_match(operation, definitions)` returns `true` only for byte-sequence operations whose index/length/start/end/value value ids have the expected scalar types: lengths, indices, starts and ends are unsigned 64-bit integers; write and read values are unsigned 8-bit; `index` and `length` must have the same scalar type; predicate/equal results are Boolean (`UV:11693-11743`). `_ => false` (`UV:11744`): the caller must route only these operations here. Keyed on `ValueId` definitions in `definitions`; a value id with no definition gives `scalar(...) == None` which fails the `matches!` (false). `element_views` module opens at `UV:11748`; its first lines (imports, `UV:11749-11752`) are the last in range. `grep` not applicable.

---

## Summary tables for the range

**Same job elsewhere (cross-range citations).**
- Length-before-write rule: `UV:6320-6333` (here, positional) versus `UV:4852-4858` (first pass, existence only).
- Unsigned-64 `matches!(scalar, Integer == IntegerType::new(Unsigned, 64))` repeated 11 times in `UV:11693-11743` alone.
- Dominance/reachability: `UV:6718`, `UV:5751-5757`, `UV:5806-5822`, `UV:6563-6583` use `block_reaches` or their own DFS.
- Type-graph walks with cycle tracking: `UV:9905`, `UV:11322`, `UV:11409`.

**Counts run (Grep tool, count mode, over `omega-rust`):**
- `block_reaches` 6 (all `UV`); `validate_total_cfg` 2 (`UV`); `current_mutable_borrows` 3 in `UV` + 1 in `current_ownership.rs`; `exact_plain_affine_structural_call` 3 (`UV`); `plain_scalar_sum_call` 3 (`UV`); `plain_record_call` 3 (`UV`); `plain_scalar_array_call` 3 (`UV`); `payloadless_selected_evidence_surface_matches` 3 (`UV`); `validate_internal_claim_transfers` 4 (`UV`); `function_entry_claim_paths` 3 (`UV`); combined `constructible_record|boundary_requirements_match|boundary_completion_matches|reconstruct_fact_index` 11 (`UV`).
- These counts include the definition and re-exports, not only call sites. I did not read the call sites after line 11750.

**Stated limits.** No comment in `UV:5751-11750` says something is unsupported or migration debt. Comments present are the `//`-free code, test names and error reason strings listed above.

---

- **Coverage:** `unit_validation.rs` lines 5751-11750 read in full, 6000 lines. Skimmed: 0. Not read: 0. (Lines 5735-5750 and 11751-11764 were read as context and are not counted.)

- **Five things a maintainer most needs to know about this range.**
  1. `validate_function` (`UV:9546-9673`) is the only place the per-function phase order is written, and it returns the first error; `structural_roots::validate_structural_root_operations` (`UV:7190-8511`) is one 1300-line function with a single `match` that ends `_ => {}` (`UV:8506`), so operations it does not name are not validated there.
  2. Several checks key on existence rather than position: `primitive_locals::local` finds any `EstablishPrimitiveLocal` for a place anywhere in the function (`UV:7037-7046`), while `mutable_views` checks the `ByteSequenceLength`-before-`ByteSequenceWrite` rule positionally within a block (`UV:6320-6333`).
  3. Five separate recognisers (`affine_calls`, `plain_scalar_sum_call`, `exact_payloadless_structural_call`, `plain_record_call`, `plain_scalar_array_call`) each accept a fixed callee shape through `plain_structural_call` (`UV:10838`); none of the five has a caller outside `UV` by grep.
  4. Fact index, identity indexes, provenance/work/effect chain and `structural_transfers` are exact-recomputation checks (equality with a rebuilt value, strict ordering), not subset checks; the membership checks (`view_memberships_match`, `copy_memberships_match`, `reference_result_memberships_match`) are subset checks against evidence from `terminal_semantics::descriptor_results`, which is outside this unit.
  5. The generic `catalog_mismatch(function)` error is used for 30+ different structural failures in `structural_roots` (`UV:7315` and later), so the error does not say which rule failed; the named errors in range are `InvalidPrimitiveLocal`, `InvalidWriteOnlyPrimitiveStore`, `InvalidStructuralScalarFieldStore`, `InvalidStructuralScalarFieldRead`, `InvalidStructuralCaseDispatch`, `InvalidOwnedValueTransfer`, `StructuralReturnSourceContractMismatch`.

- **What I am unsure about.**
  - Whether the skipped cases listed as "open" (the `Field` operands of `ByteSequenceEqual`/`ByteSequencePredicate`, the unmatched-callee `true` at `UV:8770-8772`, the `Empty` element-payload `true` at `UV:8868-8870`, the `Case`-path exemption at `UV:10208-10210`) are covered by code after line 11750 (`operation_structural_call_contract_matches`, `structural_access`, `references`), which I did not read.
  - Whether "unused" for the `plain_*_call` family means unused: the counts (3 each) include two non-definition occurrences I did not open; they are probably the two uses inside `operation_structural_call_contract_matches` after 11750.
  - The macros (`establish_record_or_case_pattern!`, `byte_sequence_store_pair_pattern!`, `exclusive_borrow_pair_pattern!` etc.) are defined outside this unit; I took their names at face value.
  - `grep -c` of the combined four-name pattern gave 11 occurrences; I did not split it per name, so no per-name call-site count is claimed for `constructible_record`, `boundary_requirements_match`, `boundary_completion_matches`, `reconstruct_fact_index`.
