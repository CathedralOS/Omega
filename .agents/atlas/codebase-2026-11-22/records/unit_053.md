# Record: unit_053 (area 02-source-stages-00-04, crate syntax-trees-to-symbol-resolved-trees)

Root `P/` = `source-rust/subsystems/compiler/source-to-psi/pipeline/02_syntax-trees-to-symbol-resolved-trees/`. All 36 files of the unit were read line by line (20,634 lines). Citations are `file:line` relative to `P/`. "read" = I read the lines; "grepped" = Grep tool count/content; "inferred" = not read at the other site.

The unit covers two directories: `constants/` (8 of its 12 files; the other four `carrier.rs`, `declaration_values.rs`, `initializer_dependencies.rs`, `initializer_leaves.rs` are outside this unit) and `declarations/` (assembly, a few single files, and `generic_data/` with `const_evaluation/`). Most of it runs as syntax-to-syntax rewriting before any symbol exists (`declarations/mod.rs:39-71`, grepped: `assembly::prepare`, `dispatches::prepare`, `builtin_type_names::validate` are called there). `constants/` runs at the end of resolution (`constants/mod.rs:147 finalize`).

---

## constants/initializer_normalization.rs (1731 lines)

1. Purpose. Checks that the receipt a constant initializer carries ("normalization receipt": its authored expression, evaluated value, and the constants, call choices and builtin operators the value came from) matches what resolution independently reconstructs from the resolved declarations, then attaches that inventory to the declaration's expression roots as authored-selection occurrences.
2. Shape. `mod resolved_payload` (30-724) with `validate` (70) and private `Encoder` (65) that re-encodes a materialized initializer into a canonical value and compares it with `receipt.result_value` (101-103). `retained_constant_custody` (741, `pub(crate)`) reads a retained base's custody. `PendingInitializer` (835), `InitializerRootCustody` (849), `retain` (871: validates one receipt and walks the authored syntax to collect names and operators), `declaration_custody` (1034), `reconstruct` (1077), `append_retained_custody` (1195), `finalize_operator_obligations` (1302), `attach_custody` (1343), `replace_initializer_lanes` (1483), `same_custody` (1523), `finalize` (1563), `constant_initializer_owns_selection` (1707), `rejoin_after_substitution` (856). Enum `InitializerCustodyPurpose` {Authored, Declaration, Value} (731).
3. In/out. Reads `SymbolResolvedTrees` (const declarations, authored selection ledger, expression table), the syntax receipt `ConstInitializerNormalization`, `Lowerer.pending_const_initializers`. Mutates: records resolved/late-bound authored selections (`attach_custody` 1411, 1439, 1452) and replaces an expression root's selection occurrence list (`replace_authored_selection_occurrences` 1515-1519). Produces `Vec<InitializerRootCustody>` (1566).
4. How it decides. Identity: a declaration owner is found by `symbol_source_span(symbol) == record.declaration && initializer == record.materialized`, exactly one required (1039-1054, 1571-1586). Value: `resolved_payload::validate` compares canonical encodings (101). Position: references/operators are keyed by `SourceSpan` (`reconstruct` 1099-1131 re-selects each dependency by `find_top_level_by_name_and_kinds_from_source(name, [Const], reference)` which is a spelling lookup anchored at the reference span, 1100-1106). Spelling: `float_format` via builtin atoms (717); `is_layout_storage` combines a name string, supply mode and the bundled Core source bytes (703-715 -> `declaration_selection.rs:78-96`). Private-foreign check: `!declaration.is_public && !same_source_package` (809, 1140, 1256).
5. Cases by shape. `retain` accepts only: Integer/Boolean/Float/String, unapplied Name, Binary, Member, Unary, Cast, Call (arguments only), ArrayLiteral, StructLiteral, Match (arms values/patterns); everything else: "authored expression is outside constant leaf evaluation" (1004-1009). `Encoder::value` accepts only fixed array, builtin scalar, named/generic data with `DataSupplyMode::CheckedShape` and no quotient (274-280); depth cap 64 (147, 636); `scalar` supports Bool, f32/f64, fixed integers and Address (425-481). `append_retained_custody` match arms (1215-1291): only Call to Machine/State/Operator, builtin Operator, StaticPathSegment (module/data/variant ignored, const recorded), struct-literal and case selections; anything else errors "no checked scalar meaning" (1285-1289).
6. Same job elsewhere. Canonical value building from an expression: `declarations/generic_data/const_evaluation/values.rs:1266-1539` (`canonicalize_const_expression_with_admission`) does the same encode job at syntax level; this file's `Encoder` redoes it at resolved level (cite 94-98 vs values.rs:1373-1376). The custody inventory is the same one built in `const_evaluation/initializers.rs:625-653` (`custody`) at fold time; `finalize` re-derives it (1599-1627) to compare. Suspected (not read): `constants/initializer_dependencies.rs` (`collect_declaration`) builds the dependency roster both consume.
7. Unused / one-caller. `retained_constant_custody`, `finalize_operator_obligations`, `rejoin_after_substitution`, `constant_initializer_owns_selection`, `is_core_layout_storage_source`: grepped `finalize_operator_obligations|rejoin_after_substitution|constant_initializer_owns_selection|retained_constant_custody|is_core_layout_storage_source` across `source-rust`: 20 occurrences in 7 files, so every one has at least one external caller (callers in `constants/mod.rs`, `const_evaluation/initializers.rs` (retained_constant_custody at 343-345), `lowering/type_reference.rs`, `pre_resolution/initializers.rs`, `resolver.rs`). I did not read those callers.
8. Stated limits. "initializer implementation exposure never becomes public index exposure" (header 7-8, a design statement, not a limit). No unsupported/TODO comments.
9. Fail-open risks. (a) `retain` returns `Ok(())` when `definition.preparation().normalization` is `None` (877-879): a constant with no receipt is not checked here. Stated by structure (only normalized constants have receipts); traced: the fold at `const_evaluation/initializers.rs:185` and `requires_const_initializer_evaluation` (`constants/mod.rs:97`) decide who gets one, so a constant whose initializer is not recognized as closed simply gets no receipt and is evaluated elsewhere. (b) In `retain`, a bare-name reference that `qualified_payload_free_case` selects is skipped without being recorded (945-955): a case name is not a const dependency. Stated by the helper name; traced no further. (c) `append_retained_custody` ignores StaticPathSegment targets of kind Module/Data/Variant (1233-1237) and struct-literal selections (1278-1284). Open: whether a retained initializer can name something else through those kinds (inferred no).

## constants/literal_declarations.rs (131 lines)

1. Purpose. Decides whether a module-level `const` is a scalar literal declaration and validates the nominal carrier of a module-owned constant.
2. Shape. `validate_module_literal` (16), `primitive_literal_declaration` (53), `generic_const_carrier_leaf` (103), `scalar_literal_tree` (117).
3. In/out. Reads `SyntaxTrees`, `ConstDefinition`, `ConstantSelection`. Returns `Result<(), Diagnostic>` / bool; no mutation.
4. Decides. `primitive_literal_declaration` is keyed on the builtin atom the selection assigns the declared type name (78-87) and on expression shape (`scalar_literal_tree` 117-130). Generic-carrier gate keyed on base-name spelling `selection.data(syntax, base_name)` having type parameters (30-39).
5. Cases by shape. Literals only: Boolean, Integer, Float, UndeterminedNaN, String, nested ArrayLiteral (120-125). Type: Named builtin or FixedArray with `Literal` length (89-93); any other type shape returns false (93). Floats allowed only when not an array (82-86).
6. Same job elsewhere. Scalar-carrier test `is_structural_scalar_carrier` shared with `scalar_carriers.rs:13`.
7. Unused. `primitive_literal_declaration` is `pub(super)`; one visible caller inside this file (25); other callers in `constants/` outside the unit (inferred).
8. Stated limits. "Any other leaf - named, constrained, builtin - keeps its existing owner." (99-102).
9. Fail-open. `validate_module_literal` returns `Ok(())` when the source has no module (`source_module(...).is_valid()` false) or the declaration is a primitive literal (21-28). Stated by the function doc ("A closed module literal ..."): non-module constants are validated elsewhere. Traced: no.

## constants/mod.rs (211 lines)

1. Purpose. Module owner for constant finalization; decides which initializers still need evaluation and orders the finalization passes.
2. Shape. Module list (52-64), re-exports (66-83), `requires_const_initializer_evaluation` (97), `finalize` (147), `finalize_operator_obligations` (204).
3. In/out. `finalize` takes `&mut Lowerer`, mutates `lowerer.symbol_resolved_trees`; sequence: bind receiving parameters (149), finalize argument selections (153), assign constant expression symbols (165), bind named implementation const calls (166), prepare initializer destinations if mode `Complete` (168-173), finalize constant expression selections (174), `initializer_normalization::finalize` (183), `bind_constant_uses` (186), `finalize_const_selections` (193), rejoin (195).
4. Decides. `requires_const_initializer_evaluation` is keyed on expression shape: Integer, Boolean, String, UndeterminedNaN, landed Float, ArrayLiteral and StructLiteral are "done"; any other node returns true (132). A `pending_target_observation` flag returns false first (102).
5. Cases by shape. Header comment: "Calls and floating computations still need evaluation." (32-33) and "Initializer preparation is a separate non-executing mode" (30-31).
6. Same job elsewhere. A near-identical walker of literal-only shapes is `literal_declarations.rs:117-131` (`scalar_literal_tree`) and the shape walk in `initializer_normalization.rs:914-1011`.
7. Unused. `requires_const_initializer_evaluation` is `pub`; used in `const_evaluation/initializers.rs:186,275` (read) and `normalization` flows (inferred others).
8. Stated limits. Header 26-39: "Remaining boundaries, enforced loudly" listing calls and floating computations.
9. Fail-open. `requires_const_initializer_evaluation` returns false (no evaluation owed) for `pending_target_observation` (102-104). Stated only by the name; open (not traced where target observation is evaluated).

## constants/normalized_expressions.rs (100 lines)

1. Purpose. Validates normalized constant-argument receipts (expression handles still valid and spans still equal the argument's reference) and that the rewritten type payload still decodes to the retained canonical value.
2. Shape. `normalization_requires_expression` (11), `validate_normalized_expression` (28), `validate_normalized_const_argument` (64).
3. In/out. Reads `ConstArgumentNormalization`, syntax expressions, type-reference node; returns `Diagnostic` errors.
4. Decides. Keyed on handle validity and `SourceSpan` equality (37-38, 49-50) and on the decoded canonical encoding (71-93). In `validate_normalized_const_argument` a Named argument is compared by its spelling: integers by `name == value.to_string()` (75), others by `CanonicalConstValue::from_atom(name)` encoding and `type_name` equality (77-90).
5. Cases by shape. Floats and StaticCallable decoded values always fail (79-80); only Integer/Boolean/Array/Record/Variant atoms pass.
6. Same job elsewhere. `retain_const_argument_normalization` writers in `const_evaluation/arguments.rs:146-160, 187-197` and `domains.rs:400-411` produce the receipts this checks.
7. Unused. `normalization_requires_expression` used also in `const_evaluation`'s `generic_data/mod.rs:184,209` (read).
8. Stated limits. none.
9. Fail-open. none seen (mismatch returns error at 94-99).

## constants/receiving_applications.rs (751 lines)

1. Purpose. Records which declaration and which generic binders a retained "receiving application" (a type application whose const arguments will receive constants) belongs to, and joins const-argument selections to their receiving slots.
2. Shape. `bind_receiving_application` (18), `bind_receiving_parameters` (49), `ConstArgumentInputs` (123) with `prepare` (136), `receiving_parameters` (206). Test module (259-750).
3. In/out. Mutates `child_type_references.receiving_applications` in `SymbolResolvedTrees`: sets `selected`, `conflict`, `parameters`. `ConstArgumentInputs` builds per-selection declaration index, duplicate flag, slot runs.
4. Decides. `bind_receiving_application`: if name, arguments span or an already selected symbol differ, records a `conflict` and keeps the original (30-39); handle-based (symbol arena index/generation, 74-81). Declaration join keyed on source span of the declaration symbol (`key(span)` 141-142, 159-175); duplicates flagged (172).
5. Cases by shape. `bind_receiving_parameters` handles data definitions, type aliases, and domains (101-115); for domains it drops the leading carrier binder when `index_binders` length differs (108-113).
6. Same job elsewhere. none.
7. Unused. `bind_receiving_application`: grepped content, 4 non-test callers (`names/domain_facts.rs:332`, `names/type_references.rs:844, 910, 1751`) plus 2 test uses.
8. Stated limits. none.
9. Fail-open. `bind_receiving_application` returns without error when `retained` handle is invalid or not valid in the arena (26-28); stated: "Keep conflicting inputs intact until finalization can report the error" is about conflicts, not invalid handles. Open: an invalid handle silently produces no binding; the later `receiving_parameters` errors if the slot has a stale handle (215-219), so traced: caught later.

## constants/scalar_carriers.rs (24 lines)

1. Purpose. Classifies builtin atoms as "structural scalar" (bool, fixed integers, Address) and "builtin scalar" (those plus f32/f64).
2. Shape. `is_structural_scalar_carrier` (13), `is_builtin_scalar_carrier` (21).
3. Pure functions over `BuiltinTypeAtom`.
4. Decides on the atom (an identity), not a name.
5. Cases by shape. fixed list: Bool, Integer, Address; F32, F64 (14-17, 23).
6. Same job elsewhere. `symbols::BuiltinTypeAtom::is_fixed_integer` is used in `initializer_normalization.rs:605`.
7. Unused. both `pub(crate)`; `is_structural_scalar_carrier` used in `literal_declarations.rs:81`, `values.rs:1104`; `is_builtin_scalar_carrier` in `values.rs:514, 799-800`.
8. none. 9. none seen.

## constants/selections.rs (293 lines)

1. Purpose. Finalizes const selections (use sites), const declarations scoped to a data carrier, and const argument selections; records them in the authored-selection ledger.
2. Shape. `finalize_const_selections` (25), `finalize_const_declarations` (55), `const_selection_record_diagnostic` (140), `finalize_const_argument_selections` (149).
3. In/out. Mutates `SymbolResolvedTrees`: ledger rows, expression selection occurrences, and rewrites `TypeReference::Named` argument symbol/name (281-291). Reads `PendingConst*` lists.
4. Decides. Declaration join is by `ConstArgumentInputs` source-span index; drift check compares `initializer_source_span` and `evaluated_value` equality (168-175). Data carrier for a type-scoped const: spelling lookup `lookup_top_level_by_name_and_kinds_from_source_matching(scope, [Data], span, |_| true)` (88-95); ambiguity and not-found are errors (98-110). Nominal compare: `same_resolved_carrier` (232); argument checked by decoding its atom name against the origin's encoding (242-246).
5. Cases by shape. `requires_nominal_slot`/`structured_argument` (176-195): only array/record/variant encodings or nominal carriers take the nominal-slot path.
6. Same job elsewhere. none.
7. Unused. `finalize_const_declarations` is re-exported `constants/mod.rs:81`; callers outside unit (inferred).
8. none.
9. Fail-open. `finalize_const_declarations` skips (`continue`) a declaration whose `scope` is empty or whose symbol has no module (85-87). Stated by the doc of the module comment ("Module-owned scoped declarations"). Traced: no. In `finalize_const_argument_selections`, when `selection.is_argument_value` is false, or no nominal condition holds, the slot is counted `matched = true` without checks beyond parameter kind (262), and only `requires_nominal_slot && !matched` errors (264-269); non-nominal constants are accepted when no slot exists at all. Open: whether a scalar constant argument with no slots is meant to pass (inferred yes).

## constants/uses.rs (144 lines)

1. Purpose. Binds each constant use (a name expression) to the declaration visible from its source location; detects duplicate and colliding consts.
2. Shape. `bind_constant_uses` (9).
3. In/out. Reads const declarations, authored expressions; mutates expression name path (`path.symbol = selected`, member symbol, 123-133) and pushes `PendingConstSelection` (134-141).
4. Decides. Duplicate const: same name string, same source package, not separate scopes (23-27). Collision with a case: splits `name` at last `::` and looks up data + `Variant` child by spelling (39-55). Free-floating const colliding with data/machine: name equality + package + scope (62-71). Privacy: `!declaration.is_public && !same_source_package(reference, source)` (96-101). Initializer mismatch check skipped in `ArgumentSelection` mode (109).
5. Cases by shape. none.
6. Same job elsewhere. none.
7. `bind_constant_uses` is `pub(crate)`, called once in `constants/mod.rs:186`.
8. none.
9. Fail-open. `selected_expression_constant(...)` returning `None` skips the occurrence (86-90): a name that does not select a constant is left for the ordinary name path. Stated by the function name; traced: no (inferred).

---

## declarations/assembly/flags.rs (139 lines)

1. Purpose. Adds generated statements so an authored assembly block that snapshots hardware flags (`pushfq`) sees flag values produced by earlier arithmetic instructions.
2. Shape. `HardwareFlags` {arithmetic_updates, snapshots, restores} (15), `record` (22), `observes` (34), `materialize` (38), `collect_generated_primitives` (125).
3. In/out. Mutates `SyntaxTrees` (adds expressions/statements), mutates `Vec<PreparedBlock>` by inserting statements before the instruction and a new first block `asm#flags_entry` (84-92).
4. Decides. Keyed on instruction shape (`AsmInstructionShape::FlagsRestore/FlagsSnapshot`, 27-31) and statement handles. `materialize` returns immediately if there are no snapshots or any restore exists (45-47).
5. Cases by shape. Arithmetic producers Subtract/Add only (via `Arithmetic`). Restore (`popfq`) disables flag materialization entirely (45-47); combination is later refused in `assembly/mod.rs:278-283`.
6. Same job elsewhere. The flag formulas live in `generated_statements.rs:273-348`.
7. Unused. `HardwareFlags` is `pub(super)`; one user, `assembly/mod.rs:87,156,272,278,301`.
8. none.
9. Fail-open. In `materialize`, when a statement is not found in any block, the loop simply does not splice (68-79, 110-120): the flag update is silently dropped. Open: statements come from the same blocks, so inferred unreachable. `unreachable!()` at 97.

## declarations/assembly/generated_statements.rs (403 lines)

1. Purpose. Constructor functions that build syntax nodes (names, casts, binary expressions, assignments) for compiler-generated assembly helpers: scratch register bindings, modular arithmetic, x86 flag formulas.
2. Shape. `name` (16), `unsigned_type` (26: always `u32`), `wide_type` (199: `u64`), `subtraction`/`addition` (60,68), `ArithmeticFlagProducer` (77), `zero_flag` (101), `bind` (119), `preserve_sub_operands` (244), `arithmetic_flags` (273), `initialize_flags` (349), `replace_arithmetic_flags` (355), `observe_environment` (359), `snapshot` (365), `observed_zero_flag` (396).
3. In/out. Mutates `SyntaxTrees`; `bind` scans every expression in the tree for single-identifier names equal to a scratch register name (128-148) and replaces them with hidden `asm#scratch#{scope}#{name}` locals with initial 0/false (150-179).
4. Decides. Keyed on source spelling: scratch names matched against `NAMES` with string equality (143-146); slot index sets `[15, 16, 19, 20]` selects `u64`, `ASM_ZERO_FLAG_SLOT` selects `bool`, else `u32` (153-161); magic values `0x8d5`, `0x9669`, `4294967296`, `0x80000000`, `64` (285, 336, 309, 356, 398). Generated names like `"asm#sub_left"`, `"asm#flags"` are string identifiers (254-255).
5. Cases by shape. All hard-wired to 32-bit arithmetic (`unsigned_type` = u32) and x86 EFLAGS layout (CF/PF/AF/ZF/SF/OF bits, 273-348). The `[15,16,19,20]` slot numbers are a list of indices.
6. Same job elsewhere. none read.
7. Unused. Several are `pub(crate)`; `NAMES` re-export used in `assembly/mod.rs:270`; `name` used in `mod.rs:142`. Grepped `is_unmodeled_aarch64_scratch_view|ASM_SCRATCH_NAMES`: 8 occurrences in 3 files.
8. Stated limits. none in file.
9. Fail-open. `bind` scans the whole tree passed to it (the block's private tree, per `assembly/mod.rs:47-48`) and renames any matching name, including a user variable that happens to share a register name; whether such a variable is excluded earlier is not shown here. Open.

## declarations/assembly/instructions.rs (659 lines)

1. Purpose. Resolves one authored assembly instruction against the language-core catalog and builds the value operations (assignments, intrinsic calls, transitions) that implement it.
2. Shape. `PreparedInstruction` (28), `Arithmetic` (20), `prepare_instruction` (148, 500-line match), helpers `reject_bracketed_asm_operand` (46), `asm_intrinsic_call` (81), `asm_intrinsic_result_assignment` (107), `arithmetic_flag_producer` (134).
3. In/out. Reads `AssemblyInstruction`, `asm_catalog_entry(mnemonic)` (168); returns `PreparedInstruction` with the generated statement and the (possibly narrowed) contract. Mutates `SyntaxTrees`.
4. Decides. Keyed on mnemonic spelling via catalog lookup (168). Catalog gives `Contract` or `Refused(HiddenControlExit | UnmodeledMemoryAccess)` (182-205); `DeriverOnly` availability rejected (206-215). Register operands are recognized by single-identifier name + `scratch_register(name)` (255-266, 296). Destination `zf` is detected by the literal string "zf" (247).
5. Cases by shape. Quote (172-177): "only known-contract instructions compile (`hlt`, `in`, `out`, `jmp`, `mov`/`movq`, `lfence`, ... `ldr`, `str`...)". Arithmetic/compare: requires "a modeled 32-bit register destination" (278-300), source "a modeled 32-bit register or a fitting unsigned 32-bit immediate" with AArch64 immediate rule `<= 4095 || (v & 4095 == 0 && v >> 12 <= 4095)` (333-338). `SubtractThreeOperand` left must be a modeled register (310-325). Bracketed `[...]` memory operands rejected (46-67). `unreachable!` for `DerivedExit | DescriptorTableLoad` "deriver-only instructions refuse before source lowering" (645-646).
6. Same job elsewhere. none.
7. Unused. `prepare_instruction` `pub(super)`, one caller (`assembly/mod.rs:151`).
8. Stated limits. "no structured operand provenance/permission contract is modeled for it yet" (198-201).
9. Fail-open. `contract.target.intersect(...)` failing is an error (265, 303-308); an operand that is not a modeled register is not checked for target compatibility (255-266 skips non-registers). Traced: the mov path accepts any expression for destination/source other than bracketed and `zf` (230-267); typing later validates. Inferred, not traced.

## declarations/assembly/mod.rs (391 lines)

1. Purpose. Expands each authored assembly statement into a control-flow graph of blocks, generated value statements and scratch bindings that later stages consume.
2. Shape. `prepare` (24, `pub(crate)`), `expand` (67), `PreparedBlock` (341), `validate_asm_clobber_contract` (350).
3. In/out. Mutates `SyntaxTrees`: for each statement body that contains `AuthoredAssembly`, builds a private `SyntaxTrees` (`local`, 47-48), expands, and copies statements back (53). Emits `StatementNode::Assembly(AssemblyControlFlow {target_requirement, blocks, scratch, primitives})` (323-334).
4. Decides. Target compatibility: intersect of catalog targets (137-139, 152-154). Clobbers: "must exactly match the realized instruction contract" (378-380) using `BTreeSet<String>` of register names. Labels: duplicate and undeclared jump targets by string equality of label text (206-231). Unmodeled register views by name (249-267).
5. Cases by shape. A control transfer must be the final instruction in its block (116-121). `ensures` requires a falling-through block (199-203, 285-292). x86 flag snapshots cannot be combined with AArch64 target (272-277); scratch registers with `popfq` refused (278-283): "scratch-register CFG flags cannot yet be combined with hardware popfq restoration" (281).
6. Same job elsewhere. none.
7. Unused. `prepare` called at `declarations/mod.rs:39` and `tests/assembly_tests.rs:11` (grepped).
8. Stated limits. quote at 253: "register aliases require shared-bit state modeling".
9. Fail-open. `prepare` returns `Ok(())` when the syntax has no authored assemblies (25-27); stated. The `nop` is dropped from block instructions (181-185). `unwrap`-style `expect("assembly has a nonempty block")` at 284 (a block list can be empty only if the parser permits it; not traced).

## declarations/builtin_type_names.rs (67 lines)

1. Purpose. Refuses declarations that introduce a type or namespace name spelled like a builtin type (`data u7`, a generic type binder named `u32`).
2. Shape. `validate` (23).
3. Reads the syntax root items (and nested machines) and the type-parameter table; returns diagnostics.
4. Decides on role of the introduced name and the spelling `is_builtin_type_spelling(spelling)` (27). Roles checked: Data, TypeAlias, Trait, type binders of kind `Type` (41-43, 57-61).
5. Cases by shape. Walks nested machines only for `Machine` items (44-49); traits/data nested elsewhere not reached. Quote: "The language has no import alias yet, so no import is checked." (13-14).
6. Same job elsewhere. none.
7. `validate` has one caller (`declarations/mod.rs:71`, grepped).
8. Stated limit above.
9. Fail-open. Domains, conformances, constants and modules do not appear in the checked roles though the header lists "module" as a refused role (12-13): modules are not iterated here. Open (not traced where module names are checked).

## declarations/callable_signatures.rs (270 lines)

1. Purpose. Computes, per callable signature, which names its own binders hide (type, value, lifetime) and the type and expression inputs the signature contributes, so substitution does not replace a signature's own formals with an equally spelled outer constant.
2. Shape. `InputScope` (13), `SignatureInputs` {scopes, types, expressions} (21), `of` (32), `hides_type/hides_lifetime/hides_value` (41, 58, 75), `bound_values/bound_types` (92, 107), `signature` (119).
3. Builds scopes recursively (nested structural machine contracts, 192-196; proposition contracts, 211-246).
4. Decides on name strings (`bound == name`) per scope chain.
5. Cases by shape. "Syntax has not selected type versus value roles for a named generic operand yet" so value names are added to type names (152-154).
6. Same job elsewhere. Scope-by-name hiding is also done with `bound: &[String]` lists in `declared_types.rs` (e.g. 12, 23-663).
7. `SignatureInputs::of` users: `declared_types.rs:1106`, `templates.rs:336,495,726` (read). 
8. Stated limit: line 152.
9. none seen.

## declarations/callable_types.rs (204 lines)

1. Purpose. Turns a const parameter whose type is a path `Trait::requirement` (or an applied trait path) into a nominal machine-parameter contract.
2. Shape. `nominal_contract` (19), `applied_member_not_implemented` (112), `is_trait_owner` (119), `split_member` (132), `joined_name` (142), `argument_type` (161).
3. Reads type-reference nodes; mutates syntax type tables; returns `MachineParameterContract::Nominal`.
4. Decides on spelling: `split_member` splits on last `::` (133); owner is a trait if `type_declaration_kind(owner) == Some(Trait)` and its head is not a bound binder name (125-129).
5. Cases by shape. Named, Generic (always errors), StaticSelection only (25-89). Member application: "applying a nominal requirement member in a callable parameter type is not implemented" (113-115). Trait lifetime arguments refused (60-65).
6. Same job elsewhere. `argument_type` (161-204) and `declared_types.rs:526-606` `argument_reference` convert `StaticMachineArgument` to a type reference and differ in handling `evidence_projection` and `application.member`. Grepped `argument_type\(|argument_reference\(` in this crate: 18 occurrences in 5 files.
7. `nominal_contract` called from `declared_types.rs:1284` (read).
8. Stated limits. 109-111: "Supporting that applied type remains compiler work."
9. none seen: unmatched shapes return `Ok(None)` (88, 91), meaning "not a nominal contract"; wrong program: a trait-owner path written as a shape not in the list stays a plain const type and is checked later. Open.

## declarations/declaration_selection.rs (1801 lines)

1. Purpose. `ConstantSelection`: a private partial namespace (header symbols, imports, builtins) built before full symbol assignment, used by preparation, initializer handling and resolution to select constants, data carriers, domains, traits and implementations.
2. Shape. `ConstantSelection` {symbols, alias_bodies, retained, array_lengths} (43-51). `SelectionBase` {Borrowed, Shared} (58). Constructors `new` (151), `with_retained_base` (178), `with_shared_base` (159), `activate_sources` (103). Selection methods: `data`/`data_in` (713, 725), `constructor` (981), `builtin_type` (1000), `type_declaration_kind` (1020), `trait_lookup` (1055), `domain` (651), `domain_family` (687), `generic_carrier_domain_address` (299), `retained_identity` (412), `retained_domain_identity` (449), `select` (1478), `const_header` (1402), `selects_const` (1162), `retained_const_name` (1083), `generic_parameter_roles` (869), `selected_array_length` (1443), `require_closed_data_leaf` (761). Free fns: `type_mentions_parameters` (1559), `resolved_generic_argument` (1619), `substituted_generic_arguments` (1641), `has_deferred_const_argument` (1663). `RootDeclarations` (1706). `GenericApplicationSubstitution = HashMap<String, TypeReferenceHandle>` (1550).
3. In/out. Reads syntax root items and an optional retained `SymbolResolvedTrees`; builds a `SymbolTable` (`SymbolTableBuilder`, 249-281). Returns handles/definitions/spans; no syntax mutation. `install_integer_types` mutates the header table (1013).
4. Decides. Name selection delegates to `symbols.find_top_level_by_name_and_kinds_from_source(name, kinds, span)` (a lookup by spelling anchored at the use span, e.g. 418, 741, 834). Declaration custody is rejoined by span + kind + name equality (`source_declaration_item_among` 1254-1299). Core layout storage is recognized by `name == CONSTRUCTOR_NAME`, `BoundaryOpaque`, toolchain origin, no package identity, path equal to a file name, and source bytes equal to the bundled file (84-95): identity by content. Domain pooling uses `display_path` string equality and `same_symbol_source_package` (667-673, 697-704).
5. Cases by shape. `with_retained_base` declares only Const, Data, TypeAlias, Domain, Trait, Machine, Conformance (alias only), and namespace Use (186-236). `retained_const_name` tries: full declaration path, import-prefix paths, bare name (1113-1135). `type_mentions_parameters`: "an expression-bearing position ... conservatively counts as open" (1555-1558, `ConstExpression => true` 1608).
6. Same job elsewhere. Domain selection by pooled candidates is repeated in `generic_data/arguments.rs:736-758` (`indexed_domain_identity`) and `facts.rs:769-821` (`evaluate_named_const_domain`), both calling `domain`/`domain_family`. `lookup_path_for_selected` (1346-1393) and `retained_const_name` (1083-1153) both build candidate import-prefixed spellings and re-check each against the selected header.
7. Unused/one-caller. Grepped `require_closed_data_leaf|has_deferred_const_argument|with_shared_base|.trait_lookup(|type_mentions_parameters(` over `source-to-psi`: `require_closed_data_leaf` 1 external call (`constants/initializer_leaves.rs:166`); `has_deferred_const_argument` 1 external call (`initializer_leaves.rs:48`); `type_mentions_parameters` 1 external call (`initializer_leaves.rs:47,68`); `trait_lookup` 4 calls (`trait_defaults.rs:127,146,210,392`); `with_shared_base` 3 external calls (`declarations/mod.rs:73`, `pre_resolution/mod.rs:177`, test `domain_exposure_tests.rs:56`). `ConstantSelection` is named 197 times in 31 files (grepped `ConstantSelection` over `source-rust/subsystems/compiler`, `*.rs`, count mode), consistent with its header claim "Preparation, initializer handling and resolution share this owner" (6-8).
8. Stated limits. 1555-1558 conservative `true`; 1630-1632 "computed constant carrier member is not yet a closed structural type".
9. Fail-open. (a) `constant selection` returns `Ok(None)` / `None` for absent or ambiguous selections and says callers "must defer it to ordinary validation" (1473-1477): stated. Wrong program: a name that finds no constant stays as written and is validated later; traced: yes for `select` callers in this unit (they `continue`/`return Ok(None)`). (b) `domain` and `domain_family` return `None` for contested owners (651-706): documented as "undecidable ... Callers keep the fact as a checked obligation" (645-650): stated. (c) `selects_const` counts private or ambiguous candidates as constants (1155-1161): stated. (d) `alias_body` returns `None` when more than one alias row has the same span (848-860): then expansion errors "selected type alias has no authored body" (`declared_types.rs:812-819`): fail-closed. (e) `retained_base()` absent: `retained_identity` etc. return `None` (423): traced no accept.

## declarations/declared_types.rs (1768 lines)

1. Purpose. Expands type aliases at their use sites (substituting arguments, lifetimes, array lengths and const expressions), wraps const callable parameter types into machine contracts, and marks alias application provenance, before generic-data synthesis.
2. Shape. `mod expression_bindings` (23-210: `bind_const_expression` 51, `supplied_expressions` 156), `mod static_operands` (211-664: statement/expression/contract/static-argument walkers on `Expansion`, `argument_reference` 526, `argument_name` 608, `proof_bindings` 631). `Expansion {selection, active, supplied}` (666). `reference` (673), `application` (802), `bind_type_occurrences` (957), `normalize` (1173, `pub(super)`), `needs_normalization` (1216), `mark_header_references` (1233), item/machine/signature/statement walkers (1257-1557). Tests (1559-1768).
3. In/out. Mutates `SyntaxTrees`: instantiates the alias expansion (`instantiate_type_reference_from`, 879), `replace_type_occurrence` (901), `retain_type_alias_application` (906-913). Records `TypeAliasApplicationOrigin`. Clones the whole tree as `templates` (1181).
4. Decides. Alias heads chosen by `selection.type_alias_source(syntax, &head)` after skipping names in the `bound: &[String]` list (699-702): spelling-keyed, with lexical binder list as shadow. Alias formals substituted by name equality with `alias.type_parameters` (1011-1019, 100-101). Recursion: `self.active.contains(&declaration)` source-span stack (828). Value-name shadowing for const expressions: hides_value, proof bindings, and literal name `"result"` (90).
5. Cases by shape. Unresolved alias expression operand errors: "transparent alias expansion cannot yet retain the declaration and supplying scopes of this expression operand" (869); "transparent alias RHS expression cannot yet preserve explicitly supplied declaration custody" (171-174). `needs_normalization` (1216-1230): runs only if aliases exist or a const parameter's type is a StaticSelection or a `::`-containing name. Alias arity must match exactly (820-826). `bind_const_expression` handles only single-name `Name` and `Name{..}`/`Call` nodes as unresolved (72-123).
6. Same job elsewhere. Parameter-binding name lists via `parameters()` (1258-1309) mirror `SignatureInputs` (`callable_signatures.rs`).
7. `normalize` called in `declarations/mod.rs` (inferred; `pub(super)`), not grepped.
8. Stated limits. quotes at 171-174 and 869 above.
9. Fail-open. `Expansion::reference` ignores (`_ => {}`) any type node kind not listed (776) and any head not selecting an alias: expected. `reference` returns early when `self.supplied.contains(&reference)` (679): a copied supplied operand is not re-expanded; stated by the comment on `supplied` ("Source application rows retain ... independently", 8-9 header). Traced: no wrong accept; supplied operands expand at their own site.

## declarations/dispatches.rs (313 lines)

1. Purpose. Expands an authored `transition` dispatch into ordered transition statements that share one evaluation of each runtime subject.
2. Shape. `prepare` (17, `pub`), `expand` (48), `capture_subjects` (87), `prepare_pattern` (127), `arm_pattern_is_condition` (178), `closed_boolean` (197), `covers_boolean_subjects` (210). Tests (238-313).
3. Mutates `SyntaxTrees` statement bodies: adds a `LocalData` (role `DispatchSubject`) per non-closed-boolean subject named `__transition_subject#{span.start}.{index}` (101-108), then a `Transition` per arm.
4. Decides. Subject evaluated once unless it is a closed boolean (`Boolean` or `bool == bool`, 197-208). Last arm unconditional only if boolean coverage is exhaustive (54-58). `arm_pattern_is_condition` classifies a `Value` pattern as condition by operator kind: And, Or, Equal, NotEqual, comparison operators, Membership, `LogicalNot` (178-195): shape-based.
5. Cases by shape. `covers_boolean_subjects`: "(1..=8).contains(&count)" subjects, single subject needs exactly 2 arms, all arms unguarded, patterns only Wildcard or Boolean Value, truth-table check over 2^count (210-235).
6. Same job elsewhere. none.
7. `prepare` is `pub` and re-exported `resolver.rs:20` as `prepare_dispatches` (grepped); called `declarations/mod.rs:40`.
8. Stated limits. none ("1..=8" subject count is a hard cap, 211).
9. Fail-open. `covers_boolean_subjects` returns false for any non-boolean-literal pattern (225-228), so exhaustiveness is conservatively not claimed. `prepare_pattern` drops `Wildcard` patterns (133). Traced: no wrong accept in this file.

## declarations/generic_data/arguments.rs (944 lines)

1. Purpose. Computes closed argument identities for generic applications (`ClosedArgumentIdentity`), argument slugs for instance names, lifetime canonicalization, and the domain-head normalization that rewrites `Carrier::Domain<..>` into a constrained type.
2. Shape. `complete_argument_tuple` (40), `monomorphizable_argument_slugs` (72), `canonicalize_monomorphizable_argument_handles` (93), `canonicalize_lifetime_bearing_type_argument` (118), `exact_synthesized_lifetime_instance` (212), `type_reference_slug` (234), `constraint_slug` (331), `closed_argument_identities` (364), `closed_parameter_argument_identity` (387), `closed_argument_identity` (430), `domain_index_telescope` (676), `selected_domain_family` (700), `indexed_domain_identity` (736), `DomainHead` (764), `DomainCarrier` (788), `domain_application_head` (823), `normalize_domain_head_positions` (868).
3. Reads syntax and `ConstantSelection`; mutates syntax in `normalize_domain_head_node` (883-943: replaces `Generic` with `Constrained`). Produces identity values.
4. Decides. Identity is by item handle for data (`Nominal(ItemHandle)` 530), by symbol for retained data (`RetainedNominal`, 532), by builtin atom (527), and by string for constants: `Constant(String)` from a bare name or a path member (504-505, 518-520). Domain identity: item handle or `Retained` symbol, with the carrier check `closed_argument_identity(domain.target_type) != base` (591-601). Fallback without a selection scans root items matching `domain.name == name` or last-`::` segment (630-654).
5. Cases by shape. Open or unsupported shapes return `None` ("Unsupported or open shapes remain on the ordinary generic path", 231-232); `Machine` and `Proposition` parameters have no closed identity (426); `FixedArray` of length 0 has no slug (300-302); lifetime canonicalization is "deliberately positional" ("Broader permutations remain on the unnormalized path", 87-92).
6. Same job elsewhere. `domain_index_telescope` (676-693) and `const_evaluation/domains.rs:59-77` (`domain_index_parameters`) both compute the index parameters after dropping a leading type binder that names the carrier; grepped `domain_index_telescope|domain_index_parameters`: 13 occurrences in 6 files; the header at 668-675 calls its own version "the one rule shared by identity, argument normalization and the structural matcher", yet a second implementation exists in `domains.rs:59` (read) and is used by `facts.rs:800,1082` and `domains.rs:188`. Also `closed_name_identity` (`substitution.rs:450-467`) mirrors the `Named` arm at 525-536 (stated at `substitution.rs:446-449`).
7. Unused. `type_reference_slug`, `closed_argument_identity`, `domain_application_head`, `normalize_domain_head_positions`, `selected_domain_family` are re-exported `pub(crate)` in `generic_data/mod.rs:47-51`; callers outside unit not counted.
8. Stated limits. 87-92 quote above.
9. Fail-open. Identity returning `None` means "not closed" and the application stays unsynthesized (`prepare_closed_application` returns `Ok(None)`, `const_evaluation/arguments.rs:278, 295`): the program then goes to the ordinary typing path. Wrong program: one whose argument is a spelling the identity function cannot classify skips specialization and is typed in its generic form. Stated at `arguments.rs:30-37` ("left UNTOUCHED for the existing type-check-only path (skip, never reject)"). `Constant(String)` identity compares spellings, so two distinct constants that render the same string would collide; not traced.

## declarations/generic_data/const_evaluation/anonymous.rs (419 lines)

1. Purpose. Exact rational evaluation of wholly anonymous (untyped literal) arithmetic, and the decision of whether a binary node has a builtin meaning or needs a user-authored operator.
2. Shape. `AnonymousNumericValue` (19), `evaluate_anonymous_integer_argument` (30), `evaluate_anonymous_numeric_expression` (40), `into_integer` (115), `binary` (137), `requires_const_operator_selection` (164), `has_builtin_const_operator` (186), `binary_operator_spelling` (201), `has_no_authored_spelling` (224), `authored_spelling_parameters` (237), `machine_parameter_types` (266), `authored_operator_can_bind` (303), `parameter_binds` (329), `anonymous_numeric_expression` (374).
3. Reads syntax; uses `BigRational`. Returns values or error strings; pushes `Diagnostic::error` into `warnings` when a fractional intermediate lands implicitly (128-131).
4. Decides. Operator meaning is decided by scanning all root items for any machine/operator/domain/trait that authors the same `OperatorSpelling` (224-259): any authored spelling anywhere makes `has_no_authored_spelling` false, unless both operands are anonymous (190-193). `authored_operator_can_bind` refines by parameter carrier names (329-372).
5. Cases by shape. Anonymous operators: Add, Subtract, Multiply, Divide only, policy must be `Exact` (399-407). Boolean/bitwise/shift operators have no spelling (214-220). `parameter_binds` match: DeclaredInteger vs Integer/Address atoms, Integer/Anonymous vs integer atoms, Boolean vs Bool, else false (356-371).
6. Same job elsewhere. `evaluate_anonymous_numeric_expression` (40) duplicates arithmetic with `AnonymousNumericValue::binary` (137-160) (same four operators).
7. Unused. `evaluate_anonymous_numeric_expression` used in `facts.rs:102,1584`; `requires_const_operator_selection` in `arguments.rs:76`, `domains.rs:421`, `facts.rs:1292`.
8. Stated limits. "Keep authored operator applications intact until declaration selection." (163).
9. Fail-open. `parameter_binds` returns `true` when the parameter type is unreadable (no named type), selection absent, or the parameter type does not name a declared type kind (`.is_none()` 349-353): comment "yields `None`, which may bind any operand" (233-236). Effect: `authored_operator_can_bind` answers "an authored operator could bind", which makes the caller decline to fold (`facts.rs:234-237`), i.e. conservative (undecided). Stated. `has_builtin_const_operator` returns `true` (builtin) when `binary_operator_spelling` is `None` (195-197): And/Or/bitwise/shift ops are always treated as builtin even when a user declares an operator; stated by the missing spelling enum; traced: those ops cannot be authored as spellings.

## declarations/generic_data/const_evaluation/arguments.rs (748 lines)

1. Purpose. Evaluates and normalizes const arguments of generic data applications (literals, named constants, anonymous arithmetic, symbolic const-parameter arithmetic), prepares a closed application (`ClosedApplication`) for synthesis, and holds the single integer-operator evaluator.
2. Shape. `prepare_closed_application` (38), `const_arguments_fit_declarations` (307), `evaluate_const_argument_expression` (356), `admit_integer_operator` (474), `apply_integer_operator` (511), `const_integer_type` (594), `validate_declared_integer` (610), `generic_const_integer_types` (626), `evaluate_declared_width_operation` (648), `EvaluatedConst` {Concrete, Symbolic} (725).
3. Mutates syntax: replaces argument type references with generated `Named` leaves holding decimal text or atoms (122-125, 161-164, 198-201, 231-234), and records const-argument normalization (146-160). Returns `Option<ClosedApplication>`.
4. Decides. Arity: skip if more arguments than parameters, or fewer and no type equations (63-67). Const argument spelling decides: canonical atom / integer / boolean via `ConstScalarSpelling::from_bare_name(name)` (109); otherwise selected via `selection.select`. Types compared by name: boolean allowed only when the const parameter type resolves to a `Named` "bool" (115-119). A name evaluates through `parameter_values.get(&name)` / `symbolic_parameters.contains(&name)` (HashMap/HashSet keyed by spelling, 378-380).
5. Cases by shape. Call-valued const expressions return `Ok(None)` (204-209); struct/array literal arguments canonicalized as a whole (169-203). Operators admitted: Add, Subtract, Multiply, Divide, Modulo, shifts, bitwise (485-497); everything else: "only integer arithmetic, shifts, and bitwise operators are supported" (499). Multiply capped by bit length versus `MAXIMUM_CONST_INTEGER_BITS` (531-541). Quote at 29-37: "Anything Phase 1 cannot lower completely ... is left UNTOUCHED ... (skip, never reject)".
6. Same job elsewhere. `apply_integer_operator` is called from `fact_values.rs:191,288` and `initializers.rs:612` (read), so it is genuinely the shared integer evaluator (comment 504-510 "Every constant evaluator in this stage computes through here"); `anonymous.rs:89-96` and `137-160` compute the four rational operators separately (different domain, exact rational).
7. Unused. `evaluate_declared_width_operation` has one caller in this file (566). `generic_const_integer_types` used by `templates.rs:570`, `substitution.rs:512`.
8. Stated limits. 29-37 above.
9. Fail-open. `prepare_closed_application` returns `Ok(None)` for non-generic base, wrong arity, call arguments, non-sluggable arguments, parameters not fitting declarations, or not fully monomorphizable (52-66, 79, 208, 274-285, 296): the application is left as written. Stated (comment 29-37). Wrong program accepted? A generic use with a call const argument is not specialized here and goes to later typing. In `ConstScalarSpelling::Boolean` case a boolean literal to a non-bool const parameter falls through to `selection.select`, and when that returns `None`, `continue` (130-133) leaves the argument unvalidated; `const_arguments_fit_declarations` later returns false for non-integer, non-canonical names (338-339), causing `Ok(None)`. Traced: skips, not accepts.

## declarations/generic_data/const_evaluation/domains.rs (463 lines)

1. Purpose. Closes const index arguments of indexed domain applications (`u64 in AtMost<256>`, `x as D<N>`) to canonical leaves, and validates the shape of each indexed domain family header.
2. Shape. `integer_literal_value` (24), `qualified_const_name` (30), `ClosedDomainFamily` (38), `domain_index_parameters` (59), `canonicalize_closed_domain_indices` (85), `canonicalize_closed_domain_application` (313).
3. Reads constrained type positions and cast `semantic_domain` paths; mutates argument type references (replacement with canonical decimal/atom, 365-372, 412-416, 449-452) and retains normalization receipts (400-411).
4. Decides. Family selected by `selection.domain_family(syntax, name, reference, None)` when a selection exists, else by `family.path == name` string equality (156-163, 286-293). Duplicate family check: same `family_path` and same checked package (264-274).
5. Cases by shape. Header must name each index binder directly in declaration order (237-258); index parameter kinds only Type or Const (200-227); "indexed domain `{}` supplies index arguments but declares no generic carrier/const telescope" (180). Argument shapes: Named (atoms, integers, booleans, selected const), ConstExpression (evaluate), else error "requires a closed const value or direct const binder" (454-459).
6. Same job elsewhere. `domain_index_parameters` vs `arguments.rs:676 domain_index_telescope` (see above, same rule, two implementations).
7. Unused. `integer_literal_value` used widely (`arguments.rs`, `values.rs`, `facts.rs`).
8. none.
9. Fail-open. `canonicalize_closed_domain_indices` skips (`continue`) when the application resolves to no family (294-296), when a Named argument is not in `concrete_data_positions` (376-418, falls to loop end), when `selection.select` finds no constant (379-381), and when a ConstExpression contains names or needs operator selection (420-424). Stated: "Prepare only the families selected by this completion's actual work" (154); other occurrences are left for ordinary resolution. Open for the `concrete_data_positions` exclusion (argument stays an unresolved name; whether later resolution rejects it was not traced).

## declarations/generic_data/const_evaluation/fact_values.rs (294 lines)

1. Purpose. Values used when discharging closed generic facts at instantiation time: anonymous rationals, plain integers, declared-type integers, booleans.
2. Shape. `ConstFactValue` (12), `ConstScalarValue` (30) with `from_canonical` (40), `into_fact_value` (58), `ConstScalarSpelling` (94) with `from_bare_name` (101), `into_integer` (113), `evaluate_const_fact_binary` (133), `evaluate_declared_integer_fact` (207).
3. Pure evaluation; warnings pushed by `into_integer`.
4. Decides on operand kind and operator. Declared integers: carriers must match unless shift (226-232); anonymous/plain integers adopt the declared carrier (236-242). Zero divisor/remainder error (268-274).
5. Cases by shape. Quote 201-203: "shifts and bitwise operators require retained declared-width semantics" for non-declared operands. Mixed Boolean/integer: error "const fact operands have incompatible types" (184, 187). Spelling recognition order: canonical atom, decimal integer, boolean literal (101-109).
6. Same job elsewhere. none.
7. `evaluate_const_fact_binary` is `pub(crate)`; used in `facts.rs:239,1669`, plus exports. 
8. none beyond 201-203.
9. none seen: unrecognized operand combinations return `Err` (244-247).

## declarations/generic_data/const_evaluation/facts.rs (1685 lines)

1. Purpose. Replays proof facts (`requires`, domain facts, memberships) over known closed values at declaration time to decide a fact true/false/undecided, for generic instances, data-constant predicates, and constrained const carriers.
2. Shape. `evaluate_const_fact_expression` (52), `evaluate_const_fact_expression_at` (73), `evaluate_const_membership_fact` (255), `ConstSubjectBinding` (319), `ConstFactSubject` (346), `const_field_projection` (417), `prove_const_data_predicates` (510), `evaluate_selected_const_fact` (630), `DomainFactProof` (713), `evaluate_const_domain_constraint` (733), `evaluate_named_const_domain` (769), `module_domain_key` (828), `carrier_satisfies_declared_bounds` (841), `evaluate_selected_domain_facts` (868), `nested_membership_operand` (991), `evaluate_indexed_const_domain` (1038), `evaluate_domain_index_argument` (1198), `prove_const_leaf_domain_constraints` (1377), `const_type_requires_declaration_proof` (1476), `evaluate_const_domain_expression` (1574).
3. Reads syntax, const parameter maps, `ConstantSelection`; pure except `warnings`.
4. Decides. The subject of a domain fact is recognized by the literal name `self` (`ConstFactSubject::matches` 370; `const_field_projection` 443-445). Domain families are selected via `selection.domain`/`domain_family` (781-786); without a selection, by `domain.name == authored || == "{carrier}::{authored}"` string compare (788-795, 1056-1065). Carrier compared by name: `domain_carrier != carrier` is an error (917-921). Recursion guard keyed on `(SourceSpan, domain_key string)` (879-883).
5. Cases by shape. `evaluate_selected_domain_facts` returns `Ok(None)` when the domain has `authored_routes` or `access_policy` (885-887), a non-named/non-array target (907-916), an empty alias (929-931). Leaf proof binds only Integer, Boolean, Record, Array, Variant nodes (1393-1455); anything else: `CONSTRAINED_CONST_FENCE` error (1454). `Binary` facts require `ArithmeticDomain::Exact` (215, 1625).
6. Same job elsewhere. `evaluate_const_domain_expression` (1574-1685) and `evaluate_const_fact_expression_at` (73-248) both implement Binary evaluation with identical `builtin`/`authored_operator_can_bind` guards (218-239 vs 1628-1670).
7. Unused. `evaluate_const_membership_fact` called in `synthesis.rs:526` (read); `const_type_requires_declaration_proof` re-exported and used by `generic_data/mod.rs:96`.
8. Stated limits. Header 9-12: "A generic carrier does not supply a selected algebra for the subject; only carrier-independent facts can close here." `CONSTRAINED_CONST_FENCE` (1328): "constrained const declarations require declaration-site proof checking before they can publish compatibility identity".
9. Fail-open. Undecided (`Ok(None)`) when: unbound name (163), unselected constant (152), unknown operators, authored routes/access policy (885-887), recursion (879-883). Consumers: `prove_const_data_predicates` treats anything but `Boolean(true)` as an error (606-620) and `prove_const_domain_constraint` maps `None` to the fence error (1363): fail-closed. In `synthesis.rs:541-544` a fact is dropped only when `const_result == Some(true)`; `None`/`Some(false)` keep the fact on the instance for later checking: traced, fail-closed. `evaluate_const_membership_fact` returns `Ok(None)` for any membership with domain arguments (263-265) or non-name value (266-275). Traced: keeps fact.

## declarations/generic_data/const_evaluation/initializers.rs (1040 lines)

1. Purpose. Folds closed scalar constant initializers (`const LIMIT: u64 = BASE + 1;`) into literals before symbols exist, leaving a normalization receipt for resolution to re-check.
2. Shape. `fold_closed_scalar_initializers` (175, `pub(crate)`), `advance` (245), `closed_shape` (420), `evaluate` (522), `materialize` (660), `Fold` {Unvisited, Active, Carried, Closed} (115), `Advance` (135), `Custodies` (87), tests (717-1039).
3. In/out. Reads constants of the forest and retained base constants; mutates `syntax.items` (replaces const item with `materialized_initializer` and `normalization` receipt, 686-708).
4. Decides. Dependency names selected via `selection.const_header(syntax, name)` (301-303), joined to forest constants by declaration span (`forest_constant`, 234-243) or retained base `const_declaration_at_source_span` (324-349). Privacy check `!is_public && !same_checked_package` (351-358). Cycle: depth-first with `Fold::Active` (313-319).
5. Cases by shape. `closed_shape` (420-469): destination must be a `Named` builtin Integer or Bool; expression only Integer literals (integer carrier), Names, Binary with `Exact` policy and `has_builtin_const_operator`; else `None` and the constant is `Carried`. Boolean constant folds only another Boolean constant name (531-535). Quote header 6-10: "An initializer that would have to run a machine (a call, a match, a cast, an operator some declaration authors) is not this stage's to evaluate and is left exactly as written."
6. Same job elsewhere. Custody construction duplicates what `initializer_normalization.rs` verifies (625-653 vs `finalize` 1599-1627), by design (header 23-28: "Resolution re-derives that inventory ... so a fold that named the wrong dependency fails there").
7. `fold_closed_scalar_initializers` re-exported (`const_evaluation/mod.rs:31`, `generic_data/mod.rs:58`); callers outside unit not grepped.
8. Stated limits. header 6-10.
9. Fail-open. `Fold::Carried` (leave unfolded) in many cases: no header, no retained base, retained without value, calls in custody, not a closed shape (296-298, 304-306, 320, 324-332, 364-366). Stated (header). A literal that does not fit its type: `evaluated_declaration_value(...)` `Err(_) => Fold::Carried` (292) with comment "its own declaration check reports it" (276-279): stated; traced no.

## declarations/generic_data/const_evaluation/mod.rs (46 lines)

Purpose: module wiring and re-exports for the const evaluator (`anonymous`, `arguments`, `domains`, `fact_values`, `facts`, `initializers`, `remainder`, `templates`, `values`). Shape: re-export lists 15-46 with visibility scoped `pub(super)`, `pub(crate)`, `pub(in crate::resolution::declarations...)`. No decisions, no fail-open. Same job elsewhere: n/a. Header: "Exact source-constant evaluation used by declaration retention and synthesis." (1).

## declarations/generic_data/const_evaluation/remainder.rs (22 lines)

1. Purpose. Rejects `%` over two wholly anonymous operands.
2. `validate_anonymous_remainder` (11).
3. Reads syntax; returns `Result<(), String>`.
4. Decides on operator `Modulo` and `anonymous_numeric_expression` of both sides (15-17).
5. Message (19): "builtin `%` requires an integer-typed operand; type an operand (for example, `7i32 % 2`), not just the destination".
6. Called in `arguments.rs:426`, `facts.rs:227,1637`, `initializers.rs:605`.
7. none. 8. none. 9. none seen.

## declarations/generic_data/const_evaluation/templates.rs (923 lines)

1. Purpose. Template-level helpers for generic data synthesis: names captured by runtime bindings (so const substitution skips them), const-name replacement, placeholder normalization of const expressions in generic templates, and the type-position traversals that feed synthesis.
2. Shape. `replace_const_expression_names_from` (25), `RuntimeTemplateCaptures` (43), `capture_machine_runtime_template_names` (54), `replace_machine_const_expression_names_from` (131), `update_captured_names` (145), `TypeExpressionInputs` (195), `TypeInputWalk` (211), `replace_uncaptured_const_names` (383), `normalize_generic_template_const_expressions` (427), `normalize_template_type_reference` (483), `collect_selected_type_reference_positions` (637), `collect_stored_type_reference_positions` (655), `collect_data_type_reference_positions` (668), `collect_type_positions` (687), `visit_type_positions` (702), `collect_owned_type_reference_positions` (803), `visit_callable_type_inputs` (868).
3. Mutates syntax: replaces Name expressions with value nodes (413-415) and replaces const-argument type references with `Named(generated(text))` (589-591, 619-622).
4. Decides. Replacement is keyed on spelling: `const_expressions.get(name.as_str(syntax))` for a single-segment Name (404-410), unless the handle is in the captured set. Captures decided by `ConstructorFrontier::captures` (outside unit), i.e. whether a name is a parameter or an earlier local (114-125).
5. Cases by shape. `normalize_template_type_reference` : StaticSelection errors "a scoped type selection must resolve before scalar template normalization" (625-627). `visit_type_positions` stops at Callable in storage traversal and at `Reference` in storage traversal (723-725, 744-746). Only data with empty `type_parameters` and constants are position owners (814-842); machines through `concrete_machine_declarations` (873).
6. Same job elsewhere. Type-position walking is also in `declarations/generic_data/uses.rs` (outside unit) and in `declared_types.rs:957-1165` (`bind_type_occurrences`); `visit_static_argument_type_inputs` imported from `uses`.
7. Unused. Many `pub(in ...generic_data)`; used in `synthesis.rs`.
8. Stated limits. Quote 421-425: "A symbolic const expression cannot cross that boundary yet, so reduce each template expression to either its concrete value or one declared const-parameter dependency."
9. Fail-open. `replace_uncaptured_const_names` replaces any same-spelled single-segment name that is in the watermark range and not captured; a name that is neither a parameter nor in the captured set but spelled like a const parameter is replaced. Mitigated by the capture analysis (54-129). Open: correctness depends on `captures` (not read).

## declarations/generic_data/const_evaluation/values.rs (1957 lines)

1. Purpose. Builds canonical structural const values (`CanonicalConstNode` -> `CanonicalConstValue`) from const declarations and index expressions, validating carrier eligibility, arrays, nominal records/variants and predicates; computes canonical carrier labels.
2. Shape. `CanonicalConstNode` (54) with `encoding` (87), `display` (129), `framed` (182); `canonicalize_selected_const_definition(_in_application)` (196, 211); `canonicalize_scoped_index_expression` (279); `syntax_type_identity` (300); `selected_data` (375); `selected_synthesized_instance` (410); `selected_constructor` (438); `canonical_const_result(_in_application)` (472, 487); `selected_type_label` (505); `generic_application_label` (562); `closed_application_argument_labels` (586); `qualify_application_label` (659); `synthesized_instance` (688); `same_carrier(_in_application)` (770, 728); `representation_semantic_type` (865); `validate_selected_const_index_type` (899); `canonicalize_selected_declaration_value(_with_admission)` (916, 932); `closed_fixed_array_length` (959); `declared_fixed_array_length` (980); `validate_selected_const_type` (1054); `canonicalize_const_expression(_with_admission)` (1266, 1283); `substituted_type_label` (1546); `canonicalize_data_const_expression_with_admission` (1602); `canonicalize_selected_data_const_expression_with_admission` (1621); `canonicalize_named_fields_with_admission` (1795); `validate_canonical_rat` (1861); `nat_value` (1911); `gcd_usize` (1941); `const_integer_within_budget` (1953); `MAXIMUM_CONST_INTEGER_BITS` (4).
3. Reads syntax + selection; returns `CanonicalConstValue {type_name, encoding, display}`; no mutation. Encoding is a length-framed string (182-194).
4. Decides. Nominal carrier equality by pointer identity of selected declarations `std::ptr::eq(selected_data(left), selected_data(right))` (804-807). Builtin vs builtin by atom (802). Generic instance identity via `closed_argument_identity` (786-790, 826-836). Labels: `format!("{}<{}>", name, slugs)` display strings (574-578), qualified with module prefix (659-681). Rat canonicality is decided by the label string `required == "Rat"` (495) and the field names `"num"`, `"den"`, `"neg"`, `"pos"`, `"prev"` and type name `"Nat"`, cases `"Zero"`, `"Succ"` (1869-1937). Floating and bool by name: `type_name == "bool"` (1378), `"f32" | "f64"` (1385).
5. Cases by shape. `validate_selected_const_type` (1054-1264): scalars (Bool/ints/Address; floats only for Declaration purpose), data with no type or lifetime parameters, not `BoundaryOpaque`, no quotient; generic applications only closed and lifetime-free; Unit; Reference/Slice/Callable/ConstExpression/StaticSelection rejected: "const index types require finite structural values with decidable equality and one canonical form" (1260). Core representation (`Layout`-storage) wrappers unwrap to the semantic type (865-897). Unit literals: "unit const values do not yet have a source literal" (1533-1535).
6. Same job elsewhere. Resolved-level encode: `initializer_normalization.rs:70-524` (see there). `closed_fixed_array_length` has a sibling in `initializer_normalization.rs:504-566` (`array_length`).
7. Unused. `canonicalize_selected_index_expression` used by `generic_data/mod.rs:219`; `closed_fixed_array_length` `pub(crate)` used by arguments/eligibility (read) and elsewhere.
8. Stated limits. 1133: "until quotient-backed canonical representatives land". `TASKS.md` EARLY-TERMINAL-JOBS names this file (crate map, not re-read by me).
9. Fail-open. `selected_type_label` falls to `syntax_type_identity` (550) for nodes it does not special-case, producing a spelling label instead of an error. `Rat` canonicality check applies only when the required label is exactly "Rat" (495-497). Wrong program: a user data type named `Rat` in another module would be forced through that check (stricter, not looser). A user type named something else that is semantically a rational gets no canonical-form check. `canonicalize_named_fields...` errors on missing/duplicate/unknown fields (1805-1827).

## declarations/generic_data/discovery.rs (156 lines)

1. Purpose. Private data types for generic data synthesis (`GenericData`, `PendingRewrite`, `ClosedApplication`, `Instantiation`, `GenericDataShape`) and item selection helpers.
2. Shape. 12, 32, 42, 57, 66; `selected_data_item` (77), `selected_data_item_in` (99), `selected_generic_data` (114), `ClosedArgumentIdentity` (128) and `ClosedConstraintIdentity` (146).
3. `selected_data_item` returns the ItemHandle whose `Data` equals the selected definition by pointer (84-86).
4. Decides. With a selection: delegates to `selection.data`; without: name equality with exactly one candidate (88-92).
5. Cases. none.
6. Same job elsewhere. `selected_data_item_in` (99) is the root-indexed variant.
7. `ClosedArgumentIdentity` (Eq/Hash-like keys) used in `synthesis.rs:192,244`; `selected_data_item` re-exported `pub(crate)`.
8. none.
9. Fail-open. `selected_data_item` without selection picks the unique name-equal data and returns `None` on ambiguity (88-92): ambiguity yields no selection.

## declarations/generic_data/domain_heads.rs (267 lines)

1. Purpose. Before substituting a carrier into a generic data type, resolves carrier-binder domain heads (`T in T::Marked`) to the exact generic-carrier domain family address, so later substitution does not change which family is selected.
2. `normalize` (27, `pub(crate)`), `named_carrier` (259).
3. Mutates syntax: rewrites `DomainConstraint.name` to a full address (227) and alias constituent paths (250-254).
4. Decides. For each data field type walk, a constraint name `Carrier::leaf` is rewritten only if `Carrier` is a binder of the data (`parameters.iter().find(name == carrier && !hidden.contains(carrier))`, 138-143); family chosen by `selection.generic_carrier_domain_address(syntax, leaf, reference)` (210, 238). Binder kind must be `Type` and the field's base type must be that named carrier else error (144-151).
5. Cases. Alias constituents: exactly `[binder, leaf]` (54-59). Walks Reference, FixedArray, Slice, Generic, Constrained, StaticSelection, Callable (101-197).
6. Same job elsewhere. `normalize_domain_head_positions` in `arguments.rs:868` does a similar rewrite for non-binder carriers.
7. `normalize` `pub(crate)`; caller not grepped.
8. none.
9. Fail-open. `continue` when `members.len() < 2`, carrier not a binder (45-53, 138-143): the name is left to ordinary lookup. Stated by header ("select the exact exposed generic-carrier family while the owning telescope is available"). Not traced.

## declarations/generic_data/eligibility.rs (357 lines)

1. Purpose. Decides whether a generic data declaration's members can all be substituted soundly ("fully monomorphizable") and detects recursion.
2. `base_is_fully_monomorphizable` (35), `generic_data_is_recursive` (93), `generic_inline_data_edges` (137), `generic_data_shape` (194), `type_reference_is_substitutable` (214), `type_reference_handle_is_substitutable` (232).
3. Pure over `GenericData` and syntax.
4. Decides by shape and parameter name: parameter set is `HashMap<String, _>` of names (44-48). Recursion: DFS over named edges, skipping names equal to a binder (113-119), using `selected_generic_data` (120).
5. Cases by shape. Records, pure sums, mixed sums (194-212). Substitutable: Named true; Generic if head not a parameter, selected as generic data, and every parameter-mentioning argument itself substitutable (242-270); FixedArray with element substitutable and length either literal/call/expression or const parameter that is a declared const binder or a closed extent (283-308); Reference, Slice, Constrained; default: not mentioning a parameter (355). Edges: Reference, Slice, Callable, ConstExpression, Unit contribute none (171-175).
6. Same job elsewhere. `type_reference_mentions_parameter` (`substitution.rs:820-876`) is the helper used here.
7. All `pub(in ...generic_data)`; used from `const_evaluation/arguments.rs:7,240,283`.
8. Header: "Closed children keep their existing sharing route".
9. Fail-open. `TypeReferenceNode::Named(_) => true` (241) regardless of whether it is a parameter or a concrete name. Header 31-34 states a field "may be exactly the parameter, a concrete Named". Traced: stated. Recursion through References, Slices and Callables is not an edge (171-175), so `data List<T> { next: &List<T> }` is not flagged recursive here; inline storage only (comment "inline storage edges" in `callable_signatures.rs:3`). Stated by the name "inline".

## declarations/generic_data/mod.rs (239 lines)

1. Purpose. Module owner and re-export surface of generic-data specialization utilities, plus three functions: `const_destination_requires_proof` (91), `qualification_names_itself` (108), `canonicalize_selected_declared_const_definition` (129), `validate_direct_const_arguments` (174).
2. In/out. Reads definitions; returns canonical values/diagnostics.
3. Decides. A constrained carrier with a Domain constraint canonicalizes at its base only if the qualification names itself, else error "constant qualification requires a checked declaration-site result judgment" (147-152). Direct arguments compared by `value.encoding != normalization.canonical_result_encoding` (231).
4. Callers (grepped): `const_destination_requires_proof` 5 (declaration_values.rs:5,143,260,328; lowering/item.rs:282; pre_resolution/mod.rs:9,187); `validate_direct_const_arguments` 1 (`lowering/type_reference.rs:369`).
5. Stated: header 1-8: "The private specializer consumes only the closed type roots introduced by equation completion... Its templates, substitutions and pending instances are private working state".
6. none for fail-open: `validate_direct_const_arguments` returns `Ok(())` if no argument has a direct normalization needing an expression (186-188); traced as non-applicable.

## declarations/generic_data/source_modules.rs (8 lines)

`module_path(selection, source)` returns the display path of the source's module via header symbols, or `None` without selection/module (4-8). Callers: `domains.rs:259`, `facts.rs:833,1070`, `synthesis.rs:96`. Used as a key prefix: judgments keyed on a module path string.

## declarations/generic_data/substitution.rs (920 lines)

1. Purpose. Substitutes type-parameter names in data members, type references, domain index arguments and cleanup selections when materializing a generic instance; decides instance-level type-equality facts and folds boolean skeletons for impossible cases.
2. Shape. `substitute_copied_type_names` (38), `substitute_member` (70), `expression_mentions_in` (134), `decide_instance_type_equalities` (163), `fold_impossible_case_memberships` (260), `fold_instance_boolean_node` (307), `VariantFactVerdict` (357), `variant_fact_verdict` (363), `closed_name_identity` (450), `substitute_data_field` (469), `substitute_type_reference` (488), `substitute_constrained_type_reference` (677), `substitute_domain_index_argument` (763), `expression_mentions_parameter` (806), `type_reference_mentions_parameter` (820), `substitute_cleanup_selection` (883).
3. Mutates `SyntaxTrees` (copy and replace type refs/expressions).
4. Decides. All substitution keyed by name string: `HashMap<String, TypeReferenceHandle>` (41, 72, 473). `variant_fact_verdict` decides on `self` spelling (399) and `head == type_name` string compare (403); integer literal decimal value 0 means Refuted, any other integer Holds (371-377). `decide_instance_type_equalities`: only `==`/`!=` between two single-name sides where one side is a listed type binder (190-202); equality is `ClosedArgumentIdentity` equality (215).
5. Cases by shape. `expression_mentions_in` returns `true` for any node shape other than Name, Binary, Unary, literals (158); `type_reference_mentions_parameter` returns `true` for unlisted shapes (874: "Conservative: ... returns `true` so the caller rejects rather than silently sharing a parameter-bearing type", 815-818). `ConstExpression` evaluation failure keeps the argument unchanged (549-552).
6. Same job elsewhere. `type_mentions_parameters` in `declaration_selection.rs:1559-1611` is a second implementation of "does this type mention a bound name" (different substitution map type but same walk); grepped 1 external caller. `closed_name_identity` mirrors `arguments.rs:525-536`.
7. `type_reference_mentions_parameter` used by `eligibility.rs:17,260,351`. Others `pub(crate)` re-exported.
8. Stated limits: header 34-37, 816-818.
9. Fail-open. `variant_fact_verdict` treats any nonzero integer literal as Holds (371-377); intended for folded type-equality witnesses (doc 347-351, "literal `0`/`1` leaves (the folded type-equality witnesses) read themselves"): stated. Wrong program: a data `requires` fact written as the literal `5` counts as Holds. Not traced to a guard. `substitute_type_reference` returns the original handle for Named not in the substitution (500-503) and for unlisted nodes (666).

## declarations/generic_data/synthesis.rs (746 lines)

1. Purpose. Discovers closed applications of generic data, runs a fixed-point loop that synthesizes one concrete `DataDefinition` per distinct (template, argument identity), rewrites uses to the synthesized name, and clones attached machines.
2. Shape. `desugar_generic_data_instances_in` (56, `pub(crate)`), `synthesize_instance` (365).
3. In/out. Reads syntax + `GenericDataWorkDomain` (type roots/declarations); mutates syntax: pushes new root `Item::Data` named `Template<args>` (638-654), and cloned machines named `{instance}::{method}` (681-684), rewrites type nodes to `Named(instance_name)` or `Generic{base: instance_name, lifetimes}` (327-340) after keeping the original in `retain_generic_application_origin` (322-326).
4. Decides. Templates exclude: quotient or conformance-bounded data (90-92); `Value`/`Machine` parameters (102-109); the exact names `"IntervalSet" | "CountedQuantity"` when unmoduled (93-99); data whose attached machines' parameters are not all covered by the data's parameters with the same const-ness (110-139). Instance reuse keyed on `template` handle and `argument_identity` equality (243-246, 375-379). Instance display name `Template<arg slugs>`, with `__{n}` suffix on display collision (253-266). Lookup name rewritten with module prefix from `selection.data_lookup_path` (268-279); no path -> `continue` (skip) (269-271). A rewrite occurs only if an instance existed already or the position is a storage position (247-249).
5. Cases by shape. Facts: type-equation facts dropped from instance (506-512); const facts evaluated, `Some(true)` dropped (541-543); impossible variants from `variant_fact_verdict == Refuted` get a witness fact `0` (`substitution.rs:117-126`). Const parameter binding requires a supported integer carrier else error (417-421); booleans handled separately (466-469).
6. Same job elsewhere. none.
7. `desugar_generic_data_instances_in` re-exported `pub(crate)`; caller not grepped. The names `IntervalSet`/`CountedQuantity`: grepped `"IntervalSet"|"CountedQuantity"` in `source-to-psi`: 1 match (synthesis.rs:95); no other use of these two spellings in that tree.
8. Stated limits. none; skip comment at 119 absent.
9. Fail-open. (a) Templates excluded at 90-109 are never specialized (not accepted/rejected here, left to ordinary path). (b) `continue` at 269-271 when `data_lookup_path` is `None`: the application is neither rewritten nor instantiated. (c) The name-keyed exclusion at 93-99: skipped only when the source has no module, i.e. an unmoduled user type named `IntervalSet` or `CountedQuantity` is not specialized (stated nowhere at the site; open). (d) `evaluate_const_fact_expression` `None` or `Some(false)` keeps the fact on the instance (541-545): fail-closed (traced).

---

## Coverage

- Read in full, in order: all 36 ranges of `unit_053.tsv`, lines 1 to the file's last line for each (1731+131+211+100+751+24+293+144+139+403+659+391+67+270+204+1801+1768+313+944+419+748+463+294+1685+1040+46+22+923+1957+156+267+357+239+8+920+746 = 20,634 lines). Large files were read in two or three chunks without gaps (e.g. `initializer_normalization.rs` 1-900 and 901-1731; `declaration_selection.rs` 1-1000 and 1001-1801; `declared_types.rs` 1-900 and 901-1768; `facts.rs` 1-850 and 851-1685; `values.rs` 1-1000 and 1001-1957).
- Skimmed: none.
- Not read: none.
- Greps used for counts: `ConstantSelection` (197 occurrences in 31 files, `glob *.rs`, count mode), `domain_index_telescope|domain_index_parameters` (13 in 6 files), `argument_type\(|argument_reference\(` (18 in 5 files), `finalize_operator_obligations|rejoin_after_substitution|constant_initializer_owns_selection|retained_constant_custody|is_core_layout_storage_source` (20 in 7 files), `"IntervalSet"|"CountedQuantity"` (1), `is_unmodeled_aarch64_scratch_view|ASM_SCRATCH_NAMES` (8 in 3 files).

## Five things a maintainer most needs to know about this unit

1. Nearly everything in `declarations/` rewrites syntax before symbols exist, using a header-only `SymbolTable` held by `ConstantSelection` (`declaration_selection.rs:43-51, 249-281`). Its name lookups go by spelling anchored at a use span (`find_top_level_by_name_and_kinds_from_source`), and results are rejoined to live declarations by `SourceSpan` + kind + name equality (1254-1299). The whole generic-data and constant machinery depends on that rejoin working; ambiguity yields `None`, never a guess.
2. Generic-data specialization is keyed on spelling in many places: substitution maps are `HashMap<String, TypeReferenceHandle>` (`declaration_selection.rs:1550`, `substitution.rs:41,72`), const-name replacement keys on the single-segment name (`templates.rs:404-410`), `variant_fact_verdict` and `ConstFactSubject::matches` compare the literal `self` (`substitution.rs:399`, `facts.rs:370`), and the `Rat`/`Nat` canonical-form check keys on label strings `"Rat"`, `"Nat"`, `"num"`, `"den"`, `"neg"`, `"pos"`, `"prev"` (`values.rs:495, 1869-1937`). Identity of closed arguments uses item handles and symbols (`ClosedArgumentIdentity`, `discovery.rs:128`) except `Constant(String)` (spelling, `arguments.rs:504-505, 518-520`).
3. Undecidable is handled by leaving things unchanged and deferring: `Ok(None)` / `Fold::Carried` / `continue` in `prepare_closed_application`, `canonicalize_closed_domain_indices`, `fold_closed_scalar_initializers`, `evaluate_selected_domain_facts`. The comments say so (`arguments.rs:29-37` "skip, never reject"; `initializers.rs` header; `facts.rs:1032-1036`). Consumers that must decide (`prove_const_data_predicates` `facts.rs:606-620`, `prove_const_domain_constraint` `facts.rs:1363`) turn undecided into errors; the specializer keeps unproved facts on the instance (`synthesis.rs:541-545`).
4. Constant initializer normalization is a double-entry system: `const_evaluation/initializers.rs` folds and writes a receipt, and `constants/initializer_normalization.rs:1563-1705` independently reconstructs the roster of dependencies, call choices and operators and refuses on any drift (`1620-1627`, `1654-1688`). Changing what the fold records requires changing `reconstruct`/`declaration_custody`/`append_retained_custody` in step.
5. Two implementations of the same rule exist: the domain index telescope (`arguments.rs:676-693` vs `domains.rs:59-77`, the former's comment 668-675 claims it is the one shared rule) and "does this type mention a bound name" (`declaration_selection.rs:1559` vs `substitution.rs:820`); also two StaticMachineArgument-to-type converters (`callable_types.rs:161-204` vs `declared_types.rs:526-606`). The assembly expander (`assembly/`) is a self-contained x86/AArch64 32-bit-only generator (`generated_statements.rs:26-29` always `u32`, EFLAGS bit constants at 285-346).

## What you are unsure about

- Whether the callers I did not read (`constants/initializer_leaves.rs`, `declaration_values.rs`, `initializer_dependencies.rs`, `generic_data/uses.rs`, `work_domain.rs`, `type_equations.rs`, `declarations/mod.rs`) constrain the fail-open spots marked "open" (for example `domains.rs:376-418` argument left as name; `canonicalize_closed_domain_indices` skip when no family; `builtin_type_names::validate` not covering modules or domains).
- `assembly/generated_statements.rs::bind` renames any bare identifier equal to a scratch register name across the block's private tree (`128-148`); whether a user variable with that spelling is excluded earlier (in the parser or `expand`) was not traced.
- `variant_fact_verdict` treats any nonzero integer literal as Holds (`substitution.rs:371-377`); I did not trace whether a user-written literal in a data `requires` can reach it.
- Whether `ClosedArgumentIdentity::Constant(String)` can collide for two distinct constants (identity by rendered spelling) was not traced.
- I did not run any program; every statement is from reading and grep.
