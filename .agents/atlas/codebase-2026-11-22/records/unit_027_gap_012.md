# Record: unit 027, second pass (gap_012)

Area `12-native-representations`. File: `source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/optimization-unit-semantics/src/unit_validation.rs`, lines 3601-9600 (6,000 lines). Everything below is inside `pub(crate) mod function_structure` (opens at line 3066, grepped), which opens at the top of the range as the tail of `mod case_arguments` (opens at 3389). All claims are from reading, unless marked "grepped" or "inferred". Paths of other files are relative to the `optimization-unit-semantics/src` directory unless full.

What the whole range is: validators run over one `PsiOptimizationFunction` (a function in the optimization unit: blocks of nodes, each node an `AbstractOperation`, plus a table of "structural places" and "structural parameters"). Each validator returns `Err(OptimizationUnitValidationError::...)` or `Ok(())`. The code is a set of nested private modules that end with `pub(crate) use` re-exports. The range ends mid-function inside `structural_transfers::validate` (line 9600).

## 1. `case_arguments::entry_membership` (3605-3670, tail of module that opens at 3389)

1. Purpose: decides whether a function's own `requires` contract already guarantees that a structural-case subject (root place plus path) is, or is not, in a given case.
2. Shape: `fn entry_membership(function, types, root, path, case, expected) -> bool` (3605) with an inner `fn entails(proposition, matches_subject, case, expected)` (3634). `pub(crate) use case_arguments::membership_on_every_path` (3672); that function itself is before the range.
3. Reads `function.verified_contract.requires` (3614, 3666), `function.structural_parameters` (3617), `terminal_semantics::canonical_structural_storage_path` (3626). Produces a bool.
4. Decides on: place identity (`parameter.place == root`, 3620; `subject.root() == root`, 3625), canonicalised path equality (3632), and case identity (`*selected == case`, 3644). Not on spelling.
5. Cases by shape: `entails` handles `StructuralCaseMembership`, `Conjunction` (any child), `Disjunction` (non-empty and all children), and `Implication` whose conclusion is `Falsehood` (3640-3663, only for `expected == false`). `_ => false` (3663).
6. Same job elsewhere: `membership_on_every_path` is the dataflow version (called at 8559, grepped: 8 lines mention it, 2 files).
7. Unused: no `pub` items here besides the re-export; see above.
8. Stated limits: none.
9. Fail-open: none seen. Every early exit and the default arm return `false` (3615, 3622, 3663), so a missing contract or unknown proposition shape means "not entailed".

## 2. `mod structural_freshness` (3673-4025)

1. Purpose: answers "has the structural data at (root, path) been changed by the operations between a producer and a use?" with a backward walk over the control-flow graph.
2. Shape:
   - `call_argument_changes` (3681): does a call argument possibly alias and change the observed region.
   - `structural_metadata_may_change(function, types, root, path, operation) -> bool` (3789, `pub(super)`).
   - `unchanged_since` (3869), `established_on_every_path` (3888), `established_on_every_path_with_edges` (3907), `established_on_every_path_with_entry` (3933). The first three are thin wrappers that pass `|_| false` / `|_,_,_,_| false` for the closures they lack (3902, 3928).
3. Inputs: `PsiOptimizationFunction`, a `predecessors: BTreeMap<BlockId, BTreeSet<BlockId>>`, caller-supplied `establishes` / `invalidates` closures. Output: bool.
4. How it decides:
   - `established_on_every_path_with_entry` (3948-4023) pops `(block, position, root)` work items, walks backwards one node at a time. If `establishes` matches a node it sets `established = true` and stops that path; if `invalidates` matches first it returns `false` (3959-3964). At a block start it follows every predecessor edge and renames the root through `arrival_storage::root_before_edge` (3999). Keyed on block position and `PlaceId`, plus the identity of the producing node.
   - `call_argument_changes` (3681-3787) walks a call argument back through its producers. It follows `EstablishElementView` and the element-view/byte-subslice pattern macros to the backing place (3752-3781). Anything else uses `may_alias`, which is `!storage_cannot_retain_loan(...)` from `current_ownership` (3697).
5. Cases by shape: `structural_metadata_may_change` (3796-3866) lists operations that cannot change metadata (the long `=> false` arm at 3833-3864) and answers `true` for everything else (`_ => true`, 3865). It lists `ByteSequenceStore`, `StructuralByteSequenceFieldStore`, `WriteOnlyPrimitiveStore`, `BoundaryCall`, `CallStructural` (only when an argument is a mutable, write-only or owned borrow, 3825-3832) with specific handling.
6. Same job elsewhere: `established_on_every_path` is used by `byte_predicates` (4030, 4108, 4208) and `fixed_byte_freshness::holds` (5752). `membership_on_every_path` (before the range) is the same backward walk specialised to case membership (inferred from its use at 8559; the body is not in this range). `current_ownership::storage_cannot_retain_loan` is called here (3697). Grep of `established_on_every_path_with_entry`: 4 lines, 1 file.
7. Unused: none of these is `pub`; callers are all within this range.
8. Stated limits: none.
9. Fail-open: for the backward walk the failure mode is "established" only when a path actually reaches an establishing node, so lookups that find nothing return `false`:
   - missing predecessor row, missing block, missing edge to the block: `return false` (3975-3980, 3982-3989, 4017-4019).
   - `root_before_edge` returning `None`: `return false` (4001-4003).
   - Fall-through of the loop returns `established`, which stays `false` if the work list is empty without finding a producer (4023). Possible oddity (inferred): if `use_site` is at position 0 of the entry block and `establishes_entry(root)` is true, `established = true` is set and the loop continues; any other path that later hits an `invalidates` still returns `false`, so one establishing path does not hide an invalidating path. Classified traced: this is correct for "on every path" because any invalidating or unreachable path returns false immediately.
   - `call_argument_changes` returns `true` (meaning "may change") on every unknown (3712, 3720-3721, 3725, 3732, 3742, 3748). Fails closed.
   - `structural_metadata_may_change` `_ => true` (3865) fails closed.

## 3. `mod byte_predicates` (4027-4808)

1. Purpose: works out which byte-sequence predicates (`AsciiOnly`, `ValidUtf8`, literal-bytes facts) are known to hold for byte sequences, and checks that byte stores which could split a UTF-8 scalar are only done when ASCII-ness is established.
2. Shape:
   - `byte_write_predicates` (4050), `validate_indexed_byte_stores` (4062), `owned_byte_predicates` (4223), `concatenation_predicates` (4263, private), `concatenation_preserves_predicates` (4293), `replacement_preserves_predicates` (4308), `whole_source_predicates` (4355), `copied_field_predicates_at_transfer` (4370) and `copied_field_predicates_at_invocation` (4379), `byte_field_predicates` (4504), private `byte_field_predicates_in` (4514), `source_predicates` (4545, memoised, recursion budget `remaining`), `descriptor_source_predicates` (4567), `derive_source_predicates` (4604), `may_change_owned_source` (4777). Private `struct Catalogs<Types, Domains>` (4499).
3. Inputs: `PsiOptimizationUnit` (types, domains), function, a place; returns `Option<ByteSequencePredicateSet>`. `validate_indexed_byte_stores` returns `Result<(), InvalidByteSequenceWrite(site)>`.
4. How it decides:
   - `None` means "no predicates known". Callers use `.implies(required)` and `.is_some_and(...)`, so `None` means the store is refused. `required.is_empty() || ...` (4345, 4302) accepts when nothing is required.
   - `validate_indexed_byte_stores` (4062-4221) handles three operation shapes: `WriteOnlyPrimitiveStore` (needs ASCII when `requires_ascii == Some(true)`; checked by `fixed_byte_freshness::holds`, 4076-4096), `ByteSequenceWrite` (only checked when `byte_write_predicates` returns `Some`, and only when the predicates include `ValidUtf8` but do not imply `AsciiOnly`, 4099-4141), and `StructuralByteSequenceFieldByteStore` (4144-4217).
   - `derive_source_predicates` (4604-4775) keys on the place's `StructuralPlaceKind`: parameter/block parameter, `ByteSequenceLiteral` (exactly one producer, 4646-4650), or `OperationResult` (subslice, leaf copy, concatenation). It searches `function.block_nodes()` linearly by `psi_operation() == Some(producer)` (4656-4658).
5. Cases by shape: `derive_source_predicates` ends `_ => None` (4773). `concatenation_predicates` only accepts `Whole` operands and `Field` operands with `SharedBorrow` access (4272-4289). `copied_field_predicates_at_invocation` (4379-4497) accepts only: a `StructuralLeafCopy` with an Ordinary path whose last segment is `Field`, parent path of `Field`/`FixedIndex` segments only (4431-4438), a `BoundedOwned` carrier, and a consumer that is `CallStructural`/`InvokeNoNormal` or a constructor with argument access `Owned` and empty path (4453-4475). It refuses if the source escapes through `Jump`, `Conditional`, `ReturnStructural`, `EstablishReference`, `EstablishElementView` or `ByteSequenceSubslice` (4483-4495).
6. Same job elsewhere: `replacement_preserves_predicates` / `concatenation_preserves_predicates` are consumed by candidate validation (not in range). Grep, whole `source-rust/`+`tools/`: `byte_write_predicates` 4 lines in 1 file, `concatenation_preserves_predicates` 3 lines in 1 file, `replacement_preserves_predicates` 3 lines in 1 file, `copied_field_predicates_at_transfer` 3 lines in 1 file, `copied_field_predicates_at_invocation` 4 lines in 1 file, `whole_source_predicates` 9 lines in 1 file, `owned_byte_predicates` 3 lines in 1 file, `byte_field_predicates` 7 lines in 1 file. All one file; the `pub(crate)` re-export at 5911-5915 is the only declared exposure. Callers elsewhere in the crate (such as `candidates.rs`) were not matched because the grep reports one file only, which is `unit_validation.rs` itself. So these `pub(crate)` re-exports have only same-file uses (grepped); whether any other file in the crate calls through `function_structure::` paths needs a grep for `function_structure::byte_` that I did not run. Marked suspected-unused-outside.
7. Unused: see 6 (counts given).
8. Stated limits: none.
9. Fail-open:
   - `validate_indexed_byte_stores` 4099-4142: if `byte_write_predicates` returns `None` (no known predicates for the destination) the `ByteSequenceWrite` is accepted with no ASCII check (`continue` at 4142). Open: no comment gives a reason. The wrong program that gets through is a write to a byte sequence whose predicates cannot be derived (for example an owned result that is not a concatenation, or a parameter with domains the `byte_parameter_predicates_in` call rejects) that would split a UTF-8 scalar. Not traced to callers; whether a neighbouring check, `validate_byte_view_source` with its `qualifications_permitted` test (5112-5117), already refuses those sources is unverified.
   - `derive_source_predicates` 4745-4750: if some operation may change the owned source, it returns `Some(declared)`, the declared qualifications only. That is the weaker answer, so it fails closed.
   - `WriteOnlyPrimitiveStore` when `requires_ascii` returns `None` or `Some(false)` is accepted without a freshness check (4076-4097). Stated by the shape of the call: `requires_ascii` is `Some(true)` only when predicates include `ValidUtf8` and not `AsciiOnly` (5733-5736). `None` arises when `primitive_store_byte_predicates` returns `None` (5724-5732). Traced to the predicate source only; not to callers.

## 4. `mod byte_views` (4809-5319)

1. Purpose: checks stores into and reads from byte sequences and byte views: which borrow kind is allowed, which carrier, whether the length used was measured from the same sequence.
2. Shape: `byte_field_store_capacity` (4831, `pub(crate)`), `byte_view_field_store_is_valid` (4887), `declared_descriptor_home` (4944, `pub`), private `writable_byte_field_carrier` (4962), `validate_byte_view_source` (5023, `pub(super)`), `validate_source` (5057), `owned_byte_result` (5279).
3. Inputs: function, one operation, the source's `StructuralPlaceKind`, type and domain catalogs. Output: `Option<u64>` capacity, `bool`, or `Result<(), InvalidByteSequence{Write,Read,Length,Subslice}>`.
4. How it decides: keyed on place identity, `StructuralAccess`, `StructuralMultiplicity` and structural type shape (`StructuralTypeShape::ByteSequence(carrier)`). `validate_source` (5057-5277) computes four booleans, `valid`, `exact_length`, `valid_result`, `writable`, and refuses if any is false (5255). `exact_length` (5178-5184) demands a `ByteSequenceLength` node whose `source.place == source`, empty path, and `result.value == length`. It is existence anywhere in the function, not dominance; position and path are not checked at this point (inferred; freshness is checked separately by `descriptor_sources::validate_whole_freshness`, 5598, for the whole-operand forms only).
5. Cases by shape: `validate_source` `permitted` (5090-5111) lists access by operation: `ByteSequenceWrite` needs `MutableBorrow` or an owned bounded parameter; `ByteSequenceLength` accepts reading borrows, owned or write-only owned bounded; store pair needs `SharedBorrow` or owned; all others need reading borrow or owned bounded. Source place kinds handled: `Parameter`/`BlockParameter`, `ByteSequenceLiteral`, `OperationResult`; `_ => None` (5157). `byte_field_store_capacity` handles `ByteSequenceStore` and the two field-store operations (4836-4874); `_ => return None` (4873). `writable_byte_field_carrier` (4962) comment-free but only accepts `Record` field parents (5006) and `is_structural_scalar_store_path` paths (4998).
6. Same job elsewhere: `descriptor_sources::source_shape` (5526) and `validate_source` both compute "what carrier does this source have under this access"; they differ in that `source_shape` uses `structural_borrowed_storage_projection` and `validate_source` uses `owned_byte_result` plus `structural_source_contract`. Grep: `byte_field_store_capacity` 6 lines, 1 file; `declared_descriptor_home` 7 lines, 3 files.
7. Unused: see counts above; `declared_descriptor_home` is `pub` and used in 3 files (grepped).
8. Stated limits: none.
9. Fail-open: none seen. Every `None`/failed lookup produces `false`, `None` or an `Err`. One check: `length.is_none_or(...)` at 5178 accepts when no length is demanded; that applies to `ByteSequenceLength` operations (the `(source, None)` default arm, 5042 returns early with `Ok(())` for non-listed operations). Traced: a non-byte operation is not this validator's job (stated by the `_ => return Ok(())` arm).

## 5. `mod descriptor_sources` (5320-5694)

1. Purpose: checks that length, subslice and whole-sequence uses of a byte sequence or element view read from a legitimate descriptor source, and that the length value used still equals the current length.
2. Shape: `valid` (5338), `fixed_bytes` (5439), `valid_whole` (5452), `valid_representation_count` (5478), `source_shape` (5526), `validate_whole_freshness` (5598).
3. Inputs: function, type and domain catalogs, operation or `StructuralArgument` source. Output: `bool` / `Result`.
4. How it decides: `valid` matches `ByteSequenceLength`, `ElementViewLength`, `ByteSequenceSubslice`, `ElementViewSubslice` and returns `false` for every other operation (5359). For a subslice it requires an earlier-existing length node over the same descriptor coordinate (`same_descriptor_coordinate`, 5424-5432). Existence is `function.block_nodes().any(...)`, not dominance. `validate_whole_freshness` (5598-5693) then checks, for whole-operand byte operations and whole-representation element views, that a producer node for the length exists and `unchanged_since` the producer holds (5662-5686). It matches names via `arrival_storage::storage_names` (5643).
5. Cases by shape: `fixed_bytes` accepts only `FixedArray` of unsigned 8-bit non-address integers (5439-5449). `valid_representation_count` accepts `ElementView` (length node) or `FixedArray` with `ArrayExtent::Literal(count)` plus a `u64` `IntegerConstant` equal to `count` (5507-5521); `_ => false` (5522). `source_shape` returns the shape from `StructuralStorageProjection::Endpoint` or `ByteField`; `CollectionWindow` returns `None` (5594).
6. Same job elsewhere: freshness of a measured length is the same job as `fixed_byte_freshness::holds` (5739) for ASCII facts: both call `structural_freshness` walkers.
7. Unused: `valid_whole`, `valid_representation_count` and `validate_whole_freshness` are `pub(super)`; callers are in `structural_roots` (7815, 7844, 7861) and presumably `function_structure::validate` (not in range, inferred).
8. Stated limits: none.
9. Fail-open: one place. `validate_whole_freshness` 5636-5640: for an element-count operand, if any node anywhere in the function is an `IntegerConstant` whose result equals `length`, it `continue`s without the freshness search. Open: no comment. What gets through is an `EstablishElementView` whose `source_length` is any integer constant (even a wrong one); but `valid_representation_count` (7815) separately checks the constant equals the fixed array's literal count, which is traced to the call at 7815. For an `ElementView` source it demands an `ElementViewLength` node (5501), so a constant there would be refused by that earlier check.

## 6. `mod fixed_byte_freshness` (5695-5910)

1. Purpose: decides whether an indexed byte store needs, and has, proof that the target array is ASCII.
2. Shape: `requires_ascii` (5709), `holds` (5739, recursion `budget`), `full_path` (5860), `declared_ascii` (5873).
3. How it decides: `requires_ascii` returns `Some(true)` only if predicates contain `ValidUtf8` and not `AsciiOnly` (5733-5736). `holds` walks backwards (5752) with `establishes` matching `EstablishZeroedStructural`, `EstablishScalarArray` whose elements are all `IntegerConstant` of value at most 127 (5760-5763), `StoreStructuralField`, `StructuralLeafCopy` (re-queries the source with the copied path), or any operation whose result declares ASCII membership. `invalidates` treats a `WriteOnlyPrimitiveStore` that touches one element and writes a literal at most 127, or into an ASCII-requiring slot, as not invalidating (5805-5821).
4. Cases by shape: one-element writes only (`[FixedIndex | RuntimeIndex]` suffix, 5809-5814). Any other overlapping primitive store invalidates. All other store/write operations listed at 5843-5850 invalidate unconditionally.
5. Fail-open: `budget == 0` returns `false` (5749). `declared_ascii` returns `false` if the path tip is unresolved (5880-5885). None seen.
6. Stated limits: ASCII literal bound `<= 127` is hard-coded (5763, 5817).

## 7. `pub(crate) use` block (5911-5917)

Re-exports from `byte_predicates` and `byte_views` listed above. Counts in section 3.

## 8. `mod control_flow` (5918-6140)

1. Purpose: structural checks of the control-flow graph of a function: blocks ordered and non-empty, edges unique, terminator last, every block reachable, optional acyclicity.
2. Shape: `FunctionControlFlow { blocks, predecessors, successors }` (5927); `ControlCyclePolicy { admitted_machines: BTreeSet<MachineId> }` with `admit` / `admits` (5934-5946); `index_blocks` (5948); `validate_nodes_and_edges` (5984); `validate_total_cfg` (6062); `block_reaches` (6123).
3. How it decides: block ids must be strictly increasing (5954-5958); entry block exists and has no parameters (5969-5980); every block non-empty with a terminator last (6000-6052); successor targets known and `psi_edge` ids unique (6025-6036); reachability from entry (6068-6085), then Kahn's algorithm for cycles (6087-6119); cycles are an error unless `admit_cycles`. `validate_nodes_and_edges` also calls `provenance_matches_operation`, `metadata_matches`, `successors_match_operation` (6009-6011).
4. Same job elsewhere: `block_reaches` is a DFS also used by provenance (6728, 6880).
5. Unused: `validate_total_cfg` 2 lines (definition and one call, 9917); `ControlCyclePolicy` 9 lines, one file; callers at 1442, 9890, 18181, 18205, 18223 (grepped).
6. Stated limits: none. `admit_cycles` is a per-machine opt-in; `.admit(machine)` is called only in a test helper (18205-18207), grepped.
7. Fail-open: `admit_cycles` accepts cyclic graphs; which machines are admitted is set by whoever builds the policy (outside range). Open for this range. `.expect(...)` calls at 6008, 6038, 6042 assume a prior validation; would panic, not accept.

## 9. `mod fact_index` (6141-6309)

1. Purpose: recomputes from scratch the list of `OptimizationFact`s the function must carry and compares it with the stored one.
2. Shape: `validate_fact_index` (6145) compares `reconstruct_fact_index(function)` (6157) with `function.facts` and errors `FactIndexMismatch`.
3. How it decides: walks nodes in order and, for a fixed list of operation shapes, pushes `OperationObligationReference { obligation, support }` or `BooleanConstant`/`IntegerConstant` facts (6165-6305). Order-sensitive equality of `Vec`s (6149).
4. Cases by shape: the long operation list at 6165-6253 (scalar-field store, byte view store, record/case establish, concatenation, subslice, write, store pair, field-byte-store, read, element view subslice/establish/read/write, exact cast), `IntegerShift` with `Exact` policy (6254), `IntegerArithmetic` with `obligation()` (6262), then `runtime_indices()` for any op with a psi_operation (6276-6283). `_ => {}` (6274).
5. Same job elsewhere: suspected: fact reconstruction at unit construction (`optimization-unit/src/construction.rs`); not read. Grep: `reconstruct_fact_index` 4 lines, 1 file.
6. Fail-open: an operation with an obligation not in this list would produce no expected fact, and `function.facts` would also have to lack it to match. Inferred (not traced): the check is symmetric, so a missing entry on both sides passes.

## 10. `mod mutable_views` (6310-6656)

1. Purpose: tracks which mutable-borrow descriptor places are still current (not retired by being passed on) at each block's entry, and rejects use of a place that is no longer available, or aliasing arguments on one edge.
2. Shape: `CurrentMutableBorrows { mutable, arrivals, possible_arrivals }` (6323) with `may_be_live` (6330); `MutableBorrowTransfer` (6337); `prepare_transfers` (6344); `current_mutable_borrows` (6409, `pub(crate)`); `validate` (6575).
3. How it decides: a fixed-point dataflow with two sets per block, "definitely available" (`arrivals`, must-analysis, starts full for non-entry blocks, 6493-6498) and "possibly live" (`possible_arrivals`, may-analysis, starts empty, 6508-6511). Loops until both stop changing (6515-6567). `validate` checks every access place is in `available` (6596), checks edge bindings for two arguments naming the same place (exclusive pair, or shared plus overlapping mutable, 6606-6622), and for `ByteSequenceWrite` demands the preceding `ByteSequenceLength` of the same place with no intervening node that has place inputs (6631-6650).
4. Cases by shape: a place counts as "mutable view" if it is an exclusive-access parameter whose type is `shared_view_declaration` or an address join (6437-6454), or an operation result with exclusive access and a shared-view type (6417-6432).
5. Unused: `current_mutable_borrows` 4 lines, 2 files (grepped), so used outside this file.
6. Fail-open: `validate` returns `Ok(())` at once when `mutable.is_empty()` (6582-6584); correct if no place is tracked. Unreached blocks (not in `reached`) start with the empty set (6525), so a use there fails. `invalid` on `ByteSequenceWrite` with no prior length: `observed.is_none_or(...)` errs (6639). None seen.

## 11. `mod parameters` (6658-6721)

Two checks. `validate_entry_claim_index` (6663): the set of claims from `entry_claim_declarations` is duplicate-free and equals `function.entry_claims` (6666-6677). `validate_parameter_metadata` (6681): each function parameter's `site` is `FunctionParameter(position)` and each block parameter's is `BlockParameter{block, position}` (6691, 6706). Keyed on position. Fail-open: none seen.

## 12. `mod provenance` (6722-6913)

1. Purpose: checks the link from optimized nodes and edges back to the Psi source sites they came from, the work accounting, and the effect chain.
2. Shape: one function, `validate_provenance_work_effects` (6730).
3. How it decides: per node, provenance non-empty unless it has successors (6740); no duplicates inside a node (6745-6754); `node.work` has exactly one settlement of 1 unit per provenance site, same order (6778-6791); each edge's provenance starts with `Edge(edge.psi_edge)` and contains only `Edge` sites (6797-6803); no site shared between nodes and edges (6810-6820); effect chain increments by exactly 1 across the whole function (6843-6852). After the walk, any site used by more than one node must be an `Edge` site on exact terminal nodes (returns and crash), and no two occurrences may be reachable from each other (6868-6891); edge sites may repeat only if neither edge's target reaches the other's owner block (6892-6910).
4. Same job elsewhere: node and edge provenance are also compared with the operation in `control_flow::validate_nodes_and_edges` (6009).
5. Oddity: `for settlement in &node.work { let _ = settlement; }` (6792-6794) is a loop with no effect (read).
6. Fail-open: none seen.

## 13. `mod results` (6914-6955)

`validate_function_results` (6918) matches each `Return`, `ReturnUnit`, `ReturnStructural` against `function.result`. For `Return` it compares result value and scalar type (6930). For `ReturnUnit` and `ReturnStructural` it only compares the kind (6931-6938), so the structural signature is compared elsewhere (`structural_roots`, 8694-8832). Other operations: `continue` (6945). Fail-open: none seen.

## 14. `mod structural_roots` (6956-9289)

This is the largest block in the range (about 2,300 lines). It contains five parts.

### 14a. `availability` (6988-7319)
1. Purpose: checks each structural place is produced before it is used, by dominance.
2. Shape: `validate_structural_place_availability` (6998), `operation_place_accesses` (7065), `operation_place_inputs` (7237), two unit tests (7260, 7293).
3. How it decides: builds `producers: place -> (block, Option<node>)` from block parameters and from `structural_operation_result_contract(...).operation_result().place`, and byte-literal establishes (7010-7020). A use is available if the producer is earlier in the same block, or its block dominates the use block (7045-7050). A place with no producer is skipped (`continue`, 7044); so entry-parameter places pass.
4. Cases by shape: `operation_place_accesses` is an exhaustive `match` on `AbstractOperation` with no wildcard arm (7066-7233). A new operation variant is a compile error; the compile-time exhaustiveness is the check.
5. Unused / same job: `operation_place_inputs` 19 lines in 5 files and `operation_place_accesses` 6 lines in 2 files (grepped). It is the shared place-use list. It adds `cleanup_rosters()` places on top (7239-7241).
6. Fail-open: `producers.get(&place)` `None` then `continue` (7041-7044): a place with no producer is treated as available. Stated (inferred): entry function parameters have no `producers` row because only block parameters are inserted (7005-7007); function-level parameters are the missing case. Traced only by reading; a place that has no producer and is also not a function parameter would pass this check. Open for that case; other checks (`validate_structural_root_uniqueness`, place-kind lookups) may reject it.

### 14b. `primitive_locals` (7320-7456)
1. Purpose: validates `EstablishPrimitiveLocal`, `PrimitiveLocalStore`, `PrimitiveScalarRead`.
2. How it decides: `local_matches` requires `Unrestricted` multiplicity, no projected qualifications, establishments or claims, no entry claims, and a primitive type equal to the scalar type (7349-7356). Reads from a parameter require reading borrow, or owned with a non-empty path, `admits_scalar_read_at`, and a projection type equal to the result type (7404-7422). `_ => true` (7424) for all other operations.
3. Cases by shape: three named operations; others pass by design (this function is called for every node, 7637).
4. Fail-open: `_ => true` at 7424 is stated by the call site structure (the function is run on every node). Traced: no other operation is subject to this check.

### 14c. Helpers at 7457-7558
`place_kinds_operation_result` (7460): the place's recorded kind is `OperationResult` with this producer and type. `ReferenceMembershipValidation` (7474): holds the descriptor graph and a `DescriptorAnalysis`; `new` (7480). `validate_structural_root_uniqueness` (7493): no two places share `structural_root_key(kind)` (7498).

### 14d. `validate_structural_root_operations` (7510-8838)
1. Purpose: the per-operation catalog check for structural operations. It checks each operation's declared result, source and destination contracts against the type catalog.
2. Inputs: unit, function, type and domain catalogs, predecessors, optional `ReferenceMembershipValidation`, optional descriptor graph (`view_graph`).
3. How it decides: first, three function-wide values: `shared_descriptor_result` (7522), `aggregate_origins_valid` (7536-7546), `descriptor_origins_valid` (7547-7556). The latter two matter to `ReturnStructural`. Then for each node: `view_memberships_match` and `reference_result_memberships_match` (7621-7636), `primitive_locals::validate` (7637), then a `match &node.operation` with arms:
   - `EstablishActivationClaim` (7645): rebuilds a `terminal_psi::Operation` and asks `activation_claim_shape_in` (7654-7672).
   - `StructuralByteSequenceFieldByteStore` (7677), store pair (7688), `StructuralByteViewFieldStore` (7708), field length/read patterns (7718), `StructuralCaseMembership` (7772, requires boolean result and a Sum or Mixed endpoint containing the case), `EstablishElementView` whole representation (7806), subslice/length (7825), `ByteSequenceEqual` and `ByteSequencePredicate` whole operands (7838, 7856), `ByteSequenceRead`/`Write` (7872, 7883), `WriteOnlyPrimitiveStore` (7894), `StructuralScalarFieldStore` (7931), `StructuralScalarFieldRead` (7999), `EstablishAccessReference` (8045), `EstablishReference` (8099), `ReleaseReference` (8271), `MoveStructuralField` (8303), `StoreStructuralField` (8347), `MoveStructuralValue`/`TransformContent` (8420), `StructuralLeafCopy` (8610), `ReturnStructural` (8694).
   - `_ => {}` (8833).
4. Judgments are keyed on: place identity, `StructuralPlaceKind`, `StructuralAccess`, multiplicity, structural type ids and shapes, field ids, membership domain ids. Not on spelling.
5. Cases by shape (quoted/limits):
   - `EstablishAccessReference` rejects `AccessReferenceBacking::BorrowedExtent(_)` outright (8051-8056), and requires exactly one access-view plan per carrier (8058-8065).
   - `EstablishReference` requires source access `MutableBorrow | SharedBorrow` (8228-8232) and `Affine` result with no claims (8169, 8226).
   - `MoveStructuralValue`/`TransformContent` error text: "transfer requires the same owned carrier, multiplicity and fresh result without issuance" (8469); "descriptor flow graph is malformed" (8473).
   - `StoreStructuralField` requires `value.access == Owned` and empty path (8366-8367).
   - `ReleaseReference` accepts empty paths or `Affine` unqualified sources and only `Field`/`FixedIndex` segments (8279-8290).
   - `ReturnStructural` tries four source kinds in order: function parameter, block parameter (Owned or when result is a shared descriptor), byte literal (fixed `Unrestricted`), operation result (8702-8795). Then compares type, multiplicity, qualifications to the signature. For exactly-bounded-owned byte results the memberships must be equal and no minted domains (8806-8809); otherwise a weakening relation is accepted (8811-8822).
6. Same job elsewhere: `descriptor_results::*` and `terminal_semantics::descriptor_results::*` are used here for descriptor analysis (calls at 7541, 7550, 8193, 8206, 8474, 8537). `window_memberships` (9058) and the `StructuralScalarFieldStore` preserve checks use `terminal_semantics::scalar_field_store_preserves_*` (7960, 7971). Suspected duplicate of reference/ownership checks in `current_ownership.rs` (not read here).
7. Unused: `validate_structural_root_operations`, `validate_structural_root_uniqueness`, `ReferenceMembershipValidation` are `pub(crate)` or private; callers are outside the range (not grepped for call counts).
8. Stated limits: error-reason strings quoted above; `// A view result owns its descriptor while its access describes the borrowed backing.` (8274-8275).
9. Fail-open:
   - `_ => {}` (8833): operations not listed are not checked here. Stated by structure (other validators cover them); not traced per operation.
   - `view_memberships_match` returns `true` for any operation other than `EstablishElementView` / `ElementViewSubslice` (9158-9161) and when `projected_qualifications` is empty (9163). Fail-closed on missing evidence: `None => false` (9195).
   - `reference_result_memberships_match` returns `true` when the callee has no machine or the call is not `CallStructural` returning structural (9088-9100). Open: `callee.machine().and_then(|machine| unit.function(machine))` returning `None` (9097-9099) returns `true` (accept). What gets through: a call to a callee with no function in the unit (a boundary or dynamic callee) whose declared result carries `projected_qualifications` not in any signature. Not traced to the callers that decide whether such a callee reaches this check.
   - `copy_memberships_match` returns `true` when result has no qualifications (9234); otherwise fails closed (`view_graph?`, 9241).
   - `ReferenceMembershipValidation` given as `None` (7519, 8192-8205): the `EstablishReference` branch is more lenient: backing memberships are not added (8192), and if projected rows are not captured and the context is `None`, it returns `false` (8205). Fail-closed.

### 14e. Window and field helpers (8840-9079)
`observed_scalar_field` (8840), `relevant_scalar_field` (8872, `allow_refined_scalar`), `readable_field_type` (8906), `borrowed_window_root` (8956, three root kinds: `OwnedBlockParameter`, `Parameter`, `OwnedResult`), `root_window_loan` (9025), `window_field_type` (9035), `window_memberships` (9058).
- `observed_scalar_field` rejects `Atomic` fields unless access is `Owned` or `MutableBorrow` (8859-8867).
- `readable_field_type` returns `None` for write-only borrows, qualified non-composite sources, entry-claimed or content-claimed places, linear sources, or places that are the result of operations with claims (8912-8951).
Fail-open: none seen; `None` leads to refusal at the callers (8014, 8310).

### 14f. Memberships (9081-9288)
`reference_result_memberships_match` (9081), `view_memberships_match` (9150), `copy_memberships_match` (9224). Described in 14d.

## 15. `mod structural_transfers` (9293-9600, partial)

1. Purpose: checks block parameters and the arguments passed to them on each edge.
2. Shape: `validate` (9309, `pub(super)`), uses helpers `plain`, `qualified_owned_arrival`, `address_join`, `owner_loan`, `qualified_owned_bytes`, `qualified_shared_view`, `shared_view_carrier`, `byte_leaf_predicates`, `qualified_shared_view_source`, `source_contract` (called at 9328-9381; definitions are after line 9600, not read).
3. How it decides (read to 9600): every block parameter must have `position` equal to its index, a unique place, be one of the allowed parameter classes (9328-9336, an OR of eight classes under a negation), have a type matching the access, and appear in `structural_places` with kind `BlockParameter { block, position }` (9349-9356); no entry claims (9357-9358). Edge bindings must equal the destination's parameter count (9373). For each binding it chooses a source-match rule by parameter class in an if/else chain (9405-9599): qualified owned arrival (9405-9427), qualified shared view (9428), owner loan or qualified owned bytes (9430-9501: forwarded parameter, copied leaf, projected leaf, or owned bounded result), address join (9502-9542), shared view carrier (9543), view descriptor with path (9549), path with owned parameter (9564-9585), and the plain default (9586-9599+).
4. Cases by shape: eight named parameter classes plus the default; anything outside them is refused (9328-9336, `Err(invalid())` at 9360).
5. Same job elsewhere: the per-class rules duplicate parts of `structural_roots` (see 14d `StoreStructuralField`, `MoveStructuralValue`) in that both compare `source.multiplicity`, `access` and path-resolved types; suspected, not compared line by line.
6. Fail-open: none seen in the lines read (9309-9600). The chain continues past 9600, so it is incomplete here.
7. Error variant for every failure is `VerifiedOptimizationUnitProjectionMismatch` (9315), which does not say which rule failed.

## Coverage

- Read in full: `unit_validation.rs` lines 3601-9600 (6,000 lines), in four chunks of 1,500, 1,500, 1,500 and 1,500 lines (3601-5100, 5101-6600, 6601-8100, 8101-9600). Wait, that is 6,000 lines: 4 x 1,500.
- Skimmed: none.
- Not read: none of the assigned range. Content past 9600 belongs to the next range (`structural_transfers::validate` continues, and the helper definitions it calls are not in this unit).

Greps run (all over `source-rust` and `tools`, `*.rs`, using `grep -rw <name> ... | wc -l` for lines and `grep -rlw ... | wc -l` for files): counts are in sections 3, 4, 8, 9, 10, 14a. Also `grep -n "TODO\|unsupported\|migration\|remains\|not yet"` over the range found no hits.

## Five things a maintainer most needs to know about this range

1. Almost every check is a backward walk or an existence scan over `function.block_nodes()`, keyed by `PlaceId` and `psi_operation`. Many helpers (`derive_source_predicates` 4656, `ByteSequenceLength` existence 5179, `valid` 5418, `declared_descriptor_home` 4950) find a producer by a linear scan, so cost grows with the function size per query (read).
2. "Length was measured" is proven in two different strengths: by mere existence of a matching `ByteSequenceLength` node (`validate_source` 5178, `descriptor_sources::valid` 5418) and by dominance/unchanged-since (`validate_whole_freshness` 5598). Which strength applies depends on the operation shape (read).
3. `operation_place_accesses` (7065) has no wildcard arm, so a new `AbstractOperation` variant forces a decision there at compile time. In contrast `structural_metadata_may_change` (3865) defaults to `true` (changes) and `validate_structural_root_operations` (8833) defaults to "no check here".
4. The ASCII reasoning for byte stores is split across `byte_predicates::validate_indexed_byte_stores` (4062) and `fixed_byte_freshness` (5695); `ByteSequenceWrite` is only checked when predicates are known (4100), the one open fail-open candidate I found in byte handling.
5. `reference_result_memberships_match` (9097-9099) returns `true` when the callee is not a function in the unit; `membership` evidence for `CallStructural` results is therefore only checked for in-unit callees (read).

## What I am unsure about

- Whether any file in the crate other than `unit_validation.rs` calls the `pub(crate)` re-exports at 5911-5917; the whole-repo grep counted one file per name, which suggests `unit_validation.rs` alone, but the `candidates.rs` consumers may use `crate::unit_validation::function_structure::...` paths that did match only this file's text. I did not open the other files.
- Whether the `None` branch at 4100 (`ByteSequenceWrite` with no known predicates) is refused earlier by `validate_byte_view_source` (5112-5117). I read both but did not trace a concrete program.
- The first fail-open in 14a (place without a producer skipped at 7041-7044): I did not check which other validators reject an unknown place.
- The helper definitions `plain`, `qualified_owned_arrival`, `address_join`, `owner_loan`, `qualified_owned_bytes`, `qualified_shared_view`, `shared_view_carrier`, `byte_leaf_predicates`, `qualified_shared_view_source`, `source_contract` that `structural_transfers::validate` uses are past line 9600; I recorded only what the call sites show.
- `case_arguments::membership_on_every_path` body is before line 3601 and is not described here.
