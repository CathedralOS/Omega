# Record: unit_089 (areas 17-omega-source and 16-tools)

Checkout `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. Every file below was read in full, in order (Read tool).
Counts come from Grep count/content runs (commands named inline). Paths are relative to the checkout.
Omega machine names below are written as in source; "stage" means a directory under
`source/subsystems/compiler/source_to_psi/pipeline/`.

Part A: Omega source (12 files). Part B: `tools/benchmark` (22 files). Part C: `tools/xtask` (15 files).

---------------------------------------------------------------------------------------------------

## Part A. Omega source

### A1. `source/subsystems/compiler/source_to_psi/pipeline/typed_trees_to_checked_trees/zone.omg` (1-493)

1. Purpose. Difference-bound matrix over a machine body's integer "subjects" (read). Subject 0 is the constant zero.
   The checker keeps one `Zone` per program point and asks it "what is the upper bound of `to - from`".
   Header says it is "the one relational engine checking uses for live integer facts" (zone.omg:6-7).
2. Shape. `Bound {finite: bool, value: ExactInteger}` (18-21). `Zone {bounds: [Bound; 4096], count: u64, infeasible: bool}`
   (101-105); entry `from*64+to` (166, 196, 353). Public: `bound_none` 23, `exactly` 27, `bound_add` 34, `bound_at_most` 69,
   `bound_negative` 97, `zone_open` 108, `zone_bound` 162, `implies` 220, `constrain` 245, `zone_forget` 319, `zone_join` 379,
   `zone_widen` 458. Private: `tighten` 189, `zone_set` 349.
3. Inputs/outputs. Imports `ExactInteger`, `ExactResult`, `exact_integer_add`, `exact_integer_at_most` from `foundation`
   (1-4). Mutates a `@mutable Zone`; reads return `Bound` or `bool`.
4. How it decides. Keyed on subject index (a dense integer), never on a name. `bound_add` overflow: positive overflow becomes "no
   bound", negative overflow is clamped to -(2^64-1) (31-33, 52-61); both only loosen. `constrain` computes the new edge's cycle
   with the back edge; a negative cycle sets `infeasible` (248-265). `implies` returns true when `zone.infeasible` (223-226, 235).
   `zone_join` loosens each pair to the looser of two zones and returns whether it changed (379-451); a join with an infeasible
   `other` returns false (382-383); an infeasible `zone` is replaced by `other` (388-398). `zone_widen` drops every entry that is
   now looser than `before` to unbounded (458-490).
5. Cases by shape. Fixed capacity of 64 subjects: `zone_open` crashes `Trap` if `count > 64` (111-113, 156-158); `zone_bound`
   and `zone_set` crash if either index is >= 64 (165-168, 352-355). `constrain` walks rows/columns `< zone.count` only (268, 289).
6. Same job elsewhere. Not seen in unit. Grepped callers in `typed_trees_to_checked_trees` (Grep content, glob `!zone.omg`):
   `zone_open` checker.omg:829, places.omg:127; `zone_join` checker.omg:924, flow.omg:645, matches.omg:490; `zone_widen`
   checker.omg:947; `constrain` facts.omg:225,242,258, flow.omg:1213-1214, places.omg:365-366,383-384; `zone_forget`
   places.omg:497, snapshots.omg:220; `zone_bound` evaluation.omg:151-152, 601.
7. Unused. `implies` (zone.omg:220): grepped `\bimplies\b` over `source/` excluding zone.omg: 0 call sites (only comments in
   wait_wake.omg:19, emit.omg:415, checker.omg:752, flow.omg:36). `bound_none`, `bound_negative`: 0 external sites (same grep).
   The "one relational engine" claim: external users are 6 files (checker, facts, flow, matches, places, snapshots, evaluation).
8. Stated limits. "A contradictory set of facts is infeasible: no execution reaches that point, and every query holds there." (13-15).
   "it is not re-closed, which could undo the widening" (456-457).
9. Fail-open. (a) `implies` on an infeasible zone returns true (223-226): stated (13-15); traced: no caller (item 7).
   (b) `zone_widen` returns early when either zone is infeasible (461-462) leaving `zone` unwidened: open (no comment).
   (c) `constrain` with a non-finite bound is a no-op (251-254): sound (stated by "tighten ... when that is tighter", 188).
   (d) `constrain` does not check `from`/`to` against `zone.count`; an index between `count` and 64 is written but the closure
   loops stop at `count` (268, 289): open (not traced; may leave the zone un-closed for such subjects).

### A2. `.../representations/checks/checks.omg` (1-326)

1. Purpose. Result record of the checker stage: diagnostics (first rejection per machine), obligation count, per-machine crash
   routes, and "slot" records for trait requirements.
2. Shape. `CheckDiagnosticCode` (14-75, 28 cases), `CheckDiagnostic` 77, `CrashRoutes` 87, `Checks` 99 (arrays `routes[10240]`,
   `diagnostics[64]`, `slot_origins[64]`, `slot_qualifications[64]`), `SlotOriginKind` 124, `SlotReturnedBorrowOrigin` 129,
   `SlotParameterQualification` 135. Machines: `open_machine` 141, `checks_reject` 145, `diagnostic` 172, `checks_routes` 191,
   `set_routes` 208, `clear_slot_records` 225, `push_slot_origin` 230, `slot_origin` 256, `push_slot_qualification` 275,
   `slot_qualification` 309.
3. I/O. Pure table storage; mutated by checker machines, read by the driver (`driver/compiler.omg`, grep count 2 for the stage name).
4. How it decides. `checks_reject` records only while `machine_accepted` is true, then clears it (146-166), so one machine
   contributes at most its first rejection. Beyond 64 diagnostics it sets `accepted = false` but drops the record (155-158).
   Keyed on machine position (`machine_accepted` flag), not on diagnostic identity.
5. Cases by shape. Capacity 64 diagnostics, 64 slot origins, 64 slot qualifications, 10240 machines (99-120, 238, 283).
6. Same job elsewhere. The guarded-read-by-count pattern (`transition i < count {true -> read, _ -> invalid()}`) repeats in
   every table accessor in A3-A10 (e.g. source.omg:546, typed_trees.omg:237); there is no shared helper.
7. Unused. Not checked per-function. `Checks` header `requires` (100-103) lists `machine_count` and `diagnostic_count` but not
   `slot_origin_count` / `slot_qualification_count`.
8. Stated limits. "A fact outside the forms checking reasons with" (19-22); "An expression form checking does not reason about yet"
   (23-24); "proof search failing is not a refutation, and it grants nothing" (11-12).
9. Fail-open. `push_slot_origin` / `push_slot_qualification` return false when full (251-253, 304-306); whether callers refuse
   on false is not traced: open. Everything else crashes `Trap` on a bad index (186-188, 203-205, 220-222): fails closed.

### A3. `.../representations/lowered_program/layouts.omg` (1-31)

Purpose: layout record for the Alpha scalar program. `ScalarLayoutKind` (5-12), `ScalarLayout {kind,size,inner,length}` (14-20),
`scalar_layout_at` (22-31) reads `program.layouts[handle]` when `handle < layout_count && handle < 8192`, else crashes (25-30).
Comment: "One packed internal memory layout ... not a foreign ABI" (1-4). Callers: grep `scalar_layout_at|scalar_argument_at|
scalar_parameter_at|scalar_machine_at|scalar_operation` over `source/`: 31 matches across 8 files (alpha/emit.omg 5,
checked_trees_to_scalar_operations/{expressions 11, layout 2, scalar 3, statements 4, storage 1}, layouts.omg 1, scalar.omg 4;
counts include definitions). Fail-open: none seen. The file refers to `ScalarProgram` defined in `scalar.omg` (same module
namespace; not imported).

### A4. `.../representations/lowered_program/scalar.omg` (1-129)

1. Purpose. The stage's final product: a closed list of scalar operations with owned storage, consumed by the Alpha emitter.
2. Shape. `ScalarOperationKind` (9-36; 24 cases incl. `LowBytes`), `ScalarOperation {kind,left,right,extra}` (38-44),
   `ScalarMachine` 46, `ScalarParameter` 55, `ScalarStatus {Complete, Unchecked, Unsupported, Capacity}` 60, `ScalarProgram` 67
   (`layouts[8192]`, `field_offsets[1024]`, `parameters[32768]`, `arguments[65536]`, `operations[16384]`, `machines[10240]`).
   Accessors `scalar_operation` 85, `scalar_machine_at` 96, `scalar_parameter_at` 107, `scalar_argument_at` 118.
3. I/O. Written by `checked_trees_to_scalar_operations`, read by `alpha/emit.omg` (grep above).
4. Decides. Value identity is the operation index; type identity indexes the layout table (comment 2-3, 68-69). Accessor guards
   test the live count and the physical capacity (88, 99, 110, 121).
5. Cases by shape. Closed operation set listed in 9-36; anything else is `Unsupported` status.
6. Same job elsewhere. `ScalarMachine`/`SyntaxTrees` accessors follow the same guard pattern (see A2.6).
8. Stated limits. "Full portable publication must additionally retain canonical declaration/contract/obligation identities;
   checker acceptance and counts cannot substitute for those records." (7-9). "closed in-process lowering slice" (1).
9. Fail-open. `ScalarStatus::Unchecked` exists as a status value (62); which consumers accept it is not in this unit: open.
   Index misses crash (93, 104, 115, 128): fail closed.

### A5. `.../representations/source_files/source.omg` (1-715)

1. Purpose. Custody of source bytes: single `SourceUnit`, module paths, and the whole `SourceSet` of a compilation (units,
   packages, dependency edges), plus byte-level span comparison.
2. Shape. `SourceId` 7, `Span` 11, `SourceSpan` 16, `SourceUnit {id, bytes[131072], length, last_retained}` 21, `ModulePath {bytes[256], length}` 92,
   `SourceRange` 124, `PackageRow {name: SourceRange, toolchain}` 134, `DependencyScope {Product, Build}` 142, `DependencyRow` 150,
   `SourceSet` 164 (`bytes[4194304]`, `path_bytes[16384]`, `units[512]`, `paths[512]`, `unit_packages[512]`, `packages[64]`,
   `dependencies[256]`). Public machines: `module_path_clear` 100, `module_path_append` 105, `source_set_clear` 186, `source_set_add`
   197, `add_package` 292, `add_dependency` 325, `source_set_byte_or_nul` 432, `spans_equal` 476, `span_spells` 510,
   `path_byte_or_nul` 546, `path_length` 587, `unit_package` 605, `package_row` 623, `dependency_row` 642, `span_names` 666.
   Private: `source_unit_clear` 31, `source_unit_append` 37, `source_unit_byte_or_nul` 62, `store_name` 379, `name_byte_or_nul` 700.
3. I/O. Reads bytes appended by the driver / construction app; writes arena ranges. `SourceId` is a unit's position (159-160).
4. Decides. Name equality is byte equality of two spans (473-475: "Identifier spellings are their source bytes under
   LEXICAL-PROFILE-V1 (no escapes)"); package-name and alias matching is also byte compare through `span_names` (666-697).
   Admission: `source_set_add` requires unit count < 512, package index < package_count, and room in both arenas (205-208).
5. Cases by shape. Fixed bounds: 512 units, 4 MiB bytes, 16 KiB path arena, 64 packages, 256 dependency edges, 256-byte path.
6. Same job elsewhere. `spans_equal` 476, `span_spells` 510, `span_names` 666 are three copies of the same byte-compare loop over three
   different right-hand sides. `source_set_byte_or_nul` 441-466, `path_byte_or_nul` 554-584 and `name_byte_or_nul` 700-715 are three
   copies of a guarded array read.
7. Unused / counts. Grep `source_unit_clear|source_unit_append|token_stream_*|source_unit_byte_or_nul|store_name` over `source/`: 52 matches
   in 3 files (lexer.omg 40, source.omg 8, tokens.omg 4); the first two names are non-`public` and are called from the lexer.
8. Stated limits. "the request's graph validation, not this set, rejects duplicate aliases of one requester and scope" (162-163).
   "Callers check the arena's provision first, so running out of it is an internal fault." (377-378).
9. Fail-open. (a) `source_set_byte_or_nul`, `path_byte_or_nul`, `name_byte_or_nul` return NUL (0) when the coordinate is outside
   (468-470, 581-583, 712-714). Comment: "the fallback makes a violated coordinate fail a comparison rather than read another unit"
   (429-431). But `spans_equal` (484-498) compares two such reads: two equal-length spans that both lie outside any unit read
   NUL on both sides and compare equal (returns true at 500-502). Open: callers' guarantee that spans are inside units not traced.
   (b) `package_row` for a missing package returns an unnamed ordinary (non-toolchain) package (635-637); `unit_package` and
   `path_length` return 0 (599-601, 617-619); `dependency_row` returns a Build edge with requester 64 (654-661, stated 640-641:
   "64 is past every package index"). The first three: open.
   (c) `source_set_add` returns false (does not crash) when full (280-282); caller handling not traced.

### A6. `.../representations/symbol_resolved_trees/symbol_resolved_trees.omg` (1-799)

1. Purpose. Name-binding tables produced by the resolver and read by typing/checking. Each table is indexed by a syntax table handle
   and holds no spelling (10-15).
2. Shape. `BuiltinType` 18 (21 cases), `TypeBinding` 50, `BinderContract` 71, `ValueBinding` 87 (22 cases incl. `Overloads`,
   `MatchedSubject`), `VisibleOverloads` 131, `OverloadLinks` 136, `BytePredicate` 145, `LayoutQuery` 152, `PathBinding` 160, `TypeScope`
   169, `DomainBinding` 184, `MachineOwner` 199, `ResolveDiagnosticCode` 206 (about 40 cases), `NameResolution` 293, `OverloadTable` 340,
   `MachineArgument` 425, `RankingView` 670. Machines: `prepend_overload` 345, `next_overload` 365, `name_resolution_clear` 381,
   `name_resolution_reject` 394, `value_binding` 405, `name_resolution_machine_argument` 433, `binder_contract` 458,
   `set_binder_contract` 475, `name_resolution_requirement` 495, `operator_home` 514, `machine_owner` 532, `domain_binding_at` 550,
   `name_resolution_domain_item` 569, `path_binding` 587, `type_binding_at` 606, `path_binding_at` 624, `value_binding_at` 643,
   `ranking_view` 681, `set_ranking_view` 698, `ranking_measure` 718, `set_ranking_measure` 735, `component_field` 754,
   `set_component_field` 771, `service_trait` 790.
3. I/O. Imports from `syntax_trees` (1-8). Written by the resolver stage; read by typer and checker.
4. Decides. Overload families are linked by machine handle; "A candidate is a machine handle plus one; zero ends the family" (128). Two
   chains (all, exported) share suffixes: `prepend_overload` returns both heads (345-362). Ambiguity is a binding case (`Ambiguous` 52, 90).
   Machine-binder argument: `Machine {handle}` -> `Declared`; generic parameter of kind `Machine` -> `Forwarded`; anything else
   `Absent` (433-452).
5. Cases by shape. Builtin names fixed list (18-40, "in the reference compiler's builtin order", line 17). Ranking views fixed to 6
   cases (670-679). `ranking_views[2048]`, `ranking_measures[2048]`, `component_fields[32000]`, `service_traits[32000]` u16,
   `binder_contracts[256]`, `requirements[2048]`.
6. Same job elsewhere. Four `*_binding_at` readers (606, 624, 643) use the table capacity as bound; four others (405, 495, 587)
   use the live count. Both patterns coexist for the same tables (`value_binding` 405 vs `value_binding_at` 643).
7. Unused. Not counted function by function.
8. Stated limits. `UnsupportedDeclaration` "A declaration kind resolution does not bind yet: a mathematical definition (`let` root) or
   a transparent trait refinement" (230-233); `UnsupportedOperatorToken`/`UnsupportedMachineContract`/`UnsupportedClause`/
   `UnsupportedGenericParameter`: "A form whose bindings resolution does not record yet rejects at that form" (234-242);
   `UnsupportedExpression` (243-246).
9. Fail-open. (a) `type_binding_at`, `path_binding_at`, `value_binding_at` read any row below table capacity (609, 627, 646) even
   when beyond the live count; the "zero binding is the unresolved state" (13-14). `name_resolution_clear` (381-392) resets
   counts and diagnostic but not the arrays, so an unbound row reads stale data unless the resolver zeroes it. Open (resolver not in unit).
   (b) `binder_contract`, `ranking_view`, `ranking_measure`, `component_field`, `service_trait` return a default (`present:false`, `Default`,
   0, 0, 0) when out of range (459-461, 682-684, 719-721, 755-757, 791-794). `ranking_view` default `Default` is a legal view:
   a clause beyond 2048 would silently get the default ranking. Open.
   (c) `operator_home` bounds by `machine_owner_count` but reads `operator_homes` (517-524): consistent only if both counts
   are equal; open.

### A7. `.../representations/syntax_trees/syntax_trees.omg` (1-1831)

1. Purpose. Fixed-capacity tables for the parser's output, plus guarded accessors. "no symbol or package meaning is assigned here" (6-8).
2. Shape. About 60 `data` types for syntax rows: `TypeReference` 50, `IntegerLiteral` 79, `DataField` 117, `DataCase` 126, `GenericParameter`
   185, `DataItem` 194, `Parameter` 232, `ExpressionKind` 262, `OperatorKind` 283, `Expression` 315, `StatementKind` 349, `TransitionArm` 385,
   `PatternField` 407, `Statement` 417, `ClauseKind` 444, `Clause` 471, `MachineContainer` 495, `OperatorToken` 513, `MachineItem` 543,
   `Contract` 590, `NamePath` 611, `TraitItem` 629, `ConformanceItem` 652, `DomainItem` 668, `ConstantItem` 690, `PropositionItem` 711,
   `DefinitionItem` 728, `RootItem` 754, `ParseDiagnosticCode` 760 (about 200 cases), `SyntaxTrees` 973 with 38 count/array pairs
   (1010-1078), `JoinedValues` 1506, `ArmRange` 1540. Machines: `static_integer_is_valid` 87, `same_type_form` 93, `syntax_trees_clear` 1083,
   `syntax_trees_reject` 1125, about 40 guarded accessors 1142-1831, `qualifier_span` 1280, `joined_values` 1513, `joined_value` 1519,
   `range_arm` 1547, `member_name` 1776.
3. I/O. Consumers: the resolver (grep imports in A6), typer, checker. Producer: the parser.
4. Decides. `same_type_form` compares kind, access and argument count, and for `Literal` rows the decoded integers via
   `static_integer_is_valid` (exact, nonempty digits, no suffix) (93-110, 87-89). Names and domains compared elsewhere (comment 91-92).
   Type arguments and array lengths "require an exact, unsuffixed integer" (86).
5. Cases by shape. Capacities: 4096 roots, 1024 uses, 32000 members, 256 data items, 256 generic params, 1024 fields, 1536 cases,
   65536 type refs, 10240 machines, 131072 expressions, 16384 statements, 8192 arms, 1024 match arms, 256 traits/conformances/domains/
   propositions/contracts/definitions, 512 constants, 65536 integer literals (975-1008). A fixed-array literal and a value-position `match`
   share `joined_values` by `kind == Match` (1513-1516). "Multiple simultaneous entries remain an explicit coverage refusal." (258-259).
6. Same job elsewhere. The accessor pattern is repeated about 40 times (A2.6). `syntax_trees_match_arm` 1570 and `syntax_trees_arm` 1487
   read two tables of the same record type `TransitionArm`.
7. Unused. Not individually grepped. Grep `ParseDiagnosticCode` cases not enumerated.
8. Stated limits. "Retirement is schema history, never a live field/case or a runtime tag." (149-150); "Identities are not runtime
   discriminants; their numbering rules are resolution's question." (114-116); `UnsupportedCastEntries` (965).
9. Fail-open. None seen: every accessor crashes `Trap` outside `[0, count)` (1139-1141 comment; e.g. 1145-1148). `erased_binding` and
   `type_lifetime` guard `index < count && index < 1024` (1799, 1817). `same_type_form` does not compare lifetimes or domains (91-92:
   stated, resolver compares them).

### A8. `.../representations/tokens/numeric.omg` (1-21)

`NumericBase {Binary,Octal,Decimal,Hexadecimal}` (1-6), `IntegerSuffix {None,U8..I64,Unknown}` (10-21). Comment: "`Unknown` is a suffix that
names no carrier, which typing rejects" (8-9). Nothing else. Fail-open: `Unknown` suffix rejection is in typing (not in unit): open.

### A9. `.../representations/tokens/tokens.omg` (1-230)

1. Purpose. Token vocabulary and the bounded `TokenStream`.
2. Shape. `KeywordKind` 11 (30 cases incl. contextual `Case`, `Copy`, `Linear`), `PunctuationKind` 46 (46 cases), `Token` 95, `TokenKind` 106
   (Identifier, Integer(...6 payload fields), Float(...), StringLiteral, Keyword, Punctuation), `LexDiagnosticCode` 130, `TokenStream` 150
   (`tokens[65536]`, `decoded[131072]`). Private machines `token_stream_clear` 164, `token_stream_reject` 175, `token_stream_push` 189,
   `token_stream_push_decoded` 212.
3. I/O. Written by the lexer (grep: 40 uses in lexer.omg); read by the parser.
4. Decides. Integer literals are decoded once by the lexer: `magnitude` plus `exact` (fit in 64 bits), "when it did not, `magnitude`
   is meaningless" (108-111). Keyword vs identifier is the lexer's choice; contextual words "remain valid identifiers outside their
   exact parser positions" (39-40).
5. Cases by shape. ASCII-only syntax: "LEXICAL-PROFILE-V1 admits only ASCII syntax; non-ASCII bytes remain opaque inside comments and
   literal bodies" (7-9). Capacity 65536 tokens, 131072 decoded bytes.
8. Stated limits. as in 5.
9. Fail-open. `token_stream_push` when full sets `last_retained = false` and returns without recording (207-209); whether the lexer
   rejects on `last_retained == false` is in lexer.omg (not in unit): open.

### A10. `.../representations/typed_trees/typed_trees.omg` (1-972)

1. Purpose. Interned type table and typing's per-syntax-handle annotations.
2. Shape. `TypeKind` 19 (14 cases), `TypeRow` 57, `TypeTable` 72 (`rows[8192]`, `arguments[4096]`), `Selection` 86, `Adaptation` 99,
   `TypeDiagnosticCode` 111 (about 45), `MemberTarget` 178, `Typing` 188 (arrays sized 131072 / 65536 / 2048 / 4096). Machines: `typing_reject` 228,
   accessor/mutator pairs 237-654, `typing_landed_integer` 394, `rational` 416, `type_row_of` 657, `type_table_row` 672,
   `type_table_argument` 689, `type_table_push_argument` 708, `type_table_intern` 730, `same_row` 791, `type_row_carrier` 833,
   `referent_of` 845, `same_carrier` 867, `qualified_row` 898, `carrier_width` 919, `reference_cast_source` 951.
3. I/O. Reads `syntax_trees`, `symbol_resolved_trees` (`BuiltinType`, `DomainBinding`), `foundation` numerics. Written by the typer, read by
   checker and scalar lowering (grep, next paragraph).
4. Decides. Type identity is the interned handle: "two types are equal exactly when their handles are" (14-16). Interning is a linear
   scan from row 1 comparing every field and the argument run (735-749, 791-830). `same_carrier` compares through reference layers with
   equal access (870-882). `reference_cast_source` follows `Cast` expressions with reference types down to the original borrow; operand
   index must be smaller than the parent else crash (959-969). `typing_landed_integer` is true only when selection is `None`, adaptation
   `Landed`, expression kind is Integer/Unary/Binary, the row is a builtin carrier, and the denominator is 1 (397-411).
5. Cases by shape. `carrier_width` handles Bool/8/16/32/64-bit integers and f32/f64, "zero for any other type" (916-945). `referent_of`
   follows at most 16 reference layers (856).
6. Same job elsewhere. `type_table_intern` scan is O(rows) per intern; no other site in unit. Grep
   `type_table_intern|typing_landed_integer|reference_cast_source|qualified_row|same_carrier|referent_of|carrier_width|type_row_carrier|
   type_table_push_argument` over `source/`: 41 matches in 15 files (typer.omg 5, expressions.omg 9, typed_trees.omg 11, loans.omg 2,
   others 1-3; counts include definitions).
7. Unused. Not individually.
8. Stated limits. "An integer literal tree that has not landed at a destination yet" (40-42); "Landing rounds once ... the value is
   evaluation's, not typing's" (43-46).
9. Fail-open. (a) `type_table_intern` returns 0 when the table is full (785-787, comment 729: "Zero when the table is full"). Row 0 is
   the absent type (16-17); a caller that does not test 0 would carry an Absent type forward: open (callers not traced).
   (b) `typing_denominator` returns 1 for stored values 0 or 1 (472-476): stated (461 "at least one").
   (c) `type_table_intern` `found` releases the tentative argument run only when `row.first_argument <= table.argument_count` (754):
   otherwise the run stays; not a safety issue.

### A11. `.../tests/fixtures/generics/run/static_arguments/build.omg` (1-7)

Build file: `Build::application(builder, "static_arguments_tests")` and four `Build::bind(... ProgramEntry, main)` for linux_x86_64,
linux_arm64, macos_arm64, windows_x86_64. No other content.

### A12. `.../tests/fixtures/generics/run/static_arguments/main.omg` (1-155)

1. Purpose. Test program, "Expected exit: 0" (1). It redefines small stand-in tables (`SyntaxTrees`, `TypeReference`, `NameResolution`,
   `Resolver`, `Typer`) and stub machines (`syntax_trees_type_reference` 22, `syntax_trees_integer_literal` 31, `type_binding_at` 37,
   `conformance_item` 61, `syntax_trees_generic_parameter` 72, `typer_bind` 82, `bind_machine` 87), then calls the real product machines
   `bind_static_arguments`, `same_head`, `same_type_row`, `generic_annotation_role`, `trait_name_admitted`, `trait_slot_accepts`
   (declared in the resolver/typer, spliced in by the driver: header comment 2-3 "The driver includes the product binder and both resolver
   queries unchanged"; the splice is done by `tests/static_arguments.py` per the crate map, not read here).
2. Shape. `main` 138 builds literals (four, hex 4, five, overflow, empty digits, suffixed) and ANDs: `check_trait_static_slots` 117,
   `same_integer` 107 (pairs: 4==0x4 true, 4==5 false, overflow/empty/suffixed false), `call_fits` 99 (machine row 6 vs 5, counts 0-3).
3. Decides (the test pins). Row 6 is a machine binder callee, row 5 is a Bool builtin; `call_fits(.., 6, 2, true)` and `(.., 5, 2, false)`;
   `call_fits(.., 6, 3, false)` (too many arguments). `accepts` cases: Const parameter with type-reference row 10 (a Trait) accepts row 12
   (a conformance of trait 0), refuses 13, 14, 0, 15, 19 and, for a Const parameter of type row 4, accepts row 0 and refuses row 12
   (122-130). `main` crashes `Trap` when the AND is false (146-154).
4. Fail-open. None in the file; the test is a positive set only for acceptances and a negative set only for refusals listed above.
   It does not run without the driver splice (open: depends on `static_arguments.py`, not read).

---------------------------------------------------------------------------------------------------

## Part B. `tools/benchmark` (22 files)

### B1. `tools/benchmark/machine-checks/harness.rs` (1-137)

Purpose: example binary body, pulled by `include!` from an example under `.../06_lowered-program-to-checked-program/examples/`
(crate map; not re-grepped). Compares the same lowered program discharged at different worker counts. Shape: `expression` 14,
`generated(shape, count, steps)` 23-94 builds `count` machines of 1 bind, `length` add-1 binds and a value arm over u32
(shapes `mixed` = 1+ordinal%4 steps except every 16th machine, `uniform`, `tiny`, anything else panics 39), `main` 96-137 takes
`<workers> <repeats> <shape> <machines> <steps>` (101), calls `lower_bodies` then `discharge`, times only the discharge (115-118),
asserts every verdict is `Ok` and `unestablished()` is empty (121-125), hashes `format!("{elaboration:?}{result:?}")` and asserts the
hash is equal across repeats (127-133). Output: one JSON line `omega-timed-work/1` with `work_units = obligations * repeats` (135).
Decision key: the Debug-format string hash (source spelling of a debug value). Stated: "Setup, input cloning and result fingerprints
stay outside the discharge timer" (2). Fail-open: none seen (panics on mismatch).

### B2. `tools/benchmark/proof-preparation/harness.rs` (1-324)

Purpose: shared body for allocation-counting and timing examples of the proof prover. Counting allocator `MeasureAllocations<const TRACK>`
(12-51, `COUNT_ALLOCATIONS` is supplied by the including example; not defined here). `inputs(path)` 62-153 lexes, parses, resolves, types
and checks a `.omg` file through `crate::frontend_fixture` (323-324, `#[path = "../../../tests/support/frontend.rs"]`), then collects each verdict's
certificate (assumptions, axioms, values, goal, expected proof). `conjunction_control("wide64"|"nested8")` 166-201 makes a synthetic
`x <= 17` goal under a 64-wide or 8-deep conjunction. `main` 203-321 per input: proves, asserts equality with the stored proof (222-224),
runs `proof_admission::accept_certificate` (225), prints a `CONTEXT` JSON line with node count, width, depth, retained bytes
(256-266), then times "prepare" (`known::Known::new`) and "prove" modes (268-320). Keyed on proof structure (node counts by
`format!("{proof:?}").matches("ProofNode {")` 234, 255: source spelling of a Debug value) and FNV fingerprints of Debug text (235-242).
Entry selection: `main` named `"main"` by string (89). Fail-open: none seen. The file has no `#[test]`.

### B3. `tools/benchmark/selected-bodies/snapshot_limits.rs` (1-57)

Purpose: parse the env string `OMEGA_SELECTED_BODY_SNAPSHOT_LIMITS` (3) `bytes,allocation,elements,depth` (default `268435456,2147483648,
134217728,128`, 4). `parse` 6-33 requires exactly four positive integers each <= a private maximum (512 MiB, 4 GiB, 268435456, 128; 19-24,
25-31). Included into `profiling.rs:14` via `#[path]`. One test 39-56. Stated: "The scalar workload exceeded the default 4,194,304-element
export allowance at 28,160,139 bytes" (1-2). Fail-open: none seen.

### B4. `tools/benchmark/service-correspondence/harness.rs` (1-180)

Purpose: timing driver for `validate_correspondence()` of a projected service surface; counting allocator (14-55, `COUNT_ALLOCATIONS` from the
including example). `typed(text)` 57-76 runs frontend fixture through typing; `projected(program, name)` 78-92 finds a trait by `row.name.as_str() == name`
(85, string compare) and calls `projection::from_typed`; `family(methods, inherited)` 94-115 generates boundary-trait source text; `workloads()`
117-136 reads `source/library/std/console.omg` by `include_str!`, slices it between the strings `"public data ByteRead"` and `"// Text adapters"`
(119-120), prepends `core/process_authority.omg`, and includes `tests/fixtures/providers/run/target_selected_implementation/main.omg`.
`main` 138-180 takes `--samples`, `--iterations`, `--only`; CSV output. Keyed on source text slices (spelling). Fail-open: none seen;
panics via `unwrap`. The second include path `../../../tests/fixtures/providers/...` is repository-root `tests/fixtures/` (not under `tools/benchmark`).

### B5. `tools/benchmark/service-correspondence/tests.rs` (1-101)

Purpose: tests for the above, included by the example. Five `#[test]`: all workloads validate (4); losing the exact requirement symbol, the declaring trait,
or the owner package identity, or popping a requirement each yields a specific message ("service row lost its exact declared requirement", "... exact declaring trait",
"... has no unique exact inherited requirement", "... lost its ordered source correspondence"; 18-39); equal leaf names and applied parent arguments keep exact owners (43-55);
duplicate canonical requirements refuse for 1 and 2 methods (58-81); ambiguous inherited declarations refuse ("service inheritance has ambiguous declarations", 98).
Pairs: each negative differs from the positive in one field. Uses a `mod tests { use super::*; ... }` without `#[cfg(test)]` at 1 (the including example presumably gates it; open).

### B6. `tools/benchmark/src/dominance_summary.rs` (1-290)

1. Purpose. Validate a graph benchmark run directory (`metadata.json`, `observations.jsonl`, `complete.json`) and summarise baseline vs candidate pairs.
2. Shape. `Metadata` 11, `Case` 19, `Role` 26, `Observation` 32, `Completion` 49, `METRICS` 53, `decode` 77-176, `run` 178-196.
3. I/O. Reads the three files (each <= 64 MiB, 183). Output: JSON array, one summary per case, via `statistics::compare` (153).
4. Decides. Digest match of observations to `observations_sha256` (82), schema string `omega-dominance-comparison/1`, 6..=1000 pairs, exactly 1 warmup pair, <= 256 unique cases (85-97);
   row count `(pairs+1)*2` per case (102-105); per row: shape, blocks, warmup flag, round, role (ABBA alternation by round parity, 109-122), `queries == 100_000`,
   `checksum <= queries`, checksum and edge count equal to the first row, first four metrics nonzero (123-133). Keyed on position in the file and role.
5. Cases by shape. Exactly one warmup pair, queries fixed at 100000 (128), cases need `blocks >= 4` (94).
6. Same job elsewhere. The alternating-role ordering is also encoded in `experiment.rs:242-246` and `records.rs:120-124`.
7. Unused. `run` is called from `main.rs:125` (1 caller).
9. Fail-open. When the baseline CPU time includes zero the interval is omitted and flagged `interval_unavailable` (160-167): stated in the text.
   `cpu_ns` zero is not rejected (only the first four metrics are, 132): stated in test name.

### B7. `tools/benchmark/src/experiment.rs` (1-692)

1. Purpose. Comparison lifecycle: prepare plan, alternate baseline/candidate, journal every observation, summarise.
2. Shape. `Report` 16, `Sample` 26, `ReportedWork` 41, `Journal` 48 (`append` fsyncs each event 53-59), `prepare_plan` 62, `compare` 151, `run` 206, `summarize` 319,
   `measure` 401, `check_output` 532, `stage_timings` 548, `snapshot` 562, `collect_inputs` 574, `resolve` 598, `write_new` 605, `write_capture` 616, `host_target` 625,
   `host_identifier` 636.
3. I/O. Reads plan JSON; runs the two commands through `process::capture`; writes `journal.jsonl`, per-sample directories, `report.json`. New output directory required; must be outside declared input dirs (164-172).
4. Decides. Input identity is the SHA-256 of each declared input file (snapshot 562-596), re-checked before and after each sample (248-250, 288-297). Symlinks refused (577-582). Compiler
   plans: candidate source bytes must be identical to baseline source (93-95); candidate source must be among declared inputs (96-102); `library_root` is forced into both environments
   as `OMEGA_LIBRARY_ROOT` (112-125); target must equal the host target (141-146). `measure` for Compiler: the published artifact is found by the text line `"published native output to "`
   in compiler stdout+stderr, exactly one (482-488), must lie inside the sample's fresh build dir (490-500). Compiler stage times parsed from lines matching `"<ms> ms  <label>"` (548-560) — keyed on printed text.
   Reported-work oracle: schema string, work_units, checksum string, elapsed in `(0, process_wall]` (456-466).
5. Cases by shape. Three measurement kinds only (`Process`, `ReportedWork`, `Compiler`, 424-528). `summarize` collects a fixed list of seven resource metrics (322-333) plus any `resources` keys.
6. Same job elsewhere. `check_output` 532 and `records::oracle` (records.rs:310) both compare exit code/stdout/stderr to `ExpectedOutput`. `host_target` 625 duplicates `progress.rs:290`, `producer.rs:17`, `samples.rs:34` (grep `fn host_target`: 4 definitions; `xtask` ones not read). `write_new` 605 duplicates `proof_preparation.rs:181`. `resolve` 598 only here.
7. Unused. `Report.name/metric/comparison` pub fields used in `main.rs:259-281`. `compare` 1 caller in main + 1 in suite.rs:41.
8. Stated limits. "`omega` reports no optimization level, so the plan author owns choosing release-built producers" (216-217). Report warnings 385-390: "Within-run bootstrap uncertainty does not cover host drift, contention,
   or workload selection." and "Only declared input paths are hashed."
9. Fail-open. (a) Inherited environment is recorded as "inherited, with plan overlays; secrets are not recorded; no contention isolation" (190): stated. (b) Only three environment variables are recorded (186): stated.
   (c) `summarize` takes `observations.first()` only for resources (344): for Compiler samples the runtime observation's resources are ignored: open.

### B8. `tools/benchmark/src/history.rs` (1-88)

Purpose: compare two completed runs as unpaired independent samples. `compare` 9-44 requires `records::validate` on both, same identified host (`host_id_sha256` string present, 12-16), same inherited settings (17), same logical plan
(`logical_plan` 46-66 strips working-directory prefixes and `OMEGA_LIBRARY_ROOT`), same input byte closure (`input_closure` 68-88 keyed on index + relative path). Uses only `candidate` role non-warmup samples (26-37)
and `statistics::compare_independent`. Warning text 42. Cases: logical-plan compares `measurement`, candidate `arguments`, `environment`, `timeout_ms`, `warmups` only (64) — `pairs` and `baseline` command are not compared: open.
Fail-open: none seen.

### B9. `tools/benchmark/src/main.rs` (1-288)

Purpose: CLI dispatch by manual argument-count matching (36-287). Subcommands: `compare`, `suite`, `validate`, `show`, `check`, `history`, `profile`, `progress`, `scaling`, `dominance-summary`, `proof-plan`, `proof-square`,
`selected-body-inputs`, `selected-body-plan`, `selected-body-source-plan`. Help text 56-67 lists all. Optional leading `--address-space-gib N` (1..=2^20) sets the cap (39-54). Exit codes: 0, 1 error, 2 usage/unavailable,
for `check` 0/2/3 mapped to "within budget"/"inconclusive"/"regression" (228-236). Stated: "README.md shows every subcommand with examples" (13): `tools/benchmark/README.md` does not exist per crate map (not re-listed; Glob of
`tools/benchmark/tests/fixtures/**` shows only fixtures). Fail-open: a subcommand with the wrong argument count falls through to the final usage check (253-256) and exits 2; none accepted wrongly.
`profile` returns exit 2 when unavailable or failed (192-194).

### B10. `tools/benchmark/src/plan.rs` (1-170)

Purpose: authored plan schema. `Plan` 9, `CommandSpec` 24, `Measurement` 34 (tagged `kind`: `compiler`, `process`, `reported_work`), `ExpectedOutput` 74, `Plan::validate` 81-127, `Plan::metric` 129.
`#[serde(deny_unknown_fields)]` on all (8, 23, 33, 73). Decides: schema string `omega-performance-plan/1`; name nonblank; inputs nonempty; warmups 1..=100; pairs 2..=1000; timeout 1000..=3_600_000 ms (82-93).
Compiler measurement: target must be one of five names including `macos_x86_64` (103-110); each command's arguments must be exactly `--offline` and `--disable-optimization <UppercaseAlnumName>` pairs, anything else refused (115-123, test 143-169).
Cases by shape: that argument allow-list. Same job elsewhere: target-name lists at `plan.rs:106`, `experiment.rs:626-632`, `proof_preparation.rs:84-87` (four names), `selected_bodies.rs:151-155` (four names): the lists differ (five vs four).
Grep `"macos_arm64"` over `tools/**/*.rs`: 14 matches in 9 files. Fail-open: none seen.

### B11. `tools/benchmark/src/process.rs` (1-146)

Purpose: run one child under `bounded_process` limits and record an `Observation`. `capture` 65-126: timeout = plan timeout + 10 s (82), address-space cap = a process-wide atomic (default 64 GiB, 24), 1 GiB, 1024, 64, 8 GiB, 8 GiB limits (79-89),
captures 1 MiB per stream (102-109), stdin `Null` when empty (97-101). `digest` 45, `file_digest` 49 (SHA-256), `resource_observations` 128-146 maps missing metrics to `{"status":"unavailable", reason}` not zero.
Stated limits: "RLIMIT_AS (Linux only) counts reserved virtual memory, not use: ... The 8 GiB memory limits below are enforced only by Windows job objects" (17-21). Callers of `capture` in the crate: grep `process::capture` plus `capture(`
(see crate grep: 17 matches in 7 files across the pattern set). Fail-open: a platform with no resource provider records "unavailable", not 0: fails safe.

### B12. `tools/benchmark/src/profiling.rs` (1-361)

Purpose: run one plan subject once under a profiler and record `profile.json`. Backends `wpr` (Windows), `perf-stat`, `perf-record`, `selected-bodies` (54-64). `run` 53-310. `OwnedWpr` guard cancels its own WPR instance on drop (17-51).
`find_perf` 319 looks only at `/usr/bin/perf` and `/bin/perf`. `parse_perf_stat` 326-348 maps `<not supported>` values to `unavailable` and fails on no counters. Decides: capability checks (118-147); after the run, the original plan is re-prepared and
its input snapshot must equal the initial one (296-299). `selected-bodies` sets `OMEGA_SELECTED_BODY_PROFILE` to `<dir>/typed` and snapshot limits env (159-170). Same job elsewhere: measurement reuse through `experiment::measure`
(210) and `experiment::snapshot` (88, 297). Fail-open: WPR cleanup failure is retained as a failure (238-240); none seen. Stated: "instrumented attribution; do not compare these durations with uninstrumented runs" (90).

### B13. `tools/benchmark/src/progress.rs` (1-694)

1. Purpose. `omega-bench progress`: run a frozen list of five workloads on this host and record per-commit speed (`omega-progress-record/1`, set `omega-progress-set/3`, 18-19).
2. Shape. `Workload` 25, `WORKLOADS` 32-87 (`one_line_check`, `addr_algebra_check`, `medium_sample_check`, `heavy_proof_check`, `native_run_both`), `Options` 89, `run` 99-234, `parse_options` 236, `git` 274, `host_target` 290,
   `build_release_compiler` 302 (runs `mbx build --release -p omega`), `accept_review` 329-370, `measure` 374-450, `parse_perf_instructions` 454, `load_average` 467, `median` 485, `summarize` 498, `validate_record` 550, `utc_timestamp` 622.
3. I/O. Reads git state, the compiler, workload paths (`tools/benchmark/workloads/one_line/main.omg`, `tools/benchmark/tests/fixtures/arithmetic/pass/addr_algebra/main.omg`, `samples/cli/arithmetic/prime_counter/main.omg`,
   `tools/benchmark/tests/fixtures/proofs/pass/runtime_inductive_gauss_sum_exit/main.omg`, `samples/cli/arithmetic/wrapping_square_sum/main.omg`; the two fixture paths exist per Glob). Writes one JSON file named `<commit12>__<target>__<time>.json`.
4. Decides. Phases per workload: 1 warmup, `aa_runs*2` alternating `aa_a`/`aa_b` runs of the same binary (noise estimate), then `runs` measured (153-157). A row is `ok` only if exit code is 0 (437). The command returns an error if any row failed, but keeps the record (224-232).
   Perf `instructions:u` used when `/usr/bin/perf` exists on Linux (399-402).
5. Cases by shape. Frozen workload table: "change either and bump the set version" (5-7).
7. Unused. `git`, `measure`, `median` are `pub(crate)` and used by `scaling.rs:19` (3 uses).
8. Stated limits. "This is project acceptance so the workload can compile, not a source audit." (326-327).
9. Fail-open. `accept_review` (329-370) rewrites the package review file so every `decision ... pending` line becomes `accept`, then resumes. Deliberately accepts all pending package-review decisions for the two named projects: stated (326-328). Removes any `omega.lock` first (331).

### B14. `tools/benchmark/src/proof_preparation.rs` (1-433)

Purpose: write comparison plans for the proof benchmark without a shell. `run` 19; `make_plan` 34-158 (`source` mode: process measurement of `<main.omg> --entry main --target T` with a fixed expected stdout `"<path>\n  part 0: accepted\n<path>\troute=accepted\n"`
103-105, target one of four hosted targets 84-89; `prepare`/`prove` modes: reported-work measurement, needs `--repeats`, `--work-units`, `--checksum`, refuses `--target`, 111-130); `prepare_square` 192-227 builds a pinned
workload from git commit `SQUARE_BASE` (13) and requires exact SHA-256 of the transformed `main.omg` and `build.omg` (14-16, 202-203, 206-210). `qualify_main` 269-282 does four exact-once text replacements. `identity_optimizations` 284-302 deletes
exactly six `Optimizations::enable(` calls. Keyed on source text and SHA-256 pins. Fail-open: none seen; replacements fail when the pinned text changes (262-267, 286-288). Plans are built with fixed `warmups: 1`, `timeout_ms: 120_000` (151-153).
Four tests (354-432).

### B15. `tools/benchmark/src/records.rs` (1-402)

Purpose: verify a retained run before showing it or gating on it. `validate` 27-308 (see below), `oracle` 310, `is_sha256` 323, `regression_exit` 328-354, `markdown` 356-385. Decides: journal must start with `start`/`omega-performance-run/2`, end with `complete`, report digest equals journal digest (38-47); only five event
names allowed (52-58); one compiler identity per role matching retained `<role>.version.stdout` (60-73); exactly one `identities` event with a SHA-256 for each command program (80-95); sample count `2*(warmups+pairs)` and event count (103-115); per sample: role/round/warmup order must match ABBA (116-133),
no failure, recorded program/arguments equal the plan, stream digests equal files (147-171), oracle recomputed from the retained captures (172-289), primary metric equals the recomputed one (290-292); finally the report must equal `summarize(...)` recomputed from the samples (296-300).
`regression_exit`: fewer than 6 pairs or no interval -> 2 (inconclusive); lower bound of 95% interval above the budget -> 3; upper bound at or below -> 0; otherwise 2 (328-353).
Same job elsewhere: ABBA ordering (experiment.rs:242, dominance_summary.rs:109-122). Same job: `oracle` vs `experiment::check_output`. Fail-open: files are limited to 64 MiB (21); `validate` does not re-hash declared input files (only the retained captures and artifact): open.

### B16. `tools/benchmark/src/scaling.rs` (1-182)

Purpose: fail when cost per added unit grows. `run` 33-148 writes programs with N copies of an uncalled machine (`machines`) or N citations of one lemma (`citations`) (`write_program` 155-182), measures `omega --check` via `progress::measure`, computes marginal cost per interval and the ratio of last to first (119-122), exits error when ratio > `--max-ratio` (default 1.3, 38; 140-145).
Needs at least three increasing sizes (74). Cost is user-space instructions where perf exists else user+sys CPU (107-110). Same job: `progress::measure` reuse. The generated source text (157-176) contains the language's `Build::application`, `@mut`, `core\Identity::<Nat>`, `saturating_sub` spellings: not checked for staleness here (grep `saturating_sub|Identity` over `source/library/core`: 372 matches in 15 files, so the names exist; `@mut` appears 762 times in 179 files including `_recycling`). Fail-open: none seen.

### B17. `tools/benchmark/src/selected_bodies.rs` (1-288)

Purpose: prepare inputs and plans for the "selected body copy attribution" profile. `prepare_inputs` 10-70 copies two kept fixtures (`tests/fixtures/calls/run/scalar_contract_route_exit`, `tests/fixtures/ownership/run/borrowed_root_replacement`, 14-23) and the baseline producer into a new directory with SHA-256s;
`capture_plan` 81-146 builds a process plan whose expected stdout is generated text `target=<name> outcome=checked` per target plus a `checked_products=...` line (98-105); `target_names` 148-165 accepts four hosted targets, refuses duplicates, sorts; `prepare_source_plan` 190-267 runs the baseline once and freezes its own stdout/stderr as the oracle (222-266), requiring exit code 0 (242).
Keyed on target-name strings; the oracle is the baseline's own output, so it detects divergence from the baseline, not correctness. Stated: "The retired settled-check counter has no production caller at this base" (103-104). Fail-open: none seen. The fixture paths are relative to the current directory (`root.join`), not to `tools/benchmark` (11, 33).

### B18. `tools/benchmark/src/statistics.rs` (1-244)

Purpose: paired comparison. `Distribution` 6, `Comparison` 17, `median` 30, `distribution` 41 (nearest-rank p95/p99, 45-47), `compare` 64-138, `compare_independent` 140-185. `compare` refuses fewer than 2 pairs, unequal lengths, nonfinite or negative values (65-76); 4000 bootstrap resamples of paired differences with a fixed xorshift seed (89-110); 95% interval is sorted entries 100 and 3899 (113); interpretation `insufficient_pairs` below 6 pairs, else lower/higher/inconclusive by interval sign (116-124); percentages only when every baseline value > 0 (87-88). Five tests. Same job: `progress.rs:485 median` (sorting `Option<f64>` variant) duplicates `statistics.rs:30`. Fail-open: none seen; seeds are fixed (89, 153).

### B19. `tools/benchmark/src/suite.rs` (1-74)

Purpose: run 1..=64 plans from a suite JSON (`omega-performance-suite/1`), each in `case-NN`, preserve each outcome. `run` 15-74: each case is `experiment::compare` then `records::validate`; failure is recorded and the suite continues; final result error if any failed (69-72). Rewrites `suite.json` and `summary.md` after each case (59-67). Plan paths relative to the suite file (41). Fail-open: none seen.

### B20. `tools/benchmark/src/workloads.rs` (1-108)

Purpose: second binary `omega-bench-workload`, Rust reference kernels (`compute`, `memory`, `calls`, `allocations`) printing `omega-timed-work/1` JSON. Units 1..=100_000_000 (44). `Transform` trait dispatch for `calls` (12-28). Stated: "This executable is Rust reference material, not generated Omega evidence." (6). Tests in `tests/runner.rs:488-508` pin four checksums. Fail-open: none.

### B21. `tools/benchmark/tests/fixtures/probe.rs` (1-80)

Purpose: `benchmark-probe` test child. Modes: `--version`, `plain`, `wrong-exit` (exit 7), `stall` (10 s sleep), `flood` (3 MiB stdout), `mutate`/`delete` (touch a given file), `timed` (known checksum loop), `resources` (32 MiB touch plus 100 ms spin),
default compiler mode (copies itself to `<build-dir>/native[.exe]`, prints `published native output to <path>` and a stage line `1.25 ms  Stage fixture`; `PROBE_COMPILE_FAIL` exits 4; `PROBE_PUBLISH_OUTSIDE` prints a path outside the build dir). Pairs: each failure mode is the positive mode with one input changed. Fail-open: none (test fixture).

### B22. `tools/benchmark/tests/runner.rs` (1-617)

Purpose: 17 integration tests running `omega-bench` against the probe. Pins: ABBA roles `baseline,candidate,baseline,candidate,candidate,baseline` and 6 samples (59-95); overwrite refused (108-112); wrong exit / stall / flood each yield a `failed` journal event whose reason contains `oracle` / `TimedOut` / `Overflow` and no `report.json` (116-144);
declared-input mutation and wrong stdout (147-164); reported-work checksum (167-193); compiler samples retain stage table and compare candidate source (196-271); compile failure (274-306); missing `stdout` in oracle rejected before creating output (309-318); lost input (321-334); stale artifact path outside the build dir refused with "fresh build directory" (337-370);
kernel resources (373-407; Windows also needs `cpu_cycles` > 0, 403); `validate` detects changed streams and a recomputed fake summary (410-448); suite preserves success next to failure (451-485); reference workload checksums (488-508); history refuses changed input bytes (511-546); NaN budget refused (549-563); unknown backend refused before running (566-584);
native execution bound to published artifact (587-617). `required-features = ["fixtures"]` per crate map. Fail-open: none (tests).

---------------------------------------------------------------------------------------------------

## Part C. `tools/xtask` (15 files)

### C1. `tools/xtask/src/alpha_forge.rs` (1-376)

Purpose: resolve an annotated Windows Alpha seed listing (hex bytes, `zN` zero runs, `.pad`, labels `name:`, `.rva`, `.rel32`, `.rel8`) to the PE image; or `--check COMMITTED` to compare byte-for-byte. `run` 12, `forge_listing` 89, `parse_listing` 122, `section_map` 216
(reads PE header at 0x3C, section table), `label_rva` 233, `relocation` 243, `file_rva` 259. Decides: label lookup takes the last label of that name (`rev().find`, 236); `.pad` backward is an error (142-144); `rel8` out of range error (253-255); a label or write offset outside every section's raw data is an error (269-271).
Test `windows_listing_reproduces_the_committed_seed` (368-375) reads `bootstrap/alpha/alpha_x64_windows.hex` and `.exe`. Fail-open: `--check` prints diff and exits 1 on any difference; a `--check` with unreadable committed file errors (57-60). None seen.

### C2. `tools/xtask/src/boundary.rs` (1-2141)

1. Purpose. `check-boundary`: enforce the Psi/Omega firewall and native-stage ownership from Cargo metadata and Rust source text.
2. Shape. `Rule` 81 (`owners`, `forbidden`, `forbidden_packages`, `except_owners`, `except_package`, `allowed`), `Allowance` 76, `Metadata/Package/Dependency` 97-117, `under` 119 (path-prefix match), `normalize_relative` 129,
   `forbidden_rules` 146-245, `production_edges` 247, `forbidden_dependency_violations` 270, `stale_allowance_violations` 297, `transitive_forbidden_violations` 428, `transitive_exact_violations` 444, `dependency_violations` 505, `pipeline_dependency_violations` 555-638,
   `call_violations_in` 755-800, `single_form_entry_violations` 894, `plan_validator_call_violations` 918, `working_program_read_violations` 931, `retained_publication_violations` 946, `image_replay_read_violations` 1002, `backend_execution_plan_read_violations` 1014,
   `installation_mutation_violations` 1028, `representation_spelling_violations` 1038, `run` 1065-1267. About 22 tests (1269-2140).
3. I/O. Runs `cargo metadata --no-deps --format-version 1` (1075-1078, plain `cargo`, not `mbx`), reads `.rs` files under `source-rust` (skipping directories named `_recycling`, 648), and `tools/xtask/pipeline-contracts.json` via `boundary_contracts`. Prints violations, exits 1; exits 2 when metadata cannot be read.
4. Decides. Keyed on repository-relative package directory (identity of package location), plus text matches on source lines for the call audits: `line_calls_any` (738-741) is a substring test on a trimmed line that does not start with `//`. `is_test_source` (734-736) exempts any path containing `/tests/` or a file named `tests.rs`.
   `representation_spelling_violations` finds `"string"` compared to a name through compacted line text (657-684, source spelling).
   Rules in `forbidden_rules`: review evidence may not reach package manager or compiler; provider correspondence may not reach source compiler or native driver; only two named owners may reach `BINDING_ISSUER`; only stage 07 and native provider demand may reach `SOURCE_PROVIDER_PUBLICATION`; source stages consumable only by source driver, numbered stages or three listed readers;
   package manager may not reach compiler/driver/native stages; native stages each readable by driver and successor only (via `native_stages::rules`); frontend lexer/parser may not import each other; source records cannot depend on the source driver.
5. Cases by shape. `SINGLE_FORM_ENTRIES` (807-843): seven named files each must define `pub fn <name>` with exactly one parameter (`parameters.len() != allowed`, 903). `public_function_parameters` splits at top-level commas (847-892). Hard-coded native-stage and form lists `AFTER_PREPARATION` 349, `AFTER_SELECTION` 373,
   `SELECTION_REPRESENTATIONS` 393.
6. Same job elsewhere. `linear_pipeline::violations` (C4) walks the same adjacency with its own stage numbering; `pipeline_dependency_violations` does a second BFS over the same edges; `driver_calls::violations` (C3) does an AST version of the call audit that `call_violations_in` does textually. `collect_rs_files` 640 is also used by `driver_calls.rs:3,105`.
7. Unused / counts. `pub` items: `NATIVE_ROOT`, `PROVIDER_SELECTION_ROOT`, `RUNTIME_ROOT`, `RETAINED_IMAGE_RUNTIME`, `NATIVE_PROVIDER_RUNTIME_ROOT`, `PERMITTED_OWNERS`: the binary crate has no external users (crate map: 0 dependents); they are consumed by sibling files (`super::RUNTIME_ROOT` etc., e.g. linear_pipeline.rs:297-299, 337-339). `Allowance`/`Rule` constructed in `native_stages.rs:56-78`.
8. Stated limits. "Cargo metadata includes optional and target dependencies even on a machine that cannot execute the target" (linear_pipeline header). `boundary.rs:622-623` "Keep walking its dependencies so a record cannot hide a call back into the source compiler."
9. Fail-open.
   (a) `call_violations_in` skips any file whose text cannot be read (786-788 `let Ok(text) ... else { continue }`): a non-UTF-8 or unreadable production file is not audited. Open.
   (b) `is_test_source` exempts every file under any `/tests/` directory (734-736), including from the audits for `bind_target_reach`, plan validator, working-program reads, image replay etc.: stated by function name; a production file placed in a `tests` directory is exempt: traced (by reading).
   (c) `line_calls_any` skips only lines starting with `//` (739-740); a call inside a `/* */` block comment or a string would still match (false positive, not accepting).
   (d) `transitive_violations` silently skips a dependency path that is not a workspace member (481-483 `else { continue }`), whereas `dependency_violations` errors on it (528-532). Open.
   (e) `production_edges` ignores dev-dependencies and registry dependencies without a path (257-262): stated by doc comment at 4-5.
   (f) `pipeline_dependency_violations` returns `Err` if a compiler root has no packages (610-612): fails closed.

### C3. `tools/xtask/src/boundary/driver_calls.rs` (1-235)

Purpose: only the drivers may call stage-level operations. Constants `SOURCE_OPERATIONS` (6 names), `NATIVE_REQUEST` (`NativeCompileRequest`), `RETIRED_NATIVE_OPERATIONS` (2 names) (8-21). `operation_owner` 23-61 returns a refusal reason unless the file is an allowed owner (`judge_source_roster` never allowed 30;
`judge_terminal_source` allowed in two files 31-35; `prepare_lower_and_check` allowed in `driver.rs` and `driver/compile_product.rs` 37-39; the rest only under the source driver and not under `representations`; the native request only in four native driver files 51-58; retired operations always refused 60).
`Operations` visitor (63-88) collects identifiers by `syn::Visit::visit_ident` (any identifier spelled as one of those names) and skips `mod` items with `#[cfg(test)]` (68-77, stated 67 "Host unit tests may prepare stage-specific fixtures"). `violations` 90-132 scans all `.rs` under `source-rust` outside tests (is_test_source) with the pre-filter `source.contains(entry)` (113-121).
Decides on identifier spelling and file path. Fail-open: (a) the `cfg(test)` skip applies only when the attribute parses as a single identifier `test` (69-74); `#[cfg(all(test, ...))]` modules are not skipped: over-reports. (b) `syn::parse_file` errors fail the check (122). (c) a name spelled differently via alias `use X as Y` is still caught at the `use` (test 216-234). None seen.

### C4. `tools/xtask/src/boundary/linear_pipeline.rs` (1-696)

Purpose: the numbered-stage layout and dependency direction. `NATIVE_STAGES` 14-31 (16 directories `00_` to `15_`), `PSI_STAGES` 56-65 (8), `FRONTEND` 49, `TEXT_READER` 47, `RETIRED` 40-46, `SOURCE_PRODUCT_ISSUERS` 111-127 (three issuer crates with their stage numbers 4, 5, 6),
`Phase` 68, `stage` 85, `may_issue` 130, `links_another_source_stage` 144, `forbidden_frontend` 152, `layout_violations` 177-205 (physical directories of each pipeline must be exactly the numbered ones plus `README.md`), `violations` 207-359 (BFS over production edges).
Decides: a package may link a product issuer only if it is the producing stage or the representation (130-137, includes dev-dependencies, 235-245); `_recycling` packages and dependencies into `_recycling` refused (223-225, 246-255); a production dependency enabling feature `test-support` refused (256-264); stage ordering: a stage may not depend on a later stage, nor on a native stage more than one before it unless a scoped reader allowance exists (315-346).
Keyed on repository-relative package path and the stage index. Cases by shape: exactly 8 source and 16 native stages (hard-coded lengths, 14, 56). Fail-open: (a) `normalize_relative(...).is_ok_and(...)` (237-240): a dependency whose path fails to normalise yields no issuer violation. Open. (b) packages that are neither in a stage, a runtime root, a representation nor the frontend are skipped (303-305 `continue`): those are covered only by `forbidden_rules`; stated by comment 317-318 only partly. (c) `names[start]` and `names.get(next)` error on undeclared workspace dependency (310-312): fails closed.
Same job elsewhere: C2 `pipeline_dependency_violations` and `forbidden_rules`.

### C5. `tools/xtask/src/boundary/native_stages.rs` (1-145)

Purpose: the native stage reader table. `native!` macro 6-10, constants 12-24, `scoped_readers` 26-58 (7 explicit owner/dependency pairs plus eight object-replay dependencies), `rules` 60-104 (one rule per native stage: owners `""` (everyone), forbidden the stage, exception package = native driver and the stage plus its successor 70-77; plus one rule for stages 6 onwards forbidding three earlier operation forms with the image-emission audit allowed 94-102).
Decides on package path. Stated limits: "Fixed-width entry layouts already contain the final call and relocation fields." (32) and "The image package still contains the requested object correspondence audit." (85). One test (112-144). Fail-open: none seen.

### C6. `tools/xtask/src/boundary/psi_ir.rs` (1-264)

Purpose: structure of `source-rust/subsystems/compiler/psi`. `GROUPS` 9-17 (seven field names with record type names), `Definition::present` 34-55 (a `pub struct` with nonempty named fields and listed public fields, or a nonempty `pub enum`), `loads_module` 58, `field_type` 79, `has_group` 91, `violations` 101-208. Checks via `syn` parsing and `toml`: crate root must be `psi.rs` (108); every `mod` must be a field module or `exports` (114-125); every directory beside it must name a public Psi field and load `<name>/mod.rs` unless it is a consumer crate (126-143); `Psi` must have public `entry` and `machines` (144); each group record exists directly or in `declarations.rs` (149-171); machine control-flow files must define `TerminalMachine {entry, blocks}`, `Block {operations, terminator}`, `Operation {kind}`, `Terminator` enum (178-206).
Cases by shape: the seven GROUPS and four machine files are hard-coded. Fail-open: `.starts_with('.')` and names `tests`, `target` are skipped (134-135): stated by code. None seen.

### C7. `tools/xtask/src/boundary_contracts.rs` (1-574)

Purpose: `pipeline-contracts inventory|check`: keep a reviewed JSON manifest (`tools/xtask/pipeline-contracts.json`, `MANIFEST` 13) in step with Rust items. `ContractAudit` 17 (version 1, roots with `allowed_kinds`, items with `file`, `item`, `kind`, `signature`, `meaning`, `producer`, `consumers`), `Kind` 44 (7 kinds), `Inventory` visitor 67-228 records modules, structs, enums, unions, aliases, functions, impls and methods, traits, `use`, macros, consts, statics, and every field, with a SHA-256 of the item's token string (60-65); `inspect` 230 appends `#n` per repeated name so cfg alternatives stay separate (237-243); `source_files` 247 canonicalises and refuses paths escaping the repository (257-262); `violations` 297-379; `unreviewed_inventory` 381; `run` 404-440.
Decides: owner chosen by longest matching root path (312-322); classification must be in the owner's `allowed_kinds` (323); meaning, producer, consumers nonblank (329-341); each observed item must have a judgment with equal signature else "missing" or "Rust contract changed" (365-371); judgments not observed are "stale" (373-377). Keyed on `file` + item name + token digest (names include scope and `#n` occurrence).
Same job elsewhere: C2's dependency rules; this is the contents-level check. Unused: `pub const MANIFEST` and `pub fn violations` are used by `boundary.rs:1114-1116` (1 call). Fail-open: (a) `visit_item_impl` ignores `ImplItem::Const` and others (`_ => {}` 175), but the impl-wide `implementation` record hashes all tokens (164), so a change to them still changes a signature. (b) macro-generated items inside expression position are not inventoried; stated nowhere: open. (c) `#n` numbering is by order: reordering cfg alternatives changes names (237-243): open.

### C8. `tools/xtask/src/branch_cleanup.rs` (1-742)

Purpose: report or delete stale remote branches (header 1-24). Constants `PROTECTED` (`main`, `work/claims`, `gh-pages`) 37, `PROTECTED_PREFIXES` 46, `EXPLICIT` 48; `Category` 60 (9); `classify` 339-386; `delete_branches` 389-444 (batches of 50, `--force-with-lease=refs/heads/<name>:<sha>`, `--no-verify`); `execute` 549-633; `run` 635.
Decides in order: protected/prefix -> `Protected`; open PR -> `OpenPr`; `EXPLICIT` -> delete; commit younger than `--min-age-hours` (default 48) -> `TooNew`; newest finished PR whose last head equals the tip -> `MergedPr`/`ClosedPr` (delete); tip merged into default -> `Ancestor` (delete); finished PR exists but tip differs -> `MovedAfterPr` (keep); else `UnmergedNoPr` (delete only with `--include-unmerged-no-pr`) (339-386). Keyed on branch name, commit SHA, PR head ref/sha from `gh api repos/<repo>/pulls`, and committer date.
Cases by shape: GitHub repo default `CathedralOS/Omega` (137). Fork PRs ignored (300-303). `max_delete` default 2000 (141).
Fail-open (toward deletion): (a) `list_heads` parses commit time with `.unwrap_or(0)` (222): an unparsable date becomes epoch 0 and passes the age check. Traced: bounded by the later PR/merge conditions and the lease on the examined SHA (398-399). (b) `linked_branches` (249-286) runs `git grep` and ignores its exit status (259-263): an error other than "no match" yields an empty link set, so a branch linked from repository files would no longer be protected. Open. (c) `pull_requests` stops when a page has fewer than 100 items (323-325); a page error returns `Err` (294).
Same job: nothing else in unit.

### C9. `tools/xtask/src/claims.rs` (1-1977)

1. Purpose. Cross-session work-claim registry on the branch `refs/heads/work/claims`: each commit's message is the whole JSON registry (`ClaimsRecord` 26, protocol `omega-claims-v1` 19, empty tree, parent = previous tip; `commit_and_push` 689-748).
2. Shape. `Request` 877 (`Status`, `Claim`, `Release`, `Renew`, `Note`), `parse` 909-1002, `run` 1004-1188, `parse_expiry` 79-148, `claim_expiry` 205, `renew_claim` 226, `revive_claim` 269, `prune_registry` 332, `status_snapshot` 351, `append_claim` 403, `append_note` 425, `Snapshot` 473 with `LegacyRead` modes, `fetch_snapshot` 539, `update_registry` 753-789 (up to 5 attempts, `WRITE_ATTEMPTS` 21), `HeldClaim` 797 (per-worktree ticket file `omega-claim-tickets` under the git dir, 22, 827-838), `generate_ticket_id` 864.
3. I/O. `git ls-remote origin` for the two refs (562-565), `git fetch --depth=1` of exact advertised commits under a shared fetch lock (550-553, 634-637), `git show -s --format=%B` (658), `git push origin <sha>:refs/heads/work/claims` plain, never forced (736-745). Prints JSON.
4. Decides. Lease 120 minutes (20). Expiry = recorded `expires_utc` capped by `max(claimed_utc, renewed_utc) + 120 min`; "a recorded expiry can shorten a claim but never lengthen it" (199-222). A claim with no parsable time stays visible (203-204, 75-77). Overlap of two claims = same path or one inside the other by `/` boundary (253-262); `claim` never refuses overlap (264-266 "Claiming never refused overlap, so neither does restoring"). Release is by ticket only (1066-1072).
   Every write drops ended claims and notes whose ticket has no visible claim (325-349), then republishes the whole record.
5. Cases by shape. Registry reading shows both the branch and the legacy ref `refs/coordination/omega-claims/main` (16-18, 462-468).
6. Same job elsewhere. `format_utc` 159 and `parse_expiry` 79 versus `progress.rs:622 utc_timestamp` (benchmark) both hand-roll civil date arithmetic; also `provenance.rs` timestamps not in unit (suspected).
7. Unused. `fetch_record` 647 is `#[cfg(test)]`. `pub fn run` 1 caller (`main.rs:118`).
8. Stated limits. "until TASKS.md `CLAIMS-LEGACY-READ-REMOVAL` deletes that read" (16-18).
9. Fail-open. (a) Unknown or malformed expiry keeps the claim visible (205-222, `is_none_or(... > now)` 231, 299, 336, 353): stated (75-77); the effect is to block, not to free, a claim. (b) `update_registry` treats a push failure as success when the freshly fetched record equals the intended record (773-775): stated ("A lost acknowledgement can hide a successful publication", 772).
   (c) `renew_claim` on a ticket with no claim returns `Ok(false)` without error; `Note` on an unknown ticket still appends a note (425-451 calls `renew_claim` but ignores false): the note is then pruned at the next write because it has no live claim (344-348): traced.
   (d) `held_claims` failures return an empty list (840-845): `renew` with no `--ticket` then errors "holds no claim" (1093-1096): fails closed.
   Tests: 26 `#[test]` (crate map).

### C10. `tools/xtask/src/doctor.rs` (1-528)

Purpose: `doctor` host preflight, seven checks: git (HEAD exists; `pull.rebase true` else warn), rustc (`rustc --version`; nightly pin vs resolved), link probe (compile and run a tiny program exiting 7), mbx (installed version must be >= pin in `tools/mbx-version`; equal string `mbx <pin>` else warn "checksum caching is unqualified"; 244-280),
nextest (>= 0.9.140, 19), claims remote (exactly one push URL; `git ls-remote` of the claims ref; else warn), host (cpu count). `check_*` 85-382, `collect_checks` 384, `run` 396-461 prints lines and optional JSON (`omega-doctor-v1`). Exit 1 only when a check is `fail` (460).
Decides on command exit codes and parsed version triples (`parse_semver_triplet` 290-301 takes the first whitespace token with three dot parts). Cases by shape: fix hints are OS-specific strings (213-227). Unused: `toolchain_channel`, `parse_semver_triplet`, `collect_checks` are `pub` but used only inside this file (grep over `tools/**/*.rs`: all hits in doctor.rs and main.rs:117). Fail-open: a newer mbx than the pin is "warn" not "fail" (269-278, test 498-503): stated. Claims-remote unreachable is "warn" (361-367). None else.

### C11. `tools/xtask/src/execution.rs` (1-541)

Purpose: run a command in its own OS container (Windows job or Unix process group via `bounded_process::ToolCommandChild`), keep receipt JSON, stdout and stderr files in an evidence directory, kill the whole tree on timeout or error, never overwrite earlier evidence (`File::create_new` 141, 145, 148). `run` 27 (default 1800 s), `run_unlimited` 33, `run_with_timeout` 111, `run_bounded` 120-195, `OwnedCommand` 45-109, `read_bounded` 197 (16 MiB), `output` 213, `digest` 222 (`certutil` / `shasum -a 256` / `sha256sum`), `write_json` 11, `reserve_directory` 16.
Decides: zero timeout refused (126-128); cleanup closes the container whether or not the leader exited (62-97). Doc comment 31-32: "Build preparation can wait behind other agents in mbx. Its wall time is progress evidence". Note `directory` is only the evidence directory: the command's working directory is whatever the caller set (receipt records `command.get_current_dir()`, 135).
"Shared" claim (header line 1: "shared by producer preparation and program gates"): grep `execution::` outside execution.rs over `tools/xtask`: users are `producer.rs` (12 sites), `qualification.rs` (7), `samples.rs` (about 24 incl. tests); `sweep` does not use it. Unused: `run_with_timeout` has callers only in this file's tests and `run` (grep `execution::` shows no external `run_with_timeout`; `producer.rs:276`, `qualification.rs:48` call `run`, `producer.rs:253` calls `run_unlimited`). 
Fail-open: `digest` returns the first 64-hex-digit word of the tool's output (243-247); an output with no such word errors (247). None seen. Nine tests including descendant-kill checks.

### C12. `tools/xtask/src/fmt.rs` (1-238)

Purpose: format workspace targets with `rustfmt` in chunks below 24,000 characters of argv (`ARGV_BUDGET` 19), one chunk per available CPU, dealt round-robin (`chunked_invocations` 95-121). `workspace_targets` 49-90 runs `cargo metadata --no-deps` and groups unique `src_path`s of target kinds `lib, bin, test, bench, example, custom-build, proc-macro` (20-28) by edition. `run` 123-199 supports `--check`, `--verbose`, `--repository`, `--` passthrough; spawns up to `lanes` processes. Fail-open: if metadata lists no targets (empty groups), no invocation runs and the command exits 0 (160-198): open. A spawn error counts as a failure (188).

### C13. `tools/xtask/src/gamma_invoke.rs` (1-372)

Purpose: frame a Gamma source (`u32` LE length, source bytes, optional sealed input; 117-133) and run an evaluator; publish stdout atomically (fsync + rename, 223-260). `parse_args` 41, `invoke` 85-110, `run_evaluator` 135-190 (two threads for stdin and stdout, poll every 10 ms, kill group on timeout), `terminate` 192, `exit_code` 209. Exit statuses: timeout or source > 2^32-1 bytes returns 3 (header 4-6; 101, 123). A nonzero evaluator status with empty stdout leaves the output untouched (105-107); any stdout is published even for a nonzero status (108-109).
Fail-open: (a) `exit_code` truncates with `code as u8` (210-212), so a child exit status of 256 would read as 0: open (Windows codes above 255). (b) The stdin writer ignores write errors (163-164). Tests (Unix only) 283-371.

### C14. `tools/xtask/src/main.rs` (1-168)

Purpose: dispatch 17 subcommands (107-129, `usage` 69-98), `repo_root` 62, `write_status` 50 (BrokenPipe treated as success). Module doc lists 17 forms (3-21): omits `pipeline-contracts` and the `claims renew` subcommand (doc 13 `claims <status|claim|release|note>` vs `claims.rs:1011`). Unknown command exits 2 (125-128). One test pins that every subcommand name appears in usage and that "Terminal Psi" does not (141-167). Modules listed 23-43 include `producer`, `provenance`, `qualification`, `repository`, `rules`, `rust_producer_omission`, `samples`, `source_closure`, `sweep`, `work_interval`, `worktree` (not in this unit). Fail-open: none.

### C15. `tools/xtask/src/portable_services.rs` (1-1051)

1. Purpose. `check-portable-services`: from source text alone, find facts one hosted target applies to a portable `std` call that the others do not, and compare them with a reviewed baseline file.
2. Shape. `HOSTED_TARGETS` 41 (4), `Row` 137, `Reason {Noise, Allowed, Known}` 168, `Baseline` 189, `parse_baseline` 197-245 (5 fields `Service.member | fact | targets | reason | note`; `known` must name a `### ` heading in `OWNER_CODE_CONCERNS.md` 230-234), `compare` 306, `collect_rows` 327-382, `provider_selections` 390-411, `family_files` 415-446, `Family` 464-564, `classify` 610, `split_toplevel` 642, `provider_members` 684, `is_translation` 712, `called_names` 730, `facts` 801-837, `limits` 841, `comparison_operand` 863, `top_level_arguments` 890. Inputs `source/library/std/build.omg`, files under `source/library/std` and `targets/<target>/`, `tools/ratchets/portable_services.txt`, `OWNER_CODE_CONCERNS.md`.
3. Decides. Facts: `refuse K` (an `ErrorKind::K` followed by `}`, 803-811); `limit OP N` (comparison with an integer literal other than 0, 1, -1 or `x.length` inside `transition`/`requires`/`ensures` clauses, 814-820, 841-860); `const f(..N..)` (a whole call argument of three or more digits, 829-833); `clamp f` (a call whose name contains `min_u64`, `saturating`, `wrapping`, 825-828).
   Closure: calls followed up to 8 levels (`CLOSURE_DEPTH` 52); error-translation machines skipped by name containing `error`, `errno`, `kind_of`, `reconstruct`, or a `_kind` word (712-726). A row exists when some but not all four targets have the fact (369). Failure on rows missing from the baseline and on baseline rows that no longer appear (306-321). Keyed on source spelling: regex-free substring scans of text with `//` comments stripped (572-586).
4. Cases by shape. Column-0 declarations only: machines, `Name satisfies Trait {` provider blocks, and the list `data, use, boundary, domain, trait, enum, const, type` (610-639); provider members only at a four-space `    machine ` line (684-697); target ownership by filename prefix `posix_`, `windows_`, `darwin_`/`macos_` (428-431).
5. Same job elsewhere. None in unit.
6. Stated limits. "The extraction is textual. It reads column-0 declarations, strips `//` comments, and does not parse Omega; a row it misses is a gap in coverage, not a pass." (30-33).
7. Unused. `pub const HOSTED_TARGETS` used 6 times, all within this file (grep). `pub fn collect_rows`/`compare`/`parse_baseline`: used in-file.
9. Fail-open. (a) A service whose `Build::select_provider` rows do not cover all four hosted targets is skipped silently (336-342 `continue`): stated by the header ("that covers all four hosted targets", 11-12). (b) Facts not matching the four extractors are not rows: stated (30-33). (c) `top_level_arguments` returns an empty list when a call has no closing bracket (909): that call contributes no `const` fact: open. (d) `member_facts` follows a called name to every machine of that name in the family (553-557); an identically named unrelated machine adds facts: false rows, not missed rows.

---------------------------------------------------------------------------------------------------

## Coverage

Read in full (every line of every range in `units/unit_089.tsv`): all 49 rows, 21,713 lines:
- Omega source rows 1-12: 5,709 lines (zone 493, checks 326, layouts 31, scalar 129, source 715, symbol_resolved_trees 799, syntax_trees 1831, numeric 21, tokens 230, typed_trees 972, fixture build 7, fixture main 155).
- `tools/benchmark` rows 13-34: 5,956 lines.
- `tools/xtask` rows 35-49: 10,048 lines (alpha_forge 376, boundary 2141, driver_calls 235, linear_pipeline 696, native_stages 145, psi_ir 264, boundary_contracts 574, branch_cleanup 742, claims 1977, doctor 528, execution 541, fmt 238, gamma_invoke 372, main 168, portable_services 1051).
Skimmed: none. Not read: none.
Not opened (outside the unit but named): the lexer/resolver/typer/checker bodies, `pipeline-contracts.json`, `tests/static_arguments.py`, the examples that `include!` the benchmark harnesses.

## Five things a maintainer most needs to know about this unit

1. Every Omega representation file is a fixed-capacity table with the same guarded-accessor pattern (`transition i < count {true -> read, _ -> invalid() crash Trap}`), copied roughly 100 times (syntax_trees.omg 1142-1831, typed_trees.omg 237-704, symbol_resolved_trees.omg 405-799). The readers by capacity (`*_binding_at`, symbol_resolved_trees.omg:606-658) and by count disagree for the same tables, and `name_resolution_clear` (381-392) does not zero the arrays.
2. `source.omg` returns NUL for any out-of-range byte (468, 581, 712), and `spans_equal` compares two such reads as equal (484-502), so two equal-length out-of-unit spans compare equal; the comment at 429-431 only says a violated coordinate "fails a comparison".
3. `zone.omg` is called from six checker files; `implies` and `bound_none` have no callers (grepped), and `constrain` closes only subjects below `zone.count` (268, 289).
4. xtask `check-boundary` is mostly text and path based: file-level exemptions for any `/tests/` path (boundary.rs:734-736), substring line matching (738-741), unreadable files skipped (786-788), and non-workspace path dependencies skipped in `transitive_violations` (481-483). The AST-based pieces are `driver_calls.rs`, `boundary_contracts.rs` (token hash per item) and `psi_ir.rs`.
5. Benchmark integrity rests on the journal: `experiment::compare` writes it and `records::validate` recomputes the report from retained captures and refuses any difference (records.rs:296-300). Some oracles are self-referential: `selected_bodies::prepare_source_plan` freezes the baseline's own output (selected_bodies.rs:222-266), and `progress::accept_review` accepts every pending package decision (progress.rs:353-361).

## What I am unsure about

- Whether the Omega resolver and typer zero their tables before use, and whether callers test the `0` return of `type_table_intern` and the `false` of `source_set_add` / `push_slot_*`; none of these callers are in the unit.
- Whether `ScalarStatus::Unchecked` is ever accepted by the Alpha emitter.
- Whether `constrain` stays closed for subjects with index >= `zone.count` (unit-local reading says rows/columns are limited to `count`).
- Whether `tools/benchmark/service-correspondence/tests.rs` is gated by `#[cfg(test)]` in the including example (its `mod tests` at line 1 carries no attribute).
- Whether `static_arguments.py` splices exactly the driver pieces `main.omg` names (not read).
- Whether the `check` exit code 2 ("inconclusive") is treated as pass or fail by any caller (no caller in the unit).
