# Record: unit_006 (areas 01-cli-foundation, 13-native-targets-driver, 09-native-stages-00-03)

Checkout `C:/Development/Omega/.agents/worktree/worker-2/` at 4f3c255d. All paths below are relative to it. Every file in
`units/unit_006.tsv` (66 rows, 21,906 lines) was read in full. "read" means I read the lines cited; "grepped" means a Grep
count I ran (command given); "inferred" means not read at the other site.

Directory shorthand:
- SR = `source-rust/subsystems/compiler/foundation/semantic-records` (package `semantic-vocabulary`, lib name `semantic_vocabulary`)
- SP = `source-rust/subsystems/compiler/foundation/source-provenance`
- WB = `source-rust/subsystems/compiler/foundation/worker-budget`
- DRV = `source-rust/subsystems/compiler/psi-to-native/driver`
- P0 = `source-rust/subsystems/compiler/psi-to-native/pipeline/00_psi-to-admitted-native-inputs`
- P1 = `source-rust/subsystems/compiler/psi-to-native/pipeline/01_admitted-native-inputs-to-target-bound-program`

---------------------------------------------------------------------------------------------------

## PART A. semantic-records (SR), 18 files

### SR/src/lib.rs (968 lines)

1. Purpose. Crate root. Declares leaf vocabulary shared by source forms, Psi and the checker, and holds several small
   vocabularies inline: crash/terminal-authority enums, callback nominal ids, foreign-locator candidates, program-entry
   schema traits, `ScalarRefinement`, `BoundedIntegerType`, byte-predicate sets, declaration-custody and
   termination/crash-policy enums. Re-exports everything from the other files.
2. Shape.
   - `TargetMachineCallable { Machine(MachineId), Boundary(BoundaryMachineId) }` lib.rs:30.
   - `CallableShapeMismatch` lib.rs:36.
   - `behavior_policy` mod: `CrashCause {Trap, Abort}` :70; macro `tagged_vocabulary!` :75; `TerminalAuthorityClass` (14
     variants, tags 0..13) :97-117 with hand-written `Record` read/write :119-135.
   - `boundary::callback_identities` :181: `nominal_declaration_id!` makes `StaticMachineBinderId`, `NativeParameterId`,
     `CallbackRequirementId` (plain `u64`, zero rejected) :207-209; `BoundaryNativeParameterOrigin` :212;
     `callback_nominal_report_fingerprint` :222 (FNV-1a over framed parts, 0 mapped to 1); `callback_requirement_id` :233;
     `nominal_callback_native_parameter_id` :240; `nominal_callback_binder_id` :253.
   - `boundary::foreign_locator` :265: `ForeignLocatorCandidate` (PeByName, PeByOrdinal, ElfVersioned, MachODylibSymbol),
     `NativeBindingLocator {Import, Syscall{number:u64}}`.
   - `boundary::program_entry` :305: `ProgramEntrySchema`, `ProgramEntryVisibleParameters`, `ProgramEntrySourceContract`,
     trait `ProgramEntrySourceCatalog`.
   - `scalars::bounded_integer_type::BoundedIntegerType` :346 (`new` :353 refuses Address carrier, unadmitted or inverted
     endpoints); `IeeeFloatComparisonOperation` :431; `QualifiedScalarType` :469; `ScalarRefinement` :488 (`new` :502,
     `SUBJECT = ValueId::FIRST` :500, `instantiate` :531).
   - `byte_predicates::ByteSequencePredicateSet(u8)` :658 (bitset over `language_core::ByteSequencePredicate`; methods
     `implies`, `subslice`, `concatenation` :697-733).
   - `compiler_evaluation` :824: `DeclarationCustody {Package, Toolchain, UnownedUser, Missing, ToolchainContract}` :828,
     `CompilerEvaluationDeclaration` :845, `DeclarationAuthorityRequest` :857, `declaration_authority_request` :865.
   - `Termination` :947, `CallableCrashPolicy` :955, `ScalarAddressModel` :963.
3. In/out. Pure data plus small constructors; reads `language_core`, `wire::fnv1a`, `record_storage`, `arena`.
4. How it decides.
   - `ScalarRefinement::new` accepts a predicate only if a bounded walk (budget 4096 nodes, lib.rs:506) sees only
     `ScalarRefinement::SUBJECT` as a value id and the predicate validates under a one-value context :508-519. Keyed on
     value identity and node count.
   - `declaration_authority_request` :865-887 keyed on the pair of `DeclarationCustody` variants (match on variant pair).
   - `ByteSequencePredicateSet::concatenation` :713: NonEmpty holds if either side implies it; every other predicate needs
     both sides and `is_concat_preserving()`.
5. Cases by shape. `TerminalAuthorityClass` is a closed list of 14; comment lib.rs:98 "additions require a target-policy
   migration". Decode rejects unknown tag :133.
6. Same job elsewhere. `semantic_id!` is defined three times: `declaration_contracts.rs:55` (u32 newtypes, 0 = NULL),
   `identity.rs:135` (`NonZeroU64`), and `nominal_declaration_id!` lib.rs:182 (u64, zero rejected). Three differently
   behaved id macros in one crate (read). Content digests of "calling requirement" ids use `wire::fnv1a` (lib.rs:223) and the
   same FNV hasher builds `CallingSignatureId` (identity.rs:315).
7. Unused or one-caller. `callback_nominal_report_fingerprint`: grepped `callback_nominal_report_fingerprint` over
   `source-rust/*.rs` = 14 hits in 5 files (lib.rs 5, calling-conventions `callback_materializations.rs` 5, P1 callback_layouts
   1, terminal_signature 2, calling-conventions lib.rs 1). `declaration_authority_request`: 3 hits (lib.rs def, 2 in
   `source-to-psi/pipeline/05_.../src/proof_scope.rs`). `ScalarRefinement::new|visit_value_ids_bounded` grepped across
   `source-rust/*.rs`: 21 hits in 7 files (semantic-records itself 13).
8. Stated limits. lib.rs:98 "additions require a target-policy migration"; lib.rs:833 comment "Issued contract closure,
   never an ambient package-selection exemption."; lib.rs:937-938 "A content place orders case names before field names."
9. Fail-open risks.
   - `declaration_authority_request` :877-879 returns `Ok(None)` (no package edge needed) for any pair containing
     `Toolchain`, including `(UnownedUser, Toolchain)` and `(Toolchain, UnownedUser)` (the `UnownedUser` arm :883 is
     reached only if neither side is `Toolchain`). So a user-source requester selecting a toolchain declaration needs no
     request. Classification: open (no comment at the arms); I did not trace the two proof_scope.rs callers.
   - `ByteSequencePredicateSet::from_bits` :663 accepts only low 4 bits (rejects others). Not fail-open.
   - None else seen.

### SR/src/closed_integers.rs (310 lines)

1. Purpose. A "closed" view of integer/scalar types: width is a concrete 1..=128 number, never a width parameter.
2. Shape. `IntegerWidthClosureError` :12; `ClosedIntegerType {carrier, fixed: FixedInteger}` :43 with
   `TryFrom<IntegerType>` :48; `ClosedScalarType {Boolean, Integer, IeeeFloat, Callable}` :146 with `TryFrom<ScalarType>` :153;
   `as_checking()` back-conversions :108,:174. Tests :190-310.
3. In/out. Reads `IntegerType` (proposition.rs) and `numerics::integer_carrier::FixedInteger`; produces closed types.
4. How it decides. `try_from` rejects `IntegerWidth::Parameter` :54, widths outside 1..=128 :58, signed address :61. Keyed on
   width value and carrier kind. `Hash` :138 hashes the `IntegerType` form so digests stay stable (test :228).
5. Cases by shape. Doc :38-41 says "Its 128-bit backing limit does not limit the language's positive-u64 width family";
   `from_fixed` returns `None` for >128 (test :306-308).
6. Same job elsewhere. `IntegerType::from_fixed` proposition.rs:546 also refuses >u16 widths; `fixed_integer_type`
   (float_integer_conversions.rs:214) converts the other way. Different direction, same 1..=128 constraint stated in three
   places (closed_integers.rs:58, proposition.rs:510, :551).
7. Unused. Grepped `ClosedIntegerType|ClosedScalarType` over `source-rust/*.rs`: 438 hits in 58 files, widely used
   (native stages 04/05 etc.).
8. Stated limits. :38-41 above.
9. Fail-open. None seen. `expect(...)` calls at :67,:74,:78,:105,:111,:114,:122,:128 panic rather than accept.

### SR/src/declaration_contracts.rs (242 lines)

1. Purpose. Retained ids and plain records for external bindings, service reach, suspension/blocking plans, and the four
   built-in ranking views.
2. Shape. `ExternalBindingMechanism` :4 (identity tags 2..6, tags 0,1 unused :23-31); `ExternalBindingIdentity` :35; local
   `semantic_id!` :55 for `SemanticDomainId`, `ServiceReachId`, `ServiceReachRowId`, `ExternalBindingId`, `RankingViewId`;
   plans `ServiceReachPlan`, `SynchronousInvocationPlan`, `SuspensionPlan`, `BlockingPlan` and summaries :77-146;
   `RankingViewDeclaration` :151; `RankingViewId` constants 1..4 :168-171, `canonical` :173, `canonical_path` :183,
   `catalog_declaration` :193, `from_catalog_declaration` :210.
3. In/out. Pure data. `canonical(path)` maps source spelling to ids.
4. How it decides. Ranking views are selected by SOURCE SPELLING: `canonical` :173-180 matches the literal strings
   `"Nat::Descending"`, `"Nat::BoundedDistance"`, `"Slice::Length"`, `"Nat::IncreasingTo"`; `from_catalog_declaration` :210
   compares `namespace`/`name` strings.
5. Cases by shape. Only two of the four views have catalog declarations: `catalog_declaration` :193-198 matches
   NAT_DESCENDING (1 subject) and NAT_BOUNDED_DISTANCE (2 subjects), `_ => return None` (SLICE_LENGTH and
   NAT_INCREASING_TO get no catalog declaration). Parameter and result are hard-coded `"u64"` :204,:206; source file
   `"nat.omg"` :148.
6. Same job elsewhere. None read.
7. Unused. Grepped `RankingViewId::canonical|canonical_path\(\)|catalog_declaration|from_catalog_declaration|RANKING_VIEW_CORE_SOURCE`
   over `source-rust/*.rs`: 23 hits in 10 files (this file 9, build_evaluation.rs 3, others 1-2).
8. Stated limits. None.
9. Fail-open. None seen (`canonical` returns `None` for unknown names).

### SR/src/declaration_descriptions.rs (29 lines)

1. Purpose. Two byte-vector records describing a parameter (`ParameterDescription` :7) and a field
   (`FieldInfoDescription` :17) so identity can cross Build/package boundaries; header comment :1-4 says they grant no
   permission.
2-9. Data only; no decisions. Fail-open: none seen. Same job elsewhere: not checked. Unused: not grepped (record types
   re-exported lib.rs:20).

### SR/src/expression_clone.rs (139 lines)

1. Purpose. Non-recursive `Clone` for `IntegerMathTerm` and `ScalarTerm` so deep proof terms do not overflow the Rust stack.
2. Shape. `clone_tree` :6 (explicit stack, visits last operand first, relies on pop order to restore declaration order, :4-5);
   `take_operand` :35; `impl Clone for IntegerMathTerm` :43; `impl Clone for ScalarTerm` :66 with a variant-table macro
   `variants!` :76 listing 37 variants :98-137 plus hand-written `StructuralRead` :85.
3. In/out. Reads operands via `append_operands` (proposition.rs:1208, scalar_terms.rs:1714).
4. How it decides. Not a judgment; mechanical copy keyed on variant.
5. Cases by shape. The `variants!` table at :98-137 must list every `ScalarTerm` variant with `{copied fields; operand
   fields}`; a variant not listed is a compile error (non-exhaustive `match`), not silent.
6. Same job elsewhere. `Proposition` has its own stack-free clone proposition.rs:1730-1784 and stack-free `==`/`Hash`; the
   three implement the same iterative idea three times (read).
7. Unused. Used via `Clone`; tested by `tests/expression_clone.rs`.
8. Stated limits. None. 9. Fail-open: none seen.

### SR/src/float_integer_conversions.rs (286 lines)

1. Purpose. Records that a float-to-integer conversion is a TOTAL RELATION: a failed conversion makes the relation false
   rather than producing a partial integer term (:1-5). Also builds the preconditions and failure condition for the authored
   toward-zero and trapping converters.
2. Shape. `FloatIntegerConversionError` :16; `FloatIntegerConversion {source, result, projection, operation}` :43 with
   `new` :51 and `validate` :69; `float_integer_preconditions` :99 (returns 3 propositions: self-equal, `< upper`, lower
   bound); `float_integer_failure_condition` :164 (a `Disjunction` of the reversed comparisons); `projection_contract` :198;
   `fixed_integer_type` :214.
3. In/out. Reads `numerics::float_projection`, `float_semantics`, `bignum::BigInt`; outputs `Proposition::ScalarIeeeFloatComparison`
   rows.
4. How it decides. `validate` :69: source/result must validate as scalar terms, projection contract must equal
   `projection_contract(source)`, result must have an integer type, and the semantic operation (found by contract identity)
   must have exactly parameters `[Meaning]` and result `Integer(carrier)` :85-90. Keyed on contract-identity digests and
   carrier equality. `float_integer_preconditions` :133 picks `<=` over `<` for the signed lower bound when the float literal
   of `minimum-1` equals the float literal of `minimum` (rounding collapse).
5. Cases by shape. Only `Binary32`/`Binary64` formats :110-113; address carriers refused :215-217; "The three rows are in
   canonical wire order: equality, upper <, lower." :144.
6. Same job elsewhere. `FloatMeaningClause` (float_meaning_clauses.rs) uses the same projection-contract lookup (:59-67).
   `psi/semantics/proofs/bounded/float_integer.rs` and `source-to-psi/.../prove/float_integer.rs` consume these
   (suspected; not read).
7. Unused. Grepped `float_integer_preconditions|float_integer_failure_condition` over `source-rust/*.rs`: 12 hits in 7
   files. Grepped `\bfixed_integer_type\b`: 16 hits in 9 files.
8. Stated limits. :99-103 comment "Exact cast eligibility implies every row, but remains a separate, stronger proposition."
9. Fail-open. None seen. `float_integer_failure_condition` returns `Err` for a non-canonical row :178,:184.

### SR/src/float_meaning_clauses.rs (505 lines)

1. Purpose. Template for "float meaning" equations: a flat, topologically ordered table of projections and semantic-catalog
   applications, with two slots declared equal. Calls substitute actual scalar sources into the template.
2. Shape. `FloatMeaningSlot` :16; `FloatMeaningClauseOperand` :19; `FloatMeaningClauseValue {Projection, Application}` :26;
   `FloatMeaningClause {values, left, right}` :51 with `conversion_roundtrip` :73, `result_equation` :113, `validate` :169,
   `map_sources` :284, `intern_sources` :295, `map_sources_or_invalid` :363, `sources` :377, `projection_sources` :394.
3. In/out. Reads `numerics::float_semantics` catalog by contract identity; outputs/validates clause tables.
4. How it decides. `validate` :169-279: rows unique; each projection's contract must equal the catalog projection for its
   scalar format; each application's operand list must match the catalog signature kind by kind (Format must equal the
   application's own format; Meaning slot index must be earlier; Integer operand's carrier must equal the catalog carrier);
   `left.format() == right.format()`; `left <= right`; every row reachable from `left`/`right` (no disconnected rows). Keyed
   on contract-identity commitments and slot positions.
5. Cases by shape. Only unary/binary/ternary meaning operations handled uniformly via the catalog's parameter list; kinds
   other than Format/Meaning/Integer are refused :145 ("unsupported argument kind").
6. Same job elsewhere. `FloatMeaningClause` and `FloatIntegerConversion` both authenticate by semantic-catalog
   contract identity (float_integer_conversions.rs:85). `intern_sources` re-validates after substitution :356.
7. Unused. `map_sources_or_invalid|map_sources::` grepped: 10 hits in 6 files.
8. Stated limits. :360 "Legacy renaming APIs cannot return an error."
9. Fail-open. `map_sources_or_invalid` :363-375 on error returns a deliberately malformed equation (empty `values`);
   stated at :360-362 ("receiving validation must reject it"). I traced: an empty-values clause fails `validate` at
   `self.values.get(self.left.0)` :247-250 so it is rejected. Classification: stated + traced. The one in-crate caller is
   `Proposition::remap_value_ids` proposition.rs:2687; that function has no `Result`, so callers rely on a later `validate`.

### SR/src/identity.rs (439 lines)

1. Purpose. Dense numeric ids (`NonZeroU64` newtypes) for Psi objects and a few small enums describing invocation sites and
   calling-ABI keys.
2. Shape. `BoundaryRequirementKind` :13; `ExternalCompletionKind` :20; `InvocationSite` :25 with `encode_identity` :33 (tag byte
   0..3 + id bytes) and `from_tagged_identity` :54; `CallableInvocationSite` :72; `BoundaryCompletion` :111;
   `WorkScheduleIdentity` :120; `semantic_id!` macro :135; ids at :171-407 (34 ids; count from grep below); `CallingAbiIdentity` :268
   with `id()` :275 and `validate()` :285; `CallingSignatureId::for_application` :310 (FNV-1a hash, domain string
   `"omega.declaration-abi-application.v1"` :316, zero mapped to 1 :332).
3. In/out. Pure; `CallingSignatureId` is derived from `(requirement, declaring application, operands)` strings.
4. How it decides. `CallingSignatureId` identity is keyed on STRING identities (requirement and operand `identity` strings
   :319-330) hashed by 64-bit FNV; `validate` :285 requires non-empty strings and strictly increasing operand ordinals.
5. Cases by shape. `InvocationSite::from_tagged_identity` :54-61 accepts tags 0,1,2 only; tag 3 (Cleanup) is NOT decodable
   there (it needs edge + action ordinal) and returns `None`.
6. Same job elsewhere. 64-bit FNV-1a fingerprints used for identity also in lib.rs:222 and storage.rs:287,:460 (read).
7. Unused. `grep -c "semantic_id!" identity.rs` not run; I counted 34 `semantic_id!(` lines by reading :171-407 (SelectionDeclarationId
   through ProfileDecisionId); crate-map says 34.
8. Stated limits. None. 9. Fail-open. `CallingSignatureId` collisions possible (64-bit hash); `callback_layouts.rs` and
   `terminal_signature.rs:339` check one such collision class for callback requirement ids ("distinct exact callback
   requirements collide on one compact report identity"), but I saw no similar guard for `CallingSignatureId` (open, not traced).

### SR/src/identity_encoding.rs (52 lines)

1. Purpose. Shared canonical byte encoding of scalar and integer types so unit identity and execution-plan identity hash the
   same bytes (comment :1-3).
2. Shape. `encode_scalar_type` :7 (generic over writer, tags Boolean=1, Integer=2, IeeeFloat=3(+1/2), Callable=4(+8 LE bytes)),
   `encode_integer_type` :38 (carrier tag 1/2, sign tag 1/2, then `IntegerWidth::write_identity`).
3-5. Keyed on type variant. Closed list: 4 scalar kinds.
6. SAME JOB ELSEWHERE (grepped). The comment says the encoders "live with the types' vocabulary" so all forms encode a type
   the same way. Users of `semantic_vocabulary::identity_encoding`: grepped `identity_encoding::` over `source-rust/*.rs`: the
   shared fns are used by `optimization-unit/src/lib.rs:366` (re-export), `optimization-unit/src/range.rs:85` (re-export),
   `operation-evidence/src/ownership_frontier.rs:160-164`, `runtime-abi/.../borrowed_storage/encoding.rs:5`. But grepped
   `fn encode_scalar_type|fn encode_integer_type` shows SEPARATE definitions at: `optimization-unit/src/identity.rs:113,122`,
   `optimization-unit/src/rewrite.rs:186`, `selected-instructions/src/plan_identity.rs:711`,
   `target-vocabulary/src/identity.rs:174,195`, `declaration-encoding/src/terms/scalar_types.rs:9,41`. I did NOT read those
   bodies (suspected duplicates or wrappers).
7. Unused. See above.
8. Stated limits. None. 9. Fail-open: none seen.

### SR/src/proposition.rs (4,867 lines)

1. Purpose. The proposition calculus used by the lowered-form checker: integer types and exact/wrapping/saturating arithmetic
   on host 128-bit values; mathematical integer terms; proof terms; `Proposition` with validation against a typing context;
   value-id traversal/remap. About 2,000 lines are implementation, about 2,800 lines are tests (tests begin at :3133; unit
   tests also inside modules at :138, :1372, :2934).
2. Shape (nested modules, all re-exported at :3121-3131).
   - `integer_types` :8: `IntegerSign` (re-export from numerics) :227; `IntegerCarrier {Fixed, Address}` :230; `IntegerType
     {carrier, sign, width}` :236; `IntegerWidth {Bits(u16), Parameter(WidthParameter)}` :276; `WidthParameter {owner:
     MachineId, value: ValueId}` :246; `IntegerValue {Signed(i128), Unsigned(u128)}` :1121; `IntegerMathLiteral` :1136;
     `IntegerMathTerm {IntegerLiteral, MathValue, Embed, Add, Subtract, Multiply, ShiftLeft}` :1191; submodule `bit_ranges` :12
     (known-bits interval transfer for and/or/xor); arithmetic methods generated by `exact_arith!` :299 / `wrapping_arith!` :315
     and hand-written (`wrapping_shift_*` :858-899, `exact_shift_*` :912-1016, `saturating_*` :1032-1117, bounds fns).
   - `proof_terms` :1451: `ProofFormalScope`, `MathematicalArgument`, `ProofTerm` (8 variants :1472), `ProofTermField`;
     `validate`/`validate_node` :1538/:1555.
   - `propositions` :1641: `Proposition` (23 variants :1990-2047), `PropositionContext {value_types, structural_places}` :2213
     (BTreeMaps keyed on `ValueId`/`PlaceId`), `PropositionError` (about 60 variants :2532), `validate` :2080,
     `PropositionContext::validate` :2255. Stack-free `Clone`/`PartialEq`/`Hash` :1653-1987.
   - `value_ids` :2647: `remap_value_ids` (scalar :2661, proposition :2673), `visit_value_ids` :2750, bounded visit :2763,
     `any_value_id` :2789, `visit_scalar_terms` :2796, shared walker `walk` :2825.
   - `#[path = "expression_clone.rs"]` :2643 and `#[path = "scalar_terms.rs"]` :2645 pull the other files in as submodules.
3. In/out. Reads `ScalarTerm` (scalar_terms.rs), `ContentConservation` (storage.rs), `FloatMeaningClause`,
   `FloatIntegerConversion`. Mutates: `remap_value_ids` rewrites ids in place; `PropositionContext::from_value_types*`
   builds typing tables.
4. How it decides.
   - `IntegerType::admits(value)` :596 returns `false` for a parameter width (no host answer), so nothing is admitted in a
     rigid-width carrier (comment :593-595). Keyed on width value and signedness.
   - `IntegerType::can_widen_to` :477 true only for two closed Fixed widths with strictly increasing width and same sign or
     unsigned-to-signed. `can_exact_cast_to` :413 true if either side is Fixed (address-to-address refused).
   - `admits_exact_add_operands` :341: address result needs (address, u64-count) in either order; fixed needs all three equal;
     `admits_exact_subtract_operands` :379 similar but only left address.
   - `Proposition::validate` :2080 requires: Equal operands same scalar type (:2456); Less* operands same INTEGER type;
     `IntegerMathEqual` operands in canonical order (`left > right` rejects :2129); symmetric float comparisons in canonical
     order :2144,:2155; `Conjunction`/`Disjunction` need >= 2 members :2179,:2189; `StructuralIdentity` needs equal structural
     types and order :2170-2175; `ByteSequenceEqual` ordered, nonempty paths.
   - `PropositionContext::validate_value_terms` :2260 checks every `Value` id exists with the exact type (:2418-2429) and every
     structural place exists; `Entry`-version field reads require a `Parameter` place :2440-2443; content terms reject
     `Result`/`OperationResult` with `Entry` version :2385-2393 and reject `TrivialAffineLocal`/`BlockParameter` places :2394-2402.
   - `same_commutative_predicate` :2050 compares Equal either order and Conjunction/Disjunction as sets (duplicates ignored),
     else `==`.
5. Cases by shape. Known-bits bound transfer only for and/or/xor (:22-62). Exact shift bounds evaluated at the 4 corner
   combinations (:963-980), a monotonicity assumption stated implicitly. `carrier_minimum/maximum` :1248/:1264 emit
   `2^N`-style `ShiftLeft` terms only for parameter widths (`width_power` :1278 panics via `unreachable!` for closed width).
6. Same job elsewhere. Wrapping/saturating/exact arithmetic is duplicated per operation in this file: `wrapping_arith!`,
   `exact_arith!` macros plus separate hand-written `saturating_add/sub/mul` :1032-1117 (each repeats the bits==128 special
   case). Same arithmetic semantics also implemented in `numerics` and in interpreter instructions
   (`subsystems/interpreter/instructions/execution.rs`, suspected, not read). Stack-free clone: `expression_clone.rs`.
7. Unused. `IntegerType` etc. widely used (ClosedIntegerType grep above shows 438 hits in 58 files for the closed forms).
   `IntegerMathTerm::operands_mut` :1226, `Proposition::as_equal` :2071 not grepped.
8. Stated limits. :541-545 "a wider carrier has no statement here yet and its reader refuses it rather than narrowing it";
   :242-244 "Imported signatures must substitute/rebase this coordinate"; :2032-2036 doc comments on the two float-exactness
   variants.
9. Fail-open risks.
   - `visit_value_ids` (Proposition) returns the `complete` flag: set `false` for `Atom`, `IeeeFloatComparison`,
     `ByteSequenceEqual`, `StructuralIdentity`, `StructuralCaseMembership`, `ContentConservation`, structural reads and field
     predicates (:2846-2851,:2912-2918). Callers that read `true` as "no hidden dependencies" are safe; callers that ignore the
     flag would treat an opaque atom as having no value ids. `ScalarRefinement::new` (lib.rs:508-514) uses the bounded variant
     and requires `Some(true)`, so opaque atoms are rejected there (test lib.rs:607-624). Classification: traced for that
     caller; other callers not traced.
   - `Proposition::validate` does not require `IntegerMathLessThan/LessOrEqual` operands in any order (correct) and accepts
     `StructuralCaseMembership` unconditionally :2177 (type/case consistency not checked here; checked by context only for
     place existence :2313). Classification: open.
   - `ProofTerm::validate_node` `Formal`/`InvocationFormal` accept any position :1607 (positions checked elsewhere; not traced).
   - `PropositionContext::from_value_types_and_places` rejects conflicting re-registration of an id :2231-2250.

### SR/src/provider_identity.rs (27 lines)

Purpose: two 32-byte digest newtypes `ProviderPlanDigest` :4 and `ServiceSchemaDigest` :17 (`from_digest`, `as_bytes`).
Data only; no decisions; fail-open none seen. Unused: not grepped.

### SR/src/scalar_terms.rs (1,845 lines)

1. Purpose. Scalar vocabulary and the scalar expression tree the checker reasons over: `ScalarType`, IEEE float
   vocabulary, structural-field handles, `ScalarTerm` (38 variants :976-1173) with typed smart constructors, validation and
   closed-term evaluation.
2. Shape. `mod validation` :14 (`ScalarTerm::validate` :23 iterative; `validate_after_operands` :40); `mod value_queries`
   :332 (`scalar_type` :337, `integer_value` :389, `boolean_value` :634, evaluation helpers :443-706); `ScalarType` :711;
   `IeeeFloatFormat` :753; `IeeeFloatValue` :802; `IeeeFloatComparisonKind` :817; fields `IeeeFloatStructuralField` :863,
   `ByteSequenceStructuralField` :883, `StructuralCaseSubject` :889, `StructuralIdentitySubject` :895; `ScalarReadProjection`
   :959; macros `scalar_term_unary_operand!` :1176, `scalar_term_binary_operands!` :1195, `integer_binary_term!` :1304,
   `integer_shift_term!` :1324, `append_scalar_operands!` :1343, `scalar_term_predicate_quint_pattern!` :1837 (matches 5
   variants despite the name "quint"); constructors :1364-1712; `mathematical_comparison` :1490.
3. In/out. Reads `IntegerType` methods (proposition.rs). Produces `Result<ScalarTerm, PropositionError>`.
4. How it decides.
   - `validate_after_operands` :40: any term whose scalar type is `Callable` is refused (`OpaqueCallableInProof` :41-43);
     `StructuralRead` requires nonempty path, no `Static(Referent)` (canonical form is `Referent`), and every index operand an
     integer that is not an address :45-69; each arithmetic variant requires operands to equal the declared type.
   - `integer_value`/`boolean_value` evaluate CLOSED terms (all-literal) with explicit stacks and return `None` when any
     operand is non-literal, mistyped, or the operation fails (e.g. `exact_add_values`). A `None` result means "not a known
     constant", not "false".
   - `mathematical_comparison` :1490 only produces a proposition when the two operand scalar types DIFFER
     (`left.scalar_type() == right.scalar_type()` returns `None` :1509) and both sides are plain values or closed constants.
5. Cases by shape. `IntegerExactCast` etc. Comparisons `integer_comparison_operands_match` :1750: same type, or both
   non-address (cross-carrier comparisons only for non-address).
6. Same job elsewhere. Typed constructors vs `validate_after_operands`: each constructor repeats the same check as the
   `validate` arm (e.g. `boolean_not` :1460 vs :81). Constant folding `integer_value` partially duplicates arithmetic
   dispatch (match at :485-609).
7. Unused. `ScalarTerm::integer_remainder_operands` :1663 not grepped. `ScalarType::from_numeric_type` :720 not grepped.
8. Stated limits. None.
9. Fail-open. `ScalarTerm::Application` validates `Ok(())` unconditionally :72 (apart from callable check): a
   `ScalarApplicationId` is accepted without checking the application exists; existence is not in the context
   (open, not traced; `PropositionContext::validate_term` :2449 recurses `operands()` which is empty for `Application`).
   `IntegerExactCast` constructors check `can_exact_cast_to` :1595 only.

### SR/src/source_review.rs (49 lines)

Purpose: two vocabulary enums: `IntegerBinaryKind` (31 variants :8-40) and `SemanticDependencyKind` (5 variants :43-49),
comment :1-5 says "Variant order and classification remain the existing encoded vocabulary". Data only. Fail-open none seen.

### SR/src/source_selections.rs (352 lines)

1. Purpose. Records of the source declarations a program used, kept beside emitted code (comment :1-3: "These rows grant no
   permission on their own").
2. Shape. `ProductTarget` (9 variants) :14 with `ALL` :27, `from_code` :39, `from_canonical_name` :46, `code` :52,
   `canonical_name` :59, `source_case` :73; `CompositionMode {Fused, Independent}` :89; `WireCompatibilityFact` :103;
   `BuildCallSite` :127; `SelectionDeclaration` :134; `SelectionPurpose {Build, Product}` :144 with `for_declaring_package` :153;
   `SelectionForm` :169; `SelectionNode` :175; `SelectionOccurrence` :199; `SelectionBindingKind` :205; `SelectionReference` :216;
   `SelectionProductDesignation` :242; `SelectionRole` :250; `SelectionQuestion` :260; `SelectionCallTarget` :269;
   `SelectionJoin` (6 variants) :280; `SelectionQuestions` :322.
3. In/out. Pure data; `IdTable` from `arena`.
4. How it decides. `ProductTarget::from_code(code)` indexes `ALL` by position :39-44; `code()` is `position` in `ALL`
   :52-57 (so SAVED CODES depend on array order; test :332-351 pins 0..8). `from_canonical_name` compares strings
   :46-50. `WireCompatibilityFact::from_name` matches source spelling :112-120.
   `SelectionPurpose::for_declaring_package` :153: an imported library's declarations are `Product` unless `build_root ==
   declaring`.
5. Cases by shape. `ProductTarget` closed list of 9 (including `MacosX86_64` and `UefiX86_64`, which are not among the four
   hosted release targets named in AGENTS.md).
6. Same job elsewhere. Target enumerations also in `target::TargetProfile` (native-targets crate, not read) and the Build
   prelude (`build/inputs/declarations/prelude/targets.rs`, grepped count hit via `.code()` search; not read).
7. Unused. I did not isolate `ProductTarget` call-site counts: grep pattern `ProductTarget::from_code|...|\.code\(\)` over
   `source-rust/*.rs` returned 75 hits in 55 files but `.code()` is a common method name, so the count is not meaningful.
8-9. Stated limits: none. Fail-open: `from_code` returns `None` for out-of-range :39-44. None seen.

### SR/src/storage.rs (808 lines)

1. Purpose. Storage-geometry and structural-place vocabulary: atomic cell types, authored bit-fragment storage, content
   conservation terms and their fingerprints, reference extents, structural place kinds, target carrier layout.
2. Shape. `atomic_cell::AtomicCellType` :13 (`is_supported` :45: Boolean or fixed 8/16/32/64-bit integer); `authored_storage`
   :57 (`AuthoredBitFragment` :66, `AuthoredBitFragments` :75 holding an `Arena` + span, `new` sorts rows :81,
   `byte_envelope` :93; `AuthoredRecordStorage` :136; `AuthoredFieldStorage` :142); `content` :183 (`ContentAlgebra` :189,
   `ContentProjectionIdentity` :201, `ContentProjectionScalar` :207, `ContentProjectionExpression` :218 with
   `report_fingerprint` :264, `ContentPlaceVersion {Entry, Current}` :294, `ContentPlaceSegment<const FIELD_FIRST>` :302 with
   custom `Ord`/`Hash` :318-345, `ContentStructuralPlace` :348, `ContentTerm` :373 with `separate` :382, `ContentConservation`
   :404 with `new` (swaps to sorted order) :411 and `validate` :439, `content_conservation_report_fingerprint` :451,
   `encode_fingerprint_string` :548); `reference_origins::ReferenceOriginExtent` :725; `structural_places::StructuralPlaceKind`
   (7 variants) :735 and `AffineConstructionElement` :792; `target_carrier_layout::TargetCarrierLayout` :800.
3. In/out. Pure data; fingerprints are FNV-1a over hand-built byte strings.
4. How it decides. `AtomicCellType::from_scalar` :20 returns `Some` only for Boolean and supported integers. `byte_envelope`
   :93 returns `None` if any fragment container width is 0 or not a multiple of 8. `ContentTerm::separate` :382 flattens,
   sorts, rejects arity < 2 and duplicates. `content_conservation_report_fingerprint` :451 returns `None` if a projection's
   domain id exceeds u32 :485 or its root place kind is `ByteSequenceLiteral`/`BlockParameter`/`ProviderAttachment`/
   `TrivialAffineLocal` :511-514; hashes `Result` places as bare tag 3 :499. Keyed on place kinds.
5. Cases by shape. Case/field ordering swaps with the const parameter (:299-300 comment "Content records encode case, field,
   then index... Service result paths compare and hash fields before cases"). `is_case_alternative` :355 compares only the
   first differing segment.
6. Same job elsewhere. FNV report fingerprints: `ContentProjectionExpression::report_fingerprint` :264 and
   `content_conservation_report_fingerprint` :451 are two near-identical encoders (kind tags 3/2, parameter string).
7. Unused. `AuthoredBitFragments::byte_envelope` not grepped.
8. Stated limits. :299-300 above. 9. Fail-open. None seen (unknown cases map to `None`).

### SR/src/structural_path_encoding.rs (55 lines)

Purpose: tags for structural path segments (`SegmentTag` :7: Field=1, FixedIndex=2, Referent=3, FixedRange=4, RuntimeIndex=6,
Case=7, RepresentationView=8, AllElements=10; tag 5 retired :19) and `encode_canonical_structural_path_segment` :36 which writes
only Field, FixedIndex, Case, Referent. `SegmentTag::from_byte` :20 rejects retired tag 5 and tags 9, 11+.
Same job elsewhere: `psi/types/structural_path_encoding.rs` (4 hits) and `runtime-abi/.../borrowed_storage/encoding/decoding.rs`
(6 hits) by grep `SegmentTag|encode_canonical_structural_path_segment` over `source-rust/*.rs` = 22 hits in 7 files (not read).
Fail-open: none seen.

### SR/src/terminal_authority.rs (138 lines)

1. Purpose. Canonical sorted sets of `TerminalAuthorityClass` plus the six portable filesystem facets mapped to classes.
2. Shape. `PortableFilesystemAuthorityFacet` :7 (6 variants, `terminal_authority_class` :26); `TerminalAuthorityDisposition`
   :45 (private sorted `Vec`), `Record` impl :49 (read rejects non-strictly-ordered classes :57-58),
   `try_from_canonical_classes` :64, `from_classes` :74, `contains_all` :93, `first_missing_class` :97 (binary search).
3-4. `try_from_canonical_classes` rejects classes not strictly increasing (`windows(2)` `>=`). Keyed on class order tags.
9. Fail-open: none seen; `first_missing_class` returns `None` when `other` is empty (empty set trivially contained).

### SR/tests/expression_clone.rs (122 lines)

Tests: cloning a 32,768-deep arithmetic premise and deep indexed-read/math terms on a 2 MiB stack (:6,:16-23), consuming one
level at a time. No decisions. Records that deep-tree clone is verified under a small stack.

---------------------------------------------------------------------------------------------------

## PART B. source-provenance (SP) and worker-budget (WB)

### SP/entry_contract.rs (16 lines)

1. Purpose. SHA-256 over `(domain string, manifest, relative_source, source bytes)` with u64-LE length framing for a program
   entry's physical contract (`entry_contract_digest` :3). Domain `"omega.program-entry-physical-contract-package-source.v1"` :7.
3-9. No decisions. Users: grepped `entry_contract_digest|toolchain_owned_identity_digest|toolchain_source_revision_digest`
   over `source-rust/*.rs`: 15 hits in 9 files (SP itself 5). Fail-open none seen.

### SP/source_provenance.rs (262 lines)

1. Purpose. Content-digest identity records for consumed sources and package keys, no file contents (comment :1-3).
2. Shape. `DependencyScope {Product, Build}` :13; `PackageKeyIdentity([u8;32])` :20 (`from_digest` rejects all-zero :23;
   hand `Record` impl :32); `PackageSourceConsumptionCommitment` :49 (public `#[doc(hidden)] from_retained_digest`);
   `ConsumedSourceUnitKind` :67; `ConsumedSourceUnit` :76 with `from_retained` :87; `canonical_consumed_unit_bytes` :156;
   `append_canonical_consumed_unit` :162; `toolchain_source_revision_digest` :200; `source_content_digest` :221;
   `toolchain_owned_identity_digest` :233.
3. In/out. Reads byte slices; outputs 32-byte SHA-256 digests.
4. How it decides. `from_retained` :87 requires coordinates to match kind: `PackageAuthored/PackageGenerated` need a package
   and no toolchain namespace; `ToolchainOwned/ToolchainContract` need a nonempty namespace and no package; `ToolchainVirtual`
   needs neither :95-109; relative path nonempty with nonempty components :118. `toolchain_owned_identity_digest` :233
   requires namespace and path components nonempty, not `.`/`..`, no `/` or `\`.
5. Cases by shape. Closed 5-kind list; kind bytes 0,1,4,2,3,5 :163-170 (generated Build = 4, generated Product = 1).
6. Same job elsewhere. `source_content_digest`, `canonical_consumed_unit_bytes`, `append_canonical_consumed_unit` grepped
   over `source-rust/*.rs` = 13 hits in 4 files (`build/inputs/source/source_consumption.rs`, `declare.rs`, `custody/src/lib.rs`
   4; SP 6). The domain strings for custody/toolchain digests appear in `source-files/custody` (not read).
7. Unused. See above.
8. Stated limits. None.
9. Fail-open. `PackageSourceConsumptionCommitment::from_retained_digest` :55 is a public (doc-hidden) constructor with no
   validation, so any 32 bytes (including zero) can be wrapped; `PackageKeyIdentity` forbids zero but this type does not.
   Classification: open (doc-hidden means "retained decode only"; no caller traced).

### WB/src/lib.rs (137 lines)

1. Purpose. One worker budget per compiler invocation: a fixed Rayon pool of 256 MiB-stack workers.
2. Shape. `WORKER_STACK_SIZE = 256 MiB` :22; `ON_BUDGET_WORKER` thread-local :24; `ExecutionBudget {pool}` :29 with
   `for_host` :36 (uses RAYON_NUM_THREADS/logical CPUs), `with_workers(n)` :41 (`n.max(1)`), `run` :60; `on_budget_worker` :66;
   `current_workers` :71 (1 off-budget); `within_budget` :84 (runs inline on a budget worker, else builds a temporary
   1-worker budget).
4. How it decides. Keyed on the thread-local marker set by the pool start handler :49.
6. Same job elsewhere / ownership claim. Comment :8 says `ExecutionBudget` "is the single owner of those workers". Grepped
   `ExecutionBudget` over `source-rust/*.rs`: production creation only in `application/src/main.rs:69` (`for_host`) and
   `lib.rs:88` (`within_budget` temporary budgets); the rest are tests/benchmarks. Other thread creation (grepped
   `rayon::ThreadPoolBuilder|ThreadPoolBuilder::new|rayon::spawn|std::thread::spawn|thread::Builder` over `source-rust/*.rs`: 42
   hits in 17 files) includes `foundation/bounded-process`, `application/src/execution/private_observation/channel_*.rs`,
   Build accounting tests, and many tests; I did not read the production ones, so I cannot say whether they bypass the budget.
7. Unused. `current_workers`/`within_budget`/`on_budget_worker` grepped with `ExecutionBudget` and `worker_budget`: 42 hits in
   18 files (parse batch, lexer, parser, resolver const evaluation, `machine_batch.rs` use them).
8. Stated limits. :19-21 "Recursive syntax, type and proof walks still need deep stacks until they move onto flat tables
   (`PIPELINE-ARCHITECTURE-ENFORCEMENT`)." Header :5-7 notes the old per-stage worker choices.
9. Fail-open. `within_budget` :90 panics ("execution budget worker must spawn") if the pool cannot start rather than running
   unbudgeted. None seen.

---------------------------------------------------------------------------------------------------

## PART C. native driver (DRV)

The driver composes: `inputs::prepare` -> `produce` -> (component exports, root lowering, admission, plan callbacks, physical
planning, encode, layout, object, image). Public entries: `compile` DRV/mod.rs:28 and `admit_runtime` :39.

### DRV/mod.rs (225 lines)

1. Purpose. Two routes from a retained Terminal artifact: native image (`compile`) or runtime admission for the Psi
   interpreter (`admit_runtime`), both via `produce` :67.
2. Shape. `NativeProductionStop {Image, RuntimeAdmission}` :54; `Produced` :59; `produce` :67-152; `AdmittedRealization` :154;
   `admit_realization` :162-225.
3. In/out. Reads `PreparedNativeCompilation` (inputs/mod.rs:15). Produces `(FinishedNativeImage, NativeStageReplay)` or
   `AdmittedRuntimeProgram`.
4. How it decides. `produce` :81 binds component exports (`image::components::bind`), emits them :91, lowers the root :95,
   builds the admission request :113, admits :119; if stop is `RuntimeAdmission`, requires `runtime_provisions` else error
   :121-123. `admit_realization` :173 requires `proposal.program_entry()` ("executable admission requires a ProgramEntry
   root") and plans runtime provisions only if `entry.checked_entry` is `Some` :187-189.
5. Cases by shape. Two stop modes; `unreachable!` if `produce` returns the other variant :35,:49.
6. Same job elsewhere. Callable (component) path in `image/components.rs:126-263` repeats lower -> admit -> plan -> physical ->
   object that the root does here and in `image/object_emission.rs:46-277` (read; see Part C file notes).
7. Unused/one-caller. `compile` and `admit_runtime` are `pub(crate)`; callers are outside this unit (not grepped).
8. Stated limits. None. 9. Fail-open. none seen.

### DRV/request.rs (56 lines)

`native_optimization_budget` :10: constants `OptimizationWorkBudget::new(1_000_000, 100_000, 4_000_000, 100_000, 1_000_000)`;
comment :9 "Per-pass traversal and validation ceilings for the reference extent (2^16 units)". `NativeCompileRequest` :15 carries
suppliers, policies, image request, scalar receipts. `image_request` :29 builds `ExecutionImageEmissionRequest::direct` and
settles application metadata, mapping 3 `ImageApplicationMetadataError` cases to messages :46-52. Limits stated: fixed budget
constants (invented numbers, count: 5, in one function; used by 3 call sites: DRV/mod.rs:134, components.rs:198, per grep not
run). Fail-open: none seen.

### DRV/observations.rs (116 lines)

Counters of entered pipeline stages (`NativePipelineStage` :44, 11 values 0..10; `NativePipelineObservation.counts[11]` :63).
Comment :1-6: "These observations never supply semantic inputs, reuse decisions, or admission evidence". With the
`test-support` feature, `Drop` records an invocation into a global `Mutex<Vec<...>>` only if `ENABLED` :100. The recorded
`stages: [u64; 9]` :23 covers stages 0..8; fragment/image emission recorded separately :107-108. No decisions on program input.
Fail-open: none.

### DRV/compile_body.rs (302 lines)

1. Purpose. Run native stages 03-15 for the application entry and each private callable: abstract optimization -> target
   lowering -> instruction selection -> selected-instruction optimization -> register allocation -> post-allocation machine ->
   frame planning -> body planning -> binding -> encoding -> layout -> function placement -> object construction.
2. Shape. `plan_native_body` :20; `NativeBodyPlanningError` :15; `PhysicalPlanningOriginals` :42; `OptimizedVerifiedPhysicalPipelineError`
   :49 (8 variants); `PhysicalBody<Stage>` :87 with aliases `PreparedPhysicalBody`/`BoundPhysicalBody` :84-85; `prepare_physical_body`
   :107; `encode_physical_body` :186; `layout_encoded_body` :212; `construct_body_object` :267; `BodyEmissionError` :286.
3. In/out. Takes `VerifiedNativeArtifactInput` and target-lowering requests; produces `BodyObject`. Retains "original"
   inputs only when `retain_original`/`originals` for replay audits.
4. How it decides. `layout_encoded_body` :216-245 requires encoding and layout to agree on replay custody: `(Some, Some)`
   retains; `(None, None)` none; mixed -> error "layout and encoding disagree on independent replay custody" :232-234; and the
   retained inputs must `belong_to` the relaxed layout :236-245. Error mapping by variant (:123-132,:193-208).
5. Cases by shape. `prepare_physical_body` error mapping singles out `RegisterEnvironment`, `BodyBindings`, `Installation`;
   all other selection errors fold into `Selection`. 7 matched enumerators across the file.
6. Same job elsewhere. Root path (`object_emission.rs:46`) and callable path (`callable_objects.rs`) both call these same
   functions: single shared body driver (AGENTS.md says the shared body driver is canonical; this file is it).
7. Unused. `plan_native_body` called from `plan_callables.rs:83,165` (read). 
8. Stated limits. None. 9. Fail-open. none seen.

### DRV/image/mod.rs (90 lines)

`prepare_program_image` :18 plans the program (`plan_program`), builds provider image inputs, emits the realization object,
settles the ELF loader into the image request :48-54, and returns `ProgramImageInput` + `NativeStageReplay`.
`selected_physical_pipeline_failed` :69 formats an error naming selected optimizations by their `build_case_name()` (source
spelling) and says "no alternate compiler route was run and no output was installed" :84. Fail-open: none.

### DRV/image/plan_callables.rs (379 lines)

1. Purpose. Plan the root body, then every private callback body reachable from it (a closure by worklist), before any
   physical planning.
2. Shape. `NativeProgramPlanningRequest` :11; `CallableTargetProduct` :21; `plan_callable` :35 (lowers one callable with its
   own runtime provisions); `PrivateEntryContract` :127 (marked `#[non_exhaustive]`); `plan_program` :160;
   `PlannedProgramBodies` :195 with `from_root` :211; `PrivateBodyDemand` :239 with `new` :247 and `enqueue` :271;
   `plan_callback_target` :338.
3. In/out. Reads `proposal.occurrences().callbacks`; produces roster of `(PrivateEntryContract, CallableTargetProduct)`.
4. How it decides. `PrivateBodyDemand::new` :247 builds `by_placement`; refuses placement outside range or duplicated.
   `enqueue` :271 requires `callbacks.psi() == psi` and `interrupts.psi() == psi`, and for each private reference identity:
   it must have a placement index, the identity's caller equals this psi, equals the occurrence's `callback_thunk_identity`,
   and match an admitted callback row OR an installed-interrupt entry on `(operation, placement, function)` :283-314;
   queued once :315; siblings sorted by index :321. Keyed on placement index and `MachineFunctionIdentity`.
5. Cases by shape. Reference must be a "callback thunk placement"; other private functions are refused with "referenced
   private function has no original placement" :288-290.
6. Same job elsewhere. `plan_callable` (components path) and `plan_callback_target` both go through `plan_callable`; but
   `plan_callback_target` :378 DISCARDS the returned `CallableAdmission` (runtime provisions and callable authority) via
   `.map(|(target, _)| target)`, while the component path uses `evidence.runtime` and `evidence.authority`
   (components.rs:241-249). Whether callbacks need those is not traced.
7. Unused. `inspect_private_body_demand` :327 is `#[cfg(test)]`; grepped `handler_planning_order|inspect_private_body_demand`
   over `source-rust/*.rs` = 14 hits in 4 files (callback.rs test 6).
8. Stated limits. :122-124 "Source lookup and native lowering end before any body asks for a numeric installed-handler stack
   guard."
9. Fail-open. none seen: every miss returns an error.

### DRV/image/callback_thunks.rs (609 lines) and callback_thunks/handler_order.rs (87 lines)

1. Purpose. Close private-callback and installed-interrupt-handler bodies: plan their physical form in dependency order so
   a handler's stack/fault summary is known before the installer's frame is planned; compute installed-entry plans; measure
   handlers; stage encode/layout/object construction of private bodies.
2. Shape. `PreparedCallbackBodies` :24; `PlannedPrivateBody` :33 (lazy `OnceLock` measurement :37); wrapper types
   `PrivateBody<Callable, Entry>` :51, `BoundPrivateBody` :56; `PreparedCallbackBodies::from_roster` :155 and `plan` :194;
   `retained_handler_measurements` :214; `retained_installations` :251; `prepare_installed_callbacks` :298; `finish_physical`
   :337; `plan_physical_callback` :355; `PreparedEntryPlans` :451 with `dependencies` :462, `retain` :488, `for_body` :559.
   `handler_planning_order` handler_order.rs:8 is an iterative DFS giving `(before_root, after_root)` orders; errors on cycle
   :40-43 and on out-of-roster ids :29,:38.
3. In/out. Reads the admitted installed-interrupt cohort from the body's `semantic_facts().admission`.
4. How it decides. `retained_installations` :251: entries sorted by `(machine, operation, application_index)`; duplicate
   triples refused :264-277; scopes kept only if `(machine, operation)` is in the plan's `installation_operations` :279-283.
   `retain` :488: exactly one entry matches the timer anchor's application index :497-502; anchor function/machine/operation/
   acknowledgement application must equal :503-510; every entry must share psi and target with the timer :514-521;
   acknowledgement application -> `NoErrorCode` plan, else needs exactly one matching `fatal_entries` entry :523-539; a
   placement slot may be re-retained only with an identical value :549-553. `for_body` :559: matches cohort entry and retained
   entry on `(machine, operation, callback_function, psi, entry)` :585-595. Keyed on identities and applicaton indices.
   `plan_physical_callback` :355-442 checks the emitted call plan arity against the boundary entry plan :385-393, and each
   formal's unique native origin and equality of placement and shape :394-426.
5. Cases by shape. Timer-acknowledgement vs fatal handler: "NoErrorCode" or fatal-context :523-539 (two kinds).
6. Same job elsewhere. `body_resources::plan` (below) and `handler_planning_order` both walk callback dependency graphs with
   explicit cycle detection (body_resources.rs:42-46; handler_order.rs:39-43).
7. Unused. Fine.
8. Stated limits. :1-4 header comment; :212-213 "Measure only placements that a surviving guard actually references".
9. Fail-open. `for_body` returns `Ok(None)` when neither the cohort nor `retained` has an entry for the placement :586
   (stated by structure: not an installed handler). A cohort entry without retained evidence is an error `_ => Err` :604.
   Traced. None else seen.

### DRV/image/body_resources.rs (67 lines)

`plan` :11 / `visit` :18: computes `StackDemand` for the root by visiting callbacks "actually invoked by a retained foreign call"
(comment :1-3); errors if an invoked callback has no retained private body :41, on a cycle :43, or on overflow :54-56. Keyed on
`MachineFunctionIdentity`. Fail-open: a body with no foreign calls yields empty demands (stated header). None seen.

### DRV/image/callable_objects.rs (127 lines)

Typestate wrapper `CallableProduct<Body>` :14 carrying custody (`code`, `coverage`, `observation`) through
prepare/bind/encode/layout/construct stages :42-126. `construct_object` :104 builds a `PrivatePhysicalOriginal` from the object's
psi identity, code and coverage :114-119. No decisions beyond error mapping :30-40. Same job as `callback_thunks.rs` wrappers.

### DRV/image/object_emission.rs (316 lines)

1. Purpose. Close physical callback/handler plans and installation requirements before encoding any body; then encode, lay
   out, construct objects for root and private roster stage by stage ("The next stage receives its input only after every
   body in the current stage succeeds" :22-23), then publish a fragment object.
2. Shape. `BodyRoster<Root, Private>` :24 with `try_map` :30; `emit_realization_object` :46-277; `EmittedRealizationObject` :16.
3. In/out. Reads the planned roster, program entry settlement, runtime provisions.
4. How it decides. UEFI check keyed on `target::TargetProfile::UefiX64` :58,:127 to build a physical root. Hosted-entry plan
   if `needs_hosted_entry(...)` :85. Physical entry planned only for UEFI :126-188; errors if no physical adapter :129, root
   is not the adapter :135, no calling plan :141, no physical contract :144.
5. Cases by shape. Entry kinds: hosted program entry, physical (UEFI) program entry, or neither; keyed on
   `entry.source().target_slot().owner == UefiX64` (a target-name comparison).
6. Same job elsewhere. Staging root and private roster in lockstep is the same pattern as `PreparedCallbackBodies`.
7. Unused. `BodyRoster` private to the file; test :279-315.
8. Stated limits. None. 9. Fail-open. If `program_entry` is `None`, no hosted receiver is admitted
   (`program_entry.map(...).transpose()` :92-111 yields `None`) and `plan_hosted_program_entry` is still called with
   `receiver: None` :113-120. What a `None` receiver permits is not visible in this file (open, not traced).

### DRV/image/components.rs (321 lines) and components/settlement.rs (146 lines)

1. Purpose. Compile each selected independently-supplied package export under its own providers, publish a callable image, and
   join it to the receiving provider occurrence with a finite stack contribution.
2. Shape. `Export` :16; `bind` :24; `Emission` :86 with `record` :100; `emit` :126-263 (recursive over supplier dependencies);
   `export_ordinals` :265. settlement.rs: `ComponentCall` :14 with `join` :23; `compact_report` :118 (first 8 digest bytes,
   `max(1)`); `impl ProviderExecutionEvidence for ComponentCall` :122.
3. In/out. Reads `TerminalNativeRealizationProposal.component_imports()`, `VerifiedSuppliers`.
4. How it decides. `bind` :55-62 skips an import call whose receiving boundary is not in the execution scope's reach
   (`continue`). Diamonds: if a product with same selection, application and artifact identity exists, reuse it :147-158.
   `join` :32-39 requires matching selection/application/plan digest; needs a finite stack ceiling :41-43. `record` :100:
   same slot commitment must have identical locator and description :110-117. `export_ordinals` requires scalar and structural
   parameter counts to match the boundary declaration and every formal to be a `SemanticFormal` :278-299. Keyed on digests,
   boundary ids and slot commitments.
5. Cases by shape. `emit` refuses private callback references in a component export: "private callback references need their
   separately compiled bodies" :204-208, and "private callback formals require component-owned thunks" :296-299 (two stated
   unsupported shapes).
6. Same job elsewhere. Root lowering (DRV/mod.rs:95-119 + object_emission.rs) repeats the lower/admit/plan/physical sequence
   at :160-227. Domain-digest commitment encoding: `ComponentCall::join` :44-67 uses `DomainDigestEncoder` with domain
   `"omega.component-native-call.v1"`.
7. Unused. `ComponentCall::settlement` used at DRV/mod.rs:111 and components.rs:186.
8. Stated limits. :208 and :298 quoted above.
9. Fail-open. `bind` skip at :56-62 omits non-reached exports from emission and from the settlement list; stated by position
   (comment-less) and consistent with the scope-reach design: an export not in reach is not called. Traced to `execution.scope().reach()`.
   `NativeApplicationSettings { application: Default::default() ...}` :66-67 gives components default application metadata
   (open: not traced what metadata a default omits).

### DRV/inputs/mod.rs (126 lines)

`prepare` :23: validates installation environment assumptions, selects the execution (stage 01 `prepare_program`), turns each
`private_entry_ram` initializer into a checked recipe (initializer must answer a declared entry position :41-50), completes scalar
applications. `validate_installation_environment_inputs` :78: each assumption must validate, match artifact psi and target
profile :93-97, name the selected entry or an advertised private callback entry :98-109, name a retained interrupt installation
:110-116, and a `(entry, installation)` pair may repeat only across kinds (separate sets for Bootstrap and InterruptDelivery)
:117-123. Keyed on psi identity, profile, machine id. Fail-open: no check that every declared entry position receives an
initializer (not in this file; open).

### DRV/inputs/components.rs (147 lines)

`verify` :37: iterative depth-first verification of supplier packages; cycle in `open` set refused "cyclic independent supplier
selection" :60; supplier whose published package differs from the requested owner refused :65; each supplier verified under its
own providers via `verify_publication` :104: receive publication, issue provider demand, admit, select providers, describe
component, `component_description::verify_component` with schema `COMPONENT_DESCRIPTION_SCHEMA_V3` and
`AcceptedComponentAssumptions::PendingConsumerAcceptance` :133-136. Keyed on `PackageKeyIdentity`. Stated limit: comment :127-128
"The receiving program must compare every environmental premise with its own acceptance before release." Fail-open risk:
`PendingConsumerAcceptance` defers environment premises to the consumer (stated :127-128; consumer check not traced here).

### DRV/inputs/published_application.rs (176 lines)

Entry points for source and saved programs: `SourceNativeTarget` :19, `SavedProgramTarget` :9, `prepare_source_program` :28,
`review_source_providers` :64, `prepare_saved_program` :128, `prepare_issued_program` :154. Decisions: `review_source_providers`
publishes an independent component description only `if has_exports` (some provider candidate has
`source_candidate_package_identity == Some(package)`) :91-103 and uses `.ok()` so a description failure becomes `None`
(optional; comment :97 "Publishing an independent component is optional until a receiving Product selects it"). Fail-open:
`.ok()` :99 swallows the description error (stated by that comment).

### DRV/inputs/scalars.rs (32 lines)

`complete` :7: returns `Ok(None)` only if demand is empty AND the artifact has no scalar conditions (:14-21, comment :13 "Scalar
conditions must be discharged even when no reachable code reads a scalar"); otherwise runs `native_scalar_inputs::complete`.

### DRV/runtime/mod.rs (97 lines) and runtime/foreign_adapters.rs (59 lines)

Runtime provisioning for the Terminal interpreter: `NativeForeignAdapterRequest` :15, `plan_adapter` :46, `plan_host_provisions`
:60, `plan_root_provisions` :79. `foreign_adapters.rs`: `compile` :18 chains plan -> bind -> encode -> place -> construct for an
adapter; test hook `compile_runtime_foreign_adapter` gated on `test-support` :46. No judgments beyond error propagation.

---------------------------------------------------------------------------------------------------

## PART D. native stage 00 (P0)

### P0/admit.rs (98 lines)

`admit` :13: takes an `AuthorizedTerminalArtifact` and an `IssuedProviderDemand`, selects the artifact the demand names
(`Unscoped{artifact}` or `Target{origins.artifact()}`) and calls `authorization.require_artifact(artifact)` :21 which compares
manifests (test :92-96 shows a debug-map change produces `ArtifactMismatch`). Output `AdmittedProviderDemand` :8. Keyed on the
artifact manifest identity. Fail-open: none seen.

### P0/host-providers/installation.rs (210 lines)

Provider installation inputs: `ProviderInstallationInputs` :25, `InstalledPackageProviders` :34, `TargetProviderInstallation` :75 with
`attach` :94 (refuses a different manifest identity :99 or profile :102), `ProviderExecutionOrigins` :17 with `into_inputs` :138
(profile check :148). Keyed on `TerminalArtifactIdentity` and `TargetProfile` equality. Test :163-209. Fail-open: none seen.

### P0/provider-demand/demand.rs (176 lines)

`ProviderDemandInputs {Unscoped, Target}` :17; `IssuedProviderDemand<Publisher>` :44; `from_publication` :58 maps
`(profile, demand)` pairs: `(None, Unscoped)` and `(Some, Target)` valid, any other pairing is "closed provider demand has a
different deployment scope" :107-110; requires `publication_metadata` :80-87. `from_saved_program` :132 attaches installation,
requires publication metadata (error text mentions "recompile it to publish entry and calling declarations" :143), sorts and
dedups requested boundaries :148-149, closes providers via `psi_provider_selections::logical_reach::close_providers`, and maps
requirement boundary ids to label text :157-164 (a requirement whose identity text is missing is DROPPED by `filter_map` :160-163).
Fail-open: `labels` silently omits boundaries lacking a declaration or identity text (open; label use not traced).

### P0/scalar-applications/scalar_applications.rs (234 lines) and completed.rs (80), completed/tests.rs (85)

`complete` :14 / `prepare` :28 run `build_scalar_evaluation::prepare_scalar_applications` per scope; per scope the closure finds
the provider proposal by target name :68-71, requires its `terminal_psi` equals the artifact :72, builds providers and a
`TargetRealizedIntrinsics` :120. `TargetRealizedIntrinsics::admit` :148: only boundaries with a nonempty identity text are
demanded; a boundary with no identity "stays unrealized" (comment :154-155). Closes address model via
`target::TargetSemantics::close_address_model`. `ProductScalarCompletion::extend` completed.rs:22 refuses a changed artifact,
target or provider proposal (`:30-47`) and returns `None` when no demand was given :48-50 (test :56-72 pins absent demand vs empty
roster). Keyed on artifact identity, target name string, proposal equality. Fail-open: stated at scalar_applications.rs:154-155
for identity-less imports (unrealized = not admitted); no other.

---------------------------------------------------------------------------------------------------

## PART E. native stage 01 (P1)

### P1/execution-scope/scope.rs (396 lines) and tests.rs (404)

1. Purpose. `derive_native_scope` :12 follows machines, callbacks and provider boundaries reachable from the selected entry.
2. Shape. `ScopeInputs` :35; `TargetReach::derive` :107-311; `ScopeBoundaryIndex::retain_boundary` :315-392;
   `NativeExecutionScopeError` (8 variants) :50.
3. In/out. Reads `TerminalModule` (machines, boundary machines, provider candidates, callback applications, dynamic dispatch
   tables, installed-interrupt helper roots); returns `ExecutionReach` + selected candidate positions.
4. How it decides. Worklist over machines: pushes the entry and EVERY machine in every dynamic-dispatch table row :180-187;
   per operation pushes module-invocation callees :227, `SelectMachine` descriptors (checked: descriptor exists, result type is
   `Callable(selected.contract)`, contract exists) :231-252, `EstablishNativeCodeLayout` machines :253, cleanup machines from
   terminators :269-277. Callback occurrences: an occurrence is reached only when its registrar operation (keyed `(machine id,
   operation id)`) executes in a reached machine :220-225; callback registrar mismatch refused :168. `retain_boundary` :315:
   boundary must exist in module (`MissingBoundary`); entry call match short-circuits to its machine :331; assumed bindings
   chase the supplied boundary :335-347; then picks the candidate whose requirement identity matches a selected adapter row
   AND whose provider identity, declaration and machine identity match STRINGS of the selected row :359-378 (exactly one, else
   `MissingSelectedCheckedProvider` / `CompetingSelectedCheckedProviders`). Keyed on identity strings and ids.
5. Cases by shape. Only `BoundaryCall` and `Terminator::ExternalCompletion` create boundary reach.
6. Same job elsewhere. `body_resources::visit` (DRV) and `PrivateBodyDemand::enqueue` (DRV) also walk reach; not duplicates.
7. Unused. Tests cover dormant scalar work, boundary demand owners, callback-only-when-registrar-runs, target-independence
   (tests.rs:129-404).
8. Stated limits. None in code (test comment :135 "The fixture has a scalar result, so including this machine must reject it").
9. Fail-open risks.
   - `retain_boundary` :348-350: `candidates_by_boundary.get(&boundary)` is `None` -> `return Ok(())` after the boundary was
     inserted into `reached` :330. A reached boundary with no provider candidate at all is accepted by this function.
   - `:351-358`: if no candidate's requirement identity has a selected adapter row, returns `Ok(())` (boundary reached, no
     body pushed). Whether a reached boundary with no supplier fails at a later stage was not traced (provider settlement in
     stage 01 `provider_bindings::bind` :100-117 puts such boundaries into `direct_demands` and then checks them with
     `VerifiedDirectProviderSelection::check`, so "traced" only for the direct-leaf route). Classification: partly traced,
     partly open. No comment at the site.
   - `TargetReach::derive` accepts any `assumptions` pair table without checking cycles; recursion guard is the `reached` set
     :324-330 (traced; terminates).

### P1/src/lib.rs (67 lines)

Crate root "provider realization" listing modules (`calling_policy_plans`, `native_bindings`, `program_entry`,
`published_application`, `target_supply`) and re-exports; `test_support::select_realization` behind a feature :50-64. Header :4-6
says "missing provider authority never becomes a source fallback or a fabricated proof". No decisions.

### P1/src/execution.rs (151 lines)

`plan_unscoped_execution` :21, `plan_published_target_execution` :32, `plan_target_execution` :77, `selected_boundary_implementations`
:111. Decisions: `plan_target_execution` realizes supply then requires selected implementations sorted-adjacent distinct per
boundary :90-97 and EQUAL to the closed logical `bindings` :98-102 ("native planning changed a closed logical provider body").
`selected_boundary_implementations` :111 skips providers whose composition mode is `Independent` :121 and providers with no
`CheckedAdapter` row :124-131. Keyed on boundary ids and machine ids. Fail-open: none seen (`FoldedEntryReach::capture` result is
stored unresolved via `.with_folded_entry_reach(folded_entry)` :68, a `Result<_, String>` kept until bound (comment :45-46)).

### P1/src/folded_entry.rs (268 lines)

1. Purpose. Commit the exact providers used by scalar (constant) evaluation into a folded artifact so a receiver can reconstruct
   them. `FoldedTargetProgram::close` :14; `FoldedEntryReach` :89 with `capture` :101 and `bind` :129;
   `folded_constant_dependencies` :196; `folded_dependencies_for_rows` :210.
4. How it decides. `close` requires the original is unbound (no folded target, no optimization selections) :19-23 and evidence
   to match when publication metadata exists :35-45. `bind` requires program/target equal and selected bodies equal to the
   captured ones :135-147; returns empty closure when the module has no scalar-application declarations :139-141. Dependencies
   kept for candidates whose boundary is in the reached set `binary_search` :226. Error if a plan has empty or different
   target :218-223. Keyed on psi identity, target string, boundary ids. Test :257 pins declaration-only programs.
9. Fail-open. `folded_constant_dependencies` returns `Default` (no dependencies) when `roots` is empty or the module has no
   scalar-application declarations :202-204. Stated by the guard; the wrong case would be a module with applications queried
   with no roots (open, not traced).

### P1/src/native_bindings.rs (4) and native_bindings/plans.rs (145), plans_tests.rs (109)

`normalize_native_binding` plans.rs:19: requires the requirement identity string and the service declaration digest to equal
the supplied ones and be non-empty/non-zero :25-30; validates the evaluation; normalizes `Import` locators through
`target::normalize_foreign_locator` or `Syscall` numbers: syscall allowed only for `LinuxArm64 | LinuxX64` :54-59, number must
fit `u32` :60-61. Hash domains `"omega.native-build-producer-closure.v1\0"` etc. Keyed on target profile enum and strings.
Cases by shape: two locator mechanisms; two syscall-capable targets (stated limit by `matches!`). Tests plans_tests.rs:49-109
include a negative case for Windows + syscall. Fail-open: none seen.

### P1/src/native_execution.rs (159 lines) and native_execution/provider_bindings.rs (247 lines)

1. Purpose. Issue the target-selected execution reach and bind reachable provider boundaries.
2. Shape. `PreparedNativeExecution` :14 (`checked`, `providers`); `prepare_program` :93; `BoundNativeCallable` :103;
   provider_bindings.rs: `selected_intrinsic_provenance` :10, `callable_coverage` :70, `bind` :80-247.
4. How it decides. `from_scoped_providers` :20 requires boundary-application coverage demands equal scope demands :24-30 and
   validates ALL published evidence seals even for dormant code :32-45. `bind` :80: per in-scope boundary requires nonempty
   identity text :91-96; boundaries without a selected requirement row become `direct_demands` :108-116; a direct foreign leaf
   adds an external binding row if none exists for `(target_name, requirement_identity)`, same row is accepted, any different
   or doubled row is "competing retained external bindings" :132-143. `selected_intrinsic_provenance` :10 finds the plan in
   the proposal by `==` equality (position) else as a direct leaf, else "unowned direct plan" :43-52.
5. Cases by shape. Two plan owners (original selection or direct leaf); two `DirectBoundaryExecution` kinds (Intrinsic /
   Foreign) :54-61.
6. Same job elsewhere. Evidence check at :32-45 repeats an unreachable-evidence validation (read) done in
   `terminal_codec`.
7. Unused. `OMEGA_TRACE_NATIVE_BOUNDARY` env var read at provider_bindings.rs:186; grepped over `*.{rs,md,toml}` in the checkout:
   4 hits (provider_bindings.rs:1, target_lowering/coordination.rs:1, two in an older atlas record). It prints to stderr only
   (bounded 4096 bytes :232), no effect on decisions (comment :187-189).
8. Stated limits. None. 9. Fail-open. none seen.

### P1/src/package_review.rs (236 lines)

Settles numeric operations and native-binding descriptions for package review (header :1-3: "Only the selected execution's own
rosters grant runtime supply; a library may expose additional authored declarations for review"). `prepare` :17,
`PreparedReview::finish` :74, `numeric` :115, `ReviewedNumericBinding::from_leaf` :195 (requires the leaf's plan target equals
the requested target and the declaration's `execution == NumericPrimitive` :201-205). Decisions: numeric leaves are looked up by
CALLABLE IDENTITY string with `binary_search_by` on a sorted list :134; ones not retained are built via
`direct_compiler_primitive_leaves` :142; duplicate callable identity refused :150-155. Results are kept as `Result<...,
Vec<Diagnostic>>` fields so one review part can fail while others succeed (`PackageTargetReview` :172). Fail-open: the review
struct stores errors rather than refusing the whole review; consumers must read each `Result`. Open (consumers not read).

### P1/src/program_entry.rs (267 lines)

`SelectedCompilerProgramEntry` :21: `from_terminal` :28 requires an independently published machine, a checked declaration,
result `Unit` :41, nonempty nominal and callable identities, builds a signature. `with_calling_plans` :88 requires plan target
equals the slot owner target, matching requirement identity strings between plans and storage contract, nonzero commitments
:124-139 and recomputed commitment equal. `bind_physical_adapter` :152: Fail-open: returns `Ok(())` WITHOUT setting an adapter
when `calling_plans` is `None` :157-159, and when no plan row matches the requirement identity `selected` stays `None` and
`physical_adapter` is set to `None` :254. Traced consumer: `object_emission.rs:126-134` errors "semantic entry requires an
explicitly selected checked physical adapter" ONLY for `UefiX64`; other targets do not need an adapter. Classification: traced.
Cases by shape: requires `case` uniqueness: exactly one explicit source provider choice :194-198, a `CheckedAdapter` binding :199,
`type_parameter_count == 0` :209, exactly one candidate position :224-229.

### P1/src/calling_policy_plans.rs (147 lines)

Module list and re-exports; `TerminalCallingPolicyInvocation` :41; `scalar_completion::complete_calling_scalar_inputs` :69: for each
scope of a `ScalarApplicationClosure`, requires `profile.layout_target_facts()` (immutable layout catalogue) :105-107; demand
passed only for the primary target scope :121-125; errors "calling scalar scope has no deployment selection" when a scope is
`None` :98. Header :1-4 says "The old source request/review preparation ... have been removed; these data contracts do not
supply a replacement source-plan producer."

### P1/src/calling_policy_plans/boundary_signatures.rs (258 lines)

`BoundaryOpaqueMovementGeometry::movement` :21 matches an opaque occurrence's shape node to exactly one top-level parameter or
the result (count of carriers must be 1 :37-42), finds the single native parameter mapping to that formal+root :46-66, maps
to the wire ordinal via `NativeCallProjection` and takes the validated placement :67-83. `boundary_shape_path` :105 walks the
shape graph (record fields/fixed arrays), requiring postorder acyclicity (`element >= root` / `child.shape >= root` refused) and
refusing a node reachable through two record fields :159-164. Cases by shape: shape classes Integer/Float/Reference/FixedArray/
Record. Fail-open: none seen. Test :183-257 for dispatch-only formal.

### P1/src/calling_policy_plans/build_time_decoding.rs (590 lines)

Decodes `BuildTimeValue` returned by an authored calling policy into `BoundaryPlanResult`/`BoundaryEntryPlan`/`CallPlan`/
`StatePlan`/`NativePlace`. `decode_boundary_plan_result` :31, helpers :52-563. Decisions keyed on SOURCE SPELLING of case names:
`case_parts` :498 strips any `::` prefix and compares the last segment (`rsplit("::").next()`) to strings like `"Accepted"`,
`"Rejected"`, `"MicrosoftX64"`, `"SystemVAMD64"`, `"Aapcs64"`, `"LinuxSyscallX86_64"`, `"LinuxSyscallAarch64"`, `"DarwinAapcs64"`,
`"X86Rax"..."Aarch64V"`, `"CallReturn"`, `"SupervisorCall"`, `"InterruptReturn"`, `"X86Long64"`, `"Aarch64A64"`,
`"Interrupted"`, `"Dedicated"`, `"ProviderSelected"`, `"NotApplicable"`, `"Masked"`, `"Nestable"`, `"ProviderDefined"`.
Unknown cases error "is outside the compiler-owned vocabulary". Counted arrays: `decode_counted_array` :465 bounds count by
capacity (`PARAMETER_CAPACITY`, `VALUE_SHAPE_CAPACITY`, `VALUE_FIELD_CAPACITY`, `CALLBACK_MATERIALIZATION_CAPACITY` imported
from other crates; `LOCATION_CAPACITY=16` :23, `REGISTER_CAPACITY=64` :25, `CALLBACK_FIELD_PATH_CAPACITY=16` :29) and
decodes only `values[..count]` :485 (extra cells beyond `count` are IGNORED, not checked as zero). `uint` :523 reinterprets
`BuildTimeValue::Int(i64)` as `u64` bits (`*value as u64`), header :1-5 states this. Struct fields are found by NAME :511-521
(source spelling). Capacities are invented numbers; maxima for locations (16) and registers (64) are fixed here.
Fail-open: ignoring cells past `count` and ignoring `payload` of `CallingConvention` case :166 `case_parts(...).0` mean extra data
passes. Classification: open for the extra cells (the downstream `validate_*` checks the decoded plan, not the raw value); not
traced further.

### P1/src/calling_policy_plans/callback_layouts.rs (538 lines)

Materializes inline callback storage paths (`TerminalCallbackLayouts` :34) for records containing private callback slots.
`for_parameter` :71; `close_record` :100-290; `walk` :292-371; `callback_storage_has_slots` :374; `OccupiedBits` :382 with
`overlaps` :396; `occupied_field_bits` :418. Decisions: slot placements must not overlap semantic field bits (contiguous,
repeated, or fragmented) :232-238, must not overlap prior slots :239-254, slot id/requirement id must not collide :257-265.
Recursion bound `active.len() >= 128` refuses as "recursive" :302. Natural (non-authored) layouts hash name/offset/size bytes
:159-163 and use fingerprint domain `"omega.callback-natural-data-layout.v1"` :198. Cases by shape: only `Record` children with
callback slots; a child that is repeated or fragmented is refused :172-178. Stated limits: header :1-6; magic 128 recursion cap
(invented). Tests :498-538 pin repeated-range edges. Fail-open: `for_parameter` returns empty when the record has no slots :76-78
(stated by function name). `OccupiedBits::overlaps` :408 returns `false` for zero-size/zero-count (an empty field never
conflicts: correct).

### P1/src/calling_policy_plans/calling_program.rs (239 lines)

`CallingProgram` :28 (Arc artifact, equality by `Arc::ptr_eq` :32-36). `evaluate` :41-112 executes an authored calling policy
under fixed limits `CALLING_EVALUATION_WORK = 100_000` :16 and sponsor limits `(100_000,1,1,1,1_000_000 x4)` :54-63, admission
check `require_invocation` :81, materialize storage, run policy. `materialize_default` :114 requires the ABI row to be
`default_eligible` (:124-128) and a valid physical owner :129. `with_storage_extents` :155 completes scalar applications for
the ABI's storage demand (policy scope vs description scope) and converts completed values to `u64` (signed negative refused
:222-223; missing or non-integer refused :224). Keyed on `CallingSignatureId`, target profile. Stated limit: header :4-5
"finite 100,000-unit semantic work allowance"; invented constants (limits). Fail-open: none seen.

### P1/src/calling_policy_plans/execution.rs (185 lines)

`evaluate_terminal_calling_policy` :17: runs a calling policy in the Terminal interpreter with compile-time float evaluation,
no host imports :49-59, requires zero missing providers `requirements.missing_providers` :64-79, completes demanded scalars,
requires exactly one structural argument and structural result :120-130, charges result custody, decodes result, validates it
against the signature :180. Keyed on entry machine id; failure of any step returns `String` errors. Fail-open: none seen. Result
statuses other than `Structural` / `ScalarCase` are errors :160-168.

### P1/src/calling_policy_plans/opaque_representations.rs (29), plan_computation.rs (92), values.rs (23), values/tests.rs (144)

- opaque_representations.rs: `BoundaryOpaqueRepresentationPathElement` :6 and `BoundaryOpaqueRepresentationMovement` :12; accessor
  methods exist only under `#[cfg(test)]` :20-29 (comment :18-19: "no production caller reads them back").
- plan_computation.rs: `evaluate_default_calling_applications` :8-92 builds default ABI plans: classify parameters via
  `BoundaryValueGraph::classify`, evaluate `evaluate_ordinary_boundary_entry_plan`, append callback materializations for
  `PrivateCallback` origins (comment :51-54), validate, check opaque movement for each opaque use, build native application. Keyed on
  policy enum `CallingPolicy`.
- values.rs: thin projection wrappers `signature_contents` :6 and `plan_value` :14 delegating to `build_value_projection`.
- values/tests.rs: round-trip of a one-field record through the interpreted policy (test :114-143).
Users: grepped `decode_boundary_plan_result|evaluate_default_calling_applications|component_calling_application|native_calling_results`:
13 hits in 8 files (all within P1 except package_review). Fail-open: none seen.

### P1/src/calling_policy_plans/policy_signature.rs (642 lines)

1. Purpose. Encode the canonical boundary signature (shapes, fields, parameters, native parameters, callback binders and demands)
   into FIXED-CAPACITY `BuildTimeValue` arrays for the authored policy, and validate the policy's returned plan against that
   geometry.
2. Shape. `CallingPolicySignature` :21; `encode_calling_policy_signature` :63-298; `validate_calling_policy_result` :379.
4. How it decides. Capacity check :69-106 refuses counts above capacity ("calling policies currently support at most
   {capacity}"). `validate_calling_policy_result` :379: if the result is `Rejected`, validates as rejected :384-387; for `Accepted`:
   parameter count equals native formal count :388-394, result presence equals signature :395-399, each parameter's shape is
   validated with `validate_abi_shape` (Semantic root) or must equal the integer pointer shape (TargetFunctionPointer) :407-430;
   if there are no callback binders AND no demands uses plain validation :441-444, else the callback-materialization validator
   :445-447. Keyed on shape roots, ordinals, requirement ids.
5. Cases by shape. Fixed arrays padded with zero rows up to capacity (`VALUE_SHAPE_CAPACITY` etc.) :107-263, with counts alongside.
   Stated limit: "calling policies currently support at most {capacity}" :103. Source spelling: case/type names such as
   `"BoundarySignature"`, `"ValueShape"`, `"NativeParameterIdentity"`, `"CallbackBinderIdentity"` :150-266 (string type names;
   comment :60-62 says case labels are "unqualified").
9. Fail-open. `encode_calling_policy_signature` writes `i64` casts of `u64` ids (`as i64`) :155-206, wrapping for ids above
   `i64::MAX` (decoder reads them back with `as u64`, so round trips; not a fail-open risk). None seen.

### P1/src/calling_policy_plans/selected_application.rs (203 lines)

`component_calling_application` :18: returns an already-settled application matching `(abi key, target)`; competing duplicates
refused :31-33; refuses if the declaring application has a selected custom calling-policy instance (`"lost its selected custom
calling application"` :45); else requires `default_abi_eligible` :48 and computes a default plan. `native_calling_results` :72
reuses retained default plans by `(boundary, receipt digest)` :97-105 only if target and artifact match :80-84, otherwise
recomputes. Policy chosen by binding kind: `Import` -> native target policy, `Syscall` -> native syscall policy :158-168.
`default_abi_eligible` :5: `default_eligible` AND non-callback formals match operands by ordinal and kind. Keyed on ABI key struct
and receipt digests. Same job elsewhere: `evaluate_default_calling_applications` called here at :51, :169. Fail-open: none seen.

### P1/src/calling_policy_plans/terminal_signature.rs (354 lines)

`CanonicalCallingSignature` :22 with `materialize` :111-336: from the declared telescope builds value layouts per parameter,
collects callback layouts, handles external-completion results, and drops the physical result for a PROVED-EMPTY result domain
(`inhabitation.status(result) != Status::ProvedEmpty` :226). `native_application` :38 builds `BoundaryCallingApplication` and
computes commitment and report fingerprint (domain `"omega.boundary-plan-application-report.v3"` :104-107). Decisions: requires
telescope/storage parameter counts equal :125, `validate_fresh_native_parameter_identity` :170, requirement identity nonempty :239,
collision check of compact callback requirement report ids :276 (`validate_callback_requirement_report_collision` :339). Stated
text :5-7 in header. Fail-open: a result whose inhabitation is unknown (not ProvedEmpty) keeps its result slot :226 (comment
:216-220: "an unknown domain ... still take their ordinary layout path"); stated.

---------------------------------------------------------------------------------------------------

## Coverage

All 66 ranges in `units/unit_006.tsv` were read in full: 21,906 lines total.
- Read in full: every file listed in the unit, lines 1..N each (SR 18 files = 11,355 lines including `tests/expression_clone.rs`; SP
  2 files = 278; WB 1 = 137; DRV 17 files; P0 6 files; P1 22 files).
- Skimmed: none.
- Not read: none.
Not read (outside the unit, so only grepped): the other `encode_scalar_type`/`encode_integer_type` definitions named in
`identity_encoding.rs` notes; consumers of `declaration_authority_request` (`proof_scope.rs`); production sites of thread creation.

## Five things a maintainer most needs to know about this unit

1. `declaration_authority_request` (SR/src/lib.rs:865-887) returns `Ok(None)` for any custody pair that contains `Toolchain`,
   including `(UnownedUser, Toolchain)`: the `UnownedUser` refusal arm (:883) is only reached when neither side is `Toolchain`.
   Its two callers are in `source-to-psi/.../05_.../proof_scope.rs` (not read).
2. The scope derivation (P1/execution-scope/scope.rs:315-392) marks a boundary as reached and returns `Ok(())` when it has no
   provider candidates (:348-350) or none with a selected adapter row (:351-358). Refusal of such boundaries, if any, happens in
   later stages (`provider_bindings.rs:100-154` for direct leaves); this function itself never refuses them.
3. Identity encodings of scalar/integer types are written many times: the shared `semantic_vocabulary::identity_encoding` has
   3 grepped users in native crates (`ownership_frontier.rs`, `runtime-abi encoding.rs`, `optimization-unit` re-exports), while at
   least 6 other `encode_scalar_type`/`encode_integer_type` functions exist (optimization-unit identity.rs:113/122 and
   rewrite.rs:186, selected-instructions plan_identity.rs:711, target-vocabulary identity.rs:174/195, declaration-encoding
   scalar_types.rs:9/41). The comment at identity_encoding.rs:1-3 claims both unit and plan identity bind these bytes.
4. Authored calling policies are decoded and encoded by string case names and field names with invented fixed capacities
   (P1/calling_policy_plans/build_time_decoding.rs:16-29,:498-521; policy_signature.rs:69-106), counts ignore cells past `count`
   (:485), and `Int` values are reinterpreted as `u64` bits (:523-528). Validation (`validate_calling_policy_result`) runs on the
   decoded plan, not on the raw value.
5. The native driver is a staged pipeline with the SAME body functions for root, private callbacks and component exports
   (`compile_body.rs`); lockstep staging is enforced by `BodyRoster::try_map` (object_emission.rs:30-43). The callback path
   discards the `CallableAdmission` (runtime provisions and authority) returned by `plan_callable`
   (plan_callables.rs:378), while the component path keeps them (components.rs:241-249).

## What I am unsure about

- Whether `PackageSourceConsumptionCommitment::from_retained_digest` (SP/source_provenance.rs:55) is only called from decoders;
  I did not grep its call sites.
- Whether `CallingSignatureId` (64-bit FNV, identity.rs:310-336) is checked for collisions anywhere; only callback requirement
  ids have a visible guard (terminal_signature.rs:339).
- What a `None` hosted receiver permits in `plan_hosted_program_entry` (object_emission.rs:113-120); I did not read
  `entry_planning::hosted_program`.
- Whether later stages refuse a boundary that `retain_boundary` marks as reached but with no candidate (scope.rs:348-358).
- Whether `ScalarTerm::Application` (scalar_terms.rs:72) ids are validated against the module anywhere before use.
- Whether `Proposition::StructuralCaseMembership` (proposition.rs:2177) validity of case-vs-place type is checked elsewhere.
- I did not read or count `ProductTarget` call sites (the grep pattern was too noisy), so I cannot say how widely its numeric
  `code()` is persisted.
- Test content in `proposition.rs` (about 1,700 lines of tests, :3133-4866) was read but summarized as one group; individual test
  names are not catalogued.
