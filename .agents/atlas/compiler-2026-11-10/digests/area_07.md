# Area 7 digest: Psi front, build-time evaluation and stages 00 to 04

Source: reader records `unit_038` to `unit_045` (eight records, all read in full; there are no second-pass gap records for these units). Code checked at commit `457adf4a11f`.

Path shorthand used in every citation:

- `BTE/` = `omega-rust/psi/compilation/build-time-evaluation/src/`
- `S00/` = `omega-rust/psi/compilation/pipeline/00_source-files-to-tokens/src/`
- `S01/` = `.../pipeline/01_tokens-to-syntax-trees/src/`
- `S02/` = `.../pipeline/02_syntax-trees-to-symbol-resolved-trees/src/`
- `S03/` = `.../pipeline/03_symbol-resolved-trees-to-typed-trees/src/`
- `S04/` = `.../pipeline/04_typed-trees-to-typed-trees/src/`

Words such as "open", "stated", "traced", "suspected", "inferred" are the readers' own classes and are kept as written. "(checked)" means I opened the cited lines myself; see "What I checked myself".

---

## What this area does

This is the front of the Psi compiler. Source text becomes tokens (stage 00), tokens become syntax trees (stage 01), names in the trees are bound to symbols (stage 02), the trees are given types (stage 03), and the typed trees are rewritten so that generic machines, operators, requirement calls and inline assembly are in the closed form the checker expects (stage 04). Beside these stages sits the build-time evaluation crate. It runs authored machines at compile time to compute constant values, const generic arguments, array lengths, layout plans, access views and Build declarations, and it writes the results back into the syntax or typed trees. To do that it calls stages 02 to 04 on small private "probe" programs.

**Build-time evaluation (`psi/compilation/build-time-evaluation`, 38,029 lines, 10 files).** In: syntax trees before resolution, then typed trees, plus a selection of which declarations to work on. Out: the same trees with folded values, installed layout geometry, generated access-view records, admitted Build declarations, and one receipt per evaluation run. File groups:
- `const_evaluation.rs`, `const_generic_expressions.rs`, `const_initializers.rs`: find pending compile-time sites (const applications, domain facts, array lengths, range endpoints, layout queries, `const` initializers), evaluate each through a probe machine, substitute the value back, and compare what the probe used against what the author's source selected ("custody") (unit 038).
- `machine_execution.rs`: decides whether a machine may run at compile time (`BuildTimeAdmissionPlan`, `:3331`; floor at `:3922-4008`), runs it, and checks the result is a const-evaluable value (unit 039).
- `layouts.rs`: runs authored policy machines on a reflected schema value, validates the returned plan (`validate_plan` `:4333-4749`), installs geometry, and generates `Access<Policy,Schema>` records and accessors (unit 039).
- `product.rs`: admits Build-issued declarations (native bindings, provider selections, exclusions, wire demands) by matching each executed event to a declared request (unit 039).
- `scalar_applications.rs`, `evaluation_accounting.rs`, `syntax_probes.rs`, `lib.rs`: Terminal-artifact scalar entries under a work meter, receipts and their charge checks, probe program preparation, and the two-phase driver (`evaluate_pre_resolution` `BTE/lib.rs:201`, then `PreCheckEvaluation` `:350`).

**Stage 00 (`00_source-files-to-tokens`, 1,196 lines, 3 files).** In: file paths, then loaded text. Out: one `TokenStream` per source, including whitespace and comment tokens; string literals stored decoded. Reports only the first failure in source order (`S00/lex_sources.rs:22-25`).

**Stage 01 (`01_tokens-to-syntax-trees`, 23,671 lines, 12 files; 8,312 are `tests.rs`).** In: token streams. Out: `SyntaxTrees` arenas, one "page" per parser worker. The parser also rewrites while parsing: `move x` becomes `x`, non-boolean transition subjects are hoisted to a generated local, atomic `let` calls become two statements, compound assignment becomes `x = x op y`, `utf16 "..."` becomes an array literal, `asm` blocks are expanded to scratch-register and flag expression trees (unit 040, "Five things" item 3).

**Stage 02 (`02_syntax-trees-to-symbol-resolved-trees`, 62,177 lines, 23 files).** In: syntax trees, an optional source map, top-level bindings, an optional retained base. Out: `SymbolResolvedTrees`, or a deferred frontier for later completion. File groups:
- `preparation*.rs` (six files): syntax-to-syntax work before any symbol exists, using a throwaway symbol table (`ConstantSelection`, `S02/preparation.rs:57`): target families, type aliases, trait defaults, generic data instances, pre-resolution const evaluation, type and machine equations (units 041, 042).
- `lowering*.rs` (three files): copy syntax rows into the resolved carrier with every symbol invalid; hoist indexed reads and match subjects into `__hoist_N` locals; reject bad operator-token bindings (unit 041).
- `symbols*.rs` (eight files): build the symbol table and stamp a symbol on every reference (unit 043).
- `selection.rs`, `selection_authored.rs`: ten selection passes that lexical lookup cannot settle, and the ledger of authored selections (unit 043).
- `constant.rs`: const initializer validation, dependency closure, custody receipts, and substitution of each const use by a fresh copy of the initializer (unit 041).
- `resolution.rs`: the entry points and the `Lowerer` working state (unit 042).

**Stage 03 (`03_symbol-resolved-trees-to-typed-trees`, 18,853 lines, 9 files).** In: `SymbolResolvedTrees`. Out: `TypedTrees`. Runs validators, types each declaration family, then a fixed 15-step settlement (`close_typed_cohort`, `S03/lowerer.rs:1849`) (unit 044).

**Stage 04 (`04_typed-trees-to-typed-trees`, 30,104 lines, 10 files; this area's records cover 8 files, 23,707 lines).** In and out: `TypedTrees`. An unbounded fixed-point loop (`monomorphize_generic_machine_value_calls`, `S04/monomorphization.rs:3746`) runs about twenty sub-passes per round: clone generic machines per bound tuple, close requirement calls, bind selected operators to calls, fill elided lifetimes, give literals and calls a result type, and normalise scratch-register assembly into a helper machine (units 044 file 10, 045). `preparation.rs` and `selection_and_reflection.rs` belong to another area's record.

---

## How it decides

What each crate's judgments are keyed on, from the records.

- **Stage 00.** Source characters only: a fixed dispatch order (`S00/lexer.rs:409-445`), a closed whitespace set (`:594-596`), a fixed escape list (`:311-312`).
- **Stage 01.** Token kind and position, and token spelling through `at_contextual(...)`, which accepts an identifier or any keyword with that text (`S01/tokens_and_storage.rs:633-642, 770-775`). Some decisions depend on whether a speculative parse succeeds (`S01/declarations.rs:2179-2183`; `S01/expressions.rs:1305-1315`). One depends on whether the machine authored a return type (`S01/bodies_and_transitions.rs:254-256`).
- **Stage 02 preparation.** Identifier text plus a `ConstantSelection` lookup `find_top_level_by_name_and_kinds_from_source(name, kinds, span)`; the syntax item is then re-found by span and name equality (`S02/preparation.rs:1388-1392, 1528-1543`). Generic data instance identity is `(template ItemHandle, Vec<ClosedArgumentIdentity>)` (`S02/preparation_generic_data.rs:1116-1131, 3293-3299`); const arguments inside that identity are the argument's text (`:550-566`).
- **Stage 02 lowering.** Syntax node kind; syntax handles for hoist-temp sharing (`S02/lowering_statement.rs:185-195`) and saved subjects (`S02/lowering.rs:974-1002`).
- **Stage 02 symbols.** Child symbols by position in the insertion order (`S02/symbols_symbol_table.rs:25-27, 978-1152`; `S02/symbols_top_level.rs:1345-1359`); references by spelling plus source visibility; call targets by a ladder on the receiver symbol's kind (`S02/symbols_targets.rs:44-278`).
- **Stage 02 selection.** The authored-selection ledger groups copies by `(source span, exposure, kind, partition)` (`S02/selection_authored.rs:1596-1601`); requirement rows by name strings and a formatted dispatch-shape string (`S02/selection.rs:1345-1361, 1511-1621`); operand owners by declaration handle (`S02/resolution.rs:1781-1797`).
- **Stage 03.** Cohort frontiers by count per family (`S03/lowerer.rs:1733-1821`); symbol identity for most lookups; spelled names for a list of special forms (below); exhaustiveness by arm shape, with subjects compared structurally (`S03/equality_and_exhaustiveness.rs:1726-1820`).
- **Stage 04.** Symbol identity for clone correspondence; normalized identity text of bound arguments for instance reuse (`S04/monomorphization.rs:3343-3354`); child order for evidence-binder placeholders (`S04/conformance.rs:2021-2027`; `S04/body_cloning_and_rewriting.rs:4169-4170`); string names for lifetimes (`S04/conformance.rs:893-896, 930-933, 997-998, 1045-1048`).
- **Build-time evaluation.** Source spans and symbols for custody (`BTE/const_generic_expressions.rs:1375, 1770, 6309`); `ScalarApplicationId` for scalar jobs (`BTE/scalar_applications.rs:97-109`); request equality for Build events (`BTE/product.rs:95, 460, 617, 836, 2261`); package authority for selections (`BTE/machine_execution.rs:3211-3239`); and many name strings (below).

### Judgments keyed on source spelling

Counting rule: one site per distinct `file:line` a record cites where a decision compares, parses, splits or formats a name. Grammar keywords in the parser are not counted. Name-plus-visibility lookups are counted once per cited call site. My totals from the records: build-time evaluation 65, stage 00 not applicable (a lexer reads characters by definition), stage 01 29, stage 02 about 155, stage 03 56, stage 04 31. About 336 in the area.

Grouped by what the spelling stands for:

1. **Compiler-owned declarations recognised by toolchain origin, file name and declaration name.** `layout.omg` + `Access`/`Accessor`: `BTE/layouts.rs:1117-1127, 1829-1835, 2300-2311, 2615-2633, 3943-3948`; `S03/lib.rs:82-104`. `reflect.omg` + five helper names: `BTE/const_generic_expressions.rs:1432-1447, 1493-1508`. `float_provider.omg` + `NativeFloats::`: `S02/lowering_machine.rs:2188-2219`. `static_interrupt.omg`: `S02/lowering_machine.rs:2226-2242`. Sealed catalog families by namespace string, leaf string and file path: `S02/lowering.rs:2501-2515, 2617-2632` (the code comment at `:2480-2481` says "never the leaf spelling"; unit 041 records that the key is namespace string + leaf string + sealed path). Layout storage, with a byte-for-byte compare of the whole source file: `S02/preparation.rs:87-102`. `extent.omg` + `Extent`: `S03/lib.rs:221-240`. `<build-prelude>` + `BuildBindingOperations`: `S03/call_and_product_selections.rs:88-102`. `core/code_layout.omg` + `CodeLayout`: `S03/expression_lowering.rs:483-490`. `X86StaticInterruptScope` + `run`: `S03/call_and_product_selections.rs:1619-1628`.
2. **Operation names with no origin gate** (unit 044: "many are not" gated): `address_of` (`S03/expression_lowering.rs:1023-1026`), `code_layout` (`S03/expression_lowering.rs:1360-1362`; `S03/statement_lowering.rs:1319`; `S02/constant.rs:2069-2071, 2368-2370`; `S02/selection_authored.rs:371-373, 1244-1246`), `Quotient` `lift`/`define` (`S03/expression_lowering.rs:1703-1719`; a valid authored symbol is an error, `:1728-1735`), machine name suffix `::provider_defaults` (`S03/declarations.rs:1550`, checked), `Equatable` by display path (`S03/equality_and_exhaustiveness.rs:417-422`; `S02/preparation.rs:2657`), `OmegaLayout` (`S03/type_reference.rs:1381`), `IntervalSet`/`CountedQuantity` skipped as generic templates (`S02/preparation_generic_data.rs:3086-3092`), `Build::bind` (`S01/statements_and_assembly.rs:2435-2509`), Build operations by call-target spelling (`S02/lowering_statement.rs:1209-1226`; `S02/selection_authored.rs:1006, 1259`; `S02/symbols_targets.rs:639-640`; `S03/call_and_product_selections.rs:1899-1903`; `S03/declarations.rs:1597, 1607-1609`; `BTE/product.rs:3140`), atomic member names (`S01/expressions.rs:959-992, 1226-1276`; `S01/statements_and_assembly.rs:92-134, 211-254`), byte-sequence predicates (`S02/symbols.rs:1128`; `S03/statement_lowering.rs:244-254`), `IntegerEmbed` name with an unresolved symbol (`S02/lowering_statement.rs:451-460`).
3. **Enum cases, fields and members decoded by name.** `Trap`/`Abort` (`BTE/product.rs:726-731`; `S01/contracts_and_parameters.rs:1571-1577`; `S01/statements_and_assembly.rs:2246-2255`), 14 `PhysicalAuthorityClass` names (`BTE/product.rs:741-761`), `Fused`/`Independent` (`BTE/product.rs:3301-3306`), access plan cases `Stable`/`External`/`Atomic`/`Inaccessible` and placement cases (`BTE/layouts.rs:1360-1397, 1478-1480, 1493-1495, 4811-4852`), `Rat` with fields `num`/`den`/`neg`/`pos` and `Zero`/`Succ`/`prev` (`S02/preparation_const_evaluation.rs:4999, 6391-6459`), struct fields and cases when materialising a value (`BTE/const_initializers.rs:1525, 1543, 1563-1566, 1710-1713, 1729`), result case by variant name (`S02/symbols.rs:576`; `S02/lowering.rs:3541-3551`), `"Carry::Portable"` (`S01/contracts_and_parameters.rs:298-306, 646`; `S01/type_syntax_and_patterns.rs:877-889`; `S02/preparation_const_evaluation.rs:2699`; `S03/type_reference.rs:30-49`).
4. **Values carried as a name.** A folded number is written back as `Named{symbol: invalid, name: "<digits>"}` (`BTE/const_evaluation.rs:855-858, 2390-2393`; `BTE/const_generic_expressions.rs:5994-5997`; `S03/type_reference.rs:967-973`; `S04/monomorphization.rs:1602-1616, 3982-4066`) and read back by parsing the name (`S02/constant.rs:3525`; `S02/lowering.rs:4465-4482`; `S02/preparation_type_equations.rs:650`; `S02/preparation_const_evaluation.rs:1644, 5557`; `S04/data_and_product_operands.rs:767-770`; `S04/body_cloning_and_rewriting.rs:3402`; `S04/contextual_and_requirement_calls.rs:4171`; `S04/monomorphization.rs:2609-2632, 3188-3197, 4490-4498`; `S04/constructed_results.rs:949` `name == "0"`). An invalid-symbol `Named` counts as closed (`BTE/const_evaluation.rs:307-309`, checked) and is compared by name (`BTE/const_evaluation.rs:693-697`).
5. **Entry state by name.** State named like the machine's last `::` segment, else the first state: `BTE/machine_execution.rs:737-752, 1833-1843`; `BTE/const_evaluation.rs:2940-2951` (all three checked); `S02/selection.rs:1645-1660` (else a state named `"entry"`). Named state else `"entry"`: `S02/symbols_targets.rs:326`; `S02/symbols.rs:4906-4911`. Machine run by name: `BTE/const_evaluation.rs:1403-1417, 1502-1509, 2882`.
6. **Primitive and scalar carriers by type name.** `BTE/const_generic_expressions.rs:6002-6009` (nine names), `:6711-6717`; `BTE/machine_execution.rs:1663, 1871, 2209`; `starts_with("Atomic")` at `BTE/machine_execution.rs:1866, 2204`; `BTE/const_initializers.rs:3035-3036`; `S02/constant.rs:486-489, 532, 4789-4794`; `S02/preparation_const_evaluation.rs:346-350, 5311-5313`; `S03/equality_and_exhaustiveness.rs:534`; accessor base chosen by schema type name `bool`/`u32`/`u64` (`BTE/layouts.rs:2583-2612`).
7. **Generated names later found by name.** `Access<{policy},{schema}>` and accessor names (`BTE/layouts.rs:4190-4196, 2583-2612`, found again at `:2184-2219`); policy machine `"{attached}::plan"` (`BTE/layouts.rs:3891, 862-865`); probe owner by name and span (`BTE/const_generic_expressions.rs:6192-6203`); `format!("{attached}::{target}")` call targets (`S02/symbols_expression_paths.rs:75-87`; `S02/symbols_targets.rs:152-172, 985-994`; `S02/symbols_top_level.rs:508-585`; `S03/call_and_product_selections.rs:427-440`).
8. **Type or path equality by rendered string.** Operand shape string for duplicate token bindings (`S02/lowering_machine.rs:265-269`); module and import paths joined with `::` (`S02/selection.rs:2593-2604`; `S03/type_reference.rs:464-486`); dispatch shape string (`S02/selection.rs:1511-1621`); call-context agreement by text (`S02/preparation_generic_uses.rs:486`); product path strings (`S04/data_and_product_operands.rs:1083-1088`; `BTE/product.rs:62, 420, 700, 889, 2092`); provider slot key `(package, display_path string, identity string)` (`BTE/product.rs:1555, 1650`).
9. **Lifetimes and binders by name.** `S04/conformance.rs:733-737, 893-896`; `S04/monomorphization.rs:169-174`; `S04/data_and_product_operands.rs:1350, 1485`; `S02/preparation_type_equations.rs:1265, 2087`; `S02/symbols_type_references.rs:1071-1081`.
10. **Whole-program spelling search.** A late-bound selection with no exact owner is accepted if every symbol in the program with the same spelled name and a matching kind passes the package check (`BTE/machine_execution.rs:2992-3045`). Stage 02 rejects an unresolved one-segment name that spells any variant of any data definition (`S02/symbols_expressions.rs:50-58, 80-85`).

---

## The same job in more than one place

Each job has a name, its sites, and how the copies differ where a record says. Cross-record pairs I opened are marked (checked).

**Across crates**

1. **Source span that identifies a called declaration (call custody).** `S02/selection.rs:493-521` over resolved trees; `BTE/const_generic_expressions.rs:43-100` over typed trees. (checked) Same job, with two shared error strings. Both ask for an evidence requirement first. Then stage 02 tests kind-based provenance before the plain source span; the build-time copy tests the plain source span (`:55`) before kind-based provenance (`:59-72`). For a source-less entry state, stage 02 takes the first State child of the parent (`:536-544`); the build-time copy requires exactly one owning machine and the first state of its roster (`:78-93`).
2. **Entry state of a machine.** Three identical bodies in one crate: `BTE/machine_execution.rs:737`, `:1833`, `BTE/const_evaluation.rs:2940` (checked). Other rules for the same question: `S02/selection.rs:1645-1660`, `S02/symbols_targets.rs:326`, `S02/symbols.rs:4906-4911`, `S02/selection_authored.rs:597-604` (first state).
3. **Is a domain name exposed to this reference.** Stated single law `symbols/src/table.rs:622` (called from `S02/preparation.rs:526, 648`, `S02/selection.rs:2213, 2224`). Re-implemented at `S02/selection.rs:2585-2605` (module paths as joined strings; `authored == domain_path` passes) and `S03/type_reference.rs:464-486` (module symbols; import paths as strings; no reference means not exposed). (checked) Same job, different keys. Three more candidate pools with their own tie-breaks: `S02/symbols.rs:1576`, `S02/symbols.rs:2121`, `S02/selection.rs:2206-2225`; visibility re-derived again at `S02/preparation.rs:299, 509, 583`.
4. **Default ranking view for a single-subject `terminates by`.** `S02/lowering_machine.rs:1956-2037` (parameter found by name string; also accepts a `.len` member; no step limit) and `S03/lowerer.rs:174-245` (parameter found by symbol; 128-step limit; runs only when stage 02 left the view empty). (checked) Same rule: slice or fixed array gives `SLICE_LENGTH`; `u8`..`u64` gives `NAT_DESCENDING`. The stage 02 comment says it mirrors the checker's `decreasing_value_kind` "EXACTLY" (`:1946-1955`); that third site was not read by any record here (suspected).
5. **Strip `Reference` and `Constrained` to the base type.** `S02/symbols_expression_paths.rs:168` and `S03/call_and_product_selections.rs:320` (checked: same loop, different child fetch). Further peelers recorded: `S03/equality_and_exhaustiveness.rs:704, 724, 1948`; `S03/statement_lowering.rs:809`; `S03/expression_lowering.rs:2288`; `S03/lowerer.rs:221`; `S02/lowering_machine.rs:2007`; `S02/preparation_generic_uses.rs:1616, 1633`; `S02/preparation_const_evaluation.rs:5524-5529`; `S02/preparation_generic_data.rs:1362-1370`. 13 sites.
6. **Walk every child of an expression node.** 20 hand-written walkers recorded: `S01/type_syntax_and_patterns.rs:49`; `S02/constant.rs:1550, 2066, 3887`; `S02/lowering.rs:1249`; `S02/lowering_statement.rs:1844`; `S02/preparation.rs:3214-3473`; `S02/preparation_generic_uses.rs:1266`; `S02/selection.rs:326`; `S02/selection_authored.rs:1087`; `S02/symbols.rs:336, 1284, 2575`; `S02/symbols_expressions.rs:927`; `BTE/const_evaluation.rs:878, 1038, 3488`; `BTE/const_generic_expressions.rs:714, 785`; `S04/conformance.rs:659`. They differ in which children they visit: `S02/lowering_statement.rs:1866-1873` ignores `machine_arguments` and `atomic.result`; `S02/selection.rs:346` skips saved subjects that `S02/selection_authored.rs:1087` follows; `BTE/const_evaluation.rs:2461-2481` (syntax level) sees only six node kinds.
7. **Toolchain-file custody test.** Written inline in each crate that needs it (sites in spelling group 1). `layout.omg` alone: five inline copies in `BTE/layouts.rs` (checked) plus `S03/lib.rs:88-98`; my grep also finds it in typed-trees (5), validation (1) and stage 06 (1), outside this area. `reflect.omg` name list: two copies in `BTE/const_generic_expressions.rs` plus `validation/src/machine_parameters_and_bindings.rs:3399` and `S04/selection_and_reflection.rs:2514` (grepped by unit 038 and by me; not read).
8. **`Carry::Portable` expansion to every carry permission.** `S01/contracts_and_parameters.rs:292-325`, `S01/type_syntax_and_patterns.rs:877-889`, `S03/type_reference.rs:30-49`; refusal by the same name at `S02/preparation_const_evaluation.rs:2699`. Unit 041 "suspected" other grammar sites; unit 040's record and my grep confirm the stage 01 pair.
9. **Crash cause by spelling.** `S01/contracts_and_parameters.rs:1566-1579`, `S01/statements_and_assembly.rs:2246-2255`, `BTE/product.rs:722-731`.
10. **`Equatable` recognised by path string.** Two constants `EQUATABLE_TRAIT_PATH = "Equatable"`: `S02/preparation.rs:2895` and `S03/equality_and_exhaustiveness.rs:334` (checked by grep). Stage 02 synthesises the `equals` member; stage 03 validates and expands it.
11. **Integer and boolean constant folding.** `BTE/const_evaluation.rs:1268-1332` (`i64`/`bool` only), `BTE/const_generic_expressions.rs:5448-5538` (carriers and policies), `S02/preparation_const_evaluation.rs:784` (`i128` + symbolic), `:1676` (`ConstFactValue`), `:44` and `:147` (anonymous rationals, same four operators twice), `S02/symbols.rs:1052` (`zero_fold`), `S04/body_cloning_and_rewriting.rs:3988-4005` (`closed_bool`: Boolean, And, Equal only). Eight folders with different operator sets.
12. **A `where` fact of literal `0` or `false` makes a variant impossible.** `S02/preparation_generic_data.rs:2400-2414`, `S02/symbols.rs:1102`, `S03/equality_and_exhaustiveness.rs:1144-1158`.
13. **Zero gate of a data type.** Set in `S02/symbols.rs:978-1048`; cleared in `BTE/const_evaluation.rs:1794-1807` (checked) by looking at the first variant only.
14. **Target-machine family grouping.** `S02/preparation.rs:2186` (syntax; key includes package, module path, name), `S02/symbols.rs:4637` (resolved; same name, module, attached data, package), `S03/lowerer.rs:1184, 1362`, `S04/monomorphization.rs:3365`. "Implemented twice" is tested by target string at `S02/preparation.rs:2241` and again at `S02/symbols.rs:4680-4700`.
15. **Fixed-array length closure.** `S02/preparation_const_evaluation.rs:5492` and `S02/preparation_type_equations.rs:1614` (near-copy plus a lexical branch). Unit 042 "suspected" the same-named hits in other crates; my grep finds a second definition, `typed-trees/src/access_views_and_commitments.rs:2103`, so there are two functions of that name (bodies not compared).
16. **Structural equality of two expressions.** `S03/equality_and_exhaustiveness.rs:1726` and `typed-trees/src/values.rs:4006` (suspected by unit 044; my grep confirms the second definition exists; not read).
17. **`symbol_key`.** `S02/symbols.rs:2946`, `S03/lowerer.rs:1724`, `S03/type_reference.rs:1854`, plus six outside the area. My grep: 9 definitions in 8 files (unit 044 wrote 10 in 9).

**Build-time evaluation**

18. Find calls in an expression: `BTE/const_evaluation.rs:1038`, `BTE/const_generic_expressions.rs:699`, `BTE/const_evaluation.rs:2461`.
19. Premise-discharge switch: `BTE/const_evaluation.rs:4422-4435, 4488-4500`; `BTE/const_initializers.rs:370-384, 1038-1060`.
20. Pick the callee machine for a constant call: `BTE/const_evaluation.rs:5114-5119` takes the first match; `BTE/const_initializers.rs:359-362` errors on two ("conflicting exact entries").
21. Round with publish and restore on failure: `BTE/const_evaluation.rs:120-202` and `:4888-4971` (near copies).
22. Decode a machine result into an integer: `BTE/const_evaluation.rs:2836, 2906`; `BTE/const_initializers.rs:471`.
23. Receipt charge check: `BTE/evaluation_accounting.rs:160-184` and `:408-445`.
24. Policy request, floor, failure admission, evaluate, validate: `BTE/layouts.rs:1622-1650, 4883-4952, 5617-5659`.
25. Array lengths, then domain facts, then plan-laid layouts: `BTE/layouts.rs:3592-3607, 3738-3753`; `BTE/lib.rs:726-768`.
26. Executed Build event must match a declared request and an activation: `BTE/product.rs:80-95, 457-464, 594-621, 835-847, 2240-2265`. Unique toolchain enum, variant by symbol, no payload: `BTE/product.rs:645-669, 765-806, 3259-3307`.
27. Six near-identical `evaluate_const_evaluable_*` entries: `BTE/machine_execution.rs:4116-4386`.
28. Float bit identity in three places: `BTE/const_initializers.rs:2641, 2788`; `BTE/const_generic_expressions.rs:5364`.
29. Probe owner check and demand loop written twice: `BTE/syntax_probes.rs:128-147, 168-238`.

**Stage 01**

30. Contract clause loops, five copies: `S01/contracts_and_parameters.rs:704-1032, 1353-1561`; `S01/declarations.rs:114-161, 2030-2083, 2855-3028`. Seven hand-written clause-end keyword lists: `S01/contracts_and_parameters.rs:1034, 1181, 1325`; `S01/declarations.rs:163, 1143, 2110, 3030`.
31. Depth-counted bracket scans, five: `S01/tokens_and_storage.rs:195, 605-631`; `S01/bodies_and_transitions.rs:560-605, 858-890`; `S01/expressions.rs:2043-2079`.
32. Save and rewind speculation, nine ad hoc sites: `S01/expressions.rs:1305-1315, 1374-1451, 1462-1467, 1513-1518, 1931-1951, 1994-2040, 2148-2182, 2443-2463`; `S01/declarations.rs:2179-2183`.
33. Atomic member recognition, four sites (spelling group 2). Range parsing, four: `S01/expressions.rs:1092-1127`; `S01/contracts_and_parameters.rs:503-539, 595, 1273-1303`. Membership chain, two: `S01/expressions.rs:265-321`; `S01/contracts_and_parameters.rs:406-501`. Item-start recognition, four: `S01/declarations.rs:2147, 2189, 2260`; `S01/bodies_and_transitions.rs:136`. Place copying, two: `S01/statements_and_assembly.rs:517-535, 2388-2426`. Source loop, two: `S01/parser.rs:19-38, 179-198`.

**Stage 02**

34. Symbol assignment over an expression table, four complete walks: `S02/symbols.rs:336, 1284, 2575`; `S02/symbols_expressions.rs:927`.
35. `MachineScope` built by hand: `S02/symbols.rs:2337, 5413`; `S02/symbols_statements.rs:613`; `S02/symbols_top_level.rs:447`; `S02/symbols_type_references.rs:1240, 1271, 1305, 1341, 1381`; beside the constructor documented as shared (`S02/symbols.rs:3836-3859`).
36. Symbol table order written twice and kept equal by hand: `S02/symbols_symbol_table.rs:978-1152` (fresh) and `:864-976` (extension). Fresh and seeded assignment tails: `S02/symbols.rs:5329-5362, 5364-5400`.
37. `TypeParameterKind` to `SymbolKind` mapping, nine copies with differing arm sets: `S02/symbols_top_level.rs:69-73, 314-317, 758-761, 988-999, 1138-1143, 1316-1320, 1510-1515`; `S02/symbols_symbol_table.rs:393-400, 738-745`.
38. Trait parameter slot table: `S02/symbols_type_references.rs:71` and `S02/symbols_top_level.rs:1220`. The shared slot-walk tail (`S02/symbols_type_references.rs:571`) is bypassed at `:360-383`, `:535-558` and `S02/symbols_top_level.rs:615-640`.
39. Trait requirement closure walks: `S02/symbols.rs:3281, 3714`; `S02/selection.rs:1485, 3849`. A fifth in stage 04: `S04/body_cloning_and_rewriting.rs:4204`.
40. Named-implementation row by requirement name: `S02/selection.rs:840, 1082, 4146-4209`. Mint-or-reuse a ledger occurrence, four times: `S02/selection_authored.rs:452-470, 781-796, 939-957, 1649-1667`.
41. Module-qualified static call rewrite, three near-copies: `S02/symbols_targets.rs:926-995, 1003-1029, 1045-1077`. Static operand ladders, three: `S02/symbols_targets.rs:344, 809`; `S02/symbols.rs:1188`.
42. Receiver path walk: `S02/symbols_expression_paths.rs:485` and `S02/symbols.rs:4319`, with two near-identical kind lists (`S02/symbols_expression_paths.rs:362`; `S02/symbols.rs:4219`).
43. Domain-head match arm to membership test: `S02/symbols_statements.rs:403-442` and `S02/symbols_expressions.rs:1119-1231`.
44. Preparation: drop the leading type parameter of a domain telescope (`S02/preparation_generic_data.rs:711`; `S02/preparation_const_evaluation.rs:1153`); select a data declaration (`S02/preparation_generic_data.rs:1065`; `S02/preparation_const_evaluation.rs:4879`); three renderers of a closed type (`S02/preparation_generic_data.rs:282`; `S02/preparation_const_evaluation.rs:4806, 5009`); three "is this type closed" walks (`S02/preparation_machine_equations.rs:309, 1443`; `S02/preparation_generic_data.rs:475`); four "mentions a parameter" walks (`S02/preparation_type_equations.rs:1084, 2353, 2420`; `S02/preparation_generic_data.rs:2849`); three integer range checks (`S02/preparation_type_equations.rs:3153`; `S02/preparation_const_evaluation.rs:944, 6370`); candidate-spelling loop twice (`S02/preparation.rs:1170-1239, 1455-1502`).
45. Provisional drive, complete equations, repeat: `S02/resolution.rs:2099-2121` (loops), `:351-372` (loops), `:2479-2490` (one pass). `resolve_numeric_probe` (`:2297`) skips it.
46. Lowering: one 19-field signature mapping filled in three places (`S02/lowering.rs:337-355, 3236-3259, 3844-3869`); lexical scope save and restore two ways (`S02/lowering_machine.rs:1379-1426, 1709-1737`).

**Stage 03**

47. Type reference lowering, two parallel lowerers of the same ten shapes: `S03/type_reference.rs:1401-1593` (tree form) and `:2504-2713` (table form). Constraints likewise: `:1243, 1284`.
48. Method target for a receiver, three ways: `S03/call_and_product_selections.rs:337, 416`; `S03/expression_lowering.rs:2360`. Data definition by name, four: `S03/equality_and_exhaustiveness.rs:343, 1645`; `S03/statement_lowering.rs:849-869`; `S03/expression_lowering.rs:2288`. Declared type of a name, three: `S03/equality_and_exhaustiveness.rs:1873`; `S03/expression_lowering.rs:2611`; `S03/statement_lowering.rs:871-903`.
49. Domain instance name and interning, three: `S03/type_reference.rs:212-222`; `S03/contracts_and_signatures.rs:462-472`; `S03/call_and_product_selections.rs:2146-2152`.
50. Query, selection, else ordinary call: `S03/statement_lowering.rs:66-139` and `S03/expression_lowering.rs:1268-1342`. Static machine argument lowering twice: `S03/expression_lowering.rs:3773`; `S03/call_and_product_selections.rs:688`.
51. Recursion guard over the Equatable relation: `S03/equality_and_exhaustiveness.rs:612-649`; `S03/expression_lowering.rs:3049, 3466`.
52. Three entry paths copy the same evidence-forwarding loop: `S03/lowerer.rs:794-806, 865-877, 1652-1656`.
53. A 20-field call node literal written out five times: `S04/assembly_and_operators.rs:1918`; `S03/call_and_product_selections.rs:155-176, 1825-1846`; `S03/expression_lowering.rs:1451-1483, 3010-3039`.

**Stage 04**

54. Walk every call site of every state, 12 separate walks: `S04/conformance.rs:594, 1440-1474`; `S04/contextual_and_requirement_calls.rs:1151, 2646, 2710, 2942, 3344`; `S04/constructed_results.rs:594`; `S04/monomorphization.rs:799, 2816, 3952-3981, 4556`.
55. Reuse an existing instance or clone, then record correspondence, seven sites, each recomputing the ordinal: `S04/monomorphization.rs:1262-1275, 1397-1426, 1978-1993, 2570-2582, 3466-3506, 4842-4877`; `S04/contextual_and_requirement_calls.rs:1659-1707`.
56. All-bindings-present check, five: `S04/monomorphization.rs:1371-1374, 1874-1877, 1970-1975, 3731-3734`; `S04/contextual_and_requirement_calls.rs:1605-1609`.
57. Find a type row equal to a node, else insert, seven inline scans: `S04/lib.rs:117-120`; `S04/constructed_results.rs:214-218, 748-758, 922-929`; `S04/data_and_product_operands.rs:676-689, 720-733`; `S04/body_cloning_and_rewriting.rs:1462-1469`.
58. Lifetime pair walk, two in one file: `S04/conformance.rs:823, 969`. Binder kind to binding value with two different supported sets: `S04/contextual_and_requirement_calls.rs:284-312, 4098-4162`.
59. Two unrelated functions named `copy_expression`: `S04/body_cloning_and_rewriting.rs:143` (returns `Result`, copies contexts) and `:1695` (bare handle, does not). Statement and expression loops of `rewrite_cloned_calls`: `:2963-3109, 3116-3236`. Publish call views twice: `S04/contextual_and_requirement_calls.rs:3139, 3203`.
60. Scope split with its own `result_type`: `S04/constructed_results.rs:505-591` and `S04/contextual_and_requirement_calls.rs:1061-1084`.
61. `validate_primitive_meanings` runs twice on the same program: `S04/assembly_and_operators.rs:613` and `S04/preparation.rs:1946` (grepped by unit 044).

---

## Fixed lists of shapes

**Build-time evaluation**
- Scalar evaluator node kinds: Call, Match, Unary (two kinds), Cast, Binary, Member, literals; `_ => Err("unsupported node in exact scalar constant expression")` (`BTE/const_generic_expressions.rs:3579`) and "...exact integer constant expression" (`:4486-4490`).
- Initializer probe body: zero or more immutable captures, then exactly one unconditional value transition; else "initializer probe changed its unconditional value exit" (`BTE/const_evaluation.rs:4133-4167`).
- Fact machine: exactly one non-mutable by-value integer parameter and a `bool` return (`BTE/const_evaluation.rs:1476-1499`). Values are `i64` only (`:1141`).
- Array length machine: "must call a zero-argument machine (const arguments are not supported yet)" (`BTE/const_evaluation.rs:2829-2830`).
- `realize_data_instance` declines templates that are instances, quotients, have `where` facts, are zero-gated, have lifetime parameters or are machine-attached (`BTE/const_evaluation.rs:426-437`); member types `Constrained`, `ConstExpression`, `DynamicTrait` give `None` (`:608-610`).
- Syntax-level call detector sees Binary, Unary, Cast, Match, Call only; `_ => false` (`BTE/const_evaluation.rs:2461-2481`).
- Closed contract expression: Integer, Boolean, const Name, Call, Binary, Unary, Cast; "`_ => false`" (`BTE/const_evaluation.rs:3526`).
- Range endpoint types: "admit only range refinements and declared integer domains" (`BTE/const_evaluation.rs:4660-4665`).
- Materialisable carriers: constrained, literal-length array, named scalar, checked-shape struct or case, closed generic data; `_ => "has no materializable carrier"` (`BTE/const_initializers.rs:1443-1445, 1673`).
- Replay leaf pairing: scalar, copied aggregate, array literal, struct literal, structured leaf; else "retained initializer has no checked aggregate leaf correspondence" (`BTE/const_initializers.rs:3237-3252`).
- Bounded analysis constants: `MAX_EXACT_POINTS = 256`, `MAX_EXACT_POINT_PRODUCTS = 4096`, `MAX_ENUMERATED_LATTICE_POINTS = 256`, `MAX_LANDING_SUMMARIES = 4096` (`BTE/const_generic_expressions.rs:3209, 3210, 3333, 3953`). `PURE_PSI_WORK_BUDGET = 100_000` (`BTE/evaluation_accounting.rs:360`).
- Layout capacities: 64 plan entries (`BTE/layouts.rs:4331`), 32 access fields and 32 boundary reaches (`:1156-1157`), 32 schema fields, cases and retired identities (`:6592, 6714-6727, 6389`), schema ABI version literal `4` (`:5607`).
- Placement shapes: single `At`, single `IntegerAt`, repeated `At`, all `Bits`; otherwise "does not have one byte placement or a completely tiled bit-fragment placement" (`BTE/layouts.rs:5843-6011`). Records only: "programmable case/tag placement is not supported by this placement vocabulary" (`:5501`).
- Access view: "takes exactly a nominal access policy and a plain schema" (`BTE/layouts.rs:3967-3970`); schema "must be a nonempty, non-generic transparent record" (`:4168-4177`); atomic fields only `bool`/`u32`/`u64` (`:2589-2594`).
- Reflected primitive sizes: I8/U8/Bool, I16/U16, I32/U32/F32, I64/U64/F64 (`BTE/layouts.rs:6793-6801`); else "type has no fixed reflected storage shape" (`:7012`).
- Const-evaluable values reject Text, Reference, Slice, ConstExpression, DynamicTrait, non-literal arrays, `Atomic*` names, open aggregates, mixed shape "outside this ConstEvaluable stage", quotients (`BTE/machine_execution.rs:1845-1939, 2023-2025`).
- Float imports: ten operations only; others "selected operation has no primitive IEEE meaning" (`BTE/machine_execution.rs:4752-4789`).
- Product declarations: `supported()` admits ServiceAudit, AssumedBindingContract, WireCompatibilityRequest (`BTE/product.rs:395-402`); composition mode zero, one or error (`:3238-3256`).

**Stage 00**: escapes `\" \\ \n \r \t \0 \xHH` (`S00/lexer.rs:311-312`); whitespace `' ' \t \r \n` (`:594-596`).

**Stage 01**
- Nesting depth 1024 (`S01/tokens_and_storage.rs:415`), applied at four sites only (`S01/declarations.rs:1471`; `S01/expressions.rs:391, 406`; `S01/type_syntax_and_patterns.rs:602`); 256 MiB parser stack (`S01/parser.rs:103`).
- Integer suffixes: 12 in a table (`S01/tokens_and_storage.rs:327-340`); float suffixes `f32`, `f64` (`:342`).
- Arm pattern is a condition only for eight binary operators, Membership and LogicalNot (`S01/bodies_and_transitions.rs:418-438`); everything else is compared with `==`.
- Named transition target: call path of at most two segments (`S01/bodies_and_transitions.rs:1264-1268`).
- Boolean-coverage rewrite only for at most 8 subjects (`S01/bodies_and_transitions.rs:1079`, checked).
- Generic parameter admission by a 7-variant table (`S01/contracts_and_parameters.rs:2213-2272`).
- Nominal requirement path: at most one trait application (`S01/contracts_and_parameters.rs:1954-1969`).
- `Name<...>` is a type expression only before one of a fixed token list (`S01/expressions.rs:2003-2033`).
- Known asm mnemonics listed in the error text (`S01/statements_and_assembly.rs:1663-1668`); scratch type by indices `[15, 16, 19, 20]` (`:765`); x86 EFLAGS constants baked in (`:919-1004`).
- Destructure alias substitution only for one-segment paths (`S01/type_syntax_and_patterns.rs:414-416`).

**Stage 02**
- Const initializers: "v0 initializers are literals all the way down" (`S02/constant.rs:721-725`); else "initializer must be a literal ... in const-v0" (`:790`).
- Computed constant carriers: `_ => "computed constant requires a closed array, nominal literal, or builtin scalar carrier"` (`S02/constant.rs:2710`); scalar list `I8..U64, Bool, F32, F64` with no `Address` (`:2594-2599`).
- Canonical encoder depth 64 (`S02/constant.rs:3093`).
- Catalog families: AtomicFence, FloatProjection, FloatSemantics, RankingView (`S02/lowering.rs:2599-2614`).
- Items lowered: 13 arms; `Item::Capability(_) | Item::Package(_) => {}` (`S02/lowering.rs:2057`).
- Ranking: three or more subjects get `(NULL, "")` (`S02/lowering_machine.rs:1929-1939`); `render_ranked_subject` renders `Subtract` only, `_ => "value"` (`:1881-1904`); unsigned list `U8|U16|U32|U64` (`:2026-2029`).
- Hoisting rewrites Binary, Unary, Cast, Membership, Range, Indexed only (`S02/lowering_statement.rs:322-427`); member-of-indexed hoist "Restricted to `self.<field>` collections" (`:497-503`).
- Anonymous arithmetic: Integer, Float, and `+ - * /` only; "operator has no anonymous const meaning" (`S02/preparation_const_evaluation.rs:104, 378-393`).
- Const argument expressions: "expression is not a symbolic integer const expression" (`S02/preparation_const_evaluation.rs:929`).
- Lifetime-bearing argument canonicalisation: "This first exact cohort is deliberately positional ... Broader permutations remain on the unnormalized path." (`S02/preparation_generic_data.rs:131-140`).
- Generic data templates skipped: quotient, conformance bounds, Value or Machine parameters (`S02/preparation_generic_data.rs:3083, 3095-3102`).
- Type-equation matching: Reference, Slice, Named, FixedArray, Generic, Constrained with exactly one `Domain` constraint (`S02/preparation_type_equations.rs:1281-1433`); constructor parameters only `Type` and `Const` (`:343-353`); depth 128 (`S02/preparation_machine_equations.rs:1450-1452`).
- `expression_types` supports Name, Borrow, Member, Call, Match; `_ => None` (`S02/preparation_machine_equations.rs:435-540`).
- Root kinds enumerated twice: `RootWatermarks` has 12 (`S02/resolution.rs:1369-1382`); `bind_roots!` omits `wire_schemas` (`:1941-1953`) (inferred by unit 042).
- Requirement row match: accept `([one],[one],_)` or `(_,_,[one])`; all else error (`S02/selection.rs:1380-1419`).
- Domain subject traversal: 4096 steps, 64 ancestors (`S02/selection.rs:2106-2110`).
- Structural type role: only `Borrow` and one-element array literal (`S02/symbols.rs:5069-5075`); depth 128 (`:5062`).
- Target family property joins: "unimplemented: target-family generic multiplicity conjunction" and "... carry conjunction" (`S02/symbols.rs:4536, 4549`).
- Call target ladder by receiver kind: Local, field, Module, parameter, BuiltinType, Data, Domain, Machine, Trait or ConformanceParameter; anything else with a receiver is invalid (`S02/symbols_targets.rs:54-222`).
- Receiver paths: SavedSubject, Name, Member, Borrow, and Indexed only with an integer literal index; nested index "has no declaration form" (`S02/symbols_expression_paths.rs:480, 562-580`).

**Stage 03**
- Build selection: seven `(request, argument count)` shapes; else "Build selection has unexpected ordinary operands" (`S03/call_and_product_selections.rs:52-87`).
- Sealed view factory: exactly two type parameters, one state, one source parameter, two result arguments (`S03/lib.rs:107, 144, 148, 193`).
- Exhaustiveness: `MAX_MATRIX_CELLS = 65_536` (`S03/equality_and_exhaustiveness.rs:1176`); case paths of exactly two members (`:257-259, 1635`); a domain is countable only if its facts are one `Or` of memberships (`:1559-1563`).
- Structural `==` operands: struct literal, borrow of one, Name, Member, Indexed; else "bind the value to a local first" (`S03/expression_lowering.rs:3207-3209`).
- Hoist-temp type inference: Borrow, layout query, byte predicate, Call, Name/Member/Indexed, boolean Binary; else `None` (`S03/statement_lowering.rs:211-367`).
- Destination-needing nodes: `StructLiteral | String | ArrayLiteral | Match` (`S03/call_and_product_selections.rs:1444-1453`).
- Ranking default: slice or fixed array, or `U8|U16|U32|U64`; else none (`S03/lowerer.rs:225-241`, checked).

**Stage 04**
- Operator binding positions: `Binary | Indexed` only; else "...position whose body supply is not implemented..." (`S04/assembly_and_operators.rs:1536-1549`).
- Scratch assembly: at most `u64::BITS` slots (`S04/assembly_and_operators.rs:299-303`); "cyclic assembly label control ... this control domain is not yet implemented" (`:1368`); `EnclosingTransition` exits rejected (`:243-247`).
- `closed_bool`: Boolean, And, Equal of integer spellings (`S04/body_cloning_and_rewriting.rs:3993-4005`).
- Dynamic family provider: "must be generic over exactly the requirement's {arity} const/value binders" (`S04/monomorphization.rs:2378-2384`).
- Operator candidates: no machine or evidence parameters, integer const values only (`S04/monomorphization.rs:4433-4441, 4490`).
- Projected provider bindings: five exact shapes; else "provider projection changed its retained operand role" (`S04/monomorphization.rs:2044-2078`).
- Runtime operand realisation: two argument-count cases; else error (`S04/contextual_and_requirement_calls.rs:3714-3736`).
- Product operand expressions: `Membership` refused, "designated constant membership must be normalized by its source operation owner" (`S04/data_and_product_operands.rs:1957-1959`); 19 binary operators (`:2037-2058`).
- Four fixed-point loops with no iteration cap: `S04/monomorphization.rs:3751, 1522`; `S04/contextual_and_requirement_calls.rs:3345, 4046`.

---

## Accepts when it does not know

The three classes are the readers'. Not merged.

### Classed "open" (no reason at the site, not traced)

**Build-time evaluation**
- `BTE/evaluation_accounting.rs:499-501, 525-527`: a non-recording `EvaluationAccounting` builds and validates no receipt, so evaluation is unmetered. Whether a production caller uses the default was not traced.
- `BTE/const_evaluation.rs:307-309` (checked): a `Named` type argument with an invalid symbol counts as closed. Reader: "A wrong type name with no resolved symbol would also count as closed."
- `BTE/const_evaluation.rs:1794-1807` (checked): the zero gate is cleared after looking at the first variant's `where` facts only. A data type whose later variant has facts is judged by the first. What the gate guards was not traced.
- `BTE/const_evaluation.rs:4933, 5019-5034`: a failed range-endpoint round or root with no pending calls is left unfolded with no diagnostic. Which later stage rejects it was not traced.
- `BTE/const_generic_expressions.rs:1814-1821`: operator selections in callee bodies are not recorded in custody. `:1837-1866`: selection kinds other than Call, Operator or a resolved Const are ignored. `:1349-1361`: a `TransitionNamed` call inside a reflection body held by the same machine returns `Ok(())` with no custody check.
- `BTE/const_generic_expressions.rs:2689-2695, 5680-5686`: in the `Closed` context, `&& || & | ^ << >>` are evaluated with builtin meaning with no operator check. Whether an overload can exist on builtin scalar operands was not traced.
- `BTE/const_initializers.rs:940-942`: any expression passes the carrier check when the destination is primitive. Not every path traced.
- `BTE/const_initializers.rs:2188-2191, 2238-2240`: replay validation skips declarations without calls or float components (inferred: these cannot hold call receipts).
- `BTE/const_initializers.rs:3288, 3323`: aggregates with a pending target observation are not handled by `target_observations`; where they are handled was not traced.
- `BTE/syntax_probes.rs:652-661`: top-level bindings and the retained base are dropped when there is no source map.
- `BTE/product.rs:242-253, 326-328` (checked): a captured native binding whose requirement source file is not loaded is skipped silently. `:2458`: an unmatched non-declaration span becomes a default span. `:3140-3142`: an unresolved call not named `select_provider` is ignored. `:1789-1790`: a selection with no target applies to every scope.
- `BTE/machine_execution.rs:455`: a call target with neither a machine-parameter nor a trait signature gets no `requires`, linearity or termination check. `:262`: no termination violation when the machine is not found. `:3410-3412`: a symbol with no machine drops out of the service, suspend and block summary.
- `BTE/layouts.rs:3955, 4011, 4023`: with no source map, "any user type named `Access` with two nominal arguments is treated as the access template". Which callers pass no source map was not traced.
- `BTE/layouts.rs:5338-5348`: open policies are skipped and never evaluated.

**Stage 00**
- `S00/lexer.rs:63-69, 116`: malformed numbers (`0x`, `1e`) lex successfully and carry flags. The reader did not have the consumer in the unit. I checked: the flags are read and rejected in stage 01 at `S01/tokens_and_storage.rs:250` ("invalid integer literal") and `:290` ("invalid float literal"), and nowhere else.

**Stage 01**
- `S01/expressions.rs:668-669, 737` (checked): `move x` parses to `x`; no node records the keyword. Wrong program per the reader: "one whose author relied on `move` to mark a transfer". A test pins the erasure (`S01/tests.rs:4029-4034`).
- `S01/bodies_and_transitions.rs:1068-1107` (checked): when Boolean-literal arm tuples cover all 2^n combinations, the last arm's guard becomes `Always`. Wrong program per the reader: one whose tuples cover all cases "only syntactically (for example subjects that are not Boolean-typed)". What stage 05 does with the unguarded arm was not traced.
- `S01/contracts_and_parameters.rs:743-757`: a repeated `terminates by` overwrites the earlier one; the last witness wins.
- `S01/declarations.rs:51-55, 760-764, 806-810`: `;` after data fields, case members and capability fields is optional, so `a: u8 b: u8` parses. `S01/expressions.rs:2081-2124`: `T { a: 1 b: 2 }` parses.
- `S01/declarations.rs:1205`: a domain on a reference or generic carrier with no type parameters is named `?::Name`.
- `S01/statements_and_assembly.rs:1900-2107`: the bracketed-operand rejection is skipped for port, MSR, control-register, system-register, flags, fence and GDT shapes; wrong program per the reader: "`out [x], y` style bracketed operands".
- `S01/tokens_and_storage.rs:117-125`: the span builder never checks that pushed handles are adjacent (callers read were adjacent; inferred).
- `S01/type_syntax_and_patterns.rs:201-210`: a destructured binding used inside a call's `machine_arguments`, `evidence_arguments` or `target` is not rewritten. `:145`: the original capture is kept when no copy matches.

**Stage 02**
- `S02/constant.rs:94-179`: non-builtin or non-numeric cases return `Ok(())` in the carrier re-landing; example given: "a `bool` const with an integer literal". `:373-456`: scalar or alias destinations skip the nominal check. `:519-564`: an alias of `f32` skips the scalar check. `:690-719`: a path with an invalid head symbol falls to a name lookup and becomes the const.
- `S02/lowering.rs:2057`: `Capability` and `Package` items produce nothing. `:578-587`: with no sources, two same-named domains from different packages share one identity string (inferred reach: tests and probes). `:1144`, `:1119-1121`: not traced further.
- `S02/lowering_machine.rs:1859`: a rank range that is not a range expression becomes the default range silently.
- `S02/lowering_statement.rs:401, 426`: indexed reads inside call arguments, struct or array literals, match arms and member receivers are not hoisted; the reader calls this "a possible gap" in the header's stated contract, not a soundness accept.
- `S02/lowering_statement.rs:1113-1140, 1537-1541` (checked): an assignment `a = b` of two bare names where either spelling is an evidence term becomes a forwarding row and the statement is dropped. Wrong program per the reader: "`x = evidence` where only `evidence` is an evidence term but `x` is an ordinary local would lose its assignment".
- `S02/preparation.rs:1267-1278` (checked): a resolver error for a constructor prefix counts as "is value". `:1008, 1012, 1091`: an ambiguous generic head yields no roles, so a nested alias is not expanded. `:5193`: Domain, Operator, Proposition, Measure items skip alias expansion. `:2836-2841`: a required trait missing from the map is skipped, so its defaults are not synthesised. `:2209-2220`: with no sources, target families merge across packages.
- `S02/preparation_const_evaluation.rs:438-733`: `prepare_closed_application` returns `Ok(None)` for many shapes, "skip, never reject" (`:429-437`); example: a generic application whose const argument is a call. `:1383-1385, 1504-1508`: domain index arguments left unrewritten. `:5373-5377`: admission modes `PendingDestination` and `CheckedDestination` skip predicate proofs; their callers are outside the file.
- `S02/preparation_generic_data.rs:699`: an unsupported argument shape means "not closed" and the application is left alone.
- `S02/preparation_generic_uses.rs:245, 280`: call signatures are keyed by machine name text across all modules. `:1126-1141`: a non-generic data type with the same name as a generic one has its attached machines excluded. A missed relabel leaves the template spelling for the ordinary resolver; whether that rejects was not traced.
- `S02/preparation_type_equations.rs:1974-1989`: `validate_materialized_type_equations` returns `Ok(())` when the base cannot be selected.
- `S02/resolution.rs:1921-1929`: a valid owner skips the root-range binding with no check that the owner exists in the trees.
- `S02/selection.rs:914-938`: a hidden name in a trait default body passes silently if it matches any top-level Machine, Operator or BuiltinFunction. `:2347-2350`: an `ensures x in D` authorises a route when `x` merely spells like an out parameter. `:3067-3084`: an evidence forwarding with no owner row keeps invalid symbols; no alias match gives `None`. `:3197-3199`: a non-`Named` argument for a requirement-identity parameter is skipped. `:3201-3212`: a visible State that is not a trait requirement is accepted, which the reader notes contradicts the comment "concrete machines and ordinary types fail closed" (`:3171-3176`) unless a later stage rejects.
- `S02/selection_authored.rs:371-373, 1244-1246`: an unresolved call spelled `code_layout` is excluded from selection.
- `S02/symbols.rs:4003-4005`: when two attachments of one owner share an entry name and both are visible, the first wins with no ambiguity error.
- `S02/symbols_expressions.rs:314-319, 554`: an ambiguity error is dropped here and re-raised later only for expressions with authored exposure (traced for those; open for the rest).
- `S02/symbols_statements.rs:257-260`: a root-binding operand that is not a local, parameter or field is set to invalid with no report.
- `S02/symbols_symbol_table.rs:1097, 1124, 1142, 1148`: if the child stream is shorter than the roster, a declaration silently gets an invalid symbol. `S02/symbols_top_level.rs:1350-1358`: a kind mismatch returns an invalid handle silently, and nothing checks the stream was consumed.

**Stage 03**
- `S03/equality_and_exhaustiveness.rs:932-934` (checked): the exhaustiveness check returns `Ok(())` for any run containing a transition with a continuation. What gets through: "a transition run with a `continuation` target and no `_` arm".
- `S03/equality_and_exhaustiveness.rs:955-967, 1801-1817` (checked): an `x == y` / `x != y` pair closes a run when the two sides compare equal structurally; that compare matches calls by target name string and ignores a cast's target type. So "two calls with equal spelled names but different resolved targets, or casts to different types, would be treated as the same expression".
- `S03/equality_and_exhaustiveness.rs:257-259, 297`: a three-segment case path is not checked (inferred). `:2160-2162`: a Machine binder with an evidence projection argument is accepted with no further check.
- `S03/lowerer.rs:1883-1895` (checked): `validate_computed_arguments` is skipped when the cohort's generic origins, operators and data definitions all equal the source counts; "a computed-argument mismatch introduced in new machine bodies would be unchecked at this point".
- `S03/lowerer.rs:322-324`: a `requires x in D` whose domain symbol is still invalid becomes no progress premise; premises are extracted at `:1854`, before membership selections settle at `:1870` (order checked). `:363-371`: subject matched by name string when symbols differ.
- `S03/call_and_product_selections.rs:2128-2136`: a cast `x as T in D` with zero or several candidate domains gets an invalid symbol and no error here. `:1379-1381, 1389-1391, 1495-1501`: no destination prepared.
- `S03/contracts_and_signatures.rs:154-198`: a membership that cannot be matched to exactly one domain keeps an invalid domain symbol. `:43-46`: unresolved invocations kept.
- `S03/declarations.rs:1550` (checked): any machine whose name ends with `::provider_defaults` gets the provider-defaults purpose; no origin check. `:1165-1169`, `:683-702`: left invalid.
- `S03/expression_lowering.rs:2918-2921`: `f() == g()` over a data type with no derivable type name is lowered as a plain `==`. `:2880-2902, 3079-3095`: any name with two or more path members skips structural expansion. `:1404-1415`: unsure whether `symbols.get(invalid)` panics.
- `S03/statement_lowering.rs:1391-1396`: `let x = <unhandled expression>` is typed `Unit` here.
- `S03/type_reference.rs:180-183`: an unresolved or ambiguous domain constraint stays as authored. `:832-857`: const arguments of applications with mismatched arity or duplicate headers are not closed or checked.

**Stage 04**
- `S04/body_cloning_and_rewriting.rs:744-746` (checked): a `requires` type predicate blocks a clone only when it evaluates to `Some(false)`; unknown is accepted.
- `S04/body_cloning_and_rewriting.rs:4796-4819`: the scan for runtime-bound value parameters in types visits only Cast, Call, TypeExpression and ZeroValue payloads; a struct literal's `application_type` is not scanned. `:3711-3714, 3784`: struct or array const arguments without a carrier are left unfolded. `:5551-5553`: binding count against operand count is not checked.
- `S04/monomorphization.rs:2667-2669` (checked): if dynamic slot demand collection errors, the pass returns `Ok(())` and does nothing.
- `S04/monomorphization.rs:3885-3903, 4806-4811` (checked): the "incomplete concrete selection" error covers statement call sites only, and partial-evidence detection leaves out type bindings. A non-generic caller's incomplete selection with only type bindings at an expression call is left unspecialised with no diagnostic from this function.
- `S04/contextual_and_requirement_calls.rs:1436-1651`: `providers::close` returns `Ok(None)` at six points and one caller just continues (`:1898-1917`). `:2614-2619`: zero or several matching closed applications give "no retarget". `:2127-2129, 3185-3187, 3254-3256`: a call whose arguments cannot be completed gets no applied view. `:2905-2907`, `:136-147`, `:491-497`, `:422-435`: skipped.
- `S04/conformance.rs:253-258, 338-340`: calls skipped. `:444-446, 534`: an elided selection whose inferred lifetimes disagree is skipped with no diagnostic. `:850-854, 863-865`: bindings not visited.
- `S04/constructed_results.rs:844-848`: a type that cannot be derived is left invalid. `:65-67`, `:130-141`, `:297-299`, `:965-969`.
- `S04/data_and_product_operands.rs:341-345`: an application with a count mismatch is not instantiated. `:826-870`: an invalid handle is stored as a field type with no diagnostic. `:183-187`. `:1496`: a lifetime name that should have been substituted passes through as written.
- `S04/lib.rs:122`: a receiving type that cannot be derived is stored invalid. `:195-197`: a rebound parameter with no formal is skipped.
- `S04/assembly_and_operators.rs:1503-1512` (checked): uses of a bodiless top-level-requirement operator with a selected float comparison are skipped. `:791-796`: a ranking witness that cannot be mapped by name stays on the owner. `:307-313`: unsure, index without an emptiness check (a panic, not an accept). `:2166-2173`: unsure, inclusive range becomes `end + 1` with no overflow guard seen.

### Classed "stated" (a comment or the structure at the site gives the reason)

- `BTE/machine_execution.rs:2507, 2548-2550, 2596-2597, 2836` and `BTE/layouts.rs:3842-3844, 4259-4261`: with no selection authority, all package checks are skipped ("stated by structure only").
- `BTE/machine_execution.rs:4010-4031`: `evaluate_machine` does not check the result is const-evaluable ("stated by name only").
- `BTE/layouts.rs:5634-5638` (open policy with unavailable static input continues); `:5750-5752` (identities in the `open` list need no plan); `:4384, 4425` (entries beyond `entry_count` ignored, by the fixed-capacity convention).
- `S01/contracts_and_parameters.rs:1650`: `reaches` with no members "stays accepted until the retirement sweep" (`:1637-1640`).
- `S02/constant.rs:1793`: a call to a non-machine target is dropped in a body-demand walk; "not evidence of checked call selection" (`:1268-1273`).
- `S02/lowering.rs:737`: obligations at recorded offsets are "left with their synthesis owner" (`:723-727`).
- `S02/lowering_machine.rs:307-312, 409`: "source-free trees cannot prove custody and are left to the remaining rules" (`:304-305`). `:342, 406`: top-level requirement machines skip the home rules. `:170-181`: operator-versus-operator duplicates are left to the typed stage (`:95-98`).
- `S02/lowering_statement.rs:451-460`: an unresolved same-spelled embedding call is accepted; a later gate "rejects ... any authored same-spelled replacement" (`:446-450`).
- `S02/preparation.rs:1514-1523`: "callers must defer it to ordinary validation, never fall back to a lexical constant map" (`:1504-1507`). `:2548-2564`: "an ambiguous authored trait name defers to ordinary resolution instead of guessing an owner". `:1982-1983`: non-Complete mode skips module constant validation.
- `S02/preparation_const_evaluation.rs:207-209`: `&& || & | << >>` never consult an authored overload in this pass ("stated by structure only; not traced further"). `:2186-2201`: "Machine-call facts stay on the concrete record".
- `S02/preparation_generic_data.rs:550-566`: an unresolved one-segment name is accepted as a `Constant` identity; "no unresolved name is an atom" (`:402-404`; the gate was not traced). `:2386-2391`: undecided variant facts keep the case possible.
- `S02/preparation_machine_equations.rs:1286`: a complete tuple forwarding open type binders "stays with the later type-predicate check" (`:1248-1260`). `:423-428`: missing type evidence does not reject.
- `S02/preparation_type_equations.rs:2514-2525`: "only patterns mentioning an omitted binder contribute inference".
- `S02/resolution.rs:1553`: with no source map every declaration is visible; "those checks stay permissive for it" (`:2017-2019`). `:2381-2383`.
- `S02/selection.rs:821-838`: an unmatched named-implementation call gets an invalid target "so checking rejects it" (`:774-778`). `:951-953`.
- `S02/selection_authored.rs:1220-1233`: an unresolved call name in a contract clause gets no ledger occurrence (`:1061-1067`).
- `S02/symbols.rs:1629-1636, 2199`: a contested membership "is retained as a checked obligation". `:1004`: an unfoldable `where` expression makes the data zero-gated ("conservative declaration gate").
- `S02/symbols_targets.rs:868-870`, `:437-439`, `:688-689`; `S02/symbols_type_references.rs:1518-1520`.
- `S03/expression_lowering.rs:2112-2191`: implicit fill of an omitted erased field (`:2141-2144`).
- `S04/data_and_product_operands.rs:1033-1065`; `S04/conformance.rs:133-135`; `S04/contextual_and_requirement_calls.rs:734-742, 766-772` (by structure); `S04/body_cloning_and_rewriting.rs:187-200` (by a test, `:365`), `:2572-2574, 2610` (by shape).

### Classed "traced" (the reader followed callers)

- `BTE/const_generic_expressions.rs:6684-6691`: origins are compared only when expected origins are supplied; data-argument probes get none (`:5952-5955`), so for those "no comparison of selected constants against the lexical selection is made". Traced from `:5775-5820`.
- `BTE/evaluation_accounting.rs:302` (a failed evaluation passes `finish` but carries no value); `:621-628` (not fail-open for `Returned`). `BTE/const_evaluation.rs:2989-2997` (const stays unqualified). `BTE/lib.rs:399` (restores an already authority-less state). `BTE/layouts.rs:855, 1041` (authority applied at the call), `:5433-5435` (closed policies error later at `:6160-6172`). `BTE/scalar_applications.rs:298-301` (traced through a test only). `BTE/machine_execution.rs:2723-2735` (traced to a validation function not read).
- `S01/contracts_and_parameters.rs:851-865` (both `crashes` orders closed); `S01/statements_and_assembly.rs:111` (unreachable for named atomic members).
- `S02/lowering.rs:1997-2004`: an evaluation error for a non-public const becomes no value; uses that need the value fail later, plain substitution proceeds ("traced (partial)").
- `S02/lowering_machine.rs:385` (covered by the primitive-family rule unless toolchain or source-free). `S02/preparation_generic_data.rs:3577-3619` (undecided or false facts are copied as obligations). `S02/selection.rs:3484, 3499, 3718` (empty reach row unreachable for known symbols). `S02/symbols.rs:3589-3591`. `S02/symbols_top_level.rs:530-533` (not followed further).
- `S03/equality_and_exhaustiveness.rs:943` (a const literally `true` is a catch-all); `S03/declarations.rs:1765-1781`; `S03/call_and_product_selections.rs:1609-1630, 1899-1913`; `S03/expression_lowering.rs:1708-1719`; `S03/lowerer.rs:251-255`; `S03/statement_lowering.rs:1039-1045`; `S03/type_reference.rs:1741-1757, 2878-2883`; `S04/assembly_and_operators.rs:117-119, 1519-1520`.
- `S04/data_and_product_operands.rs:1372-1416` (falls through to a lookup that errors); `S04/conformance.rs:1428-1431`; `S04/contextual_and_requirement_calls.rs:4003-4005`.

---

## Silent drops

Places where failing to plan or resolve produces no row and no diagnostic. My count of distinct sites from the records: build-time evaluation 13, stage 00 none, stage 01 2, stage 02 31, stage 03 17, stage 04 36.

- **Build-time evaluation.** Captured native binding not loaded (`BTE/product.rs:242-253`). Call not named `select_provider` (`BTE/product.rs:3140-3142`). Unfoldable range roots (`BTE/const_evaluation.rs:4933, 5019-5034`). Open layout policies never evaluated (`BTE/layouts.rs:5338-5348, 5634-5638`). Custody ignores some selection kinds (`BTE/const_generic_expressions.rs:1814-1821, 1837-1866`). Replay skip (`BTE/const_initializers.rs:2188-2191`). Pending aggregates (`BTE/const_initializers.rs:3288`). Missing machine in the effect summary (`BTE/machine_execution.rs:3410-3412`). Trailing plan and access entries ignored (`BTE/layouts.rs:4425, 1242`).
- **Stage 01.** `move` (`S01/expressions.rs:737`). Only the first parse error per source, and per page in parallel mode, is reported (`S01/parser.rs:156, 192`); an error with a default span prints the position of offset 0 (`S01/parser.rs:200-215`).
- **Stage 02.** `Capability` and `Package` items (`S02/lowering.rs:2057`). Swallowed const evaluation error (`S02/lowering.rs:1997-2004`). Assignment turned into a forwarding (`S02/lowering_statement.rs:1139`). Indexed reads not hoisted (`S02/lowering_statement.rs:401`). Default rank range (`S02/lowering_machine.rs:1859`). Trait defaults not synthesised (`S02/preparation.rs:2548, 2562, 2836`). Alias expansion skipped (`S02/preparation.rs:1008, 5193`). Closed application left untouched (`S02/preparation_const_evaluation.rs:438, 1383, 1504`; `S02/preparation_generic_data.rs:699`). Literal not relabelled (`S02/preparation_generic_uses.rs:800-802`). Equation check skipped (`S02/preparation_type_equations.rs:1979`). Forwarding owner or conformance not found (`S02/selection.rs:3067, 3081`). Non-`Named` requirement argument (`S02/selection.rs:3197`). Pending machine bodies with no new ordinal are dropped (`S02/selection.rs:4648-4660`). Unbound contract call and zero-length spans get no ledger row (`S02/selection_authored.rs:1220, 1409`). Ambiguity errors dropped (`S02/symbols_expressions.rs:314, 554`). Root-binding operand dropped (`S02/symbols_statements.rs:257`). Positional stream mismatches (`S02/symbols_symbol_table.rs:1097, 1124`; `S02/symbols_top_level.rs:1350`). Token-family conformance symbol cleared (`S02/symbols_top_level.rs:530`). Type value expressions not resolved (`S02/symbols_type_references.rs:1518, 626`).
- **Stage 03.** Destination not prepared (`S03/call_and_product_selections.rs:1379, 1389, 1495`). Cast domain unresolved (`:2128`). Membership domain unresolved (`S03/contracts_and_signatures.rs:154`); invocation unresolved (`:43`). Satisfied declaration unresolved (`S03/declarations.rs:1165`); home domain (`:683`). Exhaustiveness skipped (`S03/equality_and_exhaustiveness.rs:932`); equality operand unchecked (`:257`). Structural equality not expanded (`S03/expression_lowering.rs:2918`). Progress premise dropped (`S03/lowerer.rs:322`); computed arguments not validated (`:1883`); no inherited guarantee (`:251`). Local typed `Unit` (`S03/statement_lowering.rs:1391`). Domain constraint kept as authored (`S03/type_reference.rs:180`); const argument not closed (`:855`).
- **Stage 04.** Unit 045, "Five things" item 4: "Most 'cannot do this' outcomes return `None`/`Ok(None)`/`continue` and leave the program as authored with no diagnostic". Sites: `S04/contextual_and_requirement_calls.rs:136, 422, 491, 1436, 2127, 2614, 2905, 4003`; `S04/conformance.rs:253, 338, 444, 534, 850`; `S04/constructed_results.rs:65, 139, 297, 844`; `S04/data_and_product_operands.rs:183, 341, 826, 1496`; `S04/body_cloning_and_rewriting.rs:187, 3711, 4709, 5551`; `S04/monomorphization.rs:1199-1215, 1389-1392, 1562-1575, 1748-1763, 2667, 3893`; `S04/lib.rs:122, 195`; `S04/assembly_and_operators.rs:791, 1503, 1519`.

---

## Unused or unwired

With the readers' grep counts; "(checked)" means I re-ran the grep over `omega-rust/**/*.rs`.

- `evaluate_const_domain_facts` (`BTE/const_evaluation.rs:1691`), `evaluate_const_array_lengths` (`:2526`), `evaluate_zero_argument_machine` (`:2875`), `evaluate_const_range_endpoints` (`:4777`): definition and re-export only (`BTE/lib.rs:894, 897, 902`); 0 call sites. (checked: 7 lines in total, 4 definitions and 3 re-export lines.)
- `compute_native_layout_plan_for_machine` (`BTE/layouts.rs:4864`): 2 matches, definition and re-export (`BTE/lib.rs:906`). (checked)
- `endpoint_leaves` (`BTE/const_evaluation.rs:2081`) is never filled, so the endpoint block at `:2408-2457` never runs its body (inferred by unit 038). (checked: eight uses, none adds an element.)
- `_statement_index` threaded but unused (`BTE/const_initializers.rs:921, 935, 979, 1017`). Empty `impl EndpointCallee {}` (`BTE/const_evaluation.rs:4818`). `let _ = root;` after computing it (`BTE/product.rs:83-90`).
- `LayoutQueryProvider` and `LayoutQueryMeasurements`: one production implementor each, outside the area, plus a test implementor.
- `Lexer::tokenize_reusing` (`S00/lexer.rs:382`): 3 matches, 1 outside caller in `psi-compiler/src/lib.rs` (not read; suspected a second tokenizing entry that bypasses `lex_sources`).
- `parse_sources` (`S01/parser.rs:19`) and `parse_source_units` (`:97`): called only from `S01/tests.rs`. (checked) `ParsedSyntaxPages::syntax_for` and `ParsedSyntaxSource::syntax_handle`: used only at `S01/tests.rs:7499, 7500, 7574`. `parse_if_transition_statement_handle` always returns an error (`S01/statements_and_assembly.rs:507-515`).
- `select_data_declarations` (`S02/preparation.rs:1747`): 0 call sites; definition, one doc mention, re-export at `S02/lib.rs:92`. (checked)
- `resolve_numeric_probe` (`S02/resolution.rs:2297`): only caller is a test (`:2731`); re-exported at `S02/lib.rs:108`. (checked)
- `ANY_DECLARATION_LOOKUP_KINDS` (`S02/selection.rs:4538`): 0 external uses, 1 internal. `signature_free_machine_candidates`, `signature_free_trait_candidates`: 0 external uses. `normalize_service_reaches_with_retained_tables`: one caller (`S02/selection.rs:4675`).
- Unused parameters: `_visiting` (`S02/preparation_const_evaluation.rs:5353`); `_domain_name` (`S02/symbols.rs:1727`). `MachineParameterArm` has one variant (`S02/symbols_top_level.rs:1487-1489`).
- An orphan comment describing a "default-domain fact fold" sits above `lower_type_parameters` with no such code in the file (`S02/lowering.rs:285-289`).
- `pre_resolution` is documented as "the one seam" for build-time evaluation (`S02/lib.rs:44`); its outside users were not grepped by the reader.
- `type_resolved_cohort` and `close_typed_cohort` are called "two shared cohort operations" (`S03/lib.rs:9-10`): 0 uses outside `S03/lowerer.rs`, where three entry paths call them. `table_type_base_name` (`S03/equality_and_exhaustiveness.rs:724`): no outside caller. `byte_slice_text_view` (`:651`): 2 uses, both in the file.
- `let complete = true;` (`S04/body_cloning_and_rewriting.rs:735, 755`) (checked), so `structural_type_equations_pending = !predicates_closed` (`:992`) is always `false` (read by unit 045; `:992` not opened by me).

---

## Stated limits

Quoted from the records, under 20 words each.

**Build-time evaluation**
- "(const arguments are not supported yet)" `BTE/const_evaluation.rs:2829-2830`.
- "static application of ... awaits a complete validated specialization" `BTE/const_evaluation.rs:4838-4849`.
- "structural constant Match requires saved-subject value interpretation" `BTE/const_generic_expressions.rs:4577-4581`.
- "address constant evaluation requires target authority" `BTE/const_generic_expressions.rs:5605-5607`.
- "without an exact raw-NaN realization" `BTE/const_initializers.rs:1392-1395`.
- "pre-check semantic evaluation has no checked invocation proof for that premise" `BTE/machine_execution.rs:391, 399, 461`.
- "has no ordinary checked `Terminates` guarantee" `BTE/machine_execution.rs:338`.
- "symbolic application coverage is not part of this execution" `BTE/machine_execution.rs:5832, 5913`.
- "have no build-time execution support" (crash contracts) `BTE/machine_execution.rs:5739`.
- "has no host computation scheduler" `BTE/machine_execution.rs:4996`.
- "not supported by this placement vocabulary" `BTE/layouts.rs:5501`.
- "the current access vocabulary admits only Inaccessible for aggregate fields" `BTE/layouts.rs:1404`.
- "exceeds bootstrap capacity" `BTE/layouts.rs:1220, 1505`; "the current layout slice supports at most" 32 fields `:6717`.
- "is not an admitted declaration operation" `BTE/product.rs:551`; "unsupported inherited declaring-trait lifetime application" `:1002`.
- "requires exact authored selection before evaluation" `BTE/scalar_applications.rs:101`.

**Stage 00**: test comment "incomplete raw candidates are not reserved V1 syntax" `S00/lexer.rs:788`.

**Stage 01**
- "raw byte string literal requires the terminal byte-sequence lowering path" `S01/tokens_and_storage.rs:716`.
- "bare `->` transitions are not supported" `S01/bodies_and_transitions.rs:259`.
- "one-off `requires machine T::member(...)` requirements are unsupported" `S01/contracts_and_parameters.rs:1775`.
- "`reaches none` ... stays accepted until the retirement sweep" `S01/contracts_and_parameters.rs:1637-1640`.
- "`public` is not yet retained for this declaration kind" `S01/declarations.rs:2289`.
- "the initial quotient surface excludes custody" `S01/declarations.rs:439`; "does not yet retain lifetime arguments" `:457`.
- "scratch-register CFG flags cannot yet be combined with hardware popfq restoration" `S01/statements_and_assembly.rs:1254`.
- "register aliases require shared-bit state modeling" `S01/statements_and_assembly.rs:1235, 1241`.
- "no structured operand provenance/permission contract is modeled" `S01/statements_and_assembly.rs:1683`.
- Retired forms kept as dedicated error branches: `spawn`, `trap`, `if`, `relax`, `effects`, `decreases`, `increases`, `abi`, `library`, `host`, `platform`, `provides`, `target`, `invariant`, `where`, `self` receivers, `Join<T>`, `[send]`, `[zero_init]`, `&relaxed` (unit 040 "Five things" item 5; `S01/type_syntax_and_patterns.rs:767, 970, 1053, 1066`).

**Stage 02**
- "Calls and floating computations still need evaluation." `S02/constant.rs:48`.
- "richer const expressions arrive with build-time evaluation" `S02/constant.rs:792`.
- "dynamic indexing, borrowing and slicing require value-based array projection" `S02/constant.rs:5261`.
- "late native code-layout metadata is unavailable to same-product semantic constant, type, layout, or optimization evaluation" `S02/constant.rs:2073, 2372`.
- "Generic codec selection remains a separate consumer requirement." `S02/lowering.rs:4659`.
- "remaining single-subject short forms keep an empty `view_path` until TPR3" `S02/lowering_machine.rs:1809-1810`.
- "`operator` repeating another `operator` still rejects at the typed stage" `S02/lowering_machine.rs:97-98`.
- "transparent alias RHS expression cannot yet preserve explicitly supplied declaration custody" `S02/preparation.rs:3844`.
- "computed constant carrier member is not yet a closed structural type" `S02/preparation.rs:1672`.
- "quotient data ... not eligible as a structural const index until quotient-backed canonical representatives land" `S02/preparation_const_evaluation.rs:5648`.
- "unit const values do not yet have a source literal" `S02/preparation_const_evaluation.rs:5055`.
- "A symbolic const expression cannot cross that boundary yet" `S02/preparation_const_evaluation.rs:4049-4056`.
- "constrained const declarations require declaration-site proof checking before they can publish compatibility identity" `S02/preparation_const_evaluation.rs:3272`.
- "Unsupported or open shapes remain on the ordinary generic path." `S02/preparation_generic_data.rs:279`.
- "Computed, chained, and dynamic receiver selection remains resolver-owned and fails closed here." `S02/preparation_generic_uses.rs:236-238`.
- "machine structural equations currently require explicit calls" `S02/preparation_machine_equations.rs:1054`.
- "requires anonymous reference lifetimes until lexical lifetime identity is retained" `S02/preparation_type_equations.rs:912`.
- "ClosedArgumentIdentity currently omits application lifetimes" `S02/preparation_type_equations.rs:894-897`.
- "Domain applications have no closed argument identity yet" `S02/preparation_type_equations.rs:983-986`.
- "unimplemented: target-family generic multiplicity conjunction" `S02/symbols.rs:4536`; "... carry conjunction" `:4549`.
- "This discovers a route, not a grant: checking still establishes each exact carrier" `S02/selection.rs:2242-2251`.
- "A span-only lookup cannot issue custody" `S02/selection_authored.rs:669-670`.
- "A qualified machine does not inherit its carrier's fields." `S02/symbols_symbol_table.rs:15-16`.

**Stage 03**
- "cannot yet retain transparent alias application obligations" `S03/call_and_product_selections.rs:830-832`.
- "recursive Equatable implementations are not supported yet" `S03/equality_and_exhaustiveness.rs:639`; same text `S03/expression_lowering.rs:3468`.
- "too large for the exhaustiveness proof" `S03/equality_and_exhaustiveness.rs:974-976`.
- "late-selected receiver method retains an undischarged structural type equation" `S03/expression_lowering.rs:1413`.
- "unimplemented: target-family member `{}` requires {detail}" `S03/lowerer.rs:1511`.

**Stage 04**
- "this control domain is not yet implemented" `S04/assembly_and_operators.rs:1368`.
- "a position whose body supply is not implemented" `S04/assembly_and_operators.rs:1544`.
- "unconditional assembly cycle requires an explicit checked progress/control model" `S04/assembly_and_operators.rs:715-720`.
- "its complete qualified scope is not prepared" `S04/contextual_and_requirement_calls.rs:858-859`.
- "These are inference proposals, never domain, ownership or identity evidence." `S04/constructed_results.rs:348-352`.

---

## Disagreements

No second-pass record exists for these units, so there is no first-pass against second-pass conflict. What I found between records, or between a record and the code:

1. **Call custody order (unit 043 against the code).** Unit 043 says the build-time copy "tests source span FIRST" and the stage 02 copy "tests evidence requirement, then kind-based provenance, then source span". The code shows both copies test the evidence requirement first (`S02/selection.rs:497`; `BTE/const_generic_expressions.rs:47`). The order of the next two steps does differ as the record says. The record is right about the difference and incomplete about the common first step.
2. **`symbol_key` count (unit 044 against the code).** The record says 10 definitions in 9 files. My grep for `fn symbol_key` over `omega-rust/**/*.rs` gives 9 definitions in 8 files. Three are in this area (`S02/symbols.rs:2946`, `S03/lowerer.rs:1724`, `S03/type_reference.rs:1854`).
3. **Call sites of the destructure guard rewriter (unit 041, inside one sentence).** The record says "4 call sites" and then "5 lines in 2 files". The code has 5 call expressions: `S01/bodies_and_transitions.rs:540, 825, 1215`; `S01/expressions.rs:174, 176`.
4. **Where malformed numbers are rejected (unit 039 open, unit 040 silent).** Unit 039 could not say; unit 040 describes `parse_integer_literal` and `validate_float_literal` without mentioning the lexer flags. The code rejects on the flags at `S01/tokens_and_storage.rs:250, 290`.
5. **`constructor_prefix_is_value` (units 041, 042, 043).** Unit 042 was not sure which grep hits were this function. The code: the definition is `S02/symbols.rs:1882-1910`; `S02/preparation.rs:1267-1278` is a wrapper that turns the definition's ambiguity error into `true`. Unit 041 says no reason is given for that; the wrapper has no comment on the error case, but the comment on the next function in `S02/symbols.rs:1912-1916` says an ambiguous constant prefix "must never be reinterpreted as a namespace". So one definition and one wrapper, and a nearby stated reason the record did not cite.
6. **`closed_fixed_array_length` in other crates (unit 042 "suspected").** There are two definitions of that name: `S02/preparation_const_evaluation.rs:5492` and `typed-trees/src/access_views_and_commitments.rs:2103`. The hits in validation, the interpreter and provider planning cannot be the stage 02 function, which is `pub(crate)`.
7. **`resolve_numeric_probe` re-export (unit 042 unsure).** It is re-exported at `S02/lib.rs:108`; its only caller in `omega-rust` is the test at `S02/resolution.rs:2731`.
8. **Stage 02 callers of `parse_syntax_trees_with_id` (unit 040 "inferred" test code).** The other records place every cited line inside a test module: `S02/preparation.rs:5199-5333` (unit 041), `S02/resolution.rs:529-607, 940-1102, 2667-2889` (unit 042), `S02/symbols.rs:2812-2866` (unit 043). The records agree; I did not open these lines.
9. **Unit 045's own header against its footer.** The header says `contextual_and_requirement_calls.rs`, `body_cloning_and_rewriting.rs` and `monomorphization.rs` were read "partly" or "see Coverage"; the Coverage footer and the section headings ("read; completes the file") say all three were read in full. I take the footer. This cannot be checked from the code.
10. **A code comment against a record (unit 043).** The comment at `S02/selection.rs:3171-3176` says concrete machines "fail closed"; the reader records that `:3201-3212` accepts a visible State that is not a trait requirement. Not opened by me; kept in the "open" list.

---

## What I checked myself

29 claims opened in the checkout at `457adf4a11f`. No build, no run.

| # | Claim (record) | Where | Verdict |
|---|---|---|---|
| 1 | Four `evaluate_*` entry points have no caller beyond the re-export (038) | grep, 7 lines | matches |
| 2 | `compute_native_layout_plan_for_machine` has 2 matches (039) | grep | matches |
| 3 | `select_data_declarations` has 0 callers (041) | grep | matches |
| 4 | `parse_sources`, `parse_source_units` are test-only (040) | grep | matches |
| 5 | `resolve_numeric_probe` is called only from a test (042) | grep; `S02/lib.rs:108` | matches |
| 6 | `move x` is accepted and the keyword vanishes (040, open) | `S01/expressions.rs:668-669, 737` | matches |
| 7 | Boolean-coverage rewrite replaces the last arm's guard with `Always` (040, open) | `S01/bodies_and_transitions.rs:1068-1107` | matches |
| 8 | Exhaustiveness is skipped for runs with a continuation (044, open) | `S03/equality_and_exhaustiveness.rs:932-934` | matches |
| 9 | Structural compare matches calls by name and ignores cast target type (044, open) | `S03/equality_and_exhaustiveness.rs:1801-1817` | matches |
| 10 | `validate_computed_arguments` is skipped under a count condition (044, open) | `S03/lowerer.rs:1883-1895` | matches |
| 11 | Progress premises are normalised before membership selections (044) | `S03/lowerer.rs:1854, 1870` | matches |
| 12 | `::provider_defaults` suffix sets the purpose with no origin check (044, open) | `S03/declarations.rs:1549-1552` | matches |
| 13 | Clone predicate check rejects only `Some(false)`; `let complete = true;` (045, open) | `S04/body_cloning_and_rewriting.rs:735-755` | matches |
| 14 | Dynamic slot pass returns `Ok(())` if demand collection errors (045, open) | `S04/monomorphization.rs:2667-2669` | matches |
| 15 | Incomplete-selection error covers statement sites only; type bindings not counted as partial evidence (045, open) | `S04/monomorphization.rs:3885-3903, 4806-4811` | matches |
| 16 | Bodiless requirement operator with a selected float comparison is skipped (044, open) | `S04/assembly_and_operators.rs:1503-1512` | matches |
| 17 | Invalid-symbol `Named` argument counts as closed (038, open) | `BTE/const_evaluation.rs:305-312` | matches |
| 18 | Zero-gate ungating reads only the first variant (038, open) | `BTE/const_evaluation.rs:1794-1807` | matches |
| 19 | `endpoint_leaves` is never filled (038) | grep, 8 uses in the file | matches |
| 20 | Captured native binding with an unloaded source is skipped (039, open) | `BTE/product.rs:244-254, 326-328` | matches |
| 21 | Assignment of an evidence-named bare name is dropped (041, open) | `S02/lowering_statement.rs:1112-1140, 1529-1542` | matches |
| 22 | `call_custody_source` exists twice with differing order (043) | `S02/selection.rs:493-549`; `BTE/const_generic_expressions.rs:43-100` | partly (see Disagreements 1) |
| 23 | Entry-state rule duplicated (038 one site, 039 two sites) | `BTE/machine_execution.rs:737-752, 1833-1843`; `BTE/const_evaluation.rs:2940-2952` | matches; three identical bodies |
| 24 | Domain exposure re-implemented (043 one site, 044 one site) | `S02/selection.rs:2585-2605`; `S03/type_reference.rs:464-486` | matches; same job, different keys |
| 25 | Ranking default duplicated across stages (041 "mirrors the checker"; 044) | `S02/lowering_machine.rs:1956-2037`; `S03/lowerer.rs:174-245` | matches; same rule, name-keyed against symbol-keyed |
| 26 | `peel` in stage 02 is a duplicate of stage 03's (044 "suspected") | `S02/symbols_expression_paths.rs:168-178`; `S03/call_and_product_selections.rs:320-335` | matches; same loop |
| 27 | Constructor-prefix error counts as "is value", no reason given (041, open) | `S02/preparation.rs:1262-1278`; `S02/symbols.rs:1882-1916` | partly (see Disagreements 5) |
| 28 | `fn symbol_key` has 10 definitions in 9 files (044) | grep | does not match: 9 in 8 |
| 29 | Malformed numeric literals are only flagged by the lexer; consumer unknown (039, open) | `S01/tokens_and_storage.rs:250, 290`; grep for the flags | matches for the lexer; the consumer rejects them |

Also confirmed by grep while checking: five inline `layout.omg` tests in `BTE/layouts.rs` (`:1125, 1833, 2308, 2630, 3947`); four `reflect.omg` sites in the repository; two `EQUATABLE_TRAIT_PATH` constants; two definitions of `closed_fixed_array_length`; five call lines of `rewrite_destructure_guard_expression`.

Not checked: every other entry in this digest is carried from the records as written.

---

## Not covered

- **Records.** All eight records for the area were read in full: `unit_038.md` (258 lines), `unit_039.md` (220), `unit_040.md` (445), `unit_041.md` (387), `unit_042.md` (426), `unit_043.md` (420), `unit_044.md` (473), `unit_045.md` (380). No `unit_NNN_gap_*.md` file exists for 038 to 045, and no `gap_units` range names a file in these six crates.
- **Source ranges.** Per the records' Coverage sections, every file in the unit lists was read from line 1 to its end; none was skimmed. Five of the six crates are fully covered (38,029 + 1,196 + 23,671 + 62,177 + 18,853 lines; line sums match `crates.json`).
- **Stage 04 is partly outside this area.** `S04/preparation.rs` (2,137 lines) and `S04/selection_and_reflection.rs` (4,260 lines) are in `unit_046`, which is not one of this area's records. Statements here about those two files come only from call sites and greps in units 038, 044 and 045 (for example the driver order at `S04/preparation.rs:1946-2133` and the reflection name list at `S04/selection_and_reflection.rs:2514`).
- **Not traced by any record here.** Whether later stages (validation, stage 05) reject what this area leaves unresolved, unspecialised or invalid. Every reader says so in "What I am unsure about".
