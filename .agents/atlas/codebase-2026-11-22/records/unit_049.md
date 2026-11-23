# Record: unit_049 (area 07-psi-semantics, crate `terminal-semantics`)

Paths are relative to `source-rust/subsystems/compiler/psi/semantics/src/` (written `S/`). Checkout
`4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. Every file was read in full. Greps were run with
`grep -rw --include=*.rs NAME source-rust tools`; "lines" counts every matching line including the
definition and any `lib.rs` re-export, so "call sites" below are lines minus those two kinds, checked by
listing the lines for every name reported as 0 or 1.

Words used here: **Terminal Psi** = the last portable program form before machine code (internal name);
**lowered-form checker** = stage 06 of source-to-psi, `06_lowered-program-to-checked-program`.

---------------------------------------------------------------------------------------------------

## scalars/

### S/scalars/proof_bearing_scalar/canonical_goal.rs (844 lines)

1. Purpose. Turns a "partial" scalar term (exact add, subtract, multiply, divide, shift, exact cast, wrapping or
   saturating divide) into the one yes/no question that must hold for the term to be well formed (for example
   "the sum fits in the type", "the divisor is not zero"). It only states the question; it proves nothing
   (doc at :44-47).
2. Shape. `enum CanonicalScalarGoal` (6 variants, :11-41); `CanonicalScalarGoal::for_term` (:48),
   `::shape` (:123), `::kernel_proposition` (:140). Private builders: `exact_multiply_` (:213), `exact_subtract_`
   (:281), `exact_add_representability_proposition` (:335), `exact_integer_operands` (:388),
   `integer_math_operand` (:428), `exact_shift_left_` (:488), `exact_cast_representability_proposition` (:546),
   `exact_cast_to_parameter_width` (:615), `exact_shift_count_proposition` (:664), `nonzero_divisor_proposition`
   (:736), `exact_division_defined_proposition` (:780).
3. In/out. Reads `semantic_vocabulary` (`ScalarTerm`, `IntegerType`, `IntegerMathTerm`, `Proposition`) and
   `crate::OperationSemanticError`, `ScalarLeafGoalShape`. Produces a validated `Proposition` or an error. No mutation.
4. Decides by: operation variant of the term (`for_term` match :51-120); integer type identity, sign and carrier
   (`IntegerCarrier::Fixed`, sign Signed/Unsigned, `bits()==1`); literal values (two integer literals give
   `Truth`/`Falsehood` directly via `literal_pair_representability` :469); a literal operand equal to 0 or 1
   (multiply :245-255, add :364-370, subtract :310-315 also `left == right`). Type widths must be closed
   (`concrete_integer` :205) except add/subtract, which use `IntegerMathTerm::carrier_minimum/maximum` over a
   width binder (:278-280 comment), and exact cast to a parameter-width target (:552, :615).
5. Cases by shape. `for_term` handles exactly the 12 term variants listed and returns `Ok(None)` for every other
   term (`_ => return Ok(None)` :119). Exact cast requires the operand to be a `ScalarTerm::Value` or an
   integer literal, else `ExactCastRequiresValueOrLiteralOperand` (:573-575, :632-634). Division, shift,
   multiply require `IntegerCarrier::Fixed` (:218, :671-678, :741, :786).
6. Same job elsewhere. Range-of-integer-type bounds are also built in `S/structural/static_path.rs:27`
   (`runtime_index_bound`) and `S/storage/representation_regions.rs:221` (`representation_region_goal`) and
   `S/scalars/scalar_field_requirements.rs:47` (`integer_range_requirement`); each builds its own
   `LessOrEqual`/`LessThan` conjunction (read).
7. Unused or one-caller. `CanonicalScalarGoal` is pub; its users outside this file were not individually
   counted (it is re-exported through `lib.rs`; tests in `S/scalars/trapping_integer_tests.rs` and
   `symbolic_integer_widths.rs` use it).
8. Stated limits. "a rigid width contributes no guessed range even when its binder is positive" (:203-204).
9. Fail-open risks. none seen. The `Truth` shortcuts (multiply by 0 or 1, add 0, `left == right` subtract) are
   traced: they come after `exact_integer_operands` (:231-236, :293-302, :347-356) and `term.validate()` in
   `integer_math_operand` (:440), so a malformed nested operand is still refused (test at
   `formation_tests.rs:42-54`). One point to note: `exact_subtract_representability_proposition` returns `Truth`
   for `left == right` (:312) after only checking the two operand types, not the value; that is correct for x-x.

### S/scalars/proof_bearing_scalar/formation_tests.rs (116 lines)

Two `#[test]`s (:5, :58). First checks an inner computed operand and a wrapping operand stay in the outer
question (:25-40) and that zero cannot hide a malformed nested term (:51-54). Second pins exact `Proposition`
values for subtract (:69-85), underflow (`Falsehood` :86-95), wrapping divide by zero (`LessOrEqual(1,0)` :96-106,
message "wrapping does not define division by zero"), and `for_term` returning `None` for wrapping add (:107-109).
No decisions of its own.

### S/scalars/scalar_array.rs (78 lines)

1. Purpose. Finds the path to the n-th scalar in a (possibly nested) fixed array of one scalar type, and the
   scalar type plus total count.
2. Shape. `scalar_array_leaf_path` (:4), `scalar_array_leaf_shape` (:39), `scalar_array_leaf_declaration` (:51).
3. In/out. Reads an iterator of `StructuralTypeDeclaration`, `crate::structural_shape/structural_declaration`.
   Returns `Option`.
4. Decides by: declared shape of each level, literal `ArrayExtent::Literal` only (:19, :64); array-ness must be
   seen before a `PrimitiveScalar` (`if array` :71); count is `checked_mul` of lengths, 0 if any length is 0 (:67,:72).
5. Cases by shape. Only `FixedArray` of literal length down to `PrimitiveScalar`; "`_ => return None`" (:74).
   Loop bound is the number of declarations (`0..types.clone().count()` :59), not a depth constant.
6. Same job elsewhere. `S/structural/zeroed_structural.rs:413-421` and `S/storage/zero_valid_storage.rs:29-31`
   call it; `S/storage/representation_regions.rs:85-98` has a separate byte-array-only walk.
7. Counts. `scalar_array_leaf_path` lines=9 files=8; `scalar_array_leaf_shape` lines=38 files=12;
   `scalar_array_leaf_declaration` lines=8 files=5 (grepped).
8. Stated limits. none.
9. Fail-open risks. none seen (every unknown returns `None`).

### S/scalars/scalar_field_requirements.rs (153 lines)

1. Purpose. Defines what a record field of bounded-integer, refined-scalar or atomic bounded type demands of a
   value stored in it, and answers: does this field accept a literal, accept zero, or entail another field type.
2. Shape. `scalar_field_requires_evidence` (:5), `scalar_field_requires_initialization_evidence` (:12),
   `scalar_field_initialization_requirement` (:17), `scalar_field_requirement` (:24), `atomic_resident_requirement` (:37),
   `integer_range_requirement` (:47, private), `scalar_field_accepts_literal` (:65), `scalar_field_accepts_zero` (:97),
   `scalar_field_entails` (:119).
3. In/out. Calls `proof_admission::check_predicate_denotations_with_value_equalities` (:93, :141) and
   `crate::descriptors::descriptor_results::prove_payload_predicate` (:146). Returns bool or `Option<Proposition>`.
4. Decides by: `StructuralFieldType` variant (`BoundedInteger`, `RefinedScalar`, `AtomicBoundedInteger`); scalar
   carrier equality (`field_type.scalar_type() == Some(literal.scalar_type())` :72, :128); proof-kernel result
   (`checked.goal() == Truth` :94; proof certificate check :152).
5. Cases by shape. Literal kinds limited to Boolean, Integer, IeeeFloatLiteral (:69-71); zero for `Callable`
   carrier is refused (:103). No float bound field exists in `scalar_field_requirement`.
6. Same job elsewhere. Used by `S/structural/inline_carriers.rs:89` (`scalar_field_entails`), `zeroed_structural.rs:264`.
7. Counts. Not individually grepped (not flagged as one-caller).
8. Stated limits. none.
9. Fail-open risks. `scalar_field_accepts_literal` returns `true` when the field has no requirement (:88-92
   `else { return true }`): a plain scalar field accepts any literal of its own carrier. traced: the earlier
   guard at :69-75 already requires the literal's type to equal the field's, so the accepted set is exactly
   "same carrier". `scalar_field_entails` returns `true` when the required field has no requirement (:132-134):
   traced the same way (carriers compared at :126-131). Not a hole.

### S/scalars/scalar_leaf_schema.rs (164 lines)

1. Purpose. Plain data describing one "goal-free" scalar operation row: result shape, operand shape, how it
   denotes, its goal, fact, crash and work policy.
2. Shape. Enums `ScalarLeafResultShape` (:2), `ScalarLeafOperandShape` (:9), `ScalarLeafDenotation` (:23, 33
   variants), `ScalarLeafGoalShape` (:60), `ScalarLeafFactShape` (:71), `ScalarLeafCrashPolicy` (:77),
   `ScalarLeafWorkPolicy` (:83, only `ConsumeOne`), `ScalarLeafFrontierPolicy` (:88, only `PreserveLocal`);
   `struct ScalarLeafSchema` (:93); `pub(crate) const fn goal_free_scalar_leaf` (:140); alias
   `GoalFreeScalarLeafSchema` (:138).
3. In/out. No inputs besides constructor arguments.
4. Decides by: `fact` is `ResultEquation` for `BooleanConstant` and for integer/float results, and
   `BooleanResultEquationAndPolarityImplications` for other Boolean results (:150-159). Crash is always `Never`,
   work `ConsumeOne`, frontier `PreserveLocal` (:160-162).
5. Cases by shape. Single-variant policy enums (:83-90) mean no other policy can be expressed.
6. Same job elsewhere. A parallel table for structural/effect operations: `S/structural/structural_effect.rs:313-406`.
7. Counts. Not grepped.
8. Stated limits. none. 9. Fail-open. none seen.

### S/scalars/scalar_leaf_semantics.rs (510 lines)

1. Purpose. Given an `Operation` and the types of its operand values, checks the operation's shape and builds
   its result equation (`result == denotation(operands)`); also evaluates the operation when all operands are
   known constants.
2. Shape. `enum ScalarLeafLiteral` (:12), `trait ValueTypes` (:20) with impls for `BTreeMap` (:24), `arena::IdTable`
   (:30), `SortedValueTypes` (:38); `constant_goal_free_scalar_leaf` (:60); private `enum ScalarLeafInputs` (:195),
   `scalar_leaf_inputs` (:203), `validate_result_shape` (:260), `validate_operand_shape` (:275), `denotation_term` (:335);
   `pub(crate) value_term` (:243), `integer_type` (:253); `GoalFreeScalarLeafSemantics` (:471),
   `goal_free_scalar_leaf_semantics` (:486).
3. In/out. Reads `crate::operation_semantic_row` and `terminal_psi::OperationKind`; produces `Proposition::Equal`.
4. Decides by: operation kind through `OperationSemanticTag` row (:490); value types through `ValueTypes`;
   address-integer exclusion for mixed-type compares (`!left.is_address() && !right.is_address()` :308); `can_widen_to` (:321).
5. Cases by shape. `constant_goal_free_scalar_leaf` folds only: BooleanNot, BooleanEqual, IntegerEqual/LessThan/
   LessOrEqual, IntegerBitwiseNot/And/Or/Xor, IntegerWiden, wrapping/saturating add/sub/mul, wrapping shifts;
   `_ => return None` (:190). Division is absent (it has a goal).
6. Same job elsewhere. `terminal_psi::OperationKind` evaluation also exists in the interpreter (suspected; not read).
7. Counts. `constant_goal_free_scalar_leaf`: 0 call sites (only the `lib.rs:226` re-export and the definition; grep
   lines=2). `SortedValueTypes` lines=7 files=4. `ValueTypes` impl for `BTreeMap` is used by the constant folder
   (:62-63 takes `&BTreeMap` not `&dyn ValueTypes`).
8. Stated limits. none.
9. Fail-open risks. `constant_goal_free_scalar_leaf` returns `None` (no constant) on any failure, including a bad
   shape (`.ok()?` :65, :69-70): the caller sees "not a constant", not "accepted". traced: no call site, so no
   effect today.

### S/scalars/scalar_observations.rs (319 lines) and S/scalars/scalar_observations/tests.rs (159 lines)

1. Purpose. Three things: the equation for reading a catalog scalar application; the lower bound on a storage
   alignment observation; and the scalar type a path of read projections ends at.
2. Shape. `scalar_application_read_equation` (:18), `storage_alignment_fact` (:47), `enum Target` (:104),
   `structural_read_scalar_type` (:110), `structural_read_type_with_extents` (:120).
3. In/out. Reads `ScalarApplications`, `TerminalMachine::producing_operation`, `crate::ReferenceStorageGeometry`,
   `projection_stays_in_inline_storage`. Returns `Result<Option<Proposition>,&'static str>` or `Option<ScalarType>`.
4. Decides by: declared catalog carrier equality (:32); producer operation kind in an 8-kind list (:67-76); inline
   storage reachability (:77); layout and region alignment (:86-90); type declarations through a unique-match
   `declaration` closure (:126-130); path segment kind; write-only access (:143, :158, :268, :288).
5. Cases by shape. `storage_alignment_fact` gives a real guarantee only when the source's producer is one of
   `EstablishPrimitiveLocal, EstablishStructuralLocal, EstablishZeroedStructural, EstablishScalarArray,
   EstablishStructuralArray, EstablishRecord, EstablishScalarCase, EstablishStructuralCase` (:67-76); otherwise the
   fact is `1 <= alignment` (`unwrap_or(1)` :96). The path walk handles `Referent`, `Extent`, `Case`, `Field`,
   `FixedIndex`, `Index`; static `Referent` is refused (:135).
6. Same job elsewhere. Path walkers: `S/structural/primitive_place.rs:826` `walk_primitive_projection_with_lookup`
   and `S/structural/static_path.rs:263` `canonical_projection_type_with_lookup` each walk a similar path grammar
   (read); they differ in segment alphabets (`ScalarReadProjection` here, `StructuralPathSegment` /
   `CanonicalStructuralPathSegment` there).
7. Counts. `storage_alignment_fact` lines=3 files=3 (1 real caller: stage 06 `lib.rs:1668`).
   `scalar_application_read_equation` lines=8 files=5. `structural_read_type_with_extents` lines=5 files=4.
   `structural_read_scalar_type` lines=12 files=4.
8. Stated limits. "This supplies no numeric measurement: layout and evaluation still owe their own admission." (:16-17);
   "A cap is not a floor." (:84).
9. Fail-open risks. `storage_alignment_fact` always returns a positive-guarantee `Some` (:92); the weakest case is
   `1 <= result`, true of any alignment. stated: the doc comment "Every observation returns a positive guarantee."
   (:43). traced: `declared_minimum()` returns `Some(n>1)` only for a fresh inline allocation (:65-91), never for a
   borrowed referent, so no wrong stronger fact. Tests cover extent crossings (tests.rs:70-159).

### S/scalars/scalar_ordering.rs (637 lines) and S/scalars/scalar_ordering/contracts.rs (88 lines)

1. Purpose. A small forward dataflow over a machine's blocks that tracks `u64` index relations (`a < b`,
   `a <= b`), literal values, and values held in storage, so later checks can ask "is x known to be < y here" and
   "is this value unchanged between two points".
2. Shape. `Effect` (:12), `Binding` (:45), `Edge` (:52), `Step` (:61), private `Term`/`Relation`/`State` (:69,:81,:98);
   `State::{known_unsigned :107, proves :135, kill :198, intersect :211, assume :240, after :254, across :364}`;
   `pub struct ScalarOrdering` (:465) with `new` (:471), `with_contracts` (:479), `known_unsigned` (:566),
   `proves` (:574), `values_unchanged_since` (:588). In `contracts.rs`: `index` (:7), `relations` (:23),
   `instantiate_contract_ordering` (:56).
3. In/out. Input is a flat `Vec<Step>` (position, definitions, effect, successor edges), the entry position,
   parameters, entry requirement propositions, and per-return guarantees. Output is a per-step `State` table.
4. Decides by: value identity (`ValueId`) and `u64`-typed values only (`is_index` :230); positions as
   `(BlockId, u32)`; set intersection at joins (:211-228). `proves` is a graph reachability over relations plus
   literal edges (:135-196).
5. Cases by shape. Only unsigned 64-bit values (`index` in contracts.rs:7-21; `Term::Unsigned(u64)`); relation
   kinds `Equal`, `LessThan`, `LessOrEqual` (contracts.rs:32-34). Effects handled: Compare, Not, UnsignedLiteral,
   Establish/Store/ReadStorage, ForgetStorage (:12-42).
6. Same job elsewhere. Dense index/range reasoning also appears in `S/borrowed_storage.rs:998` (`window_excludes_index`
   consumes `ScalarOrdering`). A second ordering use is in `psi-to-native/.../optimization-unit-semantics/src/current_ownership.rs:1813`.
7. Counts. `with_contracts` lines=3 files=2 (one outside caller, `current_ownership.rs:1813`);
   `instantiate_contract_ordering` lines=3 files=3 (one outside caller, `current_ownership.rs:1868`). `ScalarOrdering::new`
   not separately grepped.
8. Stated limits. none.
9. Fail-open risks. `with_contracts` returns an object whose `states` are all `None` if the step list is
   inconsistent (edge target out of range, duplicate positions, entry absent: :491-504). Queries then return
   `None`/`false` (`known_unsigned` :566-572, `proves` :574-586, `values_unchanged_since` :600-602). traced: all
   queries return "unknown" in that case, which refuses, not accepts. `across` returns `Self::default()` (empty
   knowledge) on a bad edge binding (:385-397). `intersect` keeps only common facts. Reverse risk: in
   `ScalarOrdering::with_contracts` an edge to an unreached step still decrements `remaining[...]` only for
   reachable steps (:519-527), so unreachable steps stay `None`. none seen as accepting.

### S/scalars/symbolic_integer_widths.rs (97 lines), S/scalars/trapping_integer_tests.rs (129 lines)

Tests only. `symbolic_integer_widths.rs`: three `#[test]`s (:18, :46, :77) assert a parameter-width integer yields
no storage width (`reference_storage_byte_width` -> `None` :22), no reference view (:30-34), and that
`CanonicalScalarGoal` for cast/shift/nonzero-divisor with a parameter width returns `UnresolvedIntegerWidth` (:69-72);
and a 1-bit signed divisor stays `LessOrEqual` (:92-95). `trapping_integer_tests.rs`: one `#[test]` (:12) loops sign
x width in {1,13,128} x 8 operations and asserts `trapping_integer_semantics(...).completes()` equals the exact
operation's `canonical_goal()` and the result equation (:113-114), and that an empty type table or a Unit result
errors (:117-125). Neither file decides anything.

### S/scalars/unchanged_scalar.rs (43 lines)

1. Purpose. Finds the single origin identity a scalar value "keeps" across block-parameter merges.
2. Shape. `enum UnchangedScalarOrigin<Identity>` (:3, `Known` / `Incoming(Vec<ValueId>)`),
   `unchanged_scalar_identity` (:8) with an inner recursive `resolve` (:12).
3. In/out. Takes a callback `origin(ValueId)`; returns `Option<Identity>`.
4. Decides by: all incoming values must resolve to equal identities (:31). A cycle (value already on `active`) returns
   `None` (:21-23). Results are memoised in `completed` including `None` (:39).
5. Cases by shape. none (generic).
6. Same job elsewhere. none found.
7. Counts. `unchanged_scalar_identity` lines=4 files=4 (grepped).
8. Stated limits. none.
9. Fail-open risks. none seen. A cycle gives `None`, not an accepted identity.

---------------------------------------------------------------------------------------------------

## static_reach.rs

### S/static_reach.rs (467 lines)

1. Purpose. Works out which services each machine can reach, composing calls, callback substitutions, boundary
   calls and service instructions. A callback that is not yet known becomes a "parameter" in the answer.
2. Shape. `DerivationError` (:17), `CallContext` (:23), private `Contribution` (:72); `substitute` (:99),
   `validate_arguments` (:125), `derive` (:159), `applied_reach` (:393), `template_commitment` (:410).
3. In/out. Reads `TerminalModule` (machines, `declared_service_reach`, `execution.callables`, `interfaces.services`,
   `module_invocations()`, `boundary_invocations()`, `callable_invocations()`), plus `StaticReachApplication` and
   `StaticReachParameter` rows. Returns `Vec<StaticReachSummary>` or an error.
4. Decides by: machine identity via `binary_search` on `module.machines` (:91), which requires strict order (:165-175);
   invocation site `(caller, site)`; `StaticReachParameter {owner, parameter, requirement}` triple; whether a machine
   is "fixed" (has no blocks or has a declared reach, :231).
5. Cases by shape. A projection (callback requirement) must have no blocks and no declaration (:185). An
   abstract-callback call must carry its own originating parameter (:240-268). A call with a `required_contract`
   may not also carry a substitution (:274-276).
6. Same job elsewhere. `terminal_psi::static_reach` supplies the term types; stage 06 `static_reach.rs:130` calls
   `derive`, `:341` calls `applied_reach`.
7. Counts. `applied_reach` lines=2 files=2 (one real caller); `derive` has one caller (stage 06 `static_reach.rs:130`);
   `template_commitment` lines=43 files=11 (name is shared with other crates; not separated).
8. Stated limits. none; doc line 2 says "An open callback contributes a projection, never an empty service row."
9. Fail-open risks. In `derive` a machine with no blocks and no declared reach takes `abstract_reach` then the
   callable `machine_envelope`; if both give `None`, its row stays `None` (:217-229, :374). A call to a `None`
   row errors ("open call reach has no originating static parameter" :362). traced: unknown rows error, they are not
   treated as empty. `union`/`dedup` fixpoint (:345-383) terminates because rows only grow.

---------------------------------------------------------------------------------------------------

## storage/

### S/storage.rs (20 lines)

Module list. `pub mod place_paths, type_graph, activation_storage, borrowed_storage, run_custody`;
`pub(crate)` for `arrival_storage, calling_storage, collection_backing, element_representation,
representation_regions, storage_dependencies, storage_regions, structural_locals, zero_valid_storage`. No logic.

### S/storage/activation_storage.rs (116 lines)

1. Purpose. Checks that an `EstablishActivationClaim` operation has the exact shape the activation model needs.
2. Shape. `ActivationClaimShape` (:8), `activation_claim_shape` (:13), `activation_claim_shape_in` (:20); re-exports
   `validate_flow, validate_machine, validate_module` from `flow` (:116).
3. In/out. Reads the operation, its structural result, and the structural type table. Returns claim type and field.
4. Decides by: type identities from the operation (`backing_type`, `established_case`, `rejected_case`); the
   backing must be exactly `ByteSequence(BoundedOwned{capacity: bound})` with all other flags clear (:47-58);
   attempt type is a `Sum` with exactly two cases, the rejected case empty (:61-75); the claim type is a linear
   exclusive `BorrowedView{MutableBorrow}` byte descriptor (:85-97); one claim binding at path
   `[Case(established), Field(claim)]` (:98-108).
5. Cases by shape. Exactly two outcomes, exactly one payload field, exactly one binding (:64-66, :76, :98).
6. Same job elsewhere. none.
7. Counts. `activation_claim_shape` lines=5 files=5; `activation_claim_shape_in` lines=5 files=3 (grepped).
8. Stated limits. none. 9. Fail-open. none seen (all mismatches return `Err`).

### S/storage/activation_storage/flow.rs (1294 lines) and flow/batch_tests.rs (116 lines)

1. Purpose. Walks every control path of a machine, tracking live activation claims and the loans captured from
   them, and refuses programs that break serial claim rules (release order, suspension while a claim is pending,
   copy, transfer). Returns the largest stack demand.
2. Shape. Private `Place` (:12), `Live` (:17), `Loan` (:28), `State` (:35), `ModuleActivation` (:422);
   `validate_machine` (:407), `validate_module` (:414), `validate_flow` (:515, the worklist loop :586-1289);
   helpers `contains_claim` (:255), `field_segment` (:307), `referenced_subject` (:338), `call_arguments` (:377),
   `may_suspend` (:429).
3. In/out. Reads `TerminalModule` (machines, structural types, terminators and cleanup rosters). Produces
   `Result<u64,&'static str>` (maximum co-live claim bytes, via `resource_bounds::compose_claim_bounds` :1129-1141).
4. Decides by: place `(PlaceId root, Vec<StructuralPathSegment>)` with prefix test `under` (:47); operation
   identity `site` (`OperationId`); claim identity (`ClaimId`); machine identity for suspension.
5. Cases by shape (many fixed refusals, quoted): "live activation claims at suspension exceed this serial
   realization" (:599); "simultaneous live instances of one activation site exceed this serial realization" (:635);
   "activation-derived structural call result needs a retained origin mapping" (:611); "activation-bearing store needs
   proof of activation-local destination storage" (:941); "activation-bearing content transformation exceeds this serial
   realization" (:999); only operation kinds in the `match` at :627-1101 change state; every other operation falls to
   `_ => {}` (:1101); an activation-bearing structural result from any operation other than 9 listed kinds is refused
   (:1104-1127).
6. Same job elsewhere. Loan/capture tracking resembles `S/storage/borrowed_storage.rs` regions and
   `descriptor_lifetimes` (not in this unit); suspected duplication, not read.
7. Counts. `validate_flow` lines=4 files=3; `ModuleActivation` is private (1 file). `validate_module`/`validate_machine`
   are re-exported (name too generic to grep usefully).
8. Stated limits. "exceed this serial realization" appears at :599, :635, :999, :1035, :1074.
9. Fail-open risks. `_ => {}` at :1101: an operation kind not in the match changes no activation state. traced:
   guarded by the check at :1104-1127 (an operation producing a claim-containing result must be one of 9 listed
   kinds) and by the call-argument checks at :603-626 (all `call_arguments` kinds); a non-listed operation that
   merely reads or borrows a claim-carrying place is not seen here (open: which other operation kinds carry a
   `StructuralArgument` into a claim place without `call_arguments`, `Establish*Reference` or listed kinds;
   not enumerated). In the `Conditional` terminator, if the condition is not in `observations` the edge selects
   nothing (:1180-1182): pending claims stay pending, which later refuses at a suspension or release
   (traced for suspension :596-601 and release :852).
   `batch_tests.rs` pins the order of global versus per-machine errors (:53-115).

### S/storage/arrival_storage.rs (476 lines)

1. Purpose. Answers questions about how a machine's structural parameters arrive: which type/access a place has,
   what permission a projected path reaches, and what name a block parameter had before an edge.
2. Shape. `view_descriptor_projected_qualifications` (:7), `view_descriptor_parameter` (:24),
   `borrowed_storage_parameter` (:60), `borrowed_view_payload_parameter` (:78), `borrowed_storage_reference_ceiling`
   (:93), `descriptor_parameter_access` (:112), `arrival_parameter` (:142), `structural_argument_source` (:202),
   `structural_projection_authority` (:297), `root_before_edge` (:392), `projection_before_edge` (:402),
   `StorageNames` (:444), `storage_names` (:458).
3. In/out. Reads `TerminalMachine` (places, parameters, blocks, `control_graph::forwarded_bindings`,
   `reaches_by_case`), type declarations. Returns `Option` or small structs; `storage_names` returns per-block root sets.
4. Decides by: place identity and `StructuralPlaceKind` (Parameter / BlockParameter / OperationResult /
   ByteSequenceLiteral / TrivialAffineLocal, :150-191, :211-282); unique-declaration lookup (`matching.next()...is_none()`
   :19, :43, :73, via `exactly_one` :83); access kind (`Owned`, `SharedBorrow`, `MutableBorrow`...).
5. Cases by shape. `structural_argument_source` supports exactly four root kinds (parameter, operation result,
   byte-sequence literal, trivial affine local); `_ => return None` (:281). `projection_before_edge` demands that every
   forwarded binding to the target block project the same place or returns `None` (:436-438).
6. Same job elsewhere. `view_descriptor_parameter` and `borrowed_storage_parameter` are complementary
   (:24-58 vs :60-74); `crate::descriptor_results::descriptor_value_access` is used for the affine case.
7. Counts. `arrival_parameter` lines=104 files=34; `structural_argument_source` lines=19 files=10;
   `descriptor_parameter_access` lines=22 files=10; `structural_projection_authority` lines=9 files=7;
   `storage_names` lines=7 files=4; `borrowed_view_payload_parameter` lines=6 files=5;
   `borrowed_storage_reference_ceiling` lines=13 files=5; `projection_before_edge` lines=11 files=5;
   `root_before_edge` lines=6 files=3 (grepped).
8. Stated limits. "This grants no loan: resource claims and result qualification establishments still require the
   consumer's own checks." (:195-197); "Availability and live custody remain separate obligations." (:200-201).
9. Fail-open risks. none seen. `view_descriptor_projected_qualifications` returns `true` when `projected` is empty
   (:12-14): traced, an empty list asserts nothing.

### S/storage/borrowed_storage.rs (1061 lines), borrowed_storage/tests.rs (276 lines), borrowed_storage/write_frames.rs (200 lines)

1. Purpose. Models "where borrowed storage points": a `Projection` (a path with address bookkeeping: runtime index
   capture positions, windows, whether it crosses a reference) and a `Region` (root place + projection) with overlap
   tests; plus the list of referents a callee could write through, and per-call write regions from published
   frames. `write_frames.rs` infers each machine's write frame and checks any frame a module publishes.
2. Shape. `carried_exclusive_referents` (borrowed_storage.rs:24), `call_write_regions` (:116), `struct Projection`
   (:241) with fields `address: Option<Vec<AddressStep>>`, `descriptor: DescriptorPrefix`, `path`, `exact`, `indirect`,
   `window`, `captured_at`; `enum AddressStep` (:266); Projection methods `new :284, at :323, captured_storage_path :347,
   validate_captured_storage_address :367, returned_element_addresses :425, captured_address_is_prefix_of :515, window :560,
   with_window :578, without_runtime_coordinates(_with) :598/:602, enclosing_storage :697, forget_address :719,
   original_descriptor_path :733, reference_origin_path :749, stripped_origin_path :762, origin_path_with :774, then :841`;
   `Region` (:950) with `overlaps` (:965); `regions_overlap_at` (:978), `window_excludes_index` (:998), `resolve` (:1028).
   write_frames.rs: `WriteFrameError` (:9), `inferred_write_frames` (:35), `derive_write_frames` (:41),
   `frame_owner` (:96), `machine_write_frame` (:106), `validate_write_frames` (:147).
3. In/out. Reads type declarations, `ScalarOrdering` (for window/index disproof), `terminal_psi::direct_call_parts`,
   `descriptor_results` graph. Produces regions or `IdTable<MachineId, PublishedWriteFrame>`.
4. Decides by: place identity (`PlaceId`) and structural path equality/overlap (`structural_paths_may_overlap`);
   runtime index capture position `(BlockId,u32)`; for write frames, machine identity by *table position*
   (`frames.push(None) != machine.id` :47, `frame_owner` indexes `machines[id-1]` :100-102).
5. Cases by shape. `carried_exclusive_referents` returns `None` for recursive types, nesting beyond 64
   (`active.len() > 64` :40), arrays longer than 4096 (`*length <= 4096` :68), and an exclusive view of values carrying
   references (:81-84). `Projection::then` refuses composition beyond 4096 steps (:874). `call_write_regions` returns
   `Ok(None)` ("no precise frame") when the callee has no `published_writes`, when the path crosses a Referent or
   representation view, when the region is `indirect`, or the shared-mutation test is `Unbounded` or the tip is a
   reference/view/byte sequence (:134-136, :197-226, :229-231).
6. Same job elsewhere. Overlap: `S/storage/place_paths.rs:1` re-exports `terminal_psi::...::may_overlap` (resolved
   segments) while `S/structural/static_path.rs:86` `structural_paths_may_overlap` does it on raw segments; both
   treat `RuntimeIndex` as overlapping everything and `RepresentationView` as overlapping (read both). Capture
   tracking of dynamic indices appears in `AddressStep` and again as `index_captures` (:498).
7. Counts. `carried_exclusive_referents` lines=3 files=3 (two real callers: stage 05 `lower/calls.rs:280`, stage 06
   `places/log.rs:615`); `call_write_regions` lines=4 files=4; `inferred_write_frames` lines=9 files=5;
   `validate_write_frames` lines=6 files=5 (grepped).
8. Stated limits. "`None` when the referents are not a finite list of exact paths" (:21-23); "Later local summaries may
   strengthen scalar entry and result facts. They must not silently narrow a published storage-effect contract."
   (write_frames.rs:72-75); "A bodyless header cannot invent an empty or narrowed write promise." (:146).
9. Fail-open risks. `Region::overlaps` returns `true` (conflict) when either root is `None`/indirect (:965-974);
   `resolve` falls back to a `root: None` region when aliases cycle or are empty (:1052-1057): conservative
   (reports overlap). `window_excludes_index` returns `false` (not excluded) unless every capture condition holds
   (:1004-1018): conservative. Write-frame inference: `derive_write_frames(module, &|_| Ok(()))` for
   `inferred_write_frames` ignores unreadable bodies (`IncompleteMachine` -> `unreadable(error)?` -> `Ok`, :63, :38);
   stated: doc at :33-35 "An unreadable body cannot publish a precise write frame." (they get frame `None`).
   `validate_write_frames` turns that into an error only for bodies that publish a frame (:166-181). traced.
   Tests (tests.rs) cover case-payload overlap, stripped selectors, prefix refusal, reference crossings, slices.

### S/storage/calling_storage.rs (181 lines), calling_storage/tests.rs (178 lines)

1. Purpose. Finds which scalar applications (array-length extents) the layout of a boundary call's by-value data
   depends on, for a target.
2. Shape. `foreign_storage_extent_dependencies` (:8), `calling_abi_extent_dependencies` (:100),
   `callback_storage_has_slots` (:126), `storage_has_slots` (:134); re-exports `ForeignRecordStorage`,
   `foreign_opaque_carrier`, `foreign_record_storage` from `terminal_psi::layouts::record_storage` (:4-6).
3. In/out. Reads `TerminalModule` types, opaque carriers, native layouts. Returns sorted `ScalarApplicationId` list
   or `String` error.
4. Decides by: type identity and target string (`target: &str`, e.g. "macos_arm64" in tests :60); shape of each
   declaration; a record field's selected target storage `stored_scalar()` (:59-62).
5. Cases by shape. Boundary by-value data may be scalars, atomics, references, byte sequences, views, fixed arrays,
   records; `Sum`/`Mixed` return "case data is not yet classifiable as a boundary value; pass it by reference" (:71);
   `TypeParameter`/`Opaque` refused (:36-43); recursion refused (:24-26). Callback record walk limited to 128 deep (:140).
6. Same job elsewhere. `S/storage/storage_dependencies.rs:7` `inline_storage_extent_dependencies` collects array
   extents for inline storage with a similar walk (read both). Native layout does the reverse side.
7. Counts. `foreign_storage_extent_dependencies` lines=7 files=3; `calling_abi_extent_dependencies` lines=4 files=4;
   `callback_storage_has_slots` lines=7 files=3 (grepped).
8. Stated limits. "case data is not yet classifiable as a boundary value" (:71).
9. Fail-open risks. none seen (errors for every unclassifiable shape). Tests: depth 131 is accepted (:48-64),
   opaque carrier, target stored scalar skips the logical child (:116-178).

### S/storage/collection_backing.rs (176 lines)

1. Purpose. Classifies a collection type's backing (inline or borrowed; fixed, live-length, stored-length or
   parameter extent) and whether a view over it keeps element coordinates.
2. Shape. `CollectionStorage`, `CollectionExtent`, `CollectionBacking` (:6-28), `CollectionViewDescriptor` (:31),
   `collection_view_descriptor` (:36), `collection_backing` (:71), `collection_shape_backing` (:80),
   `collection_view_backing` (:135), `collection_shape_view_backing` (:145), `collection_view_preserves_coordinates` (:168).
3. In/out. Reads type declarations and `crate::element_representation_relation` (:161).
4. Decides by: type shape and element identity (`actual == element`), byte-sequence carrier variant, "u8 that is not
   an address integer" test (:55-58, :109-112).
5. Cases by shape. Supports `ElementView`, `FixedArray` (literal or parameter length), and `ByteSequence` over `u8`
   only; `_ => None` (:131). A `BoundedView` byte descriptor is a declared-length view only if element is `u8`, region
   `WholeCollection`, no trailing `FixedRange` (:48-65).
6. Same job elsewhere. The "unsigned 8-bit non-address integer" test is repeated in
   `S/storage/representation_regions.rs:90`, `S/structural/zeroed_structural.rs:429-432` and `:557-560` (read).
7. Counts. `collection_view_backing` lines=2 files=2 (0 call sites: `lib.rs:323` re-export plus definition);
   `collection_view_preserves_coordinates` lines=4 files=4; `collection_view_descriptor` lines=8 files=7 (grepped).
8. Stated limits. none. 9. Fail-open. none seen.

### S/storage/element_representation.rs (250 lines)

1. Purpose. Computes the byte layout (size, alignment, scalar leaves with offsets and bounds) of a record/array
   element, and judges whether a source element's bytes may be viewed as a target element.
2. Shape. `ElementRepresentationLeaf/Layout/Relation` (:9-25), `scalar_size` (:27), `element_representation_layout` (:61,
   inner `visit` with budget 16384 :170), `bit_intervals` (:172), `element_representation_relation` (:212).
3. In/out. Reads types and `crate::reference_storage_layout`/`reference_storage_projection` (:111, :153-156).
4. Decides by: scalar size roster (Boolean, non-address 8/16/32/64 integers, f32, f64 :27-38); offset equality
   leaf by leaf; a leaf's bit-interval containment (source interval inside target interval :237-241); for
   `MutableBorrow` the intervals must be equal (:241).
5. Cases by shape. Elements must be `PrimitiveScalar`, literal `FixedArray`, or `Record` with fields of scalar,
   bounded integer, float or structural children; erased fields or other field types refuse (:114-116, :130). Access
   must be a reading borrow pair (:218).
6. Same job elsewhere. `S/storage/representation_regions.rs:45` `representation_element_layout` wraps this layout
   dropping bounded/Boolean leaves.
7. Counts. `element_representation_relation` lines=6 files=6 (grepped).
8. Stated limits. none. Recursion depth limit 128 (:72) and leaf budget 16384 (:170) are constants.
9. Fail-open risks. none seen; each mismatch returns `None`.

### S/storage/place_paths.rs (34 lines)

Re-exports `terminal_psi::types::descriptor_locations::may_overlap` (:1) and one `#[test]` (:9) pinning that a
representation view overlaps original fields but outer siblings stay disjoint (:16-32). No logic here.

### S/storage/representation_regions.rs (254 lines)

1. Purpose. Geometry for reinterpreting a collection as another element type (`WholeRepresentation`,
   `RepresentationBytes`) and the proof goals those formations owe (no `u64` overflow, ranges and strides).
2. Shape. `RepresentationElementLayout` (:18) with `is_tightly_tiled` (:25), `representation_element_layout` (:45),
   `representation_byte_extent` (:72), `ElementViewRegionGeometry` (:101), `element_view_region_geometry` (:113),
   `element_view_shape_region_geometry` (:124), `whole_representation_count_goal` (:189),
   `whole_representation_region_goal` (:204), `representation_region_goal` (:221), `representation_region_length` (:240).
3. In/out. Reads `crate::element_representation_layout`, `collection_shape_view_backing`. Produces `Proposition` goals.
4. Decides by: element layout size/alignment divisibility (:155-162) and tight tiling (:163); region kind.
5. Cases by shape. `WholeRepresentation` is only for `SharedBorrow` and source `ElementView` or `FixedArray` (:139-146);
   leaves must be non-Boolean, non-bounded scalars (:50-56); byte extent only for `u8` `FixedArray` or `BoundedView`
   (:85-96).
6. Same job elsewhere. Goal construction as in `S/scalars/proof_bearing_scalar/canonical_goal.rs` (see there).
7. Counts. `representation_region_goal` lines=3 files=3 (one real caller `unit_validation.rs:1977`);
   `representation_region_length` lines=2 files=2 (0 call sites, re-export + definition);
   `whole_representation_region_goal` lines=3 files=3 (one real caller `obligation_evidence.rs:453`);
   `whole_representation_count_goal` lines=6 files=5 (grepped).
8. Stated limits. none. 9. Fail-open. none seen (`factor == 0`/`stride == 0` return `None` :193, :227, :245).

### S/storage/run_custody.rs (78 lines)

1. Purpose. Validates the shape of the one boundary declaration "empty-run release" that returns a run's extent.
2. Shape. `EmptyRunReleaseContract` (:8), `empty_run_release_contract` (:15), `unsigned_count` (:71).
3. In/out. Reads a `BoundaryMachineDeclaration` and types. Returns field ids/identities.
4. Decides by: `declaration.execution == CheckedBoundaryExecution::EmptyRunRelease` (:19-22); exactly one owned linear
   parameter and one linear structural result (:25-36); the run record has exactly two fields and the extent record
   exactly two (:42-58); count/length fields must be unsigned 64-bit.
5. Cases by shape. Fixed two-field records (`let [storage, count]`, `let [_base, length]` :42, :57).
6. Same job elsewhere. none.
7. Counts. `empty_run_release_contract` lines=7 files=7 (grepped).
8. Stated limits. none. 9. Fail-open. none seen.

### S/storage/storage_dependencies.rs (153 lines)

1. Purpose. Lists which type declarations and array-length applications an operation's storage use depends on.
2. Shape. `inline_storage_extent_dependencies` (:7), `storage_operand_dependencies` (:55).
3. In/out. Reads `BTreeMap<StructuralTypeId,&StructuralTypeDeclaration>` (a map keyed by identity, :9, :57), machine place
   types, `projection_storage_dependencies`, `canonical_storage_dependencies`. Mutates `roots: &mut BTreeSet`.
4. Decides by: `StructuralStorageUse` variant (Place, Type, Projection, Canonical, ObservedField, Element).
5. Cases by shape. 6 storage-use variants handled exhaustively (:67-151).
6. Same job elsewhere. `S/storage/calling_storage.rs:8` (see there).
7. Counts. `inline_storage_extent_dependencies` lines=3 files=3 (one real caller, interpreter `execution.rs:685`);
   `storage_operand_dependencies` lines=6 files=4 (grepped).
8. Stated limits. none. 9. Fail-open. none seen (`.ok_or_else(... "malformed storage projection")`).

### S/storage/storage_regions.rs (99 lines)

1. Purpose. Turns an operation's storage path into the "region" it touches (static prefix, plus whether it crosses
   an unaliased reference), and a canonical field region.
2. Shape. `StorageRegion` (:8) with `direct_path` (:14), `crosses_unaliased_reference` (:19), `projected_storage_region` (:25),
   `canonical_field_storage_region` (:58).
3. Decides by: path segment kinds; the first `RuntimeIndex`, `RepresentationView` or `Referent` ends the static prefix
   (:31-41); `indirect` = a `Referent` after the first segment (:19-23).
4. Cases by shape. `canonical_field_storage_region` returns an empty `Vec` on `Case`/`Referent` or an unresolvable
   field (:73-76, :81-93).
5. Same job elsewhere. Region building also in `borrowed_storage.rs:284` (`Projection::new`).
6. Counts. Not grepped.
7. Fail-open risks. `canonical_field_storage_region` returns an empty vector for failure (:75, :86, :89, :92): an
   empty region means "no path", and callers check `relative.len() != fields.len()` (`borrowed_storage.rs:185`) to
   refuse. traced for that one caller; others open (not enumerated).

### S/storage/structural_locals.rs (50 lines)

1. Purpose. Checks that a vacant structural local (a reserved home) declares no custody and only memberships
   that describe the later stored value.
2. Shape. `validate_vacant_structural_local` (:11).
3. Decides by: type declaration lookup unique (:15-23); shape (byte view, record/sum/mixed, reference, array/view);
   claims/establishments empty (:24); projected memberships by shape (:35-45).
4. Cases by shape. Reference: memberships must start with `Referent`; array/view: must be element memberships; other:
   none (:36-45).
5. Counts. `validate_vacant_structural_local` lines=5 files=5 (grepped).
6. Fail-open risks. none seen.

### S/storage/type_graph.rs (385 lines)

1. Purpose. A sorted-row view of the structural type table for cycle, reachability and containment checks along
   by-value ownership (records, case payloads, arrays), not through references or views.
2. Shape. `StorageGraphError` (:12), `StorageTypeGraph` (:19) with `new :24, storage_reachable_from :45, reachability :57,
   storage_dependents :65, validate_acyclic :95`; `StorageReachability` (:135); `for_each_owned_child` (:163); tests (:206-385).
3. In/out. Borrows declarations; produces sorted id lists or `StorageGraphError{MissingType,DuplicateType,Cycle}`.
4. Decides by: type identity via binary search over sorted rows (:37-41); iterative DFS with explicit stack (:109-128).
5. Cases by shape. Owned children = record/sum/mixed fields with structural type and `FixedArray` element; references,
   views, byte sequences, atomics, scalars, opaques own nothing (:176-202).
6. Same job elsewhere. `S/structural/owned_structure/complete_claims.rs:79` `compute_claim` and
   `S/structural_qualifications.rs:1114` `type_has_defaults` and `zeroed_structural.rs:118` `zeroed_declaration_graph`
   each run their own by-value type walk with their own cycle detection (read). Four independent walkers over the same
   containment relation, with differing erased-field handling: `for_each_owned_child` does not skip erased fields (:169-172)
   while `child_types` in `owned_structure.rs:211` does (:217).
7. Counts. `StorageTypeGraph` lines=17 files=5 (grepped).
8. Stated limits. none. 9. Fail-open. none seen (missing types error).

### S/storage/zero_valid_storage.rs (69 lines)

1. Purpose. Judges whether all-zero bytes are a valid value of a record/array/sum type.
2. Shape. `zero_valid_record_storage` (:7), `scalar_declaration_accepts_zero` (:63).
3. Decides by: declaration shape; the caller's `zero_valid_field` callback for fields; a sum or mixed needs only its
   first case valid (:38-52); `ByteSequence(BoundedOwned)` is zero valid (:56); everything else false (:57).
4. Cases by shape. Record, FixedArray (empty scalar array valid directly), Sum, Mixed, PrimitiveScalar, BoundedOwned
   bytes; `_ => false` (:57).
5. Counts. `zero_valid_record_storage`: 0 external call sites (grep lines=3: re-export `lib.rs:252`, definition, recursion
   :35); `scalar_declaration_accepts_zero` lines=4 files=3.
6. Fail-open risks. none seen; unknown shapes are `false`.

---------------------------------------------------------------------------------------------------

## structural/

### S/structural.rs (25 lines)

Module list (14 submodules, 2 test modules). No logic.

### S/structural/atomic_cell.rs (21 lines)

`atomic_cell_type` (:4): returns the supported `AtomicCellType` at a path, from the field's type if the path ends in a
field, else from the declared tip shape (:9-20). `cell.is_supported()` gates the result (:20). No fail-open.

### S/structural/call_claim_transfer_tests.rs (121 lines)

Three `#[test]`s on `validate_claim_transfers`: current owned siblings under a projected argument transfer exactly
once in order (:19-60); overlapping or duplicate owned arguments cannot duplicate a claim, claim-free copies allowed
(:62-97); independent claims and case paths stay distinct (:99-121). No decisions.

### S/structural/inline_carriers.rs (135 lines), inline_carriers/tests.rs (434 lines)

1. Purpose. A record field of scalar/atomic/byte type may name an "inline carrier" type declaration for its storage.
   These functions find the carrier, validate that the field really satisfies the carrier's contract, and decide whether a
   requested presentation type is compatible.
2. Shape. `InlineCarrierError` (:8), `inline_carrier_declaration` (:13), `validate_inline_carriers` (:50),
   `inline_field_presentation` (:113).
3. In/out. Reads field and carrier declarations (unique lookup `exactly_one`), `crate::scalar_field_entails`,
   `byte_carrier_geometry_matches`.
4. Decides by: type identity (`inline_carrier` id), carrier shape, scalar carrier equality or entailment, atomic cell
   equality plus no scalar restriction on the carrier, byte carrier geometry.
5. Cases by shape. Field type must be Scalar/IeeeFloat/BoundedInteger/RefinedScalar with `PrimitiveScalar` carrier, or any
   field with `AtomicCell` or `ByteSequence` carrier; `_ => None/false` (:46, :102). Structural and erased fields may not have an
   inline carrier (:75-79).
6. Same job elsewhere. `inline_carrier_declaration` (:13-48) and `validate_inline_carriers` (:50-111) both judge field/carrier
   compatibility with two different rules: the first compares scalar types for equality (:29), the second uses
   `scalar_field_entails` against the carrier's restriction (:88-93). Test `stronger_field_predicate_survives_a_weaker_declared_carrier`
   (tests.rs:217-248) pins that difference.
7. Counts. Not grepped (no flag).
8. Stated limits. none. 9. Fail-open. none seen.
   tests.rs: 7 tests; also covers byte literal/default qualifications (:49-188).

### S/structural/nominal_cleanup.rs (292 lines), nominal_cleanup_tests.rs (68 lines)

1. Purpose. Checks that a type's selected cleanup machine is the right shape and always returns normally; and who may
   take apart a wrapped record.
2. Shape. `validate_selections` (:6), `validate_normal_returns` (:44), `owned_target` (:165), `validate_owned_target` (:193),
   `decomposition_authorized` (:215), `decomposition` (:245). Tests (nominal_cleanup_tests.rs :21-68) use real identity strings.
3. In/out. Reads module machines, types, identity table, `control_graph::control_cycle_members`.
4. Decides by: identity (`selection.declaration.identity() == declaration.identity`), copyable/no-code flags, package owner
   of the carrier vs the machine (`closed_conformances::identity_package`, compared with `==`, :235), `Termination::Terminates`.
5. Cases by shape. Cleanup target must have exactly one owned parameter, unit result, no scalar parameters (:198-208).
   Normal-return proof refuses: waiting/suspending/crash routes (:80-85), service reach (:86-92), no blocks (:96), unranked
   cycle (:101), terminators `Crash, ExternalCompletion, InvokeNoNormal, EliminateEmpty` (:106-115), dynamic/boundary calls (:143-153),
   a required contract without `Terminates` (:131). Decomposition: owned, no Referent crossing, no runtime index, record only,
   no erased fields (:251-277).
6. Same job elsewhere. none.
7. Counts. `validate_selections` (cleanup version) lines=4 files=3 (1 real caller foundation_validation.rs:12025);
   `validate_normal_returns` lines=3 files=3 (two callers: foundation_validation.rs:12027, stage 06 lib.rs:484);
   `owned_target` lines=9 files=8; `decomposition_authorized` lines=5 files=3 (grepped).
8. Stated limits. "recursive generated cleanup has no finite completion proof" (:75); "automatic cleanup with service or callback
   authority is not served" (:89); "nominal decomposition of a non-record is not served" (:273).
9. Fail-open risks. `decomposition` only runs the package authorization when the declaration has a `nominal_cleanup`
   selection (:280-290); a record with no selection is decomposable by any machine. traced: stated by the shape of the rule
   ("A record with a selected cleanup may be taken apart by ..." nominal_cleanup_tests.rs:1-4); a record without one has no
   wrapper to protect. `decomposition_authorized` with unequal missing owners: `carrier_owner.is_some() && == machine_owner`
   so two `None` owners are not equal (:235, doc :213-214 "Missing package identities never compare equal").

### S/structural/owned_structure.rs (607 lines), owned_structure/complete_claims.rs (253 lines), complete_claims/tests.rs (92 lines)

1. Purpose. Claims are linear ownership obligations carried by nominal values. These functions list which paths of a
   type hold claims, check that a call transfers exactly the held claims, and check that a consumed parent accounts for
   every descendant claim.
2. Shape. owned_structure.rs: `validate_claim_transfers` (:10), `unknown_owned_claims` (:88), `intrinsic_claim_required` (:95),
   `intrinsic_claim_root_by` (:113), `validate_intrinsic_claim_roots` (:138), `claim_catalogue` (:157), `child_types` (:211),
   `contains_owned_boundary` (:245), `validate_accounted_descendants` (:270), `selected_contains_owned_boundary` (:382),
   `validate_owned_claim_frontier` (:415), `selected_owned_children` (:543), `selected_children` (:553). complete_claims.rs:
   `validate_complete_claim_frontier` (:9), `ClaimTypes` (:43), `compute_claim` (:79), `contains_claim` (:116), `visit` (:129).
3. In/out. Reads type declarations (sorted vectors, binary search), a `selected(path, case)` callback for the active case,
   claim paths. Produces `Result<(), &'static str>` or path sets.
4. Decides by: `owns_linear_claim` flag, `TypeParameter && !copyable` (unknown claims), multiplicity `Linear`, structural path
   prefix; the `selected` callback chooses the live case of a sum (`exactly_one(...selected(path, case.id))`).
5. Cases by shape. `intrinsic_claim_required` is true only for Linear `Record|Sum|Mixed|Opaque` with no owned component
   claim (:100-108). Array walks require closed literal extent (:180, :322, :487; `claim_catalogue` caps rows at `maximum`
   :183). `child_types` skips arrays of length 0 (:237).
6. Same job elsewhere. Type-containment walkers (see type_graph.rs). `contains_owned_boundary` (:245) uses `BTreeMap`/`BTreeSet`
   caches keyed by `StructuralTypeId`, while `ClaimTypes` uses dense sorted vectors; same question (does a type contain an
   obligation) two data structures.
7. Counts. `validate_claim_transfers` lines=22 files=5; `claim_catalogue` lines=5 files=5; `validate_accounted_descendants`
   lines=3 files=3 (two real callers: `current_ownership.rs:8821`, interpreter `execution.rs:2005`);
   `selected_contains_owned_boundary` lines=3 files=2 (one outside use, `current_ownership.rs:3440`, as an alias);
   `selected_owned_children` lines=3 files=2; `contains_owned_boundary` lines=7 files=2; `intrinsic_claim_required` lines=3 files=2
   (one outside caller, stage 05 `lower/places.rs:562`); `intrinsic_claim_root_by` lines=3 files=3;
   `validate_owned_claim_frontier` lines=5 files=3; `validate_intrinsic_claim_roots` lines=10 files=5;
   `validate_complete_claim_frontier` lines=16 files=7 (grepped).
8. Stated limits. "A call transfers every currently held claim beneath an owned argument, exactly once" (:6-7);
   "An unbounded parameter may contain linear claims even when its carrier bound is affine." (:85-86).
9. Fail-open risks. In `validate_claim_transfers`, if a held claim has no owned argument prefix (`selected == None`) no row is
   required and none is consumed (:70-74): such a claim is simply not transferred. traced: this is the meaning of "claim
   stays with caller"; the final `rows.next().is_some()` check (:76-78) refuses surplus rows. `validate_owned_claim_frontier`
   `continue`s for a type with no claim (:477-479). `contains_claim` for an absent type errors (:121-123). `compute_claim` reports a cycle only if encountered while
   `Active` (:94); a type that owns a claim returns early (:88-90) before cycle detection, so a self-containing claim-owning
   type is not flagged here (traced: `type_graph.rs` `validate_acyclic` is the cycle check; not verified to run before this).

### S/structural/partial_affine.rs (460 lines)

1. Purpose. Given paths already moved out of an affine value, computes the remaining live residual paths (with their
   types) that still need cleanup, bounded by `max_residuals`; also tests whether a region is fully vacant.
2. Shape. `structural_region_is_vacant` (:28), `partial_affine_residuals` (:60), `partial_affine_residuals_in_regions` (:80),
   `partition` (:97), `visit` (:163), `fields_residuals` (:345), `region_children` (:395), `child` (:409).
3. In/out. Reads types, moved path set, optional region set, `admitted_case` callback. Produces `Vec<(path, type)>`.
4. Decides by: path prefix relations; no path may be a prefix of another (:109-114); a count pass then an emit pass
   (`visit(... emit=false)` then `true` :134-159) so output is allocated only after the bound is proved.
5. Cases by shape. Record (non-empty), literal `FixedArray` non-zero, `Sum`/`Mixed`; `_ => return None` (:340). A byte
   field residual is `BoundedOwned` (no residual) or other carrier via `inline_carrier_declaration` (:372-375). Erased
   field refuses (:366-368) unless outside the requested region (:360-365).
6. Same job elsewhere. none.
7. Counts. `structural_region_is_vacant` lines=5 files=4; `partial_affine_residuals` lines=7 files=5;
   `partial_affine_residuals_in_regions` lines=15 files=5 (grepped).
8. Stated limits. none. 9. Fail-open. A moved path that equals a field with a moved empty path inside a larger set returns
   `None` for `descendants.len() != 1` (:453-454); everything unrecognised returns `None`. none seen as accepting.

### S/structural/primitive_place.rs (1227 lines)

1. Purpose. One path-walker (`walk_primitive_projection_with_lookup`) over declared storage, and about 35 questions built on
   it: scalar type at a path, canonical (declaration-id) form of a path, whether a projection stays in inline storage, the
   borrowed presentation type, runtime index bound goals, byte-array element access.
2. Shape. Walker `walk_primitive_projection(_with_policy,_with_lookup)` (:759, :805, :826) returning `ProjectionTip`
   (:701; `Declared`, `ScalarField`, `ByteElement`); `enum Position` (:405, not the `(BlockId,u32)` type used in scalar_ordering.rs
   and borrowed_storage.rs); public queries `primitive_place_type :12, fixed_array_place_shape :41, primitive_operation_projection_type
   :70, primitive_projection_type :86, primitive_projection_declaration :101, primitive_projection_restriction :135,
   primitive_read_requirement :157, borrowed_primitive_projection_declaration :170, primitive_read_projection_type :230,
   primitive_read_field :245, canonical_primitive_read_type :279, canonical_primitive_path :305, canonical_primitive_read_path :319,
   canonical_inline_region :354, canonical_static_projection :379, runtime_read_projection_tip :414, runtime_structural_path_tip :433,
   borrowed_storage_field_type :456, borrowed_projection_type :468, runtime_structural_copy_type :493, projection_stays_in_inline_storage
   :526, inline_descriptor_type :560, inline_backing_container :572, StructuralStorageSelection :602, stored_descriptor_paths :654,
   byte_carrier_geometry_matches :715, projection_storage_dependencies :777, preceding_collection_length :1043, scalar_index_bound
   :1081, visit_runtime_index_goals :1112, projection_tip_with_policy :1209`.
3. In/out. Reads the type table via `exactly_one` (unique lookup, :814), inline carrier declarations, descriptor facts
   (`descriptor_results::is_descriptor`). Returns types, ids, canonical paths, goals.
4. Decides by: declaration identity (unique), path segment kind, field identity (`live_field`), field type (`atomic_cell_type`,
   `byte_sequence_carrier`), array length literal vs parameter, scalar carrier. A `may_step` callback lets each caller
   forbid steps (e.g. `projection_stays_in_inline_storage` forbids `Referent`, and indexing through a descriptor :537-551).
5. Cases by shape. Walker segments handled: `RepresentationView`, `FixedRange` (must be followed by a representation view
   :892-894), `Case`, `Referent` on reference types, `Field` on record/mixed/case payload, `FixedIndex`/`RuntimeIndex` on
   `FixedArray`/`ElementView`/`ByteSequence`; `AllElements` and everything else `_ => return None` (:1010). Parameter-length arrays
   reject `FixedIndex` (:963). `visit_runtime_index_goals` supports four extent sources: literal, parameter length,
   `BoundedView` capacity, or a measured live length (:1147-1196); open extents return `OpenExtent` (:1193).
6. Same job elsewhere. Path-walking duplicates: `S/structural/static_path.rs:263` `canonical_projection_type_with_lookup` (canonical
   path alphabet) and `S/structural/record_field.rs:263` `walk_record_fields` (canonical path to runtime path) and
   `S/scalars/scalar_observations.rs:120` (read projections). Four walkers; each with its own segment cases (read).
7. Counts. `primitive_place_type`: 0 call sites (lines=2 = re-export `lib.rs:264` + definition); `fixed_array_place_shape`: 0
   (lines=2); `primitive_read_requirement`: 0 (lines=2); `primitive_operation_projection_type` lines=3 files=3;
   `primitive_projection_declaration` lines=19 files=13; `primitive_projection_restriction` lines=7 files=5;
   `borrowed_projection_type` lines=5 files=4; `runtime_structural_copy_type` lines=4 files=3; `inline_backing_container` lines=3
   files=3; `inline_descriptor_type` lines=6 files=6; `StructuralStorageSelection` lines=4 files=3; `stored_descriptor_paths` lines=6
   files=4; `preceding_collection_length` lines=4 files=4; `scalar_index_bound` lines=4 files=3; `visit_runtime_index_goals` lines=4
   files=4; `canonical_primitive_path` lines=6 files=5; `canonical_primitive_read_type` lines=4 files=4; `canonical_inline_region`
   lines=8 files=7; `borrowed_storage_field_type` lines=4 files=3 (grepped).
8. Stated limits. "Path identity and read typing must recognize the same primitive leaves." (:267); "This supplies neither bounds
   evidence nor permission to access the selected storage." (:466-467); "Only adjacent length observations may intervene" (:1041-1042).
9. Fail-open risks. `primitive_projection_restriction` (:135-155) returns `None` (no restriction) when the field is a plain
   scalar with no evidence required, and otherwise falls to the declaration's `scalar_restriction`; a missing restriction means no
   extra proof is required. traced: `scalar_field_requires_evidence` limits evidence to `BoundedInteger | RefinedScalar` fields
   (scalar_field_requirements.rs:5-9). `walk_primitive_projection_with_lookup`: when `dependencies.is_some()` the walk accepts a
   trailing unresolved `Case` payload (:1013-1015) and parameter-length `FixedIndex` via `canonical_projection_type_with_lookup:344`;
   dependency mode is for type-collection only. open: which caller passes `dependencies` for a path used for access judgments.
   `preceding_collection_length` returns `None` (no length) at the first non-matching operation (`_ => return None` :1069): refuses.

### S/structural/record_field.rs (348 lines)

1. Purpose. Resolves record/sum field declarations from canonical or runtime paths, and decides whether a sequence field
   permits observing its length or contents.
2. Shape. `SequenceFieldObservation` (:9), `sequence_field_allows_observation` (:14), `RecordFieldCarrier` (:51),
   `record_field_carrier` (:56), `CasePayloadCarrier` (:72), `case_payload_carrier` (:79), `observed_field_declaration` (:102),
   `static_field_projection` (:122), `projected_field_declaration` (:152), `runtime_structural_field` (:166),
   `resolved_structural_path` (:182), `observed_field_backing_access` (:243), `walk_record_fields` (:263).
3. In/out. Reads types; produces field declarations, canonical paths, access.
4. Decides by: field/case identity with `live_field_by_id` and `exactly_one`; access of a byte carrier (`access.unwrap_or(SharedBorrow)`
   :24, :35, :41); `WriteOnlyBorrow` blocks content reads but not length (:47-48).
5. Cases by shape. `walk_record_fields` refuses `Referent` to a `WriteOnlyBorrow` reference (:281-283) and any non-structural field
   (`field.field_type.as_structural()?` :340). `resolved_structural_path` refuses `AllElements` and `FixedRange` ("storage projection
   outside plain records and fixed array elements" :212-216).
6. Same job elsewhere. `S/structural/primitive_place.rs` path walker (see there).
7. Counts. `sequence_field_allows_observation` lines=5 files=5; `static_field_projection` lines=6 files=5; `resolved_structural_path` lines=8
   files=7; `observed_field_backing_access` lines=8 files=7; `record_field_carrier` lines=11 files=6; `case_payload_carrier` lines=6 files=5 (grepped).
8. Stated limits. none. 9. Fail-open. `observed_field_backing_access` downgrades a read through a view to `SharedBorrow` (:255-259):
   conservative (weaker access), not a hole.

### S/structural/shared_mutation.rs (146 lines)

`shared_mutation` (:14) classifies a type as `None | Inline | Unbounded` shared interior mutability, by recursion over its shape.
Unknown, hidden or unfound types are `Unbounded` (:26, :43, :46, :51-53); a type seen again while active is `None` (:36-38, the
cycle case) ; `AtomicCell` is `Inline` (:55); a non-shared `Reference` or a reference to anything not `None` is `Unbounded` (:58-62); a byte
view with non-shared access is `Unbounded` (:137-143). Counts: `shared_mutation` lines=7 files=7. Fail-open: a type revisited while `active` returns
`None` (:36-38): traced, the cycle's contribution is taken from the first visit of that type.

### S/structural/static_path.rs (401 lines)

1. Purpose. Static path utilities: runtime index bounds, path overlap, canonical path tips, canonical-to-storage paths, interior borrows.
2. Shape. `constant_runtime_index` (:11), `runtime_index_bound` (:27), `runtime_index_selects` (:61), `structural_paths_select_same_location`
   (:69), `structural_paths_may_overlap` (:86), `interior_member_borrow` (:147), `canonical_structural_path_tip` (:172),
   `canonical_case_leaf_type` (:183), `canonical_structural_storage_path` (:194), `canonical_storage_dependencies` (:204),
   `visit_canonical_inline_carriers` (:225), `canonical_projection_type(_with_lookup)` (:253, :263), `structural_memberships_disjoint_from_write`
   (:367), `qualification_path_carrier` (:378).
3. In/out. Reads types and machine operations (`constant_runtime_index` scans all operations for the producer, `exactly_one`).
4. Decides by: ids and path segment equality, with `RuntimeIndex` compared by selector value only (:79-81), and treated as overlapping any
   other segment (:137-138); ranges overlap by interval test (:125-128); an empty range overlaps everything (:125-126, :136).
5. Cases by shape. `interior_member_borrow` admits parent access to child access pairs: mutating parent -> shared/mutable/write-only child,
   write-only parent -> exclusive child, shared parent -> none (:148-158).
6. Same job elsewhere. `S/storage/place_paths.rs:1` `may_overlap` (resolved segments).
7. Counts. `interior_member_borrow`: 0 call sites (lines=2 = `lib.rs:280` re-export + definition); `constant_runtime_index` lines=4 files=4;
   `runtime_index_selects` lines=4 files=4; `qualification_path_carrier` lines=38 files=16; `canonical_structural_storage_path` lines=39 files=23;
   `structural_memberships_disjoint_from_write` lines=10 files=6; `visit_canonical_inline_carriers` lines=3 files=3; `canonical_case_leaf_type`
   lines=6 files=6 (grepped).
8. Stated limits. none.
9. Fail-open risks. `structural_paths_may_overlap` returns `true` (overlap) when the two paths differ in `Case` (:96-101), view, runtime index or
   empty range: conservative. When one path is a prefix of the other it returns `true` (loop ends, `true` :145). In
   `canonical_projection_type_with_lookup` a `FixedIndex` on a non-literal length is accepted only in dependency mode (:344): open, see above.

### S/structural/structural_claims.rs (116 lines)

`structural_claim_paths_may_overlap` (:7) (static projection of both paths then prefix test), `validate_result_claim_catalog` (:26) (type roster
unique, static root, no overlapping catalogue paths, catalogue equals the typed frontier exactly, return rows match :56-68),
`result_claim_paths_match_returns` (:78), `matches_paths` (:85). Refusals via `ResultClaimCatalogError` (4 kinds :19-24). In `matches_paths` a catalogue path
containing a `Case` segment may be absent from a return (:105-110, "path.iter().any(Case)"): conditional claims are optional. Counts: `structural_claim_paths_may_overlap` lines=7
files=4; `validate_result_claim_catalog` lines=10 files=5; `result_claim_paths_match_returns` lines=7 files=6. Fail-open: that `Case`-optional rule accepts a return that
omits a case-conditional claim; stated at :50-51 "Every returning edge still proves its own active membership."

### S/structural/structural_declarations.rs (16 lines)

`structural_declaration` (:4) and `structural_shape` (:11): first-match `find` over declarations by id. Other code uses unique-match `exactly_one`
(e.g. `S/structural/primitive_place.rs:814`). Both are `pub(crate)`. A duplicated declaration id yields the first row here, but the second row elsewhere
would make `exactly_one` fail: inconsistent lookup policy (read). Users: `scalar_array.rs:16,60`, `record_field.rs:63,273,310,334`, `collection_backing.rs:76`.

### S/structural/structural_effect.rs (835 lines) with byte_equal.rs (45), byte_extent.rs (52), byte_predicate.rs (48), element_extent.rs (215), observation.rs (815)

1. Purpose. A table of 39 structural/effect operation kinds (reads, stores, views, establishments, port write) with a fixed schema per kind, plus functions that
   (a) validate the table, (b) turn one `Operation` into a typed `StructuralEffectObservation`, and (c) build proof equations from observations
   (lengths, predicates, byte equality).
2. Shape. structural_effect.rs: enums `StructuralEffectResultShape :79, Custody :91, Action :122, ExternalEffect :164, WorkPolicy :170, GoalShape :175,
   FrontierPolicy :186`; `StructuralEffectLeafSchema :201`; `structural_effect_leaf :255` (derives the goal from the action :268-292); macro
   `structural_effect_rows!` (:313) and its 39-row table (:366-406); `structural_effect_semantic_row :408`; `validate_structural_effect_semantic_rows
   :418`; `validate_structural_effect_schema :430` (restates every row's expected result/custody/frontier :549-778); `validate_structural_effect_result :784`;
   `subslice_extent_equation :817`; `byte_sequence_field_path :29`; `u64_extent_proposition :50`. observation.rs: `StructuralEffectObservation` (:5, 39 variants),
   `structural_effect_leaf_observation_in` (:202, the 40-arm match :215-783), `structural_effect_leaf_observation` (:787), `same_descriptor_coordinate` (:793).
   byte_equal.rs `byte_equal_equation :7`; byte_extent.rs `literal_length_equation :7`, `subslice_length_equation :29`; byte_predicate.rs `byte_predicate_equation :7`;
   element_extent.rs `element_subslice_length_equation :10`, `element_establishment_length_equation :35`, `ElementViewLeaf :68`, `element_view_leaf :72`,
   `element_view_leaf_scalar :164`, `element_view_read_scalar :175`, `element_view_write_goal :191`.
3. In/out. Reads `Operation`, type declarations. Produces observations, `Proposition`s, or `OperationSemanticError`.
4. Decides by: operation tag (`OperationSemanticTag::for_operation`), schema row (action/custody/frontier), result shape, `u64` result type for counts, `u8` for bytes
   (:792-798), reference source access (`MutableBorrow|SharedBorrow` only :227-231, storage queries `SharedBorrow` only :280).
5. Cases by shape. Observation: exact (action, operation-kind) pairs; any other pair gives `StructuralEffectActionShapeMismatch` (:778-782). `ScalarStructuralField` on a
   `Callable` result returns `Ok(None)` ("Opaque code has no mathematical field equation." :673-675). `element_view_leaf` accepts path of Field/FixedIndex/RuntimeIndex
   only and leaf kinds Structural primitive, BoundedInteger, RefinedScalar, Scalar, IeeeFloat (:129-148); byte predicate equation only for `ByteSequence` or fixed byte array
   (:27-33 of byte_predicate.rs).
6. Same job elsewhere. The 39 rows are restated twice: in the macro table (:366-406) and in `validate_structural_effect_schema` (:549-778). The scalar table of the same
   shape is `S/scalars/scalar_leaf_schema.rs`; the full operation tag list is in `crate::operations` (not in this unit).
7. Counts. Not grepped (public names exported via `lib.rs`).
8. Stated limits. none. `expect("validated structural result")` appears at observation.rs:379, :450, :624, :647, :664, :706, :747, :772 (panic sites guarded by
   `validate_structural_effect_result` :214).
9. Fail-open risks. none seen. Note: the observation of `ReleaseReference` and `PrimitiveScalarRead` etc. does not look at the operation result beyond the shape check (:247-253, :288-300).

### S/structural/structural_store.rs (268 lines)

1. Purpose. Decides when a stored-over ("replaced") structural value may be discarded: the value must be unrestricted, or affine with no-code/descriptor cleanup, hold no claims,
   and have only disposable storage.
2. Shape. `structural_store_value_admitted` (:10), `validate_structural_store_values` (:57), `StructuralStoreValueError` (:158), `discardable_store_value` (:166),
   `disposable_storage` (:185), `fields_are_disposable` (:242).
3. In/out. Reads machine parameters, operations, entry claims, types. Returns bool/Result.
4. Decides by: place identity; multiplicity; `no_code_affine_cleanup`/`descriptor_cleanup` flags; shape (`Opaque` needs `no_code_affine_cleanup`; `TypeParameter` never; references and views need
   `descriptor_cleanup`; accessor/access-view records never :208-212).
5. Cases by shape. `StoreDisposition::Vacant` skips everything (:16-18, returns `true`). A `ReplaceDiscardable` store checks the value (:73).
6. Same job elsewhere. Cleanup rosters live in `nominal_cleanup.rs`; `descriptor_cleanup` flag appears in `activation_storage.rs:49` too.
7. Counts. Not grepped.
8. Stated limits. none.
9. Fail-open risks. `structural_store_value_admitted` returns `true` for `Vacant` (:16-18): stated by `S/storage/structural_locals.rs:6-10` (a vacant home holds nothing).
   `validate_structural_store_values` only checks stores whose disposition is `ReplaceDiscardable` and which appear in `stores` (:70-76): a store absent from the roster is not checked
   (traced: roster is produced by the same reconstruction; open whether a missing row is itself refused by the caller).

### S/structural/structural_window.rs (100 lines)

`structural_window_origins` (:9): true only if at least one origin exists and each is live storage or a non-linear parameter admitting the required access (:16-29).
`structural_window_selects_child` (:32), `structural_window_projection` (:51) uses `projection_tip_with_policy` allowing Field/Case/Referent, `FixedIndex` on literal-length arrays,
`RuntimeIndex` on arrays/views (:67-95). Counts: `structural_window_origins` lines=5 files=5; `structural_window_selects_child` lines=11 files=10;
`structural_window_projection` lines=20 files=11. Fail-open: none seen (`found_origin` must be true).

### S/structural/zeroed_structural.rs (599 lines)

1. Purpose. Decides whether a type can be provisioned as zero bytes and lists the leaves that zero provisioning must initialise (scalar fields, empty views, case selections,
   byte arrays); also whether zero bytes satisfy byte-domain qualifications.
2. Shape. `ZeroedStructuralLeaf` (:13, 7 variants), `EstablishedErasedStructuralField` (:54), `zeroed_structural_shape` (:84), `zeroed_structural_fields_satisfy` (:91),
   `zeroed_structural_types_and_fields_satisfy` (:101), `zeroed_declaration_graph` (:118), `zeroed_structural_leaves` family (:283-303), `zeroed_structural_leaves_impl` (:305),
   `zeroed_byte_qualifications_hold` (:499), `zeroed_projected_byte_qualifications_hold` (:576). Constant `MAX_ZEROED_LEAVES = 131_072` (:10).
3. In/out. Reads types, domains, field declarations; produces leaf lists.
4. Decides by: declaration shape; each field's `scalar_field_accepts_zero`; first case of a sum is the zero case (`cases.first()` :205, :460); erased fields refuse unless an
   "established erased field" row matches exactly once (:338-358); byte fields only for `BoundedOwned` or shared/unspecified `BorrowedView` whose predicates hold on empty (:269-281).
5. Cases by shape. `FixedArray` of `u8` -> single `ByteArray` leaf (:422-439); empty scalar array -> `EmptyScalarArray` (:413-421); other arrays enumerate elements (:440-458);
   `ElementView` only with access `None | SharedBorrow` (:166-169, :400); `_ => return false/None` (:202, :488).
6. Same job elsewhere. `S/storage/zero_valid_storage.rs:7` answers "is zero valid" with a different recursion (no leaf list, different field callback).
   `S/structural_qualifications.rs:1016` `zero_defaults_supported` is a third zero-vocabulary check with its own list (Scalar/Atomic fields, shared byte views, parameter-length arrays).
7. Counts. `zeroed_structural_shape` lines=11 files=9; `zeroed_structural_fields_satisfy` lines=3 files=2 (0 outside callers: internal + re-export `lib.rs:302`);
   `zeroed_structural_types_and_fields_satisfy` lines=4 files=3; `zeroed_structural_leaves` lines=4 files=4; `..._modulo_references` lines=4 files=4;
   `..._with_established_erased_fields` lines=4 files=3; `zeroed_projected_byte_qualifications_hold` lines=8 files=7 (grepped).
8. Stated limits. leaf budget "MAX_ZEROED_LEAVES" 131072 (:10, :332, :388, :449, :482, :490).
9. Fail-open risks. `zeroed_projected_byte_qualifications_hold` returns `true` for a qualification whose domain has no `byte_predicate` (`domain.byte_predicate.is_none() || ...` :591):
   stated: ":589-590 Ordinary projected memberships are judged by the structural membership owner; this query decides only byte contents." (the other judge: `structural_qualifications.rs:2060-2089`).
   `modulo_references` mode skips structural fields whose type is a `Reference` (:213-223, :240-251, :361-371): used only when the caller passes `true`.

---------------------------------------------------------------------------------------------------

## S/structural_qualifications.rs (3035 lines), structural_qualifications/default_requirement_tests.rs (350), structural_qualifications/operation_permissions.rs (686)

1. Purpose. The single judgment that a lowered module's structural values carry exactly the domains (named, optionally predicate-bearing, optionally access-controlled
   memberships) their destinations declare, that memberships change only at an explicit "qualification change" (cast) edge, that each cast names an authorised issuer, and that
   access-controlled values are only read/written by permitted callers. Also the "default requirement" inventory: which declared predicates must hold at each storage occurrence.
2. Shape (structural_qualifications.rs). `StructuralQualificationError` (:45); `Memberships` (:59) with `of :65, beneath :84`; `formal_domains_cover_requirements` (:95);
   `Declared` (:133) root-place table; `DefaultRequirement` (:259), `DefaultRequirements` (:308) with `new :317, storage :442, declared :452, operation :643, clause :959`, helpers
   `domain_clauses :401, walk :491, fields :594, backings :761, declared_operation :850, owners :931, append_lost :732`; `default_domain_clause` (:981); `zero_defaults_supported`
   (:1016); `type_has_defaults` (:1114); `Rerooted` (:1353) and `DefaultBindings` (:1148) implementing `ClauseBindings`; `validate_declared :1285`; `displayed :1332`;
   `validate_change :1453`; `byte_change_judgment :1633`; `validate_machine :1726`; `selected_field :2118`; `validate_arguments :2142`; the caller-description readers
   `MachineCallerDescription :2205`, `read_machine_caller_declaration :2307`, `machine_caller_application_matches :2375`, `caller_matches_machine :2471`; `validate_access_policy :2540`;
   `same_access_policy :2774`; `validate_access_policies :2836`; `validate_shape :2860`; `validate_shape_for_bodies :2871`; `validate_change_owners :2886`;
   `byte_qualification_change_judgments_in :2903`; `validate :2989`. operation_permissions.rs: `Subject :17`, `Permissions :34` with `requirements :111, query :230, callers :290,
   is_entry :331, route :353, rules :411, subject :472, operation :496`; `validate :570`.
3. In/out. Reads `TerminalModule` (machines, structural types and domains, `structural_qualification_changes`, `proofs.scalar_block_invariants`, identity table, entry),
   `crate::membership_facts`, `descriptor_results::DescriptorAnalysis`, and `application_identity`/`schema_reflection`/`access_admission`. Produces `Result<(), StructuralQualificationError>`
   (machine + a static detail string) and `Vec<(EdgeId,bool)>` byte-establishment judgments; `DefaultRequirements` rows for the proof obligations (consumed by stage 06).
4. Decides by: place identity (`Declared.declared` sorted vector, binary search :231); domain identity `StructuralDomainId` and exact path equality (`require_exact` compares whole
   `Memberships` :1270-1281); issuer authorisation by checked declaration/machine identity strings through `issuer_route_authorizes` (:1588-1608) and caller description comparison
   by owner form, package digest and path text (:2351-2362, :2411-2419) -- these compare identity *spellings* parsed by `visit_type_identity_package_owners`; block invariants by
   `(machine, header)` and `retains_conjunct` (:1542-1549). Access rules by caller description (`caller_matches_machine`) and by machine entry.
5. Cases by shape. Each operation kind is matched for what it must keep: `MoveStructuralField`, `StructuralLeafCopy`, `MoveStructuralValue`, `EstablishRecord`, `EstablishStructuralCase`,
   `EstablishStructuralArray`, `EstablishElementView` (whole collection) / `EstablishMutableElementView` / `ElementViewSubslice`, `EstablishReference`, `EstablishStructuralLocal`,
   `EstablishZeroedStructural`, and `StoreStructuralField` (:1853-1871, :1875-2105). Fallthrough: "ordinary structural operation cannot establish a declared domain" (:2099-2103).
   A policy caller route must be `ExactMachine`; `CheckedRequirement` and `BoundaryRequirement` callers fail "requirement caller needs its exact checked public application"
   (:2493-2501); a caller with a `Machine` or `Proposition` closed-reach parameter fails "needs its complete checked machine or proof argument account" (:2455-2459); a capture is refused (:2430-2434).
   Default inventory: `walk` supports literal and (zero-only) parameter arrays, `Record`, `Sum`; other shapes in zero mode "zero storage has no supported default inventory" (:588).
6. Same job elsewhere. Scalar domains are judged by `crate::scalar_qualifications` (not in this unit; imported :41). The module doc (:1-21) says "Source lowering, the lowered-form checker and
   artifact ingress share this one judgment": grepped callers of `structural_qualifications::validate*`: artifact ingress `psi/artifact/wire/src/semantic_module.rs:4289` (`validate`),
   stage 06 `formation.rs:87` (`validate_shape_for_bodies`); none found in stage 05 (lowering) (grep over `source-rust tools`). Hence two of the three named users.
7. Counts. `formal_domains_cover_requirements` lines=2 files=2 (1 caller: stage 06 `call_contracts.rs:556`); `same_access_policy` lines=2 (1 caller:
   `execution-inputs/providers/component/memberships.rs:194`); `byte_qualification_change_judgments_in` lines=2 (1 caller: stage 06 `places.rs:5054`);
   `caller_matches_machine` lines=5 files=3 (1 library caller `operation_permissions.rs:371`, plus 3 test calls); `validate_access_policy` lines=4 files=2 (internal x2, one test);
   `validate_shape` lines=46 files=16 (name shared); `validate_shape_for_bodies` lines=3 files=2; `DefaultRequirements` lines=18 files=8; `default_domain_clause` lines=3 files=2
   (1 outside caller `bytes/fixed_arrays.rs:109`); `operation_writes_default_storage` lines=3 files=2 (1 outside caller: stage 06 `places.rs:1272`) (grepped).
8. Stated limits. "memory access checks for these saved rules are not implemented yet" (:2851, when a domain's access plan has observations or service reach, :2844-2848);
   "the compiler cannot check access through this public entry yet" (:2596, boundary entry); "the compiler cannot check this helper's public call contract yet" (operation_permissions.rs:309);
   "the compiler cannot check the selected public caller's access yet" (:364); "access through a reference stored inside this value has not been checked yet" (:214);
   "the compiler cannot check access to this earlier stored value yet" (:552); "A cast adds a domain with predicates" must continue in a block whose invariant states them (:2-21 doc).
9. Fail-open risks.
   - `validate` (:2999-3014) builds the descriptor graph with `filter_map(Result::ok)`, dropping bodies whose storage history cannot be read. stated: ":2996-2998 An unsupported body remains absent and cannot add evidence; it must not erase independent supported rows." traced: byte-change establishment for an unreadable body then has no incoming facts (`incoming_byte_sequence_argument_predicates` returns `None` -> only reconstructed source predicates used :1707-1719), so such a cast can only be established by the local facts, not accepted without them.
   - `validate_machine` continues (accepts) for any operation with no structural result and not `StoreStructuralField` (:1852-1873), e.g. other stores; other operations are only checked if they produce a structural result. open: whether another judgment covers memberships delivered by `StructuralByteViewFieldStore`/`StructuralScalarFieldStore` (those produce no structural result).
   - `OperationKind::EstablishStructuralLocal {}` (:2054) and `_ if produced.is_empty() => {}` (:2091) accept without checks (stated :2052-2053 "A reserved home holds nothing until a store fills it; the store delivers its domains.").
   - `judged_elsewhere` (:1247) exempts byte and content domains from the carrier-equality check (:1311-1313) and from `_ if produced...all(judged_elsewhere)` (:2094-2098); stated ":1245-1246 A byte or content domain, which its own judgment governs".
   - `validate_change` with a domain that has no `predicate` and no `establishment_routes` and whose required memberships are all held is accepted as a plain add (:1557-1574): traced: matches `(None, None) => {}`.
   - `has_rules` in `operation_permissions::validate` limits checks to domains with non-empty readers/writers; a domain whose plan has only `observations` is refused earlier (:2844-2848).
   - `zero_admits` (:1257) accepts a projected domain under zero provisioning when it has no routes/requirements/byte/content parts; stated :1253-1256.
   default_requirement_tests.rs: 7 tests pin rerooted-goal clauses (:125), exclusive calls owing store-owner defaults (:154), additional-input membership not a default (:200),
   no per-element enumeration of `u64::MAX` arrays (:236), universal marker (:250), nested element paths (:281), reference display refusal (:321).

---------------------------------------------------------------------------------------------------

## S/substitution.rs (783 lines)

1. Purpose. Replaces value ids or place ids in propositions and scalar terms (when a callee contract is instantiated at a call site, or a predicate is rebased onto another place); and substitutes
   crash-route guards.
2. Shape. `substitute_proposition_values(_with)` (:18, :25), `substitute_proposition_structural_places(_with)` (:151, :160), `substitute_integer_math_term_values` (:324),
   `substitute_integer_math_term_places` (:360), `substitute_scalar_term_places` (:378), `substitute_content_term_places` (:439), `pub(crate) SCALAR_EXPRESSION_DEPTH = 64` / `SCALAR_EXPRESSION_NODES = 4096`
   (:468-469), `substitute_bound_scalar_term` (:471), `substitute_scalar_term_values(_with)` (:509, :516), `substitute_crash_routes` (:748). Submodule `clauses` (:8, in a different unit).
3. In/out. Pure functions over `Proposition`/`ScalarTerm`/`IntegerMathTerm`/`ContentTerm`; maps keyed by `ValueId` or `PlaceId` (`BTreeMap` :20, :153, :511, :750) or callbacks.
4. Decides by: value identity (`ValueId`) and place identity (`PlaceId`); `substitute_bound_scalar_term` binds only the first occurrence layer of a callee value and does not rebind values inside a bound argument
   (:481-483); it refuses when an argument is missing, its scalar type differs, depth exceeds 64 or nodes exceed 4096 (:487-495).
5. Cases by shape. Value substitution leaves `IeeeFloatComparison`, `ByteSequenceEqual`, `StructuralIdentity`, `StructuralCaseMembership`, `ContentConservation` and `Atom` unchanged (:54-55, :80-90, :109-122,
   :145-147), because they hold places not values. Place substitution also rebases `IeeeFloatComparison` fields and `ByteSequenceEqual` fields and `StructuralIdentity` (sorted afterwards :280-283) (:217-263, :272-284).
   Crash route substitution drops `Falsehood` alternatives and replaces any bucket containing `Truth` by one `Truth` (:763-776).
6. Same job elsewhere. `S/structural_qualifications.rs:1353` `Rerooted` does a third form of re-rooting through `ClauseBindings` in `substitution::clauses` (not in this unit).
7. Counts. `substitute_crash_routes` lines=2 files=2 (1 caller: `psi-to-native/.../abstract_optimization/validation.rs:2898`); `substitute_bound_scalar_term` lines=2 (1 caller: `descriptors/replay/contents_queries.rs:1432`);
   `substitute_proposition_values` lines=11 files=6; `substitute_proposition_values_with` lines=10 files=4; `substitute_proposition_structural_places` lines=6 files=6; `..._places_with` lines=8 files=3 (grepped).
8. Stated limits. none.
9. Fail-open risks. In `substitute_integer_math_term_values`, a `MathValue` whose replacement has a different integer type, or an `Embed` whose replacement cannot be embedded, keeps the *original* term
   (`.unwrap_or_else(|| term.clone())` :331, :339) rather than failing: the callee's value id then remains in the caller-side proposition. open: whether a callee id left unreplaced is caught later
   (no check in this file). `substitute_content_term_places` drops the replacement's `prefix` and keeps only the replacement root (`.map(|(root, _)| *root)` :454-455) while the scalar equivalents
   (:393-403, :410-419) prepend the prefix: a content term rebased through a place whose replacement path is non-empty loses that path. open: not traced to a caller that passes a non-empty prefix.

---------------------------------------------------------------------------------------------------

## Coverage

Read in full (21,863 lines): all 64 ranges listed in `units/unit_049.tsv`, each from line 1 to its last line:
canonical_goal.rs 1-844; formation_tests.rs 1-116; scalar_array.rs 1-78; scalar_field_requirements.rs 1-153; scalar_leaf_schema.rs 1-164;
scalar_leaf_semantics.rs 1-510; scalar_observations.rs 1-319 and tests.rs 1-159; scalar_ordering.rs 1-637 and contracts.rs 1-88;
symbolic_integer_widths.rs 1-97; trapping_integer_tests.rs 1-129; unchanged_scalar.rs 1-43; static_reach.rs 1-467; storage.rs 1-20;
activation_storage.rs 1-116, flow.rs 1-1294, batch_tests.rs 1-116; arrival_storage.rs 1-476; borrowed_storage.rs 1-1061, tests.rs 1-276,
write_frames.rs 1-200; calling_storage.rs 1-181 and tests.rs 1-178; collection_backing.rs 1-176; element_representation.rs 1-250; place_paths.rs 1-34;
representation_regions.rs 1-254; run_custody.rs 1-78; storage_dependencies.rs 1-153; storage_regions.rs 1-99; structural_locals.rs 1-50;
type_graph.rs 1-385; zero_valid_storage.rs 1-69; structural.rs 1-25; atomic_cell.rs 1-21; call_claim_transfer_tests.rs 1-121;
inline_carriers.rs 1-135 and tests.rs 1-434; nominal_cleanup.rs 1-292; nominal_cleanup_tests.rs 1-68; owned_structure.rs 1-607,
complete_claims.rs 1-253, complete_claims/tests.rs 1-92; partial_affine.rs 1-460; primitive_place.rs 1-1227; record_field.rs 1-348;
shared_mutation.rs 1-146; static_path.rs 1-401; structural_claims.rs 1-116; structural_declarations.rs 1-16; structural_effect.rs 1-835,
byte_equal.rs 1-45, byte_extent.rs 1-52, byte_predicate.rs 1-48, element_extent.rs 1-215, observation.rs 1-815; structural_store.rs 1-268;
structural_window.rs 1-100; zeroed_structural.rs 1-599; structural_qualifications.rs 1-3035, default_requirement_tests.rs 1-350,
operation_permissions.rs 1-686; substitution.rs 1-783.
Skimmed: none. Not read: none.
Some test files (symbolic_integer_widths, trapping_integer_tests, formation_tests, batch_tests, tests.rs files) are summarised in a few lines
each because they contain tests only, not decisions.

## Five things a maintainer most needs to know about this unit

1. `S/structural_qualifications.rs` (3035 lines) and its `operation_permissions.rs` are one judgment with many deliberate refusals marked "not implemented yet" /
   "cannot check ... yet" (:2851, :2596; operation_permissions.rs:214, :309, :364, :552). Any domain with access observations or service reach is refused wholesale (:2844-2848).
   Its doc says three users share it (source lowering, the lowered-form checker, artifact ingress); the grep found artifact ingress and stage 06 only.
2. Four separate walkers cover the same "follow a storage path through types" job with different segment alphabets and different failure behaviour:
   `primitive_place.rs:826`, `static_path.rs:263`, `record_field.rs:263`, `scalar_observations.rs:120`. Type-containment (by-value ownership) is also walked separately in
   `type_graph.rs`, `owned_structure.rs`/`complete_claims.rs`, `zeroed_structural.rs`, `structural_qualifications.rs:1114`, and `storage_dependencies.rs`.
   Lookups differ: first-match `find` in `structural_declarations.rs` versus unique-match `exactly_one` elsewhere.
3. Several `pub` functions have no call site (0 beyond the definition and the `lib.rs` re-export): `constant_goal_free_scalar_leaf`, `collection_view_backing`,
   `representation_region_length`, `zero_valid_record_storage`, `primitive_place_type`, `fixed_array_place_shape`, `primitive_read_requirement`, `interior_member_borrow`.
4. Fixed-shape limits are constants spread through the files: 64 and 4096 in `carried_exclusive_referents` (borrowed_storage.rs:40, :68); 4096 steps in `Projection::then` (:874);
   128 depth and 16384 leaves in `element_representation.rs:72, :170`; 131072 leaves in `zeroed_structural.rs:10`; 64 depth / 4096 nodes in `substitution.rs:468-469`;
   128 depth in `calling_storage.rs:140`; 128 in the machine-caller parser (`structural_qualifications.rs:2217`).
5. `activation_storage/flow.rs` is a serial-only model: every unsupported combination is refused with a message containing "serial realization", and any operation not named in the big
   `match` (:627-1101) changes no claim state, relying on the producer check at :1104-1127 and the call-argument checks at :603-626.

## What I am unsure about

- Whether operations not named in the `flow.rs` match (:1101 `_ => {}`) can move or borrow a claim-carrying place without one of the listed kinds or `call_arguments` (not enumerated).
- Whether `substitute_content_term_places` ignoring the replacement prefix (substitution.rs:454-455) is reached with a non-empty prefix; and whether a callee value id left in place by the
  `unwrap_or_else(|| term.clone())` branches (:331, :339) is caught elsewhere.
- Which callers of the primitive-path walker pass `dependencies` (the mode that accepts trailing case payloads and parameter-length indices, primitive_place.rs:1013, static_path.rs:344).
- Whether `validate_acyclic` in `type_graph.rs` always runs before `compute_claim`/`visit` in `complete_claims.rs`, since a claim-owning type returns before the cycle check (:88-90).
- Counts for names shared with other crates (`validate_shape`, `template_commitment`, `validate_module`) include unrelated functions; call-site separation was done only for the names listed with explicit line lists.
- Whether `S/scalars/scalar_leaf_semantics.rs:60` duplicates an interpreter-side constant folder (suspected, not read).
