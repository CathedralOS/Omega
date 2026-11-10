# Unit 109 notes

Files covered (all under `omega-rust/psi/foundation/`, 20,308 lines, every line read in full):

1. numerics/src/lib.rs (1187)
2. numerics/src/literals.rs (921)
3. optimization/src/lib.rs (437)
4. semantic-vocabulary/src/identity.rs (421)
5. semantic-vocabulary/src/lib.rs (1382)
6. semantic-vocabulary/src/proof_applications.rs (1184)
7. semantic-vocabulary/src/proposition.rs (4460)
8. semantic-vocabulary/src/scalar_terms.rs (1754)
9. semantic-vocabulary/src/storage.rs (720)
10. source-audit/src/lib.rs (107)
11. source/src/capture.rs (819)
12. source/src/lib.rs (1569)
13. symbols/src/builtin.rs (1130)
14. symbols/src/fast_map.rs (89)
15. symbols/src/lib.rs (59)
16. symbols/src/retained_declaration.rs (278)
17. symbols/src/symbol.rs (333)
18. symbols/src/table.rs (3458)

Paths below are relative to `omega-rust/psi/foundation/<crate>/src/` unless full. Counts quoted as "grepped" were made with the Grep tool in count mode (`output_mode=count`, `glob=*.rs`, path `omega-rust`); they count matching lines, so a hit inside the defining file (often its own tests) is included and I say so. A pattern with several alternatives counts all of them together.

Not in my unit but referenced: numerics `bignum.rs` and `float_semantics.rs` (declared at numerics/lib.rs:32-33), symbols `tests.rs` (declared symbols/lib.rs:58-59). I did not read them.

---------------------------------------------------------------------

## 1. numerics/src/lib.rs

**Purpose.** Root of the `numerics` crate. Holds six inline modules of exact integer and predicate laws shared by checking, proof and lowering: the arithmetic-domain enum, an integer-interval recipe evaluator, a table of integer policy obligations, a scalar predicate evaluator, f32 NaN-payload conversion, and the "Float.meaning32/64" projection catalog. Declares `bignum`, `float_semantics`, `literals` as file modules (32-34) and re-exports `float_semantics` under a second name (36).

**Shape.**
- `arithmetic::ArithmeticDomain {Exact, Wrapping, Saturating, Trapping}` (44), `from_name` (52), `name` (61), `combine` (70). `ArithmeticPolicyAdapter {None, FloatSaturatingOverflowOnly{format}, FloatTrappingNonFinite{format}}` (79), `float_format` (91).
- `integer_bounds::IntegerBoundStep<Observation>` (105): `Missing, Bounds{low,high}, Observation(o), Add, Subtract, Multiply, Union, Interval, Intersect`; `map_observation` (122); `evaluate` (145) = postfix stack machine returning `Result<(BigInt,BigInt), String>`.
- `integer_policy`: five small enums (`IntegerPolicyPrimitive` 7 members, `IntegerResultLaw` 3, `IntegerFormationCondition` 3, `IntegerTrapPredicate` 4, `ShiftCountLaw` 4), `IntegerPolicyBridge` (361), `integer_policy_bridge(primitive, policy)` (392) is a `const fn` of three independent `match (primitive, policy)` blocks producing result law, formation conditions, trap predicates and shift-count law.
- `scalar_predicates`: `ScalarValue {Unknown, Integer(BigInt), Boolean}` (558), `PredicateOperator` (572), `PredicateNode<Reference, Leaf> {Missing, Value, Leaf, Forward, Not, Binary}` (598), `value_kind` (613), `try_map` (654), `evaluate_nodes` (678) = explicit-stack evaluator with a cycle guard.
- `float_representation::binary32_from_bits` (950), `binary32_to_bits` (960): move an f32 to/from an f64 while keeping NaN sign and payload.
- `float_projection`: `FloatProjectionOperation {Meaning32, Meaning64}` (996), `FloatProjectionContractIdentity` (987: format, operation, declaration tag, catalog_version, 32-byte commitment), `FloatProjectionRule` (1002), `contract_identity` (1011), `from_source_identity` (1058), `rule` (1066), `project_f32/f64` (1079/1083).

**Inputs/outputs.** Pure functions over their arguments. `float_projection` calls `FloatMeaning::from_f32/from_f64` and `FloatFormat::BINARY32/64` from `float_semantics` (not read). `contract_identity` hashes with `crate::digest_encoder::DomainDigestEncoder` (line 30: `pub(crate) use optimization::digest_encoder`).

**How it decides.**
- `integer_bounds::evaluate`: keyed on the recipe's step kind and stack position. Refuses on `Missing` (152), inverted `Bounds` (154), inverted `Interval` (178), empty `Intersect` (186), stack underflow (164-169), and a final stack that is not exactly one value (218). Error values are `String`.
- `integer_policy_bridge`: keyed on the pair (primitive, policy), not on any type. Exact `Add/Sub/Mul` -> ResultRepresentable; Exact `Div/Rem` -> NonZeroDivisor+ResultRepresentable; Wrapping/Saturating `Div/Rem` -> NonZeroDivisor; Exact `ShiftLeft` -> shift-count+representable; Saturating `ShiftLeft`, and Exact/Saturating `ShiftRight` -> shift-count only (401-420). Trapping adds trap predicates (421-437). Wrapping shift = `EuclideanModuloWidth`, Trapping = `TrapsWhenOutOfRange`, Exact/Saturating = `MustBeWithinWidth` (438-452).
- `evaluate_nodes`: keyed on node reference identity (`active.contains`, 697) for cycles; returns `None` for Missing node, unresolved leaf, `Not` of a non-boolean, mismatched operand kinds, ordering operator on booleans, and when the caller closure `comparison_is_admitted` says no (743-747). `And`/`Or` short-circuit on the left value before the right is entered and before `comparison_is_admitted` is consulted (732-734, 743).
- `ArithmeticDomain::combine` (70): `(Exact, d) | (d, Exact) => d`, otherwise the left operand wins; there is no conflict signal.
- `FloatProjectionOperation::from_source_identity` (1058-1063): string compare on namespace `"Float"` and name `"meaning32"` / `"meaning64"`.
- `contract_identity` (1011-1045) hashes literal strings, including `"private-contract-free-ordinary-tokenless-one-parameter"` (1031), source file names `"float_operations.omg"`/`"float_meaning.omg"` (978-979) and `"toolchain::FloatMeaning"` (981). The commitment therefore depends on those spellings.

**Cases by shape.**
- Exactly two projection operations (Meaning32, Meaning64) (996-999); `from_source_identity` returns `None` otherwise ("source identities are exact and closed", test 1117).
- `integer_policy_bridge` covers 7 primitives x 4 domains through wildcard arms: `_ => &[]` (419, 436) and `_ => ShiftCountLaw::NotApplicable` (451).
- `binary32_to_bits` forces a quiet NaN when the shifted payload is zero (968-970).

**Same job elsewhere.**
- Integer vocabularies: `IntegerType`(semantic-vocabulary/proposition.rs:244) carries sign+bits+carrier with its own wrapping/saturating/exact operations (e.g. `wrapping_add` 662, `saturating_add` 942); `ArithmeticDomain` here names the same four policies at the enum level. Both are in my unit; they are separate types with no conversion between them inside the unit.
- Interval arithmetic: `integer_bounds::evaluate` (corners for `Multiply` 191-209) vs `IntegerType::exact_shift_bounds` (proposition.rs:859) and `bitwise_*_bounds` (proposition.rs:22-62): three different interval evaluators in one workspace area, one per operation family.
- Predicate evaluation: `scalar_predicates::evaluate_nodes` (678) vs `ScalarTerm::boolean_value` / `integer_value` (scalar_terms.rs:630, 385): two independent evaluators of closed boolean/integer terms over different node types.

**Unused or one-caller.**
- Header (lines 1-8) says the crate is a "dependency-free mathematical substrate", but line 30 imports `optimization::digest_encoder`, which wraps `sha2` (optimization/lib.rs:115). Read.
- Header (20-26) says these modules are "shared ... without duplicating interval or predicate evaluators". Grepped `integer_bounds::evaluate|evaluate_nodes\(`: outside numerics, `integer_bounds::evaluate` has 8 call sites (validation/terminal-installation/scalar_applications.rs:1148, 2633; checked-trees/contract_and_obligation_facts.rs:621, 791, 869, 878, 897, 911); `evaluate_nodes` has 4 (terminal-psi/terminal_module.rs:480; checked-trees/contract_and_obligation_facts.rs:182, 371; checked-trees/facts.rs:292). So both have outside users.
- `integer_policy_bridge|IntegerPolicyBridge`: 43 lines in 4 files; 21 outside numerics (validation/arithmetic_domains.rs 15, 06 operation_emission.rs 2, terminal-psi semantics/scalars.rs 4).
- `ArithmeticPolicyAdapter`: 62 lines in 13 files, 3 in numerics -> 59 outside.
- `FloatProjectionOperation|float_projection::`: 82 lines in 13 files; 20 in numerics -> 62 outside, mostly validation/float_and_mathematical_contracts.rs (24) and 05 published.rs (15).
- `PredicateNode::value_kind`: grepped `value_kind\(` under omega-rust/psi/compilation: 11 lines in 4 files. The name is generic, so which are this method is suspected, not checked.

**Stated limits.** None in comments. Tests pin catalog behavior ("shift_count_policy_matches_the_settled_catalog", 515).

**Fail-open risks.**
- `integer_policy_bridge` wildcard arms (419, 436, 451): an unlisted (primitive, policy) pair yields an empty obligation list instead of a compile error. Today the empty cases are Wrapping/Saturating/Trapping `Add/Sub/Mul`, Trapping `Div/Rem` formation, Wrapping/Trapping shifts, which the tests (470-552) pin partly. A new `IntegerPolicyPrimitive` would silently get no conditions and no traps. Status: open (no comment at the site).
- `ArithmeticDomain::from_name` returns `None` for "Exact" (52-59): a source can never name Exact. Not an accept path.
- `ArithmeticDomain::combine` left-wins (70-75): two conflicting non-Exact domains combine to the left one with no error. Status: open (no caller traced).
- `evaluate_nodes`: none seen; every unknown returns `None`. The only permissive part is the short-circuit that skips `comparison_is_admitted` for `And`/`Or` (743). Tested (816, "without observing skipped leaves").

---------------------------------------------------------------------

## 2. numerics/src/literals.rs

**Purpose.** Canonical carriers for integer and float literals before and after "landing" on a concrete type. Integers keep spelling (radix, sign) and keep a `u64` fast path; floats keep exact decimal text (so one later rounding happens) or an exact presented bit pattern.

**Shape.**
- `LandedIntegerType {I8..I64,U8..U64,Addr}` (14): `from_name` (27), `bounds` -> `(i128,i128)` (42), `bit_width` (55), `is_signed` (64), `name` (68).
- `IntegerLanding {landed_type, domain: ArithmeticDomain}` (84).
- `FloatFormat {F32, F64}` (90): `widen_representation` (103), `capture_wide_representation` (112).
- `FloatLiteral {text: Arc<str>, landing: Option<FloatFormat>, presented: Option<(FloatFormat,u64)>}` (125). Methods `parse` (150), `from_f64` (165), `from_bits` (173), `value_f64` (190), `value_f32` (204), `landed_f64` (227), `negated` (251), `with_landing` (239).
- `IntegerLiteral {representation: IntegerRepresentation}` (322); `IntegerRepresentation::{Small{magnitude u64, radix, negative, landing}, Large{text: Arc<String>, landing}}` (327). Methods `from_parts` (412), `from_value` (472), `negated` (510), `value_i64/u64/bignum` (537/553/564), `bits_u64` (593), `decimal_value_i128/bignum` (599/616), `canonical_spelling` (629, inline 67-byte buffer).
- `IntegerRadix {Binary, Octal, Decimal, Hexadecimal}` (294).

**Inputs/outputs.** Reads `crate::bignum::{BigInt, ExactFloat}` and `crate::float_semantics::{FloatSemantics, FloatFormat::BINARY32/64}` (not read), `float_representation` (this crate). Produces literal values; no mutation.

**How it decides.**
- Equality ignores landing for both kinds: `IntegerLiteral::eq` (383-407) compares Small to Small on (magnitude, radix, negative) and Large to Large on text; Small vs Large is `false` (404). `FloatLiteral::eq` (131-135) compares text and `presented`, not `landing`.
- `from_parts` (412-470): a digit that is not valid for the radix returns `"invalid integer literal"`; underscores skipped; overflow of the running `u64` switches to the `Large` text form; `negative` is dropped when the magnitude is 0 (441).
- `negated` (510-535): Small keeps a negative sign on a non-decimal zero (`!(magnitude==0 && radix==Decimal) && !negative`, 522) while `from_parts(true, Hex, "0")` drops it (441). Test 904 pins this.
- `strip_float_literal_suffix` (283-291): "f32" / "f64" suffix sets landing; otherwise `trim_end_matches(['f','F'])` removes every trailing f/F character.
- `FloatLiteral::parse` validates through `ExactFloat::from_decimal_str` (157) but later reads values through `FloatSemantics::from_decimal` (196, 211), a different function.

**Cases by shape.**
- Carrier names: exactly nine, matched by string in `LandedIntegerType::from_name` (27-40) ("i8".."u64","addr"); anything else is `None` (tested 846: "bool","f32","address","usize","U8","").
- `canonical_spelling` buffer `[u8; 67]` (346) fits 64 binary digits plus "0b" plus "-".
- `FloatLiteral::from_bits` accepts F32 bits only when `u32::try_from(bits)` works (106).

**Same job elsewhere.**
- A second integer carrier vocabulary: `IntegerType` (semantic-vocabulary/proposition.rs:244, sign+bits 1..=128, plus Address carrier) and `BuiltinTypeAtom` fixed integers (symbols/builtin.rs:17, `is_fixed_integer` 115). `LandedIntegerType::bounds` (42) and `IntegerType::minimum_value/maximum_value` (proposition.rs:408/416) compute the same limits in different types. Conversions between them are outside this unit (suspected).
- Two or three float format enums: this `FloatFormat {F32,F64}` (literals.rs:90), `IeeeFloatFormat {Binary32,Binary64}` (scalar_terms.rs:738), and `float_semantics::FloatFormat::BINARY32/BINARY64` (used at literals.rs:197, 212; file not read). `FloatFormat::widen_representation` (literals.rs:103) and `float_representation::binary32_from_bits` (numerics/lib.rs:950) are the f32<->f64 bit bridge.

**Unused or one-caller.**
- `LandedIntegerType`: 231 lines in 28 files, 20 in literals.rs -> 211 outside.
- `FloatLiteral::new|from_f64` (`\bFloatLiteral::(new|from_f64)\(`): 20 lines in 8 files, all outside numerics (obligations.rs 12, others 1-2).

**Stated limits.** None.

**Fail-open risks.**
- `value_f64` (190-202) and `value_f32` (204-217): `FloatSemantics::from_decimal(...).map(...).unwrap_or(0.0)`. If decimal conversion fails, the literal reads as +0.0. Reachable only if `from_decimal` rejects text that `ExactFloat::from_decimal_str` accepted (parse path 157) or text from `from_f64`'s `{value:?}` (166). I did not read either function. Status: open.
- `strip_float_literal_suffix` (290) strips all trailing f/F so `"1.0fF"` is accepted as `1.0`. The remainder is still validated by `parse` (157), so this admits a spelling, not a wrong value.
- `FloatLiteral::negated` for non-presented text (260-269): prefixes/strips '-' on text without checking it, so `"-NaN"` forms rely on the parser. Not traced.

---------------------------------------------------------------------

## 3. optimization/src/lib.rs

**Purpose.** Psi-side home for the optimization-selection vocabulary plus four small shared codec helpers: a bounded byte `Cursor`, `push_encoded`, a SHA-256 `DomainDigestEncoder`, and the `structural_variant_accessor!` macro.

**Shape.** `codec_bytes::push_encoded` (42); `cursor::{Truncated (49), TakeError (52), Cursor (57: take/array/byte/u16/u64), truncated_from_adapter! (96)}`; `digest_encoder::DomainDigestEncoder(Sha256)` (117: new, finish, byte, len, u16, u64, raw, bytes); `variant_accessors::structural_variant_accessor!` (159); `optimization_selections` (171): `PsiOptimization` 9 variants tags 1..=9 declared through `psi_optimization_vocabulary!` (178-216), `PsiOptimizationSelections` (219: new sorts and rejects duplicates, encode, decode, identity), `PsiOptimizationSelectionIdentity` (314), `DuplicatePsiOptimization` (327), `PsiOptimizationSelectionDecodeError` (338, 7 cases).

**Inputs/outputs.** `encode` writes `PSIOPT\0\0` + u32 version 1 + u32 count + one tag byte per selection (249-258). `identity` hashes the encoding under domain `"psi.optimization-selections.v1\0"` (176, 305-310). `decode` (260-303).

**How it decides.** `decode` is strict and keyed on position and value: length < 16 -> Truncated, wrong magic, version != 1, declared count vs actual length (Truncated / TrailingBytes), unknown tag, equal neighbors -> Duplicate, descending -> NonCanonicalOrder. `from_tag` (195) searches `ALL`.

**Cases by shape.** Closed list of exactly 9 optimizations (`ALL: [Self; 9]`, 187; comment at 6 says "closed 9-member"). The `9` is a literal argument to the macro (206), so the count is hand-maintained.

**Same job elsewhere (grepped definitions).** `omega-rust/omega/foundation/optimization-core/src/` has its own `cursor.rs` (`Truncated` line 9, `Cursor` line 17), `codec_bytes.rs` (`push_encoded` line 8), `digest_encoder.rs` (`DomainDigestEncoder` line 11) and `variant_accessors.rs` (`macro_rules! structural_variant_accessor` line 6). Header at 26-28 says hosting these here "eliminates crate-local copies ... across Psi identity and artifact codecs"; the Omega copies are suspected duplicates (I did not read them). Other `Cursor` structs: selected-instructions/src/effects/program/encoding.rs:9 and selected-instructions/analyses/src/lib.rs:571 (grepped `pub struct Cursor`).

**Unused or one-caller (grepped).**
- `structural_variant_accessor!`: 7 lines in 7 files; the Psi macro's outside users are terminal-psi types_and_values.rs:1 and control_flow.rs:1; the other four (omega abstract-operations values.rs, calls.rs, machine-code boundary.rs, and the Omega macro definition) may use the Omega copy (suspected).
- `truncated_from_adapter!`: 16 lines in 15 files; Psi users seen: terminal-psi/codec sections.rs:1, canonical_artifact.rs:2; the rest are in omega crates and may call either crate's macro (not traced).
- `PsiOptimizationSelections`: 41 lines in 8 files, 17 here -> 24 outside (psi-compiler lib.rs 5, terminal-production 2, 07 psi_optimization.rs 3, terminal-psi codec sections.rs 6, omega abstract-operations rules.rs 2, pass_manager.rs 3, optimization-core 3).

**Stated limits.** None beyond the 9-member closure.

**Fail-open risks.** None seen. `decode` rejects every malformed case, `new` rejects duplicates.

---------------------------------------------------------------------

## 4. semantic-vocabulary/src/identity.rs

**Purpose.** Typed identifiers for the terminal Psi vocabulary, plus a few identity enums used in wire and review formats.

**Shape.**
- `semantic_id!` macro (118) makes `struct $name(NonZeroU64)` with `new(raw)` (None on 0), `get`, `PsiSemanticId` impl, `Display`. 29 invocations (grepped `^semantic_id!\(` in identity.rs: 29): ValueId, ScalarApplicationId, ScalarDomainId, StructuralTypeId, StructuralFieldId, StructuralCaseId, StructuralDomainId, DomainSemanticId, ServiceId, BoundaryMachineId, MachineId, CallingSignatureId, BlockId, PlaceId, ClaimId, ContentDomainId, OperationId, SuspensionCrossingId, EdgeId, PropositionId, EvidenceTermId, ContractId, ObligationId, RecursiveComponentId, CycleComponentId, RankingRelationId, AdmissionSiteId, EvidenceIdentity, ProfileDecisionId.
- Not macro-made: `ScalarQualificationSetId(u64)` with `ZERO` (170), `WorkScheduleIdentity(NonZeroU32)` (98), `PackageKeyIdentity([u8;32])` (85, rejects the all-zero digest, 88), `InvocationSite {Operation, ExternalCompletion, InvokeNoNormal}` (25, tag+u64 codec 32-49), `BoundaryRequirementKind` (3 members), `ExternalCompletionKind {ProcessExit}`, `BoundaryCompletion`.
- Calling ABI: `CallingOperandKind` (6 kinds, 226), `CallingStaticOperand` (236), `CallingAbiIdentity {requirement_identity, declaring_application, operands}` (243: `id`, `validate`), `CallingGenericFormal` (278), `CallingSignatureId::for_application` (285).

**How it decides.** `CallingSignatureId::for_application` (285-311) is a 64-bit FNV-1a over `"omega.declaration-abi-application.v1"`, the requirement string, the declaring-application string, and each operand's (ordinal, kind tag, identity string); a zero hash becomes `NonZeroU64::MIN` (307-310). So that id is derived from source-spelled strings, not allocated densely. `validate` (260) rejects empty strings and non-increasing ordinals.

**Cases by shape.** `from_tagged_identity` (42-49): tags 0,1,2 only. `for_application` kind tags 0..5 (296-303).

**Same job elsewhere.** `callback_requirement_id`, `nominal_callback_binder_id`, `nominal_callback_native_parameter_id` (semantic-vocabulary/lib.rs:194-223) are the same "FNV over canonical strings, zero becomes nonzero" pattern, in a different ID family (they use `0 -> 1`, lib.rs:188-191; this one uses `NonZeroU64::MIN`).

**Unused or one-caller (grepped).**
- Header (lines 1-6) calls the ids "dense"; `CallingSignatureId` is a hash, and `IdTable::from_rows` requires dense runs (lib.rs:1054), so a `CallingSignatureId` table could only be built with `from_rows_with_gaps`. Read.
- `PsiSemanticId`: 29 lines, 7 files; 6 here, 23 outside (terminal-psi codec wire_fields.rs 11, omega optimization-unit identity.rs 6, arrival_helpers.rs 3, control_graph.rs 2, legalized-operations tests.rs 1).
- `WorkScheduleIdentity|InvocationSite`: 618 lines in 133 files (heavily shared).
- `CallingAbiIdentity|CallingSignatureId::for_application`: 73 lines in 33 files, 4 here.

**Stated limits.** None.

**Fail-open risks.** None seen. `PackageKeyIdentity::from_digest` and `Record::read` both refuse the zero digest (88, 389-391).

---------------------------------------------------------------------

## 5. semantic-vocabulary/src/lib.rs

**Purpose.** Crate root. Mixes many small inline modules and re-exports. Contents: crash/authority vocabulary, callback and boundary identities, native-binding locators, program-entry catalog trait, bounded integer type, scalar refinement, byte-sequence predicates, build-time-evaluation floor checks, FNV-1a, dense id tables.

**Shape.**
- `behavior_policy`: `CrashCause {Trap, Abort}` (33); `tagged_vocabulary!` macro (38); `TerminalAuthorityClass` 14 members tags 0..=13 with manual `Record` (80-96) that rejects unknown tags (94).
- `boundary::callback_identities` (142): macro `nominal_declaration_id!` -> `StaticMachineBinderId`, `NativeParameterId`, `CallbackRequirementId` (`u64` newtypes, zero rejected); `BoundaryNativeParameterOrigin` (173); `callback_nominal_report_fingerprint` (183); `callback_requirement_id` (194); `nominal_callback_native_parameter_id` (201); `nominal_callback_binder_id` (214).
- `boundary::foreign_locator` (226): `ForeignLocatorCandidate {PeByName, PeByOrdinal, ElfVersioned, MachODylibSymbol}` with `case_name` (249); `NativeBindingLocator {Import, Syscall{number}}` (260).
- `boundary::program_entry` (266): `ProgramEntrySchema {HostedApplication, ProgramStorageApplication}`, `ProgramEntryVisibleParameters {None, ImageAndInitialStorage}`, `ProgramEntrySourceContract` (owner, slot_name, schema, semantic_arrival_requirement, visible_parameters), trait `ProgramEntrySourceCatalog::source_contract(slot) -> Result<_, String>` (288).
- `scalars::bounded_integer_type::BoundedIntegerType::new` (314): requires Fixed carrier, both bounds admitted, min <= max. `ieee_float_comparison_operation` (6 ops, `ordinal`). `qualified_scalar_type::QualifiedScalarType`. `scalar_refinement::ScalarRefinement` (449): `SUBJECT = ValueId::FIRST`, `new` (463), `instantiate` (492).
- `byte_predicates::{ByteSequencePredicate {ValidUtf8, NoNul, AsciiOnly, NonEmpty}, ByteSequencePredicateSet(u8)}` (611-761).
- `compiler_evaluation` (851): trait `DeclarationSelectionAuthority` (854), `DeclarationCustody`, `CompilerEvaluationEffects {services, may_suspend, may_block}`, `CompilerEvaluationDeclaration`, `DeclarationAuthorityRequest`, `require_compiler_evaluation_floor` (904), `require_compiler_evaluation_failures` (931).
- `fnv1a::Fnv1a` (940): byte/flag/u16/u32/u64/`u64_mix`/framed and terminated variants, `finish_or`.
- `id_table` (1027): `IdTable<Id,T>` (first, Vec values) with `from_rows` (dense, 1054), `from_rows_with_gaps` (1081, span cap), `get/get_mut/keys/iter/map`; `IdSet<Id>` bitset (1206).
- `mod identity; mod proposition; mod storage; pub mod proof_applications` and the re-export list (1334-1382). `scalar_terms` is included from `proposition.rs` via `#[path]` (proposition.rs:2386).

**How it decides.**
- `ScalarRefinement::new` (463-482): a predicate is accepted only if `visit_value_ids_bounded` with budget 4096 returns `Some(true)` (fully closed, within budget) and every value id equals `SUBJECT`, then `PropositionContext::validate`. Opaque forms (`Atom`, structural reads, structural field terms) make `complete=false`, so they are rejected as `NotClosed` (tested 567).
- `ByteSequencePredicateSet::concatenation` (740): `NonEmpty` holds if either side implies it; the others need both sides. `subslice` keeps only NoNul and AsciiOnly (`is_subslice_preserving` 655). `implies` (662): equal, or AsciiOnly -> ValidUtf8.
- `require_compiler_evaluation_floor` (904-929): collects violations from non-empty `effects.services`, `may_suspend`, `may_block`, plus two caller-supplied optional strings; error names the machine.
- `IdTable::position` (1144): offset from `first` and bounds-check against `values.len()`.

**Cases by shape.**
- `ForeignLocatorCandidate` closed list of four loader-name shapes (PE by name, PE by ordinal, ELF symbol+version, Mach-O install name+symbol) (228-246); `NativeBindingLocator` Import or Syscall (260).
- `ProgramEntrySchema` exactly two (268-271).
- `AtomicCellType`-style limits live in storage.rs, not here.
- `ByteSequencePredicateSet::from_bits` rejects bits above 0x0f (691).

**Same job elsewhere.**
- `Fnv1a`: grepped `struct Fnv1a|pub struct Fnv`: other copies at `omega/compilation/backend/target/src/compatibility_fingerprint.rs:9` and `omega/compilation/backend/calling-conventions/src/plans/plan_identity.rs:18` (suspected duplicates, not read).
- `callback_nominal_report_fingerprint` vs `CallingSignatureId::for_application` (identity.rs:285): two FNV id derivations, different framing strings.
- `IdTable` vs `Arena`/`HandleSpan` (used in proof_applications.rs, storage.rs): two dense-storage styles in one crate.

**Unused or one-caller (grepped).**
- `IdSet`: 32 lines in 12 files; 7 here; outside: emission.rs 1, terminal-psi semantics descriptors.rs 4, 8 omega provider-admission windows_handle_* files and others (total 25 outside).
- `IdTable`: 125 lines / 17 files; 30 here -> 95 outside.
- `from_rows_with_gaps`: 12 lines in 3 files; 7 here (def + tests), 5 outside (emission.rs 1, descriptors.rs 4).
- `ProgramEntrySourceCatalog|ProgramEntrySourceContract`: 14 lines / 6 files; 4 here.
- `callback_*` / `nominal_callback_*`: 76 lines / 20 files; 9 here.
- `DeclarationSelectionAuthority|require_compiler_evaluation_*|CompilerEvaluationDeclaration`: 49 lines / 18 files; 4 here.
- `ScalarConditionRole`, `ScalarRefinement::new`: grepped together (`\.from_rows_with_gaps|ScalarRefinement::new|ScalarConditionRole`, 45 / 13 files); not separated.
- `ByteSequencePredicateSet|ByteSequencePredicate::`: 177 lines / 37 files, 24 here.

**Stated limits.** `TerminalAuthorityClass` doc (59): "additions require a target-policy migration". `ScalarRefinement` budget 4096 (467). No other limit comments.

**Fail-open risks.**
- `DeclarationSelectionAuthority::declaration_selection_scope` default returns `None` (863-868). I did not trace what `None` means to implementers; status: open.
- `require_compiler_evaluation_floor` (904): `closure_violation`/`selection_violation` of `None` means "no violation" and the function returns Ok if the effects are clean. The two options come from callers I did not read; status: open.
- `ByteSequencePredicate::is_concat_preserving` (649) is a `match` listing every variant as `true`, so it never rejects. Not a fail-open for current members; a new member must be added to the arm (no wildcard, so it breaks at compile time).
- `ByteSequencePredicateSet::implies` with an empty `required` returns true (724) by construction (nothing required).

---------------------------------------------------------------------

## 6. semantic-vocabulary/src/proof_applications.rs

**Purpose.** Walks the static call/binding structure of a proof recipe to decide which machines and declarations a target's proof obligations reach, rebasing family-interface machines onto the selected target member. It is generic over a `Declaration` handle type and an operational call source.

**Shape.**
- Data: `ProofApplicationMachine<D>` (10: machine, states, binders, authored_target, source_target, family_interface, family_members, dynamic_realizations, product_selected), `ProofApplicationTargetMember` (23), `ProofDeclarationDependencies` (29), `ProofApplicationPlan<D>` (36: machines, target_members, symbols, declaration_dependencies, declarations, obligation_owners, provider_candidates, `has_obligations`, `unscoped_recipe`).
- Plan methods (49-158): `retain_product_machines`, `callable_for_target`, `declared_callable_for_target`, private `called_machine/matching_machine/target_machine(_matching)`, `declaration_closure`, `declaration_membership`, `declaration_dependencies`, `retain_declaration_signatures`. `try_map` (808).
- `Binding`/`Application` (160-171), `ProofInvocationEdge` (179: source, target, call_ordinal, provider, static_delegation), `ProofApplications<'a,D,Calls>` (187) with lazily built `MachineRows` (193), `ProofApplicationReachability` (199).
- `ProofApplications` methods (205-612): `new`, `rows`, `matching_machine`, `called_machine`, `target_machine`, `scopes` (299), `needs_scope` (314), `declarations`, `required_scopes` (335), `membership` (370), `reachability` (386), `invocation_reachability` (416), `walk_applications` (432).
- Trait `ProofApplicationCalls<D>` (650: `declaration_order`, `visit_calls`, `visit_bindings`, `visit_cleanup`); `ProofCallGraph<D>` (670) implements it with `machines`, `calls`, `bindings`, `cleanup` arenas, an optional sorted `machine_index` (`index_machines` 773, `machine_row` 788); `ProofCallGraph::capture` (909).

**Inputs/outputs.** Reads a plan and an operational call source; returns machine lists, call lists, invocation edges. Plan `try_map` and `ProofCallGraph::capture` rebuild the structures under a handle mapping.

**How it decides.**
- `matching_machine` (73): first machine whose `machine == symbol`, otherwise one whose `states` span contains `symbol`; filtered by a caller `accepts` predicate (`product_selected` for "called", always-true for "declared").
- `target_machine_matching` (97): family interface -> member whose `target` string equals the requested target (108); otherwise the machine itself when `authored_target` is `None` or equals the target (111-115). String compare on target names ("macos_arm64" etc.).
- `reachability` (386): seeds from explicit roots plus every `product_selected` machine whose `source_target` equals the target, or (no roots and `selected_product` and no source_target) (401-411).
- `walk_applications` (432): `visited` per machine order dedups exact `Application` values (bindings included); rebinding of formal binders when a family head differs from its member (457-470, repeated at 585-605); provider edges join when the requirement matches callee, the called machine, state or operator (514-536).
- `needs_scope` (314): true if scope equals primary or `unscoped_recipe`, else any owner among `obligation_owners` is reached by `membership`.
- `machine_row` (788): uses the sorted index only when `indexed_at == (revision, len)`, otherwise a linear scan.

**Cases by shape.** `Application` bindings nest recursively (`resolve_bindings` 614). Provider edges are `(requirement, realization)` pairs (515). The comment-free `declaration_dependencies` closure is a work list with `contains` checks (131-149).

**Same job elsewhere (within this file).**
- `ProofApplicationPlan::{matching_machine 73, called_machine 69, target_machine_matching 97, target_machine 89}` and `ProofApplications::{matching_machine 252, called_machine 271, target_machine 278}` do the same lookup twice; the second uses a sorted index with `partition_point` (258-268), the first scans. The family-member branch is written out in both (103-110, 283-291).
- The family binder rebasing loop is written twice (457-470 and 585-605).
- `ProofApplicationPlan::try_map` (808) and `ProofCallGraph::capture` (909) both rebuild arenas through a mapping closure.

**Unused or one-caller (grepped).** `ProofApplications|ProofApplicationPlan|ProofCallGraph`: 67 lines / 10 files; 27 here -> 40 outside, production users include build-time-evaluation/product.rs (6), terminal-psi terminal_module.rs (5), checked-trees facts.rs (16, mostly tests from line 1533 on), attached_unit.rs 3, published_facts.rs 2, prepared_providers.rs 1. Production call lines seen (grep of selected files): product.rs:1442 `callable_for_target`, 1749 `declaration_closure`, 1848 `reachability`, 1902 `membership`; prepared_providers.rs:401 `scopes`; attached_unit.rs:8099 and published_facts.rs:4662 `index_machines`.
For `needs_scope|required_scopes|invocation_reachability|declared_callable_for_target|retain_product_machines`: 22 lines / 9 files, 8 here (including tests at 998, 1177), 5 in checked-trees facts.rs (test lines 1666-1683 seen). In the 8 files I grepped by name, I saw `needs_scope` and `retain_product_machines` only in facts.rs tests; I did not trace the other 9 lines (the names may belong to other types), so production use of these five is not established.

**Stated limits.** None.

**Fail-open risks.**
- `target_machine_matching` (111-115): a machine with `authored_target == None` is accepted for any target. That is the "portable body" case; no comment. Traced only by tests (1102 "family_lookup_keeps_exact_members_and_missing_targets" shows a non-target member is refused for a different target). Status: traced by test.
- `walk_applications` line 451-456: `called_machine` or `target_machine` returning `None` just `continue`s, so an unresolvable callee contributes nothing to reachability. Whether the consuming obligation logic treats "not reached" as safe is outside this file; status: open.
- `has_summary == false` (539): a machine absent from the call graph skips cleanup and dynamic-realization edges.

---------------------------------------------------------------------

## 7. semantic-vocabulary/src/proposition.rs

**Purpose.** The proof proposition calculus and its supporting number types, in nested modules: fixed-width integer semantics (`integer_types`), proof terms (`proof_terms`), the `Proposition` enum with validation and context (`propositions`), value-id remapping/walking (`value_ids`). Includes ~1,700 lines of tests (`mod tests`, 2760-4460).

**Shape.**
- `integer_types` (8): `IntegerSign`, `IntegerCarrier {Fixed, Address}`, `IntegerType {carrier, sign, bits: u16}` (244) with `new` (454, bits 1..=128) and `address` (465, same range), `admits` (500), `compare`/`compare_with` (518/522), bitwise ops (554-615), exact ops via `exact_arith!` (666: add, sub, mul, div) and `exact_rem` (673), wrapping ops (`wrapping_arith!` 266; wrapping_div 692, wrapping_rem 709), saturating ops (726-1024), shifts (763-927), widen/cast (367-452), exact-add/sub with address/count operand roles (295-365), interval bounds for and/or/xor (inner `bit_ranges` 12-136, `KnownBits`) and shifts (841-889). `IntegerValue {Signed(i128), Unsigned(u128)}` (1030), `IntegerMathLiteral` (1047), `IntegerMathTerm {IntegerLiteral, MathValue, Embed, Add, Subtract, Multiply, ShiftLeft}` (1102) with `embed` (1147) and `validate` (1162).
- `proof_terms` (1298): `ProofFormalScope`, `MathematicalArgument`, `ProofTerm` (8 variants, 1321), `ProofTermField`; `validate` (1387).
- `propositions` (1493): `Proposition` (1806: Truth, Falsehood, Atom, Equal/LessThan/LessOrEqual over `ScalarTerm`, three `IntegerMath*` comparisons, `IeeeFloatComparison`, `ScalarIeeeFloatComparison`, `ByteSequenceEqual`, `StructuralIdentity`, `StructuralCaseMembership`, `Conjunction`, `Disjunction`, `Implication`, `ContentConservation`); manual non-recursive `Clone` (1572), `PartialEq` (1628), `Hash` (1724) but `PartialOrd, Ord, Debug, Record` derived (1805); `validate` (1880); `PropositionContext` (1977: `value_types` and `structural_places` as `BTreeMap`) with `validate` (2019) and helpers; `PropositionError` (2278, 51 cases); export macros `scalar_comparison_operands!`, `ordered_comparison_operands!`, `integer_math_comparison_operands!` (2245-2275).
- `value_ids` (2388): `remap_value_ids` (2402), `visit_value_ids` (2475), `visit_value_ids_bounded` (2487), `any_value_id` (2512), `walk` (2517).

**Inputs/outputs.** Pure data plus validation returning `Result<(), PropositionError>`; `remap_value_ids` mutates in place.

**How it decides.**
- Integer typing: every operation checks `admits_both` and the sign pairing (`dispatch_sign` 617). Mixed sign comparison is allowed in `compare_with` (522-552) except when either type is `Address` and the types differ (530).
- `Proposition::validate` (1880-1973): canonical form checks (`IntegerMathEqual` operands ordered `left <= right` 1893; symmetric float comparisons ordered 1908, 1919; `ByteSequenceEqual` ordered 1929; `StructuralIdentity` same structural type and ordered 1934-1940; conjunction/disjunction need >= 2 members 1943, 1953). No check on `StructuralCaseMembership` (1941) or `Atom` (1885).
- `PropositionContext::validate_value_terms` (2024): value ids must be in `value_types` with exact `ScalarType` equality (2170-2179); structural roots must be in `structural_places` (2039-2066); content terms: `Entry` version on a `Result`/`OperationResult` root refused (2136-2143); `TrivialAffineLocal` and `BlockParameter` roots refused as content (2145-2153).
- `IntegerMathTerm::validate` (1162): `Embed` must wrap an integer scalar that is not already a plain `Value` or `Integer` (1169-1177) (canonical form).
- `Proposition::same_commutative_predicate` (1850): only `Equal` operand order and set-equivalence of conjunction/disjunction members; everything else `==`.
- Tag-like values: `wrapping_shift_count` (1200): count reduced modulo width for unsigned, `rem_euclid` for signed.

**Cases by shape.**
- Address arithmetic: `admits_exact_add_operands` (295-306): address result needs `(self, u64 count)` or `(u64 count, self)`; fixed result needs both operands equal to self. `admits_exact_subtract_operands` (333-340): address left operand with `(self,count)` or `(count, left)`; fixed requires same type and non-address.
- `exact_cast_value_to`/`can_exact_cast_to` (367-369): allowed when either side is Fixed.
- `can_widen_to` (425-434): both Fixed, strictly more bits, same sign or unsigned->signed.
- `saturating_rem` is defined as `wrapping_rem` (747-753).
- `bitwise_bounds` handles sign mixed-range intervals by returning no known bits (98-101).
- `ProofTerm` validates each of 8 shapes in a fixed match (1393-1472), reusing `EmptyProofTermTypeIdentity` for several different emptiness failures (1423, 1437, 1446, 1462).

**Same job elsewhere.**
- Typed integer arithmetic: `IntegerType::wrapping_*/saturating_*/exact_*` (here) vs `ArithmeticDomain` + `integer_policy_bridge` (numerics/lib.rs:44, 392) vs `LandedIntegerType::bounds` (literals.rs:42). Three descriptions of the same policies.
- `ScalarTerm` evaluation: `ScalarTerm::integer_value` (scalar_terms.rs:385) dispatches to these `IntegerType` methods; `remap`, `walk`, `visit_value_ids` each repeat the same match over `ScalarTerm` and `Proposition` shapes (value_ids 2401-2463, 2517-2601; `PropositionContext::validate_term` 2165; `ScalarTerm::visit_value_ids` scalar_terms.rs:1639). Five traversals share the `scalar_term_predicate_quint_pattern!` / operand macros.
- Clone/Eq/Hash written by hand as iterative loops, with `PartialOrd`/`Ord` still derived (1805) and therefore recursive. Test 4299 exercises depth 8192 for clone/eq/hash only.

**Unused or one-caller.** `same_commutative_predicate`: 16 lines in 2 files, 15 here (def 1850 + tests), outside only terminal-psi semantics/descriptors.rs:1 (one production call site). `IntegerMathTerm::embed`, `Proposition::as_equal`: not grepped.

**Stated limits.** `MAX_CONTENT_TERM_DEPTH` lives in storage.rs. In this file: `IntegerType::new` width range 1..=128 (455). Test 3796 states terms validate "off the call stack" for depth 100,000.

**Fail-open risks.**
- `Proposition::validate` (1885, 1941): `Atom` and `StructuralCaseMembership` pass unconditionally; `PropositionContext::validate_value_terms` (2028) also lets `Atom` through without a lookup, and `StructuralCaseMembership` only checks the root place (2064-2068) not the case id. Whether an unknown `PropositionId` or case id is caught elsewhere is not traced. Status: open.
- `wrapping_div` / `saturating_div` (692-745) return `None` for divide-by-zero via `checked_div`, so they refuse, not accept.
- `PropositionContext::validate_term` catch-all arm `_ => pending.extend(term.operands()...)` (2195): a new `ScalarTerm` variant with operands is traversed generically; one without operands is accepted without checks. Status: traced (current variants all covered by explicit arms or operand tables).
- `Proposition::Ord` is derived over a recursive enum, so comparing two very deep conjunctions can recurse (`PartialOrd, Ord` at 1805; contrast manual iterative `PartialEq`). `validate` uses `left > right` for ordering checks only on non-recursive operand types, so I did not see a path that compares deep propositions. Status: traced for validate, open elsewhere.

---------------------------------------------------------------------

## 8. semantic-vocabulary/src/scalar_terms.rs

**Purpose.** The scalar expression language used inside propositions: `ScalarType`, `ScalarTerm` (about 40 variants), structural path and field subjects, and the validation / constant-evaluation passes over `ScalarTerm`.

**Shape.** `validation` module (14): `ScalarTerm::validate` (23, two-phase explicit stack Visit/Finalize), `validate_after_operands` (39). `value_queries` module (328): `scalar_type` (333), `integer_value` (385, explicit stack), `boolean_value` (630). Types: `ScalarType {Boolean, Integer, IeeeFloat}` (707), `CanonicalStructuralPathSegment {Field, FixedIndex, Case, Referent}` (730), `IeeeFloatFormat` (738), `IeeeFloatValue {Binary32(u32), Binary64(u64)}` (753), `IeeeFloatComparisonKind` (768: 6 kinds, `negated`, `is_symmetric`), `IeeeFloatStructuralField`, `ByteSequenceStructuralField`, `StructuralCaseSubject`, `StructuralIdentitySubject` (`root`, `path`, optional `structural_type`), `ScalarReadProjection {Referent, Extent, Static, Index(Box<ScalarTerm>)}` (910), `ScalarTerm` (927). Macros: `scalar_term_unary_operand!`, `scalar_term_binary_operands!`, `scalar_term_predicate_quint_pattern!` (1745, the five structural leaf variants), constructors via `integer_binary_term!` and `integer_shift_term!` (1252-1289), `operands`/`operands_mut` (1609/1624), `mathematical_comparison` (1406).

**Inputs/outputs.** Constructors return `Result<ScalarTerm, PropositionError>` after operand-type checks; `integer_value`/`boolean_value` return `Option` for closed terms.

**How it decides.**
- Constructors and `validate_after_operands` (39-291) both check operand `scalar_type()` equality against the term's declared `IntegerType`; `StructuralRead` requires a non-empty path, no `Static(Referent)` (duplicate spelling, 45-53), and each index operand an integer non-address (56-64).
- `integer_value` (385-437, 439-608): returns `None` if an operand is not a closed integer value, types differ, or the underlying `IntegerType` op refuses (overflow, bad shift count, divide by zero).
- `mathematical_comparison` (1406-1462): only for `IntegerEqual/LessThan/LessOrEqual` whose operands have *different* `ScalarType`s (`left.scalar_type() == right.scalar_type()` returns `None`, 1425) and still pass `integer_comparison_operands_match`; converts values and closed constants to `IntegerMathTerm` and orders equality operands (1448).
- `integer_comparison_operands_match` (1659): left type exact; right must be integer; address types must be equal.

**Cases by shape.**
- `scalar_term_predicate_quint_pattern!` is the closed set of five structural leaf terms: `BooleanField, ByteSequencePredicate, IeeeFloatField, IntegerField, ViewExtent` (1745-1754).
- `validate_after_operands` lists `Application, Value, BooleanField, IntegerField, ViewExtent, ByteSequencePredicate, IeeeFloatField, Boolean, IeeeFloatLiteral` as no-check arms (68-76).
- Exact operations in the evaluators: shifts (4 variants), exact add/sub (own operand-role rules), a six-variant block for wrapping/saturating add/sub/mul, a seven-variant block for exact mul/div/rem and wrapping/saturating div/rem (565-605).

**Same job elsewhere.**
- Validation vs construction: every `integer_*` constructor (1587-1607, 1464-1535) repeats the check done in `validate_after_operands`; the macro funnels most through `validate_integer_operands`, but `boolean_not` (1376), `boolean_equal` (1387), `integer_bitwise_not` (1466), `integer_widen` (1483), `integer_exact_cast` (1504) restate their checks inline next to the same checks in `validate_after_operands` (77-127, 141-156).
- Value-id visiting: `ScalarTerm::visit_value_ids` (1639) vs `value_ids::walk` (proposition.rs:2517); the latter additionally sets `complete = false` for structural reads and leaf terms, the former does not track it.

**Unused or one-caller.** The exported macros `scalar_term_unary_operand!`, `scalar_term_binary_operands!`, `scalar_term_predicate_quint_pattern!` are used in this unit by proposition.rs (2187, 2451, 2588); not grepped outside.

**Stated limits.** None.

**Fail-open risks.**
- `validate_after_operands` arms for `Application` and `Value` (68-70) accept without checking `scalar_type` against anything; `Value` types are checked only in `PropositionContext` (proposition.rs:2169-2180). Status: traced (context check exists for propositions; a bare `ScalarTerm::validate` does not check value types).
- `boolean_value_after_operands` for `IntegerEqual` (666-675) only checks the *left* type equals `scalar_type`; the right type is checked inside `compare_with`'s address rule. Mixed-sign comparisons are intentionally allowed (test 3940). Status: stated by tests.
- `integer_value` returns `Some` only for fully closed terms; `Value`, field and read terms yield `None` (606). Not fail-open.

---------------------------------------------------------------------

## 9. semantic-vocabulary/src/storage.rs

**Purpose.** Geometry vocabulary: atomic cell types, authored bit-fragment layouts, content-conservation terms and their fingerprint, structural place kinds, reference-origin extents, and target carrier layout.

**Shape.**
- `atomic_cell::AtomicCellType {Boolean, Integer, BoundedInteger}` (15); `is_supported` (47): non-address integers of 8/16/32/64 bits only.
- `authored_storage`: `AuthoredBitFragment` (5 u64 fields, 68), `AuthoredBitFragments` (storage arena + span, sorted on construction 84-89; `byte_envelope` 95), `AuthoredRecordStorage`, `AuthoredFieldStorage {Scalar, Repeated, Direct, Fragmented}` (146).
- `content`: `ContentAlgebra {kind, parameter}`, `ContentAlgebraKind {IntervalSet, CountedQuantity}`, `ContentProjectionIdentity`, `ContentProjectionScalar`, `ContentProjectionExpression`, `ContentPlaceVersion {Entry, Current}`, `ContentPlaceSegment`, `ContentStructuralPlace` (`is_case_alternative` 254), `ContentTerm {Projection, Separate}` (272; `separate` 281 flattens, sorts, rejects arity < 2 or duplicate), `ContentConservation` (303; `new` orders left <= right 310-324; `validate` 338), `content_conservation_report_fingerprint` (350), `encode_fingerprint_string` (447), `validate_term` (454).
- `reference_origins::ReferenceOriginExtent {Projected, WithinRegion}` (633).
- `structural_places::StructuralPlaceKind` (645: Parameter, BlockParameter, Result, OperationResult, ByteSequenceLiteral, ProviderAttachment, TrivialAffineLocal{construction: Option<AffineConstructionElement>}); `produced_structural_type`, `as_operation_result`.
- `target_carrier_layout::TargetCarrierLayout` (712: six u64 size/alignment fields).

**How it decides.**
- `byte_envelope` (95-112): returns `None` when any fragment container width is 0 or not a multiple of 8, or on overflow.
- `content_conservation_report_fingerprint` (350-445): `None` if a subject root is missing from `structural_places` (393) or is one of `ByteSequenceLiteral | BlockParameter | ProviderAttachment | TrivialAffineLocal` (410-413), or the domain id exceeds `u32` (384). Algebra kind tags are 3 for IntervalSet and 2 for CountedQuantity (366-367); this is a hash input, FNV-1a over the buffer (359-361).
- `validate_term` (454-501): depth cap 256 (452), zero fingerprint refused, empty case/field names refused, `Separate` needs >= 2, no nested `Separate`, strictly increasing order.
- `ContentStructuralPlace::is_case_alternative` (254): same root and version and the first differing segment pair are both `Case`.

**Cases by shape.** `StructuralPlaceKind` fingerprint encodes only Parameter (tag 1), Result (3), OperationResult (4) (394-409); tag 2 is not used there. Atomic cell: exactly 8/16/32/64-bit non-address integers (50-52).

**Same job elsewhere.** `ContentTerm::separate` (281) and `ContentTerm::validate_term` both enforce the sorted, deduplicated, flat shape (295, 491). `AuthoredBitFragments::new` sorts rows (85); equality/ordering/hash then go through the sorted slice (115-135).

**Unused or one-caller.** Not grepped individually; re-exported at semantic-vocabulary/lib.rs:1341-1380.

**Stated limits.** `MAX_CONTENT_TERM_DEPTH = 256` (452). Tests at 589-625 state that block parameters cannot supply content authority.

**Fail-open risks.** None seen. Unknown roots and unsupported place kinds produce `None`/errors. `AuthoredBitFragments::byte_envelope` on an empty fragment list returns `Some((u64::MAX, ...))`? `start=u64::MAX, end=0`, then `end.checked_sub(start)` fails -> `None` (111). So an empty list gives `None`.

---------------------------------------------------------------------

## 10. source-audit/src/lib.rs

**Purpose.** Test helper: walks a crate's `src/` directory and returns non-comment source lines, and reports lines containing forbidden substrings except for exact whitelisted declarations. Used by per-crate `source_guards.rs` tests.

**Shape.** `source_lines(manifest_dir, root) -> Vec<(file, line_number, line)>` (27), `violations(manifest_dir, root, forbidden, allowed) -> Vec<String>` (86).

**How it decides.** Skips directories starting with '.' and named `tests`, `examples`, `target` (56); skips files `source_guards.rs`, `tests.rs`, `*_tests.rs` and non-`.rs` (61-67); skips lines whose trimmed start is `//` (76). Does not skip block comments (`/* */`) or inline `#[cfg(test)]` modules inside a file. `violations` matches by `line.contains(needle)` and exempts a hit only when the file path ends with a suffix AND the trimmed line equals the declaration exactly (96-100).

**Unused or one-caller (grepped).** `source_audit`: 10 lines in 10 files (Cargo.toml 1, omega cli main.rs 1, packages.rs 1, terminal-psi lib.rs 1, checked-trees lib.rs 1, and five `source_guards.rs` under omega/compilation). Those lines are mentions; I did not read them. Header (10) says it is a dev-dependency "never linked into shipped binaries"; not checked against Cargo files.

**Stated limits.** Header: skips `source_guards.rs`, `tests.rs`, `*_tests.rs` "where the forbidden needles themselves are spelled out".

**Fail-open risks.** `source_lines` skips `//` lines only; a forbidden needle inside a `/* ... */` block comment is flagged, not skipped; a needle in a string would also be flagged. The reverse risk (guard misses code) arises from skipped file names (`tests.rs`) and directories; status: stated.

---------------------------------------------------------------------

## 11. source/src/capture.rs

**Purpose.** An immutable captured source tree, a filesystem router that mounts captures at absolute roots (with host-link following), and a read-only snapshot protocol.

**Shape.**
- `CapturedSourceNode {Directory, File{bytes: Arc<[u8]>, executable}, Symlink{target}}` (22); `CapturedSourceTree {entries: BTreeMap<PathBuf, Node>}` (29): `new` (34, requires canonical relative names, unique, every parent an explicit Directory entry), `entries`, `canonicalize` (74), `exists`, `is_file`, `is_dir`, `read_to_string` (177).
- `SourceFileSystem {captures: BTreeMap<PathBuf, Arc<Tree>>}` (245): `has_captured_namespace`, `join_captures` (258), `add_capture` (283), `canonicalize` (313), `exists/is_file/is_dir`, `read_to_string` (355), `route` (376), `route_capture` (428).
- `SourceSnapshotEntry`, `SourceSnapshotMetadata`, trait `SourceSnapshotStorage` (488), `SourceSnapshot(Arc<dyn Storage>)` (494: `lookup`, `metadata`, `read`, `read_link`, `list`), `SourceSnapshotError` (497), `source_snapshot_path_is_canonical` (591).

**How it decides.**
- `canonicalize` (74-152): resolves `..` lexically first (refuses escape 79-85), then walks components; symlink targets must be relative and contained, at most 64 link traversals (117), empty targets refused (124), file with remaining components -> NotADirectory (107), missing -> NotFound (143).
- `route` (376-426): no captures -> physical path; otherwise make absolute, check mount prefix via `route_capture`; if not under any mount, re-read each host path prefix with `std::fs::read_link` and re-route up to 64 times; no host link -> `Physical(path)` (414-416). So a path outside every captured root reads the host.
- `route_capture`: strips `.`/`..` lexically; `..` at the capture root is `PermissionDenied` (435-440).
- `SourceSnapshot::lookup` (511): path must be canonical (no backslash, no drive prefix, no `.`/`..`/empty components, UTF-8, no NUL, 591-600), each prefix directory must be a Directory.
- `join_captures` (258): conflict only if the same root maps to a different tree (compared by value); otherwise union.

**Cases by shape.** `add_capture` roots must be absolute and canonical (284-302). Snapshot entries are only Directory, File, Symlink.

**Same job elsewhere.** `CapturedSourceTree::canonicalize` and `source_snapshot_path_is_canonical`/`lookup` each validate "canonical relative path" rules: tree uses `relative_components` (218) and `validate_entry_path` (198), snapshot uses a byte-level check (591). Two path-validation implementations over `PathBuf` vs `&[u8]` in one file.

**Unused or one-caller (grepped).** `CapturedSourceTree|SourceSnapshot\b|SourceSnapshotStorage`: 74 lines / 23 files; 22 here, 5 in source lib.rs -> 47 outside (checked-interpreter filesystem*.rs 4; omega package-compilation source_snapshot.rs 7, anonymous_owner.rs 9, others).

**Stated limits.** Header: "missing captured entries never reopen host files"; test at 810 pins replaced-host-directory behavior (unix-only tests, `#[cfg(unix)]` at 622, 696, 731, 808, 814).

**Fail-open risks.**
- `SourceFileSystem::exists/is_file/is_dir` return `false` on any routing error (332-352): a `route` failure (alias cycle, escaped `..`) reads as "absent", not as an error. Not an accept.
- `route`: a path outside any capture root falls through to the host disk (414-416). Stated in header and test (723). The wrong program that would get through: a source path that lies outside every mounted root reads from the real filesystem. Status: stated.
- `route_capture` final `strip_prefix(root).or_else(|_| normalized.strip_prefix(root))` (452-455). Not traced further.

---------------------------------------------------------------------

## 12. source/src/lib.rs

**Purpose.** Core source coordinate and custody types: spans, source ids, `SourceFile`, `SourceMap` (the selection-aware file list with package/scope/stratum visibility rules), `SourceCatalogue` (append-only backing store with forked selections), `SourceBytes`, read accounting, and consumed-source identity digests.

**Shape.**
- `Span`, `SourceId(usize)`, `SourceSpan`, `SourceOrigin {User, Toolchain}`, `SourceResolutionStratum {Base, CurrentActivationExtension, DependencyActivationExtension}`, `DependencyScope {Product, Build}`, `SourceFile` (122), `SourcePosition::of` (150), `SourceContents`, `LoadedSource(s)`, `SourceBytes` (215), `display_literal_bytes` (52).
- `read_accounting` (296): atomic counter and `OMEGA_SOURCE_READS` env reporting.
- `source_identity` (338): `ConsumedSourceUnitKind {PackageAuthored, PackageGenerated(scope), ToolchainVirtual, ToolchainOwned}`, `ConsumedSourceUnit` (private fields), `from_retained` (365), `consumed_source_unit` (436), `append_canonical_consumed_unit` (536), `canonical_path_components` (570), `toolchain_source_identities` (601), `toolchain_source_identity_digest` (614), `canonical_source_entry` (633).
- `SourceMap` (744: files, `SourceSelection {All, Explicit(Vec<bool>)}`, `source_rejoins`, `product_dependencies`, `query_dependencies`); `SourceCatalogue` (760: `begin_selection`, `admit_checked_instance` 789, `admit_source_occurrence` 858, `activate_instance` 883).
- `SourceMap` methods 912-1337 including `retain_source_rejoin` (927), `rejoined_source_id` (973), `select_checked_instance` (1007), `bind_anonymous_owner` (1044), `retain_product_dependency_scope`, `product_dependency_at` (1113), `add*` family, `same_package` (1211), `same_checked_instance` (1218), `reference_can_see_declaration` (1232), `reference_can_see_stratum` (1257), `reference_can_see_stratum_at` (1273), `captured_reference_can_see_stratum` (1290), `resolution_strata_separate` (1307).

**How it decides.**
- Stratum visibility (1273-1287): a `Base` declaration is visible to all; a `CurrentActivationExtension` declaration only to a reference of the same stratum; a `DependencyActivationExtension` declaration is visible unless the reference is `Base` and in the same package (1283-1286).
- `reference_can_see_declaration` (1232-1255): requires the declaration (and a non-empty reference) selected under `Explicit` selection; requires equal `dependency_scope` and stratum visibility.
- `same_checked_instance` (1218): equal dependency scope and (equal package identity, or both anonymous with equal `package_root`). `same_package_files` (1339) repeats the identity/root match without the scope.
- `admit_checked_instance` (789-856): for non-extension, non-anonymous-user sources it looks for an existing selected row with an identical 7-field key (path, source text, package_root, package_identity, origin, stratum, scope); more than one selected match is an error (822).
- `retain_source_rejoin` (927-967): refuses unless every one of origin User, same text/path/root/origin/stratum/scope, identity unchanged if set, original not an extension (941-952); one destination per original.
- `consumed_source_unit` (436): User sources need a package identity and a path under `package_root`; Toolchain virtual detection is by path shape `<name>` (648-657, one component starting `<` ending `>`); Toolchain owned requires a UTF-8 `package_root` file name as namespace.
- Digest framing: tags 0,1,4 (Product/Build generated),2,3 (544-543); SHA-256 with domain strings (343, 623).

**Cases by shape.** Virtual toolchain path: exactly one normal component wrapped in angle brackets (648). `PackageGenerated` Product/Build get tags 1/4 (539-540).

**Same job elsewhere.**
- Exact-key source match (path, source, root, identity, origin, stratum, scope) at lib.rs:809-819 and 1018-1026, and a near-copy in symbols/retained_declaration.rs:148-158 (without package_identity equality).
- `same_checked_instance` (1218-1228) vs `same_package_files` (1339-1345) vs symbols `same_source_package`/`same_symbol_source_package`/`same_product_package_instance` (symbols/table.rs:1627, 1660, 1640), which delegate to these.
- Package-identity-by-string: `SourceMap::product_dependency_at` (1113) and symbols `product_dependency_target` (table.rs:1388) are the same lookup at two layers.

**Unused or one-caller (grepped).** `consumed_source_unit|toolchain_source_identit|canonical_source_entry|append_canonical_consumed_unit`: 47 lines / 14 files; 19 here -> 28 outside (package-compilation source_consumption.rs 12, checked_sources.rs 2, lib.rs 2, review-evidence locations.rs 2, others). `SourceCatalogue::fork`, `admit_source_occurrence` not grepped.

**Stated limits.** `SourceResolutionStratum` comments (108-109). Header notes physical paths and load order are not semantic identity.

**Fail-open risks.**
- `reference_can_see_declaration` (1244-1252): an empty-span reference returns `true` once selection passes; a missing file for the reference or declaration also returns `true`. Test 1470 names the empty-span case "source_free_reference_remains_permissive". Status: stated for source-free; the missing-file case (1247-1252) is open.
- `reference_can_see_stratum` (1262-1270): same permissive result for empty span and for missing files. Status: open for missing files.
- `SourceMap::text_at` / `SourceFile::text_at` return `""` for unknown ids or bad ranges (145, 1332-1336); test 1462 pins it. A caller that treats `""` as "no text" would be silently satisfied; not traced.
- `SourceSelection::includes` for `Explicit` returns `false` on out-of-range (907), so unknown ids are unselected. Not fail-open.

---------------------------------------------------------------------

## 13. symbols/src/builtin.rs

**Purpose.** The compiler-installed roster that fills the root's first children: 21 source-free types, 105 builtin functions (mostly unnameable asm/float intrinsics), 5 fixed `core::*` mathematical declarations, and the one member under `addr`.

**Shape.**
- `BUILTIN_TYPE_COUNT = 21` (8); `BuiltinTypeAtom` (17, 21 members), `ALL` (47), `ordinal` (80, const loop), `is_atomic` (96), `is_canonical_const_scalar` (106), `is_fixed_integer` (115), `SPELLINGS` table (134: atom, diagnostic identity, symbol name), `identity`, `symbol_name`.
- `BuiltinType {UInt, Int}` (168) with `from_name`, `name`, `ordinal` (defers to the atom).
- `BuiltinFunction` enum (209) with 105 variants in declaration order; `COUNT = 105` (411); `ALL: [(Self,&str); 105]` (425-544) in a *different* order from the enum, slot = array position; `from_ordinal` (546), `from_name` (555, string compare), `name`, `ordinal` (593, linear `position`), `is_value_position_asm_intrinsic` (574), `asm_intrinsic_service_name` (606), `is_float_family` (672), `has_empty_write_frame` (738), `is_asm_intrinsic` (747, `name().starts_with("asm#")`).
- `builtin_type_symbols`, `builtin_function_symbols`, `fixed_core_declaration_symbols`, `FixedCoreDeclaration` (776: Level, Type, Strict, Squash, Identity; `COUNT = 5`; `name`, `ordinal`, `from_ordinal`, `from_name`), `BuiltinTypeMember::AddressBound` (870; `ordinal()` is `0`, 882), `builtin_type_member_symbols` (887).

**How it decides.** Identity is slot position, not name. Names are spellings used for installation and lookups; `from_name` and `is_asm_intrinsic` are keyed on spelling (`"asm#"` prefix, 747-749). `asm_intrinsic_service_name` (606-665): 34 MachineControl intrinsics, `AsmPortOut|AsmPortIn` -> "PortIo", float family -> None, a listed set of fences/hints/proof-only builtins -> None, then `_ => None`.

**Cases by shape.** Value-position asm intrinsics are exactly three: `AsmPortIn, AsmSnapshotFlags, AsmReadMsr` (574-579; test 995). Float family is a 49-line explicit `matches!` list (672-725). Comment 538 "Slots 100..=102: appended after the published 0..=99 roster" is followed by five entries (539-543): slots 100-102 (aarch64 fences) and 103-104 (`sgdt`, `lgdt`).

**Same job elsewhere.**
- Spelling roster, ordinal roster and enum are three parallel lists for functions: `enum BuiltinFunction` (209-402), `ALL` (425-544), `is_float_family` (672-725); the `roster!` macro (404) and plain tuples are mixed inside `ALL`. For types: `enum`, `ALL` (47-69), `SPELLINGS` (134-156).
- `BuiltinFunction::ordinal` is a linear search (593-598) while `BuiltinTypeAtom::ordinal` is a const loop (80-86); both are re-derived on each call. `table.rs:builtin_function_symbol` (3157) and `root_slot_ordinal` (3237) are the table-side consumers.
- Symbol-table-side classification keyed on slot: `SymbolTable::builtin_type_atom`/`builtin_function_for_symbol` (table.rs:3223, 3169) vs name-keyed `from_name` here.

**Unused or one-caller (grepped).**
- `has_empty_write_frame`: 21 lines / 9 files (1 here); 20 outside... note the 21 count also includes the combined pattern file listings? Grepped alone: 21 lines in 9 files, 1 in builtin.rs, 20 outside (calls.rs 7, flow_mutation.rs 4, execution_terminal_unit_control.rs 3, validation lib.rs 2, others 1 each).
- `asm_intrinsic_service_name`: 10 lines / 4 files; 7 here (def plus tests); 3 outside (effect_inference.rs, machine_calls_core.rs, checks_multiplicity_borrowed_windows.rs, 1 each).
- `BuiltinFunction::from_name|BuiltinType::from_name|BuiltinFunction::from_ordinal|FixedCoreDeclaration::from_name`: 15 lines / 6 files; 8 here (tests), 1 in table.rs, 6 outside (symbols.rs 3, symbols_targets.rs 1, review-evidence intrinsic.rs 1, expressions.rs 1).
- Header of `FixedCoreDeclaration::from_name` (837-838) says it serves "the transitional unresolved-name classifiers downstream".

**Stated limits.** "transitional unresolved-name classifiers downstream" (838); "during the provider migration" (322); "(PROOF-CONTRACT-MIGRATION)" (770); "append-only" roster (420-423, with the reason: review evidence records the ordinal).

**Fail-open risks.**
- `asm_intrinsic_service_name` `_ => None` (663) after an arm that already lists every other variant. A new `asm#` privileged builtin added to the enum and `ALL` but not to the first arm would report no service reach and be treated like a fence. The tests (1083-1089) only check "has a service implies is asm", not the converse. Status: open.
- `has_empty_write_frame = is_float_family` (738-740): an addition to the float family silently becomes a "writes nothing" call. Stated rationale at 728-737 (floats do not write caller storage).
- `BuiltinFunction::ordinal` `.expect(...)` on a missing member (597) would panic, not accept.

---------------------------------------------------------------------

## 14. symbols/src/fast_map.rs

**Purpose.** An FxHash-style multiply-rotate hasher and two type aliases for fast `HashMap`s keyed by symbol handles or tuples.

**Shape.** `SymbolHasher(u64)` (19), `BuildSymbolHasher` (74), `SymbolMap<V>` (86) = `HashMap<SymbolHandle, V, BuildSymbolHasher>`, `SymbolKeyMap<K,V>` (89). `write(bytes)` adds each byte (35-39); integer writes add the value (42-70).

**Unused or one-caller (grepped).** `\bSymbolMap\b|\bSymbolKeyMap\b`: 195 lines / 37 files; 13 in symbols (table.rs 8, lib.rs 2, fast_map.rs 3) -> 182 outside across 34 files (largest: validation/contract_entailment.rs 20, published_facts_crash_calls.rs 20, frozen_program_and_queries.rs 28).
Header (4-12) says the keys are "already well-mixed handles"; inside the same crate `SymbolKeyMap<Box<str>, ...>` is used with string keys (table.rs:127-128, 175-182), which hashes one byte at a time through `write`. Read.

**Fail-open risks.** None (hash only).

---------------------------------------------------------------------

## 15. symbols/src/lib.rs

**Purpose.** Crate root and documentation: declares `builtin`, `fast_map`, `retained_declaration`, `symbol`, `table`, `tests` and re-exports (42-56). Header says identity is the handle and spelling is metadata, and stage 02 builds the table.

**Shape.** Re-exports only. No logic. `#[cfg(test)] mod tests;` (58-59) points at `tests.rs`, not read.

**Stated limits.** "once resolution finishes, a string name never stands in for a symbol" (lines 15-17). Contrast with code that keys on names: `find_child_by_name` (table.rs:2588), `find_top_level_*` (2629+), `find_declaration_from_source` (1573-1574), `BuiltinFunction::from_name` (builtin.rs:555). Read.

---------------------------------------------------------------------

## 16. symbols/src/retained_declaration.rs

**Purpose.** Carries a chosen declaration (as an inert source anchor) across independently allocated symbol tables and finds the same declaration again.

**Shape.** `SourceSpanCandidates {built_for, rows}` (24) with `at_span` (34): sorted (start, end, handle) rows for exact-span lookup, rebuilt when the table address or arena revisions/lengths differ (37-44). `RetainedDeclaration {source: SourceFile, span, kind}` (67): `from_retained_parts` (79), `capture` (106), `rejoin` (129).

**How it decides.** `rejoin` iterates candidate handles; skips invalid handles, a different `SymbolKind` (136), different span (142), missing source (145), or any mismatch in path, package_root, origin, stratum, dependency_scope, text (148-158); a stored owner must equal the candidate's owner (153-156), an anonymous stored source may match any owner. Two distinct matching candidates -> error "ambiguous" (161-163); none -> "absent or changed" (166-168). `from_retained_parts` requires a non-empty in-range span and resets `source_id` to 0 (84-88).

**Cases by shape.** Kind is a single `SymbolKind`; span is exact (start,end).

**Same job elsewhere.** The exact-key source comparison also appears at source/lib.rs:809-819, 1018-1026, 941-952.

**Unused or one-caller (grepped).** `SourceSpanCandidates|RetainedDeclaration`: 71 lines / 12 files; 9 here (8 + lib.rs re-export 1) -> 62 outside (language-semantics declaration_selection.rs 15, build-time-evaluation product.rs 10, package-compilation provider_defaults.rs 10, provider_selection_codec.rs 8, provider_selections.rs 4, validation frozen_program_and_queries.rs 3, others).

**Stated limits.** Doc comment: "transfers an already selected declaration, never visibility, checking evidence or execution authority" (3-6). `SourceSpanCandidates` marked "GUARDED" (18) with its key.

**Fail-open risks.** `SourceSpanCandidates` cache key includes `std::ptr::from_ref(symbols) as usize` (38): a different table placed at the same address with equal revisions and lengths would reuse stale rows. Stated as guarded (18-21) via revision and length; address reuse after a drop is not excluded in the key. Status: open.

---------------------------------------------------------------------

## 17. symbols/src/symbol.rs

**Purpose.** The `Symbol` record and its name, kind and path types.

**Shape.** `SymbolKind` (8, 32 variants: Unknown, Root, Module, BuiltinType, BuiltinFunction, Data, Domain, Field, Variant, Machine, Operator, Measure, Proposition, MathematicalDefinition, State, Trait, Conformance, Const, ConformanceParameter, WireSchema, Parameter, TypeParameter, MachineParameter, PropositionParameter, PropositionMachineParameter, Local, HostCapability, Object, Function, Section, Import, TypeAlias). `SymbolName {storage}` (84) over `SymbolNameStorage` (89: Missing, Source(span), SourceSlice{Arc<str>, span}, OwnedSource{value, span}, Static(Cow), Owned(Arc<str>)); `SymbolNameRef` (235: Borrowed, Source, OwnedSource, Static); `SymbolPath {root, members}` (284); `Symbol {parent, declaration_owner, children, kind, name, generated_from}` (299); aliases `SymbolHandle`, `SymbolSpan`, `SymbolNameHandle` (294-296); `HierarchyNode` impl (317).

**How it decides.** `SymbolNameStorage::eq` (121-135): if both are "retained spellings" (`SourceSlice` or `OwnedSource`) they compare as (text, span); otherwise same-variant structural equality, different variants unequal. `from_ref_with_sources` (184-203) stores a `SourceSlice` only when the source bytes at the span equal the semantic spelling exactly. `as_str` of `Source(span)` returns the text from the source map, or `""` when no map is supplied (209-211).

**Cases by shape.** Six name storage forms; `SymbolNameRef::as_str` for `Source` returns `""` (250).

**Fail-open risks.** `SymbolName::as_str` / `SymbolNameRef::as_str` return `""` for a source-backed name without a map (209-211, 250), so name comparisons with an absent source map compare against the empty string. Not traced to callers; status: open.

**Stated limits.** `SymbolKind::Proposition` is "the retired dedicated-formula surface" (27-29). `generated_from` must be an "earlier symbol" (311-313), enforced in `is_generated_extension_of` (table.rs:2192-2196).

---------------------------------------------------------------------

## 18. symbols/src/table.rs

**Purpose.** The symbol table: a hierarchy arena of symbols plus append-only supplemental children, builder/extension types, deferred lookup indices, module and import namespace logic, and all scoped top-level lookups. It also decides source visibility (selection, stratum, package scope) for declarations.

**Shape.**
- Inline modules: `child_batches` (13, linked batches of appended children), `indices` (113, four `OnceLock` caches built through macro `index_cache!` 190, ignored by `PartialEq`), `modules` (282: import and module-path logic, `ModuleImport`, `SourceScopedTopLevelBinding::module_import` 317, `register_source_module` 374, `register_source_import` 439, `validate_source_module_import` 468, `source_module_import_target` 527, `domain_name_reaches` 622, `carrier_qualified_domain_reaches` 695, `import_exposes_symbol` 758, `exact_import_exposes_source` 793, `namespace_parent` 824, `select_namespace_candidates` 919, `find_module_qualified_candidates` 1047, `JoinedPath` 1260, `same_semantic_name` 1285), `source_queries` (1306: dependency scope, provenance, product declaration lookups, same-package predicates), `tests` (1779, cfg(test)).
- `SymbolTable` (1867), `SymbolTableBuilder` (1912), `SourceScopedTopLevelBinding {reference_source, declaration_source, name, module_import, producer}` (1928), `BindingProducer {Loader, AuthoredImport}` (1937), `SymbolLookup {NotFound, Unique, Ambiguous}` (1968), `TopLevelSelection {Ordinary, SignatureFree}` (1978), `SymbolTableAppender` trait (2101), `SymbolChildHandles` (3355), `SymbolTableExtension` (3396).
- `SymbolTable` main API: `is_generated_extension_of` (2137), `append_children` (2246), `append_generated_children_from` (2263), `record_declaration_owner` (2327), `rename` (2359), `begin_extension` (2383), `begin_source_activation` (2404), `insert_generated_root_from` (2463), `insert_generated_children(_from)` (2495/2513), `name` (2544), `child_handles` (2556), `find_child_by_name(_and_kind)` (2588/2603), `find_top_level_by_name_and_kinds_from_source(_matching)` (2629/2643), `lookup_top_level_by_name_and_kinds_from_source_matching` (2663), `lookup_signature_free_top_level_from_source_matching` (2685), `top_level_candidates_from_source` (2806), `lookup_top_level_candidates` (2835), `find_top_level_declaration_or_operator_family_from_source` (2950), `source_scopes_separate` (2985), `source_reference_can_see_symbol` (3049), `symbol_source_is_selected` (3079), `source_reference_may_select_symbol` (3115), `find_descendant_by_path` (3139), builtin slot lookups (3157-3242), `display_path` (3269), `path_from_members`, `resolve_child_path`.
- `source_queries` highlights: `find_product_declaration_from_source` (1421), `find_build_subject_declaration_from_source` (1446), `find_captured_product_declaration` (1467), `find_declaration_from_source` (1514), `source_reference_can_describe_product_symbol` (1604), `same_source_package` (1627), `same_product_package_instance` (1640), `same_symbol_source_package` (1660), `symbol_source_origin`, `symbol_package_identity` (1690), `symbol_product_package_identity` (1705), `authored_origin` (1741), `provenance_source_span` (1754).

**Inputs/outputs.** Reads `SourceMap` (shared `Arc`), `PackageKeyIdentity`, builtin rosters. Mutating methods clear the four index caches (`root_names`, `binding_source_index`, `module_path_index`, `module_scope_index`).

**How it decides.**
- Top-level precedence (`lookup_top_level_candidates` 2835-2942), in order: (1) source-backed reference whose source is unselected -> empty (2844-2851); (2) module-qualified candidate set if `find_module_qualified_candidates` returns `Some` (2852-2861); (3) root children by full name (plus leaf name if the reference has a namespace context 2866-2869) filtered by kind, caller predicate and `source_reference_can_see_symbol` (2871-2876); (4) a reference with no source span returns all (`SignatureFree`) or the first one (2878-2883); (5) a same-spelled `SourceScopedTopLevelBinding` without import restricts to its declaration source (2885-2897); (6) namespace context -> `select_namespace_candidates` (2899-2901); (7) SignatureFree prefers same-package candidates, else all (2902-2912); (8) Ordinary: same-source, then same checked instance and unseparated stratum, then unseparated stratum, then first (2914-2941).
- `select_namespace_candidates` (919-1010): local module/source first, then imported (by logical path or exposed leaf), then (no current module) same-package unmoduled, then non-User-origin unmoduled.
- Name matching is string equality on `self.name(candidate)` (e.g. 934, 972, 990, 1005, 1139) and on `display_path(..)` strings (1573-1574, 1499-1500).
- `find_declaration_from_source` (1514-1598): candidates with a flag `is_public`; dependency alias selection via `product_dependency_target` (1533); exact display-path match beats short name (1577); a second same-rank candidate is ambiguous unless both are operators with the same path/package/instance, or both machines in `token_bound_machines` (1582-1593).
- `source_scopes_separate` (2985-3040): machine-parent mismatch -> separate (2990-2995); different source ids: same package but different checked instance -> separate; different packages -> separate unless an import exposes one to the other; different logical module -> separate; different stratum -> separate; else a same-name loader binding naming either source.
- `domain_name_reaches` (622-684) and `carrier_qualified_domain_reaches` (695-748): exact path wins; carrier spelling needs the carrier resolved in the reference's source to equal the carrier resolved from the domain's own source (736-740), plus an import.

**Cases by shape.**
- Carrier kinds list `[BuiltinType, Data, Machine, Trait]` appears twice (640-648 inline, 715-720 as a const).
- `record_declaration_owner` (2327-2345): only a `Machine` declaration with a `Conformance` owner.
- `builtin_type_member` (3206-3210): only `BuiltinTypeMember::AddressBound`.
- `find_child_by_name` ambiguity: only a `Module` parent refuses a second match (2597, 2619).
- Slot layout (3157-3242): types at root child 0..21, functions at 21..126, fixed core at 126..131; `root_slot_ordinal` (3237) uses arena distance and confirms with `nth`.
- Operator family: `find_top_level_declaration_or_operator_family_from_source` (2950-2979) returns a representative only when a same-name `Operator` family has two or more members (`family.next().map(|_| representative)`, 2978), and a loader binding exists.

**Same job elsewhere.**
- Several overlapping "pick a declaration for a name from a source" implementations in this file: `lookup_top_level_candidates` (2835), `find_module_qualified_candidates` (1047), `select_namespace_candidates` (919), `find_declaration_from_source` (1514), `find_captured_product_declaration` (1467). The last two share the selected/exact/ambiguous loop (1483-1511 vs 1538-1596).
- `same_source_package` (1627), `same_symbol_source_package` (1660), `same_product_package_instance` (1640), `symbol_is_in_dependency_scope` (1321) and source/lib.rs `same_package`, `same_checked_instance` all answer package/scope equality with different permissive defaults for "no source map".
- `module_path_index` snapshot vs `indexed_display_path` fallback to `display_path` (3250-3267).
- `child_handles` via `SymbolChildHandles` is repeated in `module_scope_index` and `root_name_index` iteration.

**Unused or one-caller.** Many `pub` methods; grepped only these: `register_source_import|begin_source_activation|rejoin_product_occurrence`: 45 lines / 13 files; 5 here, 3 in symbols/tests.rs, 37 elsewhere (product_and_trait_declarations.rs 7, data_and_product_operands.rs 13, product.rs 5, frozen_program_and_queries.rs 3, others). The comment at 1623 cites a spec file `wiki/spec/build/scoped_execution.md`; not checked.

**Stated limits.**
- 1623-1625: "Source-free lowering is used by focused representation tests; those trees model one package and retain the historical all-local behavior."
- 1384-1387: product dependency `None` means "not an authorized product alias", "never as license to widen the search."
- 1043-1046: "An empty roster is an authoritative failed logical lookup."

**Fail-open risks.**
- `same_source_package` (1627-1631): `is_none_or` -> true when the table has no `SourceMap`. Stated (1623-1625). A source-free table treats every pair as the same package.
- `same_product_package_instance` (1645-1651): true when there is no `SourceMap`, and true when the declaration file is missing (`is_none_or`). First case stated by the "source-free" convention; second open.
- `source_reference_can_see_symbol` (3054-3063): true when no `SourceMap`, or the symbol has no provenance span. Stated at 3046-3047 ("Source-free consumers retain their existing behavior"). `reference_can_see_declaration` additionally treats an empty-span reference as visible (source lib.rs:1244).
- `symbol_is_in_dependency_scope` (1326-1328): `is_none_or` -> true without provenance or source file. Stated at 1317-1318.
- `inherited_symbol_is_selected` `unwrap_or(true)` (3096-3101): symbols allocated after the last activation frontier count as selected. Stated at 1882-1886 ("Later allocations belong to the new cohort").
- `symbol_source_is_selected` (3079-3094): no map -> true; no provenance -> true; Module with no provenance -> true.
- `lookup_top_level_candidates` last resort `candidates.first()` (2939): when no candidate is in the same source, same checked instance, or unseparated stratum, the first already-visible candidate is chosen. Reached only when `has_namespace_context` is false (2899) and selection is Ordinary. Input reaching it: a reference from a toolchain-origin or extension source, or User-origin sources only when `has_namespace_context` is false, which it is not for base User files (909-917). Status: traced (partially), no comment at the site.
- `lookup_top_level_candidates` (2878-2883): a reference with an empty span under `Ordinary` selection takes the first eligible candidate silently even if several exist. The doc at 2679-2684 describes only the `SignatureFree` case. Status: open.
- `find_child_by_name` / `find_child_by_name_and_kind` (2588-2623): outside `Module` parents the first match wins silently; no ambiguity check. Status: open.
- `SymbolTable::name` returns `""` for a source-backed name when `sources` is `None` (2547 via symbol.rs:209); `find_declaration_from_source`'s `self.name(symbol) != declaration_path` then compares to `""`. Status: open.
- `find_module_qualified_candidates` returning `None` (1054-1060, 1058) hands off to ordinary precedence; stated at 1043-1046.
- `register_source_module` (374): returns `Err` for a second module in one source; `register_source_import` (439): silent early return if the same selected import already exists.

---------------------------------------------------------------------

## Coverage

Read in full (no skimming): every range in the unit file, 20,308 lines in total:

| file | lines read |
| --- | --- |
| numerics/src/lib.rs | 1-1187 (1187) |
| numerics/src/literals.rs | 1-921 (921) |
| optimization/src/lib.rs | 1-437 (437) |
| semantic-vocabulary/src/identity.rs | 1-421 (421) |
| semantic-vocabulary/src/lib.rs | 1-1382 (1382) |
| semantic-vocabulary/src/proof_applications.rs | 1-1184 (1184) |
| semantic-vocabulary/src/proposition.rs | 1-4460 (4460) |
| semantic-vocabulary/src/scalar_terms.rs | 1-1754 (1754) |
| semantic-vocabulary/src/storage.rs | 1-720 (720) |
| source-audit/src/lib.rs | 1-107 (107) |
| source/src/capture.rs | 1-819 (819) |
| source/src/lib.rs | 1-1569 (1569) |
| symbols/src/builtin.rs | 1-1130 (1130) |
| symbols/src/fast_map.rs | 1-89 (89) |
| symbols/src/lib.rs | 1-59 (59) |
| symbols/src/retained_declaration.rs | 1-278 (278) |
| symbols/src/symbol.rs | 1-333 (333) |
| symbols/src/table.rs | 1-3458 (3458) |

Skimmed: none. Not read: none. The two test-heavy files (proposition.rs 2760-4460, numerics tests inside each module, source lib tests) were read but my entries describe the production code and cite tests only where they pin behavior.

## Five things a maintainer most needs to know about this unit

1. **"No source map" and "no provenance" mean "visible / same package / selected" almost everywhere in `symbols/table.rs` and `source/lib.rs`.** `same_source_package` (table.rs:1627), `same_product_package_instance` (1645), `source_reference_can_see_symbol` (3054-3063), `symbol_source_is_selected` (3080-3092), `symbol_is_in_dependency_scope` (1326), `SourceMap::reference_can_see_declaration` (source lib.rs:1244-1252) all return permissive answers for source-free tables, empty-span references or symbols with no span. Several have a "source-free tests keep historical behavior" comment (table.rs:1623-1625, 3046); the missing-file branches do not.
2. **Declaration lookup in `table.rs` is a ladder of at least five separate selection routines keyed on name strings and display paths** (`lookup_top_level_candidates` 2835, `find_module_qualified_candidates` 1047, `select_namespace_candidates` 919, `find_declaration_from_source` 1514, `find_captured_product_declaration` 1467), despite `symbols/lib.rs:15-17` saying names never stand in for symbols after resolution. The final `candidates.first()` fallback (2939) and the silent first-match in `find_child_by_name` (2588) are the places where more than one candidate collapses to one without an error.
3. **The proposition/scalar machinery is a family of near-identical traversals and per-operation tables.** `Proposition` and `ScalarTerm` are walked by at least `validate`, `PropositionContext::validate_*`, `remap_value_ids`, `walk`, `visit_value_ids`, `operands`, `integer_value`, `boolean_value` (proposition.rs, scalar_terms.rs); integer operations are described three times (`IntegerType` methods, `integer_policy_bridge` in numerics, `LandedIntegerType::bounds`). `Proposition` has hand-written iterative `Clone`/`Eq`/`Hash` but derived (recursive) `Ord` (proposition.rs:1805).
4. **Builtin identity is slot position in a hand-maintained roster.** `BuiltinFunction::ALL` is a 105-row array in an order different from the enum, `COUNT = 105` and the root slot offsets (21 types, 105 functions, 5 fixed core, table.rs:3157-3242) are literals, and the roster is declared append-only because package review evidence records the ordinal (builtin.rs:420-423). `asm_intrinsic_service_name` ends in `_ => None` (663), so an unlisted privileged asm intrinsic reaches no service.
5. **Helper duplication across crates.** `Cursor`, `push_encoded`, `DomainDigestEncoder`, `structural_variant_accessor!` exist both in `psi/foundation/optimization` and in `omega/foundation/optimization-core`; `Fnv1a` exists in semantic-vocabulary and at least two Omega backend files (grepped definitions; Omega copies not read). Within this unit, `ProofApplicationPlan` and `ProofApplications` each implement the same machine/target lookup (proof_applications.rs:73-116 vs 252-297).

## What you are unsure about

- Whether `FloatLiteral::value_f64/f32`'s `unwrap_or(0.0)` (literals.rs:200, 216) can ever fire: depends on `ExactFloat::from_decimal_str` vs `FloatSemantics::from_decimal`, in files I did not read.
- Whether `DeclarationSelectionAuthority::declaration_selection_scope` returning `None` (semantic-vocabulary/lib.rs:863-868) means "no restriction" or "unknown" to its consumers.
- Which of the five `ProofApplications` methods (`needs_scope`, `required_scopes`, `invocation_reachability`, plus plan `declared_callable_for_target`, `retain_product_machines`) have production callers; my grep hits (22 lines / 9 files) were not separated from same-named methods, and the one file list I checked showed only tests in checked-trees `facts.rs`.
- Whether the Omega-side `optimization-core` copies of `Cursor`, `codec_bytes`, `DomainDigestEncoder` and the other `Fnv1a` definitions are byte-for-byte equivalent to the Psi ones; I only grepped their definitions.
- The `value_kind` and `same_commutative_predicate` caller counts: the former matched a generic name (11 lines / 4 files); the latter has one non-test call site (terminal-psi semantics/descriptors.rs:1), not opened.
- Counts quoted from combined-alternative greps (for example `integer_policy_bridge|IntegerPolicyBridge`, `ProofApplications|ProofApplicationPlan|ProofCallGraph`) are sums over all alternatives and may include test lines in the defining file.
