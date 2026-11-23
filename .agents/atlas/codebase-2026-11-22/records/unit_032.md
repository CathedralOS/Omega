# Record for unit_032 (area 12-native-representations)

Checkout `C:/Development/Omega/.agents/worktree/worker-2/` at `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. Read-only; nothing was built or run.
Paths below are relative to `source-rust/subsystems/compiler/psi-to-native/representations/` (written `R/`). "read" means I read the lines; "grepped" means a Grep count or content search I ran over `source-rust/`. Grep counts include the definition line and test lines unless stated.

Unit contents (54 files, 21,796 lines), grouped by crate:
- `selected-instructions/src` (6 files): plan_identity, program_contracts, provenance, structural_case, terminator_encoding, values.
- `structural-storage/queries/src` (5 files) and `structural-storage/src` (2 files).
- `target-bound-program` (8 files, including `execution-reach/reach.rs`).
- `target-operations/legalized-operations/src` (15 files), `target-operations/src` (3 files), `target-operations/target-vocabulary/src` (15 files).

---

## 1. selected-instructions (crate `selected-instructions`)

### 1.1 `R/selected-instructions/src/plan_identity.rs` (1954 lines)
1. **Purpose.** Computes the SHA-256 identity of a `SelectedInstructionPlan` and of each function and block in it. It keeps pre-encoded "prefix pages" so register-pressure recovery re-hashes only the pages and blocks that changed (header comment, lines 1-14).
2. **Shape.**
   - `selected_instruction_plan_identity` (41): header bytes, then per-function page digests. Domain `omega.terminal-selected-instructions.v64\0` (44).
   - `RetainedPlanPrefix` (31), `RetainedHeaderKey` (104, compares ten function fields, `matches` at 133), `RetainedRegisterPage` (152, 64 registers), `RetainedFunctionPrefix` (158).
   - `selected_function_digest_with_prefix` (232), domain `omega.terminal-selected-instruction-function.v7\0` (180). `selected_function_chunks_identity` (221, pub). `selected_function_digest` (350) and `selected_block_digest` (526) carry `#[allow(dead_code)]` (349, 525).
   - `selected_block_digest_with_encoding` (403) with `INSTRUCTION_PAGE_SIZE = 64` and `MIN_REUSED_CHUNK_SIZE = 32` (359-360). Block domain `omega.terminal-selected-instruction-block.v3\0` (374). Blocks under 64 instructions are hashed whole (409-421).
   - `encode_instruction` (530), `encode_successor` (554), `encode_work` (694), `encode_scalar_type` (711), private `mod contracts` (761: `encode_static`, `encode_memory_access`, `encode_structural_block_parameters`, `encode_boundary_settlements`), `mod ordinary` (1062), `mod register` (1301).
3. **Inputs and outputs.** Reads `plan.psi`, `plan.work_schedule`, `plan.target`, `plan.entry`, `plan.contracts`, `plan.functions` (47-56), `SelectedFunction` / `SelectedBlock` fields. Outputs byte strings and `[u8; 32]` digests. Mutates only the plan's memo slot via `plan.functions.store_plan_prefix` (91).
4. **How it decides.** Reuse of a retained prefix is keyed on byte equality of the header (59, 67), `Arc::ptr_eq` of pages (70), deep equality of the ten-field header key (133-144) or of the 64-register chunk (266), and on a caller-supplied `prefix_verified` flag that skips those comparisons (238, 258, 299). Block chunk reuse is keyed on caller-supplied `unchanged_prefix` / `unchanged_suffix` instruction counts (426-461).
5. **Cases by shape.** `encode_kind`-style tags are in other files. Here: `SelectedBlockOrigin` four arms with tags 0, 3, 2, 1 (376-398); `SelectedMemoryAccessRole` about 22 arms with explicit tags (894-1037); `SelectedStructuralTransport` four arms (626-689).
6. **Same job elsewhere.**
   - `encode_scalar_type` (711-742) writes the same bytes as `R/target-operations/target-vocabulary/src/identity.rs:174-193` (both tag 3/0/1/2; integer is carrier, sign, width). `structural_case.rs:16` imports the target-vocabulary one instead. Read both.
   - `encode_work` (694-709) repeats `identity.rs:83-98`.
   - The `CallSiteOwner` tag table in `contracts::encode_static` (836-865) repeats `R/target-operations/legalized-operations/src/identity/scalar_graph.rs:1469-1498` (tags 0 Operation, 3 InvokeNoNormal, 2 ExternalCompletion, 4 Installation, 1 CleanupAction).
   - The `Crash` terminator guard encoding in `terminator_encoding.rs:51-77` repeats `identity/scalar_graph.rs:1616-1643`.
7. **Unused or one-caller.**
   - `selected_function_chunks_identity`: grepped 3 occurrences, all in `plan_identity.rs` (definition and two tests). No caller outside tests.
   - `selected_function_digest_with_prefix`: one call site (`functions.rs:119`). `selected_block_digest_with_encoding`: one non-test call site (`blocks.rs:173`) plus `selected_block_digest` (527).
   - `encode_structural_block_parameters`, `encode_memory_access` and `work_settles_only_claimed_sites` were grepped together (21 occurrences, 6 files); I did not split the counts.
8. **Stated limits.** Comment 10-12: "Every field of every function reaches the identity". The test `shared_function_chunks_keep_the_v6_canonical_identity` (1539) and its comment ("Function v6 binds structural block parameters", 1541) disagree with the domain constant `...function.v7` (180). Read.
9. **Fail-open risks.**
   - `prefix_verified == true` reuses `previous.header` and all full register pages without comparing them (238-261). A caller that sets it wrongly would get a stale digest. Class: open; I did not read `functions.rs` where it is set (call at `functions.rs:119`).
   - `.expect("validated crash predicate")` (terminator_encoding.rs:69) panics rather than rejects when a predicate has no canonical order key.
   - `expect("chunk has exact REGISTER_PAGE_SIZE length")` (291), cannot fail by construction.

### 1.2 `R/selected-instructions/src/program_contracts.rs` (361 lines)
1. **Purpose.** Holds the facts object publication needs after instruction selection: borrowed-storage layouts, target profile, ordered ABI rows, publication facts and execution facts, with a metadata digest computed once in the constructor (1-6, 47-108).
2. **Shape.** `SelectedProgramContracts` (15) with private fields and `new` (25-109), `profile`, `function_abis`, `function_abi` (binary search by machine, 119-124), `publication`, `execution`, `identity_bytes` (136, domain `omega.selected-program-contracts.v3\0`). `Default` (154) builds the empty case.
3. **Inputs and outputs.** Takes `LegalizedFunctionAbi` rows, `NativePublicationFacts`, optional `NativeExecutionFacts`. Returns `Result<Self, &'static str>` with fixed refusal strings.
4. **How it decides.** Keyed on machine ids: ABI rows strictly ascending by `machine` (32-37); publication functions equal the ABI roster position by position (38-46); parameter count of `entry_scalar_codecs` equals `call_plan.parameters` (62); for `BorrowedScalarReference` the codec must be `None` or decodable, otherwise exactly `Some(0)` (71-80); every `scalar_values()` type must convert to `ClosedScalarType` (82-86); execution is present exactly when the roster is non-empty (91); the execution entry has an ABI row (94-100).
5. **Cases by shape.** The codec rule has two shapes only (borrowed scalar reference vs everything else, 71-77).
6. **Same job elsewhere.** Roster ordering check is similar to the ABI binary search in `LegalizedOperationPlan::function_abi` (`legalized-operations/src/lib.rs:93-98`).
7. **Unused or one-caller.** `SelectedProgramContracts` grepped 30 occurrences in 12 files (users include `register-homes/src/recovery/program_contracts.rs` 8, `selected-instructions/src/lib.rs` 2, plus machine-effect and image code).
8. **Stated limits.** Doc 1-6 only; no "unsupported" comments.
9. **Fail-open risks.** `codec.is_none_or(...)` (74) accepts a `None` entry codec for borrowed scalar references. Class: stated, at `target-vocabulary/src/function_abi.rs:19-22` ("`None` means a closed scalar referent has no supported entry conversion"). A `None` row is accepted here and refused (or not) by consumers I did not read.

### 1.3 `R/selected-instructions/src/provenance.rs` (110 lines)
1. **Purpose.** Holds the semantic ids and logical work charged to one selected instruction (1-9).
2. **Shape.** `SelectedInstructionProvenance` (16: `operations`, `values`, `edges`, `obligations`, `work`); `work_settles_only_claimed_sites` (25).
3. **I/O.** Pure check over its own fields.
4. **How it decides.** Keyed on `PsiProvenance` site identity: each `WorkSettlement` has nonzero `units`, a site not seen before (BTreeSet insert, 28-29), and the site is listed in `operations` or `edges` (30-33).
5-6. None.
7. Combined grep with plan_identity (see 1.1): 10 occurrences in this file.
9. Returns `true` for empty `work` (tested, 64). A program with no settlements passes; that is by design, no wrong program reaches it from this function alone.

### 1.4 `R/selected-instructions/src/structural_case.rs` (131 lines)
1. **Purpose.** Records where a selected case-dispatch edge reads the sum, which case/tag it matches, how payload fields are transported and which cleanup actions run on the branch (1-10).
2. **Shape.** `SelectedStructuralCaseEdge` (19), `SelectedCaseDispatchSource` (28: `Local`, `Borrowed`), `SelectedCasePayloadBinding` (63), `SelectedCasePayloadTransport` (69: `Unmaterialized`, `Unused`, `Registers`); `encode_identity` (81-130).
3. **I/O.** Reads `terminal_psi::TerminalAffineCleanupAction`, `LegalizedStructuralCasePayload`; writes identity bytes; calls `effects::program::identity::encode_cleanup` (128, not read).
4. **How it decides.** Pure encoder; no judgment. `place()` on a `Local` source returns `slot.structural_place()` which may be `None` (42).
7. Not grepped separately.
9. None seen.

### 1.5 `R/selected-instructions/src/terminator_encoding.rs` (134 lines)
1. **Purpose.** One shared byte encoder for every `SelectedTerminator` arm, parameterised by instruction and successor callbacks so plan identity and fixed-view-copy recovery write the same bytes (1-8).
2. **Shape.** `encode_terminator<E>` (12-134), tags 0..=10 by arm.
3. **I/O.** Callbacks supplied by callers. A successor error returns immediately (92, 101-102, 119-120, 129-130).
4. **How it decides.** Match on terminator variant; the only judgment is `.expect("validated crash predicate")` (69).
5. **Cases by shape.** Comment says "the same ten terminator arms" (line 5-6). The match has 11 arms (EliminateEmpty 10, OperationInvokeNoNormal 9, InvokeNoNormal 8, OperationTrap 7, Crash 6, HostedExitProcess 5, Jump 4, ConditionalBranch 0, Return 1, U64LessThan 2, I64LessThan 3).
7. Grepped `terminator_encoding::encode_terminator`: 5 occurrences in 2 files (`plan_identity.rs` 3, `register-homes/src/recovery/selected_blocks.rs` 2). The "shared by two producers" claim is true for these two sites.
9. Panic on an unencodable crash predicate (stated by `validated` in the message; callers not traced).

### 1.6 `R/selected-instructions/src/values.rs` (159 lines)
1. **Purpose.** Virtual registers, their origin and operand constraints before physical assignment (1-5).
2. **Shape.** `VirtualRegister` (13), `VirtualRegisterOrigin` (23, nine variants) with `encode_identity` (66-147, tags 9, 8, 7, 6, 4, 5, 3, 0, 1), `SelectedOperand` (151: `operand`, `virtual_register`, `access`, `class`, `fixed_view`, `tied_to`, `early_clobber`).
3-4. Data only; `encode_identity` is a match.
9. None seen. Comments at 19, 156 state the fixed views are constraints, not assigned homes.

---

## 2. structural-storage

### 2.1 `R/structural-storage/src/lib.rs` (5 lines)
Declares `pub mod structural_type_catalog` and `#![forbid(unsafe_code)]` (read).

### 2.2 `R/structural-storage/src/structural_type_catalog.rs` (272 lines)
1. **Purpose.** One shared, copy-on-write catalog of structural declarations plus referents, access-view plans, view inputs, quotients, access lifecycles and owned-sequence bounds that native forms carry by `Arc` (1-4).
2. **Shape.** `StructuralTypeCatalog` (17, a seven-field tuple struct of `Arc`s and `Option<Arc>`s); `RetainedQuotients` (34), `RetainedAccessLifecycles` (28); `ptr_eq` (46, compares all seven slots); builders `with_access_lifecycles` (64), `with_quotients` (89), `new` (107), `with_owned_sequence_bounds` (127), `with_access_views` (136), `with_access_view_inputs` (145); `access_parameter_demand` (154); `make_mut` (205); `unique_structural_type` (263, free function).
3. **I/O.** Builders encode via `psi_declaration_encoding::encode_access_lifecycle_catalog` / `encode_quotient_catalog` and keep the canonical bytes next to the rows (71, 93).
4. **How it decides.** `access_parameter_demand` (154-195) joins a placed formal to exactly one input (keyed on `machine`, `runtime_position`, 160-166), exactly one carrier declaration (167-172, keyed on `structural_type`), requires equal `carrier.identity` / `access` and non-owned access (173-178), then exactly one plan for the carrier (179-185) whose five identities and placement commitment equal the input's (186-193). `with_quotients` treats a catalog equal to `EMPTY_QUOTIENTS` as absent (90).
5. **Cases by shape.** None beyond the one-to-one joins.
6. **Same job elsewhere.** The "exactly one match" filter-and-next pattern is repeated inline at 160-185 and in `unique_structural_type` (263-271). Inline uniqueness checks also at `owner_loans.rs:108-112`, `structural_inputs.rs:682-688`, `757-763`.
7. `unique_structural_type`: grepped 11 occurrences in 4 files (queries `address_joins.rs` 5, `owner_loans.rs` 4, this file 1, `04.../legalization.rs` 1). `access_parameter_demand`, `shared_view_declaration`, `with_access_view_inputs` were grepped together (24 occurrences, 8 files); not split.
8. Comment 125-126: "No constructor means no bound, rather than permission to invent one for an external input."
9. `PartialEq<Vec<..>>` (239-261) says a catalog equals a bare `Vec` only when every extra table is empty. None seen as a fail-open.

---

## 3. structural-storage queries (crate `structural-storage-queries`)

### 3.1 `R/structural-storage/queries/src/lib.rs` (8 lines)
Declares four modules (read).

### 3.2 `access_reference_validation.rs` (122 lines)
1. **Purpose.** Validates retained native access-reference backings against the authenticated view plan, and derives a borrowed-view formation from lifecycle declarations (1-8).
2. **Shape.** `validate_backing` (16), `borrowed_view_formation` (63).
3. **I/O.** Reads `AccessReferenceBacking`, `TerminalAccessViewPlan`, structural declarations; returns `Result<(), &'static str>` / `Option<(path, backing)>`.
4. **How it decides.** On source type id, schema, carrier (23-28); then per backing kind: `OrdinaryRam` compares to `terminal_semantics::access_ram_plan_correspondence` (31-36); `Resident` compares `placement_commitment`, resolves the owner by id, calls `validate_resident_owner_fields`, refuses equal `granted`/`resident` roles (39-48); `BorrowedExtent` compares commitment and owner fields (51-58). `borrowed_view_formation` keys on the boundary id (`plan.boundary == declaration.id`, 89), the `View` operation, no vacant/resident storage (90-95), the view slot's carrier (96-101) and equality of schema, policy identity, placement (102-107).
5. **Cases by shape.** Three backing kinds, closed match (30, 38, 50).
7. Grepped `borrowed_view_formation|validate_backing`: 16 occurrences in 9 files; callers in pipeline 04 (`legalization_reference_custody.rs` 4, `target_lowering/references.rs` 3, `control_flow_storage.rs` 2), 05 `selection_inputs.rs` 1, optimization-unit-semantics 2, psi semantics 2.
9. `borrowed_view_formation` returns `None` in every refusal path (71-107); callers decide. None seen as accepting.

### 3.3 `address_joins.rs` (485 lines)
1. **Purpose.** Classifies borrowed block-parameter joins whose carrier is an address or a view descriptor, and resolves referent type and access ceiling along paths with reference crossings (1-13).
2. **Shape.** `is_address_join` (22), `is_descriptor_join` (39), `is_descriptor_join_in` (61), `is_address_join_in` (70), `is_static_projection` (79), `is_address_projection` (88), `loan_access_permitted` (101), `borrowed_referent_type` (150), private `borrowed_address_access` (161), `referent_type` (196). Tests 207-485.
3. **I/O.** Reads `StructuralParameterDeclaration`, declaration iterators; calls `terminal_semantics::{borrowed_projection_type, runtime_read_projection_tip}`.
4. **How it decides.**
   - `is_address_join`: access is shared / mutable / write-only, multiplicity `Unrestricted`, the declaration exists, ids equal, shape is not a view descriptor (26-34).
   - `is_descriptor_join`: shared or mutable, `Unrestricted` or `Affine`, the declaration is a view descriptor, and projected qualifications are empty unless the shape is `ElementView` with only element-membership rows (43-58).
   - `loan_access_permitted` (101-148): ceiling starts at source access, each `Referent` segment attenuates it (write-only refuses, shared stays shared, owned/mutable takes the reference's declared access, 119-139), then a table of allowed (ceiling, destination) pairs (140-147).
   - `borrowed_address_access` (161-194): a `Referent` is allowed when the reference's permission equals the requested access, or is mutable and the request is shared/write-only; a write-only reference must be the last segment (182-189).
5. **Cases by shape.** `is_address_projection` allows only `Field`, `Case`, `FixedIndex`, `Referent`, `RuntimeIndex` (88-99); `is_static_projection` only `Field`, `Case`, `FixedIndex` (79-86).
6. **Same job elsewhere.** The reference-crossing permission rule at 170-192 repeats at `R/target-operations/legalized-operations/src/structural_inputs.rs:1673-1701` (`reference_projection`: same `granted == access || granted == MutableBorrow && ...` test and the write-only later-`Referent` test). Read both.
7. Grepped: `is_descriptor_join_in|is_address_join_in` 25 occurrences in 7 files (7 in this file); `is_static_projection|is_shared_byte_view|...|is_shared_view_carrier_loan` 18 occurrences, 7 files; `loan_access_permitted|borrowed_referent_type` 24 occurrences, 6 files (12 in this file).
9. None seen as accepting on missing data: every `None` from a lookup returns `false`/`None` (e.g. 32-34, 109-117, 174-175).

### 3.4 `owner_loans.rs` (158 lines)
1. **Purpose.** Classifies bounded byte owner storage at invocation (owner loan vs by-value) and shared loans of records whose members are plain storage or shared-view descriptors (1-6).
2. **Shape.** `is_bounded_owner_loan` (18), `_in` (29), `is_shared_byte_view` (38), `is_owned_bounded_byte_parameter` (53), `_in` (64), `is_shared_view_carrier_loan` (73), `_in` (85), private `view_carrier_shape` (96). Re-export `loaned_field_type = terminal_semantics::borrowed_storage_field_type` (16).
3. **I/O.** Declaration lookups; recursion on child structural types with an `active` stack (99-125).
4. **How it decides.** Keyed on parameter `access`, `multiplicity`, empty `projected_qualifications` and declaration shape. `view_carrier_shape` accepts record/mixed shapes whose fields are all either erased or relevant and one of: structural child that is an element view / borrowed byte view or recursively a carrier (115-126), bounded-owned byte sequence, borrowed-view byte sequence, `Erased`, or any field with `scalar_type().is_some()` (128-144). A child that is missing, duplicated or already on the stack fails (109-114).
5. **Cases by shape.** Record or mixed only (`record_or_mixed_shape_pattern!`, 147); all other shapes return `false` (156).
9. `member_valid` returns `true` for `Erased` fields and for any scalar-typed field (142-143). A record with an erased or scalar field is a valid view carrier; class: stated in file header ("members are plain storage or shared-view descriptors").

### 3.5 `storage_projection.rs` (449 lines)
1. **Purpose.** Answers which storage a structural path selects: an endpoint type, a collection window, or a byte field (1-4).
2. **Shape.** `shared_view_declaration` (196), `StructuralStorageProjection` (202: `Endpoint`, `CollectionWindow`, `ByteField`) with `matches` (216), `structural_storage_projection` (249), `structural_borrowed_storage_projection` (269), private `byte_field` (288), `structural_projection_type` (321), `resolve_path` (342). Test module sits at 13-195 above the code.
3. **I/O.** Declaration iterator, root id, path; `terminal_semantics::{fixed_collection_storage_window, primitive_projection_declaration, borrowed_referent_matches, borrowed_primitive_projection_declaration, representation_view_declaration}`.
4. **How it decides.** `resolve_path` walks segments: `FixedRange` needs a literal-length `FixedArray`, `start <= end <= length`, no pending case payload and a following `RepresentationView` (353-372); `Case` needs `Sum`/`Mixed` and an existing case (373-390); after a case only `Field` is allowed (391-393); `RepresentationView` refuses a `Reference` shape (402-404); `FixedIndex < length` on literal arrays; `RuntimeIndex` on any `FixedArray`; both index kinds on `ElementView` (426-445); anything else returns `None` (445). Ends with `payload_fields.is_none()` (448). `structural_projection_type` falls back to `primitive_projection_declaration` when `resolve_path` fails (337-339).
5. **Cases by shape.** The match at 394-446 is a fixed list: representation view, field, fixed index (literal arrays), runtime index (arrays, element views), fixed index (element views).
7. Not separately grepped.
9. `structural_borrowed_storage_projection` (269-286) returns `Endpoint` from a primitive-projection shortcut for any non-owned access before the normal path; stated nowhere in comments (class: open for the question whether the shortcut can accept a path the normal route refuses; I did not read `borrowed_primitive_projection_declaration`).

---

## 4. target-bound-program (crate `target-bound-program`; lib path `bound_reach.rs`)

### 4.1 `bound_reach.rs` (179 lines)
1. **Purpose.** Binds reach, storage geometry and scalar demand to one native entry (1).
2. **Shape.** `TargetBoundReach` (31: `target_context`, `entry`, `emitted: Arc<ExecutionReach>`, `layouts`, `scalar_applications`, `owned_sequence_bounds`, `demands`, `selected_candidate_positions`); `bind` (45-109); `full_module_owned_sequence_bounds` (112); `full_module_scalar_demand` (118); accessors; `contains_candidate_position` (174, binary search).
3. **I/O.** Reads `TerminalArtifactView`, `TerminalTargetContext`, `TargetProfile`, `ExecutionReach`, optional parent. Calls `closed_owned_sequence_bounds`, `module.boundary_application_demands_for_machines`, `psi_target_layouts::admit_target_layouts`, `ScalarDemand`.
4. **How it decides.** Rejects when the target context's artifact identity or target name differs (54-58), when the entry is not in the reach (60-62), when any selected candidate position is at or past the catalog length (64-71). A child scope reuses the parent's layouts only if `parent.target_context == targets` (78-84); otherwise it admits address representation and layouts itself (86-92).
5. **Cases by shape.** None.
7. `bind`: one call site, `native_execution_scope.rs:204`. `full_module_*`: grepped 5 occurrences in 3 files (`bound_reach.rs` 3 and two pipeline-02 files).
9. `selected_candidate_positions` may be empty and passes (64-71). Class: traced as intentional (empty selection).

### 4.2 `builtins.rs` (138 lines)
1. **Purpose.** Records the compiler builtins selected for reachable boundaries before instruction selection (1).
2. **Shape.** `SelectedCompilerBuiltin` (13: `requirement`, `plan_index`, `selection`, `artifact`, `boundary`, `identity: Option<..>`, `format`, `satisfied_schema`); `select` (27-106); accessors; `belongs_to` (135).
3. **I/O.** Consumes `SelectedProviderPlanFacts`, intrinsic provenance iterator, demanded boundary names (a `BTreeSet<String>`, 30), direct-boundary leaves; calls `visit_selected_intrinsics`, `selected_external_execution`, `reconcile_numeric_meaning`.
4. **How it decides.** Direct leaves are matched by `leaf.plan == plan` (47); two matches are an error ("competing direct callable custody", 48-54); with a leaf it validates custody and requires `satisfied_schema` and `execution == Intrinsic(selected_row.execution)` (57-67), without one it asks `selected_external_execution` (69-77). `belongs_to` accepts pointer equality or value equality (136).
5. Builtins are demanded by name strings (`demanded_boundaries: &BTreeSet<String>`, 30) and `requirement` is a `String` copy (92). Keyed on source spelling (requirement identity text), then resolved to plan index.
9. `identity: Option<CompilerIntrinsicExecutionIdentity>` is kept even when `None`. Class: stated at 19-20, "A selected row can lack a closed identity. The receiving callable refuses it when its execution actually reaches this boundary."

### 4.3 `entry_call.rs` (42 lines)
`SelectedProgramEntryCall` (9) holds boundary id, checked entry receipt, callee machine, provider plan digest; constructor `settled` is `pub(crate)` (17). One producer: `bind_selected_program_entry_call` (`entry_settlement.rs:701`). Read in full. No decision code.

### 4.4 `entry_settlement.rs` (922 lines)
1. **Purpose.** Pairs the selected source entry declaration with its evaluated calling contracts, replays the canonical artifact and target machine selection, and seals owned entry custody (1-9).
2. **Shape.** Four inline modules.
   - `calling_plans` (10-267): `validate_paired_calling_plans` (19), `has_internal_storage_signature` (159), `selected_method` (197), `join_application` (243); one `#[test]` inside the module (218).
   - `model` (268-536): `NativeProgramEntrySettlement` (280, borrowed record), `validate_for_target` (354), `ValidatedNativeProgramEntrySettlement` (396), `NativeProgramEntrySettlementError` (516, eleven variants).
   - `physical_adapter` (537-691): `PhysicalProgramEntry` (543), `bind` (577).
   - `semantic_call` (692-826): `bind_selected_program_entry_call` (701).
   - Top level: `validate_native_program_entry_settlement` (839-922).
3. **I/O.** Reads `SelectedProgramEntrySignature`, `BoundaryCallingApplication` x2, `SelectedProgramStorageEntryPlan`, canonical artifact, provider selection, adapters. Produces the validated settlement or a `String` / error enum.
4. **How it decides.**
   - Two calling surfaces must both be present or both absent: `(false, None, None, None)` ok, `(true, Some, Some, Some)` validated, else error (370-385). "Declares two surfaces" is true when any of five target-slot fields is set (365-369).
   - Methods are found by splitting a spelling at the last `::` into owner and method name and matching `requirement_owner` and `name` (201-206). Keyed on source spelling.
   - Semantic and physical arrival requirement identities must differ (48) and the semantic text must equal the storage requirement identity (43-47).
   - Calling policy must equal `for_program_entry_convention(...)` for each surface (66-76).
   - Semantic pairing has two cases keyed on `slot.schema` (85-106): `ProgramStorageApplication` (declared type signature equal and parameter shapes equal) and `HostedApplication` (no visible parameters, two parameters of equal type identity, shapes `integer(16, 8)` twice, internal storage signature check).
   - Physical contract is compared field by field (117-152).
   - `bind_selected_program_entry_call` requires the owner to be `TargetProfile::UefiX64` (792) and exactly two structural parameters, no scalar parameters, unit result (798-802).
   - Top-level function checks target, source signature identity, declaration text equality, canonical Psi identity twice, physical adapter belonging, exactly one machine with the entry id, and declaration machine equality (845-903).
5. **Cases by shape.**
   - `has_internal_storage_signature` (159-195): "two parameters, no result, each a record of two 8-byte integer fields at offsets 0 and 8, record size 16, alignment 8".
   - Hosted arm: `semantic_shapes == [integer(16, 8); 2]` (103).
   - `bind_selected_program_entry_call` handles one target profile only (792).
6. **Same job elsewhere.** `entry_count != 1` count at 889-896 and the `validate_empty_erased_invocation_inputs` calls at 665, 818, 905 repeat per path (three call sites in this file).
7. `PhysicalProgramEntry` is exported `pub`; `bind_selected_program_entry_call`: one caller (`native_execution_scope.rs:94`). `NativeProgramEntrySettlement::with_checked_entry`: not grepped.
8. None quoted.
9. `validate_for_target` errors are discarded: `.map_err(|_| CallingPlanPairingDrift)` (851). Not an accept. The `Component` root branch of `NativeScopeSource::admit` (below) passes with only a supplier identity check. None else seen.

### 4.5 `execution-reach/reach.rs` (51 lines)
`ExecutionReach` (9: sorted `machines`, sorted `boundaries`, `callbacks`), `ReachedCallback` (16), `new` sorts machines and boundaries but not callbacks (27-28), lookups by binary search (41, 45). Read in full. `ExecutionReach::new` does not remove duplicates.

### 4.6 `native_execution_scope.rs` (371 lines)
1. **Purpose.** Keeps each entry's reachable machines with the program and provider choices used to find them (1).
2. **Shape.** `NativeRoot` (13: `Program`, `Component`), `NativeScopeSource` (55) with `admit` (66-117), `program` (123), `NativeScopeBinding` (137) with `checked` (146) and `bind_target_reach` (199), `NativeExecutionScope` (226) with `require_source_input` (237), `require_source` (297), `ScopedProviderSelection` (313) with `bind` (320) and `callable` (359).
3. **I/O.** Reads `AuthorizedTerminalArtifact`, selection, adapters, callbacks, `native_bindings: [u8; 32]`, intrinsic executions. Produces scopes carrying `TargetBoundReach`.
4. **How it decides.** Artifact identity equality (81, 127, 327); adapters belong to the selection (76-78); program root's physical adapter selection digest equals adapters' digest (86-93); component root's `export.provider()` equals the artifact identity (100-104). `entry_call()` is returned only for a scope with no parent (`parent.is_some()` at 188; `program_entry` flag at 287). `require_source_input` compares artifact, entry, target name, selection and adapter rows by value (245-251).
5. **Cases by shape.** `NativeRoot` has two shapes; a `Component` root gets `entry_call = None` (99-106).
7. `NativeScopeSource::admit` and `ScopedProviderSelection::bind`: not separately grepped. `TargetBoundReach::bind` via `bind_target_reach`: one caller (`01.../execution-scope/scope.rs`).
9. None seen.

### 4.7 `provider_bindings.rs` (164 lines)
1. **Purpose.** Provider adapter rows selected for one entry's reachable boundaries (4).
2. **Shape.** `ScopedProviderBindings` (5), `SelectedProviderAdapter` (13: strings plus a `ProviderPlanDigest`), `SelectedProviderAdapters` (24) with `from_selected_plans` (30), `without_imports` (41), `require_selection` (56), `retain_selected_provider_adapters` (77-164).
3. **I/O.** Reads `SelectedProviderPlanFacts`; produces adapter rows sorted by (requirement text, plan digest bytes) (145-153).
4. **How it decides.** Rows with `ProviderBinding::CheckedAdapter` only; refuses an empty provider type, empty requirement text, empty machine identity, or a machine package identity differing from `plan.origin_package_identity` (91-100); one selecting declaration per plan, non-empty (101-112); repeated (requirement, plan digest) is an error (154-162). `without_imports` drops rows whose plan digest is in the imported list (46-51). Keyed on identity text strings (requirement, machine, provider declaration).
7. `without_imports`: grepped 2 occurrences (definition and `component_inputs.rs:1`). `retain_selected_provider_adapters` is also called from `from_selected_plans` (34).
9. None seen.

### 4.8 `scalar_demand.rs` (216 lines)
1. **Purpose.** Finds scalar applications and storage extents used by the selected machines and boundaries (1).
2. **Shape.** `ScalarDemand` (6: `emitted: Option<&ExecutionReach>`, `owned_sequence_bounds`); `scalar_application_demand` (22-65); `storage_extent_demand` (75-214).
3. **I/O.** Reads the module's machines, operations (`ReadScalarApplication`), scalar catalog, structural types, layouts, boundary machines. Produces sorted, deduplicated `Vec<ScalarApplicationId>`.
4. **How it decides.** `emitted == None` means all machines and boundaries are in scope (13-20); otherwise membership in the reach. An `Evaluate` application whose entry machine is in scope is demanded only if the condition catalog has it as a subject (41-60). Storage roots come from machine parameters (skipping direct borrowed slices, 91-100), results, places, operation results and storage uses (102-189), and boundary parameters/results (190-205). Uses `BTreeMap`/`BTreeSet` keyed on structural type id and place id (71, 85, 101, 137).
6. Layout of borrowed-slice detection appears twice (92-98 and 128-135).
7. Not separately grepped (`full_module_*` counted in 4.1).
9. `emitted: None` accepts every machine and boundary (13-20). Class: traced, only `full_module_scalar_demand` passes `None` (bound_reach.rs:123-127); stated at bound_reach.rs:111.
Note: dense-id ratchet applies to `BTreeMap`/`BTreeSet` of ids (AGENTS.md); I did not check `tools/ratchets/dense_id_maps.txt`.

---

## 5. legalized-operations (crate `legalized-operations`)

### 5.1 `src/lib.rs` (306 lines)
1. **Purpose.** Defines the sealed stage-04 output: the legalized plan and its prepared (sealed) wrapper with a receipt (1-35, 54-259).
2. **Shape.** `LegalizedOperationPlan` (54: `psi`, `optimization_unit`, `work_schedule`, `target`, `profile`, `entry`, `scalar_functions`, `installation_operations`, `function_abis: Arc<[..]>`, `borrowed_storage_layouts`, `publication`, `execution: Option<..>`), `entry_function` (86), `function_abi` (93); `PreparedExecutionPlan` (109) with `seal` (118-148), `callable_invocations_from` (182), `is_intact` (198); `PreparationReceipt` (209); `LegalizedOperationPlanIdentity` (245); inline `mod legality` (261-270) holding `LegalizedExactIntegerOperator`; re-export blocks 273-306.
3. **I/O.** `seal` takes the plan and an `OptimizationValidatorIdentity`, copies execution facts, checks four roots equal the plan's, computes the identity (118-148).
4. **How it decides.** `seal` refuses when `execution` is `None` (122-126) or when `psi`, `optimization_unit`, `work_schedule` or `entry` roots differ (128-134). `callable_invocations_from` returns `None` unless a scalar function with that machine id exists (186-193). `is_intact` recomputes the identity and compares five receipt fields (198-205).
5. `function_abi` uses binary search over `function_abis` (94-97) and so relies on the sort order documented at 71.
6. Header comment 27-35 says "four subordinate owners ... `legality` ... `calls` ... `control_flow` ... `identity` and `validation`"; the crate also has `selection`, `function_abi`, `native_geometry`, `structural_inputs`, `installed_interrupt_geometry` (47-51, 305).
7. Grepped: `is_intact` 3 occurrences (definition, `04.../legalization.rs`, `tests/physical_publication/mod.rs`); `entry_function(` 15 occurrences in 7 files; `callable_invocations_from(` 8 occurrences in 6 files.
9. None seen. `seal` does not verify `plan.scalar_functions` against `function_abis`; I did not find that check in this crate.

### 5.2 Small modules
- `calls.rs` (8): re-exports from `target_vocabulary` (read).
- `control_flow.rs` (34): re-exports from `control_flow_scalar_graph.rs` via `#[path]` (3) and defines `LegalizedStructuralCaseSuccessor` (24: edge, target, case, tag, payloads, structural bindings, cleanup actions, cleanup calls, work).
- `function_abi.rs` (3): re-export (read).
- `selection.rs` (16): `PreparedInstructionSelection` (9), a plain pub-field record of an `Arc<PreparedExecutionPlan>`, identities, `selections`, `budget_per_pass`.
- `validation.rs` (33): `LegalizedScalarInstruction::has_valid_hosted_read_byte_shape` (8-32) accepts exactly one shape: no result, ownership equal to `[ClaimCompletion([])]`, affine result with no qualifications or claims, layout `integer(8, 4)`, tag at 0 of `integer(4, 4)`, no common fields, payload at 4, two cases, case 0 empty, case 1 one `integer(4, 4)` field at 4 (9-31). Grepped `has_valid_hosted_read_byte_shape|stored_plan_placements|validate_no_normal_shape` together: 15 occurrences, 8 files; `has_valid_hosted_read_byte_shape` appears in pipeline 04 `legalization_source.rs` and pipeline 05 `selection_*` files. Note `HostedReadByteRealization::supports_target` is always `false` (`target-vocabulary/src/boundary/realizations.rs:35-39`), yet this validator still describes the hosted read shape.
- `identity.rs` (61): `legalized_operation_plan_identity` (7-44), domain `omega.terminal-legalized-operations.v95\0` (11), then `ordinary-scalar-graph.v47\0` (37). Re-exports all of `target_vocabulary::identity` (5). It encodes `plan.profile` as an option of its target-name string (17-21) and the plan's installation operations (23-27).

### 5.3 `control_flow_scalar_graph.rs` (1550 lines)
1. **Purpose.** Defines ordinary scalar/Unit instruction graphs: functions, blocks, instructions, terminators (1-10).
2. **Shape.**
   - `LegalizedScalarFunction` (30) with `block_index` (48, binary search by `BlockId`), `references_value` (52), `visit_closed_types` (71), accessors.
   - `LegalizedScalarBlock` (147), `LegalizedScalarInstruction` (167) with `references_value` (177) and `visit_scalar_operands` (183-470).
   - `LegalizedScalarInstructionKind` (474-1029, about 100 variants), `get_structural_result` (1032-1135).
   - `LegalizedScalarComparison` (1255: `Equal`, `LessThan`, `LessOrEqual`), `LegalizedStructuralBinding` (1262), `LegalizedScalarSuccessor` (1268), `LegalizedStructuralCaseSource` (1278, four variants), `LegalizedScalarTerminator` (1318-1394, ten variants) with `checked_completion_call` (1397), `cleanup_calls` (1433), `references_value` (1457), `LegalizedScalarReturnValue` (1514), `LegalizedScalarReturn` (1532).
3. **I/O.** Data records; the visitors produce `ValueId` callbacks.
4. **How it decides.** `block_index` is a binary search, so it relies on blocks being in strictly increasing `BlockId` order (comment line 4-5; no check in this file). `references_value` is a name/identity test on `ValueId`. `LegalizedScalarTerminator::references_value` (1457-1508) answers per terminator shape.
5. **Cases by shape.** Both `visit_scalar_operands` and `get_structural_result` are exhaustive matches over every instruction kind with no wildcard (184-469, 1033-1134), so a new variant forces an edit. `LegalizedScalarTerminator::references_value`:
   - `Crash` returns `false` (1483).
   - `InvokeNoNormal` checks call arguments and structural argument indices but not `call.target.selector()` (1471-1475), whereas the instruction visitor visits the selector (415-417).
   - `ExternalCompletion` checks scalar `arguments` only; its `structural_arguments` (paths may hold `RuntimeIndex` values) are not visited (1476-1478).
   - `ReturnCleanup` checks `returned` only, not the `calls` (1496-1499); `Jump`/`Conditional` check `bindings` and `structural_bindings` indices but not `successor.cleanup_calls` (1458-1469).
   - `Return(Structural { source })` is not visited (1497-1499).
6. **Same job elsewhere.** Two walkers of the operand lists: `visit_scalar_operands` (183) and `visit_closed_types` (71).
7. `references_value`: callers `selection_inputs.rs:2989, 2996` and `selection_scalar_graph.rs:3220, 3242` (grepped content search) plus tests. `visit_scalar_operands(`: grepped 4 occurrences (1 in pipeline 05 `selection.rs`).
8. None.
9. Terminator `references_value` omissions above. Class: open. The callers decide whether a definition has a use; I did not read those four sites, so I cannot say whether a missed use there drops a needed value.

### 5.4 `identity/scalar_graph.rs` (1844 lines)
1. **Purpose.** Writes the canonical bytes of each `LegalizedScalarFunction` into the plan identity (1-16).
2. **Shape.** `encode` (40-1337): function header, parameters, blocks, per-instruction tag plus fields plus work, effect, ownership roster, terminator, frontier fact identities. Helpers: `encode_atomic_event` (1339), `encode_structural_source` (1426), `encode_successor` (1449), `encode_cleanup_calls` (1463), `encode_terminator` (1505), `trapping_tag` (1758), `saturating_tag` (1762), `encode_store_destination` (1777), `encode_obligated_pair` (1800), `encode_case_pointer_slots` (1813), `encode_byte_operand` (1823).
3. **I/O.** Reads the function; appends bytes; `cached_catalog` is a one-slot cache keyed by `StructuralTypeCatalog::ptr_eq` (43-47, `identity.rs:315-319`).
4. **How it decides.** Pure encoder. Tags are chosen per variant. `saturating_tag` keeps legacy tags for `U64` Subtract/Add (35, 36) and `I32` Add/Subtract/Divide (40, 41, 42), else `43/51/59/73/87 + carrier.ordinal()` (1762-1775). Trapping forms use `95 + ordinal` (1759).
5. **Cases by shape.** Exhaustive match over instruction kinds (96-1326).
6. **Duplicate first-byte tags (read; grepped `bytes.push(N)` for N in 24, 92, 40-44, 46, 88-94).** The header comment says "Every instruction variant ... maps to a distinct tag byte so no two instruction shapes collide" (4-5). The code writes the same first byte for these pairs:
   - 24: `StructuralCaseMembership` (555) and `BitwiseAnd` (1290).
   - 92: `StructuralLeafCopy` with a `Case` path (574-577) and `StructuralByteViewFieldStore` (846).
   - 40, 41, 42: `EstablishElementView` (999), `ElementViewSubslice` (1060), `ElementViewRead` (1078) versus saturating `I32` Add / Subtract / Divide (1766-1768).
   - 43, 44, 46: `ElementViewLength` (1097), `ElementViewWrite` (1041), `ByteSequenceEqual` (986) versus saturating Add with carrier ordinals 0 (`I8`), 1 (`I16`), 3 (`I64`) (`43 + ordinal`, 1769; ordinals at `target-vocabulary/src/arithmetic.rs:126-137`).
   - 88, 89, 90, 91, 92, 93, 94: `StoreStructuralField` (618), `EstablishTrivialAffineLocal` (649), `StructuralByteSequenceFieldRead` (770), `EstablishZeroedStructural` (632), the 92 pair above, `EstablishStructuralLocal` (637), `EstablishAccessReference` (437) versus saturating Multiply `87 + ordinal` for `I16`..`U64` (1773).
   The bytes after the tag differ between each pair (I compared the field lists only), so I did not construct an identical byte string. These are tag collisions in the first byte only.
7. Not separately grepped.
8. None quoted.
9. Panics: `.expect("validated crash predicate")` (1635). `claim_transfers` encoding is in `identity/calling.rs:236-240` (see 7.5).

### 5.5 `installed_interrupt_geometry.rs` (173 lines)
1. **Purpose.** Derives offsets and sizes of the installed x86 interrupt-table operand records from selected storage layouts (1).
2. **Shape.** `InstalledInterruptOperandLayout` (10, 12 fields) and `derive` (61-172); private `fields` (31), `element` (48), `scalar` (24), `failure` (7, identity function).
3. **I/O.** Reads `NativeGeometry`, six structural type ids; uses `structural_inputs::structural_reference_input` (shape, project, primitive_geometry, case_layout, align).
4. **How it decides.** Hard numeric limits: table size nonzero, at most 4096, multiple of 16; GDT 16..=65536, multiple of 8; task state exactly 104 bytes (71-78). Stack row: element view of a record with two runtime fields, first an exclusive-borrow `ElementView` of `u64`, second `u8` at some offset (83-114). Arrival row: first field `u16`, second a sum of three empty cases with a 4-byte tag at offset 0 (115-151). Tags are fixed constants `0, 1, 2` (168-170).
5. **Cases by shape.** `fields.len() != 2` (40); `cases.len() != 3` (135); fixed geometry above.
6. Stage 05 consumes this in `selection_interrupts.rs` (not read).
7. `InstalledInterruptOperandLayout`: grepped 7 occurrences in 4 files (stage 05 `mod.rs`, `selection_interrupts.rs`, image-emission `function_fragments.rs` 3).
9. None seen.

### 5.6 `native_geometry.rs` (564 lines)
1. **Purpose.** Shares prepared native layouts across queries in one lowering operation (1-5).
2. **Shape.** `NativeGeometry` (17: `physical`, `carriers`), `with_owned_sequence_bounds` (30), `location_alignment_limit` (56), `storage_region` (64), `storage_view_source_compatible` (72), `RecordStorageResolver` impl (91-133), `NativeGeometries` (142) with `new` (147) and `get` (181), `exact_cast_has_native_carriers` (199), `NativeInput` (212) and alias `NativeFunction` (235).
3. **I/O.** Wraps `calling_conventions::terminal_layout::NativeGeometry`; `Deref` to it (84-89).
4. **How it decides.** `NativeGeometries` keys catalogs by slice pointer address and length for both declarations and bounds (169-179), sorts and dedups by that key (157-158), and looks up by binary search (186-193). The record resolver maps a physical layout error to a `StorageGeometryError` (97-117) and always refuses `realization` (131, comment 129-130: "This owner consumes closed native declarations, not an earlier module's publication.").
5. None.
7. `NativeGeometries`: grepped 11 occurrences in 5 files. `NativeInput|NativeFunction`: 290 occurrences in 14 files (mostly pipeline 04 and 05).
9. None seen as accepting. Lookup of a catalog by a different slice with equal contents returns `None` (key is the address).

### 5.7 `structural_inputs.rs` (2790 lines)
1. **Purpose.** Projects retained native layouts into pointer/descriptor slots, collection views and unit parameter contracts shared by stage 04 and stage 05 (1-6).
2. **Shape.** Two public modules: `structural_reference_input` (8-2692) and `structural_unit_input` (2693-2788).
   - Key types: `RuntimeElement` (86), `PointerSlot` (142), `ReferenceSourceGeometry` (148), `StorageProjectionRegions` (435), `PrimitiveAccessGeometry` (546), `ReferenceProjection` (1652), `ProjectedByteField` (2511), `collection_views::{ProjectedCollection, CollectionViewGeometry}` (1006, 1040), `BoundedByteViewGeometry` (1354), private `Projection` (1791).
   - Key functions: `storage_projection_regions` (444), `descriptor_projection` (320), `primitive_geometry` (676), `primitive_read_geometry` (771), `primitive_store` (597), `atomic_cell_geometry` (554), `collection_view_region_geometry` (1064), `bounded_byte_view` (1360), `bounded_byte_field_borrow` (1434), `reference_projection` (1660), `project_storage` (1868-2006), `runtime_storage_projection` (2066), `scalar_field_geometry` (2191), `field_read` (2387), `case_field_read` (2463), `projected_byte_field` (2519), `typed_collection_layout_matches` (2654); in `structural_unit_input`: `accepts_graph` (2717), `owned_indirect_pointer` (2764).
3. **I/O.** Reads a `NativeGeometry` and declarations; returns `Option<..>` geometries (byte offsets, sizes, runtime index elements, pointer slots). Almost every function returns `None` for anything it cannot place.
4. **How it decides.**
   - `storage_projection_regions` (444-544): at each `Referent`/`FixedIndex`/`RuntimeIndex` segment it resolves the carrier, requires the carrier shape to be exactly `integer(descriptor_size, 8)` with descriptor size 16 for a scalar referent else 8, and the access to be in storage bounds (515-525).
   - `scalar_field_geometry` matches field type against requested scalar by case: bounded/borrowed byte sequences only with `observes_byte_length` and `u64` (2254-2283); element-view / borrowed-view child likewise (2284-2316); `Scalar`/`Atomic`, `IeeeFloat`, `BoundedInteger`, `RefinedScalar` compare the carrier (2317-2330); everything else `false` (2331).
   - `collection_view_region_geometry` (1064-1313) accepts a fixed set of (source access, argument access) pairs (1072-1086), multiplicity cases (1087-1099), element type defaulting to the first unscoped `u8` declaration (1132-1151, keyed on integer width 8 unsigned), and several region cases.
   - `accepts_graph` (2717-2762) recomputes the call plan and compares for equality (2754-2761).
5. **Cases by shape.** `primitive_array_shape` loops at most `declarations.len()` times over `FixedArray` nesting and accepts only fixed-width, non-address integer elements (889-913). `field_read` only accepts `Field`, `FixedIndex`, `Case` path segments (2394-2403). `case_field_read` refuses `RuntimeIndex`, `Referent`, `FixedRange` (2471-2480). `owned_indirect_pointer` accepts exactly one indirect location with a copy offset (2774-2786). Comment 906: "Scalar operation eligibility is narrower than storage layout."; 338-339: "Only an inline field without a declared carrier uses the older shape-based lookup. An authored carrier must resolve exactly."
6. **Same job elsewhere.**
   - The reference-crossing access ceiling test at 1673-1701 repeats `structural-storage/queries/src/address_joins.rs:161-194`.
   - Inline "exactly one declaration with this id" filters at 682-688 and 757-763 repeat `unique_structural_type`.
   - Two nearly equal borrowed-slice carrier checks appear in `scalar_demand.rs:92-98, 128-135` (different crate).
7. `structural_reference_input`: grepped 146 occurrences in 21 files (pipeline 04 `legalization_scalar_graph.rs` 27, pipeline 05 `selection_inputs.rs` 24, image-emission 7). `owned_indirect_pointer`: 4 occurrences in 4 files (one definition, three call-site files). Several `pub` helpers (`reference_source_geometry`, `leaf_copy_projection`, `bounded_byte_field_owner`, `field_store_geometry`) were not grepped.
8. None quoted besides the two above.
9. None seen as accepting on missing data; every lookup failure is `?` or `false`. One accepting branch: `scalar_access_within_storage` returns `true` for any root that is a borrowed element view or borrowed byte view without a bounds check (2365-2376); class: stated implicitly by the name `descriptor_frame`, no comment.

### 5.8 Tests (read in full; summarised, not reviewed)
- `tests.rs` (1092): shared fixtures `call_aware_plan` (85), `installed_provider_plan` (321), `scalar_call_unit_plan` (405) and tests of `validate_source` and `validate_shape` (539-1092). A fixture uses `NativeTarget::from_omega_target_name(Some("uefi_x86_64"))` (166).
- `tests/identity.rs` (969): identity drift tests, helper `assert_identity_drift` (29).
- `tests/scalar_operations.rs` (885): identity drift tests for scalar, structural-edge and lifecycle instructions; `saturating_multiply_identity_binds_carrier_operation_and_operands` (679) loops over `SaturatingCarrier::ALL`.
- No test asserts the distinctness of first-byte tags across instruction kinds (grepped `bytes.push` is not used in the tests; I read the three files).

---

## 6. target-operations (crate `target-operations`)

### 6.1 `src/lib.rs` (60 lines)
`TargetOperationPlan` (28: `psi`, `target`, `profile`, `entry`, `functions`, `native_callback_arguments`, `borrowed_storage_layouts`, `publication`) with `function` linear search (42-46). `pub use target_vocabulary::*` (49).

### 6.2 `src/control_flow.rs` (271 lines)
`TargetFunction` (18), `TargetStructuralContract` (31), `TargetControlGraph` (40), `TargetControlBlock` (56), `TargetOperationMetadata` (68), `TargetControlTerminator` (97-174, ten variants), `TargetStructuralReturnSource` (177), `TargetStructuralCaseSource` (184), `TargetControlCaseSuccessor` (232), `TargetControlSuccessor` (252). Data records with a few accessors; read in full. Comment 60: "Nonterminal operations only; control is owned by `terminator`."

### 6.3 `src/operations.rs` (737 lines)
1. **Purpose.** Ordered nonterminal operations of a target block (1-5).
2. **Shape.** `TargetInstructionSource` (34), `TargetInstruction` (44), `TargetUnitOperation` (50-551, about 75 variants), `installation_operation` (554), `structural_result` (561-624), `structural_result_home` (626-736).
3-4. `structural_result` and `structural_result_home` are matches over a fixed list of variants (563-611, 630-733). `structural_result` for the fallback arm requires the home's `OperationResult` origin operation to equal the producing operation (612-622); `ByteSequenceSubslice`/`ElementViewSubslice`/`EstablishElementView` require `place == result.place` (596).
5. **Cases by shape.** `structural_result_home` lists 17 producing variants explicitly and returns `None` for all others (734).
7. `installation_operation`: one call site (grepped, `04.../legalization.rs`).
9. None seen: a variant omitted from the lists yields `None` (treated by callers as "no structural result"); a new producing variant added without extending these lists would be silently skipped. Class: open (callers not traced).

---

## 7. target-vocabulary (crate `target-vocabulary`, files in this unit)

### 7.1 `arithmetic.rs` (282 lines)
`SaturatingCarrier` (25, eight carriers with `ordinal` 126), `SaturatingOperation` (37), `TrappingOperation` (141, nine, `ordinal` 169), `TrappingForm` (189, `COUNT` 72, `ordinal` 217, `from_ordinal` 221). `from_integer` accepts only fixed-width signed/unsigned 8/16/32/64 (57-72). Long "why" comments 3-20. Tests 237-282 check ordinals are dense 0..71.

### 7.2 `boundary.rs` (236 lines)
`BoundaryScalarArgument` (30), `BoundaryByteSequenceArgument` (38), `BoundarySettlementBinding` (46), `ExternalCompletionBinding` (53: `Boundary`, `Checked`), `conventional_float_class_layout` (62-92), `conventional_atomic_once_layout` (94) and `_closed` (103). `conventional_float_class_layout` accepts only the layout of a five-case sum (case 0 empty, cases 1-4 one `integer(1, 1)` field) with shape `integer(8, 4)`, tag 4 bytes at 0, fields at 4 (68-90). Float class shape is a fixed list of five cases (65-77). `conventional_atomic_once_layout_closed` refuses address integers (113-115).

### 7.3 `boundary/execution.rs` (575 lines)
1. **Purpose.** Boundary execution bindings and the stable-ordinal IEEE float operation catalog (1-2).
2. **Shape.** `ProviderPlanReportIdentity` (5), `ProviderExecutionBinding` (18), `NativeBindingExecution` (29), `ExternalExecutionBinding` (63), `IeeeFloatArithmeticOperation` (114-148, 22 shapes), `IeeeFloatIntegerCarrier` (158), `ALL: [Self; 73]` (231-300), `ordinal` (306-358), `operand_count` (369), `operand_type` (397), `result_type` (412), `signature_matches` (437), `CompilerBuiltinExecution` (481), `BoundaryExecutionBinding` (509), `from_execution_record` (522), `external` (568).
3-4. `NativeBindingExecution::from_commitments` refuses any all-zero 32-byte commitment (41-43). `ProviderExecutionBinding::from_execution_record` refuses any zero of four identities (529-537). Keyed on identity values. `ALL` is built by assigning slots by `ordinal()` into an array pre-filled with `Self::Add` (232-299), so an unassigned slot reads as `Add`; the test `float_operation_extensions_preserve_unique_stable_ordinals` (458) asserts ordinal equals index for all 73.
5. `FromInteger` ordinal arithmetic has three ranges (332-341); `TruncateToInteger` has three ranges (342-356).
8. Comment line 1-2: "append-only ... stable ordinals".
9. None seen.

### 7.4 `boundary/realizations.rs` (671 lines)
1. **Purpose.** Closed set of builtin boundary realizations and normalized foreign/system-call bindings (1-2).
2. **Shape.** `MetadataOnlyPortRealization` (9), `DirectPortReadU8Realization` (17), `HostedExitProcessI32Realization` (23), `HostedReadByteRealization` (32), `HostedWriteByteI32Realization` (65), `NearestIeeeFloatArithmeticRealization` (74) with `supports_target` (81), `CompilerImport` (115, one variant `WindowsProcessExit`), `NormalizedForeignCallBinding` (189), `NormalizedForeignCallMechanism` (195, six variants), `NormalizedSystemCallBinding` (249), `NormalizedForeignExecution` (257), `ForeignCallOperands` (573), `BoundarySettlementRealization` (605), `BoundaryRealization` (633, 13 variants), `AccessorScalarAccessRealization` (655).
3-4. `has_valid_entry_mechanism` (412-480) validates per mechanism: compiler import (entry plan equals the fixed plan, non-zero plan id, non-empty requirement); component (policy, call control, slot commitment recomputed via `target::component_export_slot`); hosted import (receipt non-zero, hosted program-entry schema); import/table (policy and call control only); system call (number nonnegative, non-empty requirement, complete execution, call plan recomputed with `native_syscall_for_target` and compared). `matches_execution` (312-323) matches an execution binding by plan report identity or receipt. `NearestIeeeFloatArithmeticRealization::supports_target` (81-94) allows fused multiply-add on x86-64 only with an admitted AVX+FMA provider whose profile target equals the target.
5. `native_target` (358-378) returns `None` for a system call on any profile other than Linux x64/arm64 (371-376). `HostedReadByteRealization::supports_target` is `false` for every target (35-39); comment 36-37: "This retired intrinsic has no input-error case. Both provider admission and target lowering must refuse it rather than trap on ordinary errors."
7. Not separately grepped.
9. None seen.

### 7.5 `call_contracts.rs` (71), `call_validation.rs` (397), `calls.rs` (269), `case_slots.rs` (14), `code_catalogs.rs` (99), `descriptor_input.rs` (11)
- `call_contracts.rs`: `LegalizedStructuralContract` (25; `structural_place` and `structural_type` are linear searches, 35-50), `LegalizedBoundarySettlement` (54), `LegalizedCallUnitParameter` (68).
- `call_validation.rs`: `LegalizedScalarCall::stored_plan_placements` (30), `validate_source` (42), `validate_shape` / `validate_no_normal_shape` (49-55), `validate_invocation_shape` (57-142), `validate_call_source` (145-223), `validate_completion` (225-262), placement predicates `direct_aggregate_registers` (264, at most two register locations, at most 16 bytes), `direct_scalar_placement` (303), `direct_scalar_register` (322), `indirect_aggregate_result` (337). Decisions: origin `ProgramEntry` needs the callee equal to `entry` and a non-zero source signature (157); `Authored` ownership must be empty or exactly `[ClaimTransfer(claims)]` (163-171); `InstalledProvider` checks boundary/provider match and `call.target.machine() == provider.candidate` (180-188), argument signature (199-217, structural arguments only after the scalar prefix, 200-206); completion receipts sorted strictly (250), argument indices below the structural count (251-253), ownership equal to `[ClaimCompletion(receipts)]` (258-259). Scalar placement widths limited to 1, 2, 4, 8 (305). `validate_invocation_shape` rejects non-`CallReturn` entry control and non-empty callback materializations (62-66). `stored_plan_placements`: grepped 7 occurrences in 3 files; `validate_no_normal_shape`: 6 occurrences in 6 files (1 definition, 5 callers).
- `calls.rs`: `TargetNativeCallbackArgument` (21), `TargetAccessViewInput` (36), `ScalarAbiValue` (45), `ScalarFunctionAbi` (52), `MixedStructuralScalarFunctionAbi` (59), `TargetBoundaryResult` (67) with `declared_reference_codec` (84-99), `TargetUnitScalarArgumentSource` (124, six variants, `source_value` 151, `scalar_type` 162), `TargetUnitScalarCallArgument` (175), `TargetStructuralResultContract` (192), `TargetDescriptorOrigin` (202), `TargetCallResult` (217), `selector_carrier_type` (240, `u64`), `NativeCallTarget` alias (247), `TargetAccessCallBinding` (250), `TargetAccessCallAssociation` (257), `TargetAccessInterpretation` (266). Comment 70-72 on `ScalarReference`: "`codec` ... interprets backing and grants no reference permission."
- `case_slots.rs`: `TargetCasePointerSlot` (9: `byte_offset`, `index_start`, `index_end`, `scalar_descriptor`).
- `code_catalogs.rs`: `NativeCodeCatalogs` (12) sealed once with canonical bytes (26-39). Grepped 18 occurrences in 7 files.
- `descriptor_input.rs`: `DescriptorInputGuard` (6: `count_scalar_argument`, `structural_argument`, `max_bytes`, `count_type`).
- Fail-open: none seen in these; every validator returns an error or `false` on mismatch.

### 7.6 `execution_facts.rs` (288 lines)
`NativeExecutionRoots` (19), `NativeExecutionFacts` (44) with `seal` (62-113), lookups (115-144), `identity_bytes` (142), `encode` (146), and `validate_summary` (167-201). `seal` refuses a main summary that omits its entry (68-72), entry rows that are unordered or differ from the main summary's other roots (73-84), per-entry summaries that omit their own entry (90-94), and summaries with unordered roots/rows or callers not in `reached_machines` (170-195). Domain `omega.native-execution-facts.v1\0` (16). Tests 221-288 include a refusal for a missing activated entry summary (272). Grepped `callable_invocations_from(`: 8 occurrences in 6 files. None fail-open seen.

### 7.7 `function_abi.rs` (103 lines)
`LegalizedFunctionAbi` (15: machine, call plan, `entry_scalar_codecs: Vec<Option<u64>>`, scalar and structural parameters, optional `scalar` and `mixed_structural_scalar`), `scalar_values` (36) and `scalar_values_mut` (51), `parameter_abi` (68), `publishes_parameter_abi` (89-103). Decision: a function with no result publishes a parameter ABI if it takes scalar inputs or owned structural inputs; a function with a result publishes it unless it has a scalar or mixed ABI (95-102). Comment 19-22 is the stated reason for `None` codecs (see 1.2).

### 7.8 `identity.rs` (410 lines), `identity/calling.rs` (489 lines), `identity/function_abi.rs` (54 lines)
1. **Purpose.** Canonical byte encoders for contracts and operands carried unchanged across native forms (identity.rs:1-6, 8-13).
2. **Shape.** `identity.rs`: `encode_runtime_indices` (66), `encode_work` (83), `encode_ids` (100), `encode_len` (108), `encode_option` (112), `encode_integer` (126), `encode_register` (139), `encode_bindings` (165), `encode_scalar_type` (174), `encode_integer_type` (195), `encode_layout` (233), `encode_access_input` (253), `encode_access_call_bindings` (285), `encode_structural_contract` (304, uses the one-slot catalog cache 315-319). `calling.rs`: `encode_call_plan` (30), `encode_callback_materialization` (68), `encode_native_place` (73), `encode_placement` (95), `encode_shape` (157), `encode_call` (185-249), `encode_boundary_execution_identity` (286-363), `encode_binding` (365-420), `encode_accessor_scalar_demand` (422). `function_abi.rs`: `encode_function_abi` (9-41).
3. **I/O.** Pure functions appending bytes.
4. **How it decides.** Tag tables. `encode_boundary_execution_identity` and `encode_binding` both use tag numbers that overlap between their own arms by design (execution tags 9, 10, 8, 7, 6, 5, 0, 11, 1, 2, 3, 4; settlement tags 8, 12, 13, 11, 10, 9, 0-7). `encode_register` writes `(tag, payload)` with payload 0 for fixed registers (139-163).
5. **Cases by shape.** Closed matches over `CallingPolicy` (six, 31-38), `EntryControl` (three), `ValueClass`, `ValueLocation`, `BoundaryRealization` (13).
6. **Same job elsewhere.** `encode_scalar_type` and `encode_work` duplicated in `selected-instructions/src/plan_identity.rs` (see 1.1).
7. Not separately grepped.
9. Omission: `encode_call` writes `transfer.claim` and `transfer.argument_index` for each `ClaimTransfer` but not `transfer.path` (`calling.rs:236-240`); `ClaimTransfer` has a `path` field (constructed with `path` at `legalized-operations/src/tests.rs:273-282`). For `InstalledProvider` origins the receipts, which include `path`, are encoded elsewhere (`encode_call_source`, not read); for an `Authored` origin I found no other place that encodes the path in this unit. Class: open (I did not read `identity/structural.rs`).
Panics: `.expect("canonical access-view plans")` (identity.rs:54), `.expect("canonical placed-formal demands")` (61), `.expect("canonical call crash continuations")` (calling.rs:246).

---

## Coverage
Read in full (all lines of every range in `units/unit_032.tsv`, 54 files, 21,796 lines):
- `selected-instructions/src`: plan_identity.rs 1-1954, program_contracts.rs 1-361, provenance.rs 1-110, structural_case.rs 1-131, terminator_encoding.rs 1-134, values.rs 1-159 (2,849).
- `structural-storage/queries/src`: access_reference_validation.rs 1-122, address_joins.rs 1-485, lib.rs 1-8, owner_loans.rs 1-158, storage_projection.rs 1-449 (1,222). `structural-storage/src`: lib.rs 1-5, structural_type_catalog.rs 1-272 (277).
- `target-bound-program`: bound_reach.rs 1-179, builtins.rs 1-138, entry_call.rs 1-42, entry_settlement.rs 1-922, execution-reach/reach.rs 1-51, native_execution_scope.rs 1-371, provider_bindings.rs 1-164, scalar_demand.rs 1-216 (2,083).
- `legalized-operations/src`: calls.rs 8, control_flow.rs 34, control_flow_scalar_graph.rs 1550, function_abi.rs 3, identity.rs 61, identity/scalar_graph.rs 1844, installed_interrupt_geometry.rs 173, lib.rs 306, native_geometry.rs 564, selection.rs 16, structural_inputs.rs 2790, tests.rs 1092, tests/identity.rs 969, tests/scalar_operations.rs 885, validation.rs 33 (10,328).
- `target-operations/src`: control_flow.rs 271, lib.rs 60, operations.rs 737 (1,068).
- `target-vocabulary/src`: arithmetic.rs 282, boundary.rs 236, boundary/execution.rs 575, boundary/realizations.rs 671, call_contracts.rs 71, call_validation.rs 397, calls.rs 269, case_slots.rs 14, code_catalogs.rs 99, descriptor_input.rs 11, execution_facts.rs 288, function_abi.rs 103, identity.rs 410, identity/calling.rs 489, identity/function_abi.rs 54 (3,969).
Skimmed: none. Not read: none. Files outside the unit that I cited but did not read: `selected-instructions/src/{functions.rs, blocks.rs, effects/program/identity.rs}`, `target-vocabulary/src/identity/{structural.rs, structural_types.rs, normalized_foreign.rs}`, pipeline 04/05 call sites named in greps.

## Five things a maintainer most needs to know about this unit
1. Two canonical-byte producers write the same structures with separate code: selected-instructions `plan_identity.rs` (scalar type 711, work 694, `CallSiteOwner` 836-865, crash predicate `terminator_encoding.rs:51-77`) and the target-vocabulary/legalized encoders (`identity.rs:174`, `:83`, `identity/scalar_graph.rs:1469-1498, 1616-1643`). They match today by hand.
2. `identity/scalar_graph.rs` begins with a comment that every instruction variant has a distinct tag byte (4-5), but the first byte repeats for `StructuralCaseMembership`/`BitwiseAnd` (24), `StructuralLeafCopy(Case)`/`StructuralByteViewFieldStore` (92), the element-view and byte-sequence variants versus saturating `I32`/`I8`/`I16`/`I64` Add/Subtract/Divide (40-44, 46), and `StoreStructuralField`/`EstablishTrivialAffineLocal`/etc. versus saturating Multiply (88-94). Fields after the tag differ, so I found no identical byte string.
3. `LegalizedScalarTerminator::references_value` (control_flow_scalar_graph.rs:1457-1508) does not visit several operand-bearing places that the instruction visitor visits (call target selector on `InvokeNoNormal`, `ExternalCompletion.structural_arguments`, cleanup calls, structural return source). Four sites in stages 05 use it (`selection_inputs.rs:2989`, `selection_scalar_graph.rs:3220, 3242`).
4. `entry_settlement.rs` is the only place that binds a program entry call, and it hard-codes `TargetProfile::UefiX64` and a two-record-parameter shape (792, 798-802, 159-195). The hosted arm hard-codes two `integer(16, 8)` parameters (103).
5. Several pieces of geometry and permission logic exist twice: reference-crossing access ceilings (`address_joins.rs:161-194` vs `structural_inputs.rs:1673-1701`), "exactly one declaration with this id" (`unique_structural_type` vs inline filters at `structural_inputs.rs:682, 757`, `owner_loans.rs:108`, `structural_type_catalog.rs:160-185`), and borrowed-slice carrier detection (`scalar_demand.rs:92, 128`).

## What I am unsure about
- Whether the duplicated first-byte tags can produce an identical full byte string: I compared field lists by eye, not by running anything.
- Whether `references_value` omissions matter: I did not read the four caller sites.
- Whether `claim_transfers[].path` is encoded elsewhere for `Authored` calls (`identity/structural.rs` unread).
- Whether `prefix_verified == true` is only passed when prefix equality is proven (`functions.rs:119` unread).
- Grep counts in the combined patterns (noted per entry) are not split by name; I gave per-pattern counts only where I ran one pattern.
- I did not check `tools/ratchets/dense_id_maps.txt` for the `BTreeMap`/`BTreeSet` uses in `scalar_demand.rs` and `selected-instructions` `std::collections::BTreeSet` at `provenance.rs:26`.
