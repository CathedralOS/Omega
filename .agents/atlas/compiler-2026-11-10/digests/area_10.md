# Area 10: Psi stages 06 to 08, lowering to Terminal Psi

Source: records `unit_066.md` to `unit_072.md` (7 records, 2,386 lines, all read in full; this area has no
second-pass gap records). Code at commit `457adf4a11f`.

Path convention. A bare file name is under
`omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/`. `07/` means
`.../07_lowered-psi-to-lowered-psi/src/` and `08/` means `.../08_lowered-psi-to-terminal-psi/src/`.
"Reader" means the author of one record. "Open", "stated" and "traced" are the readers' own classes
(BRIEF.md item 9). Where I opened the code myself the line says "checked".

## What this area does

The checker (stage 05) hands over checked source trees plus frozen fact tables: types, loans,
permissions, cleanup plans, contracts. Stage 06 turns each selected machine and everything it reaches
into Lowered Psi: machines made of blocks, each block a list of operations ending in a terminator
(jump, branch, case dispatch, return, crash), with explicit cleanup actions on every edge, plus
contracts, crash routes, proof evidence and many side tables. Stage 07 runs the selected
target-neutral optimization passes on that and seals a record of what ran. Stage 08 validates and
seals the result as Terminal Psi (the `PSITERM\0` artifact, or an in-process carrier) and binds the
program-entry receipt. Stage 06 is 123,583 lines in 19 files; stage 07 is 1,812 lines; stage 08 is 668.
Every stage 06 file is one flat file of nested inline `mod` blocks, and several files glob re-export
each other (`emission.rs:7-9` and `emission_cleanup.rs:7`; `expression_bindings.rs:6` and
`expression_preparation.rs:7`; `scalar_computations.rs:6`; `unit_arguments.rs:6-9`), so names are
found by module path, not by file.

**Stage 06, entrance and pipeline** (`lib.rs`, `machine_lowering.rs`, `attached_unit.rs`, `unit.rs`).
In: `CheckedTrees` and a machine selection. `lower_machine` (machine_lowering.rs:1673) refuses two
preconditions, picks a plan family in a fixed order (`lower_selected_machine`, machine_lowering.rs:1476),
and reaches the one real pipeline function, `lower_unit_closure` (attached_unit.rs:16769-17314). That
function runs about 30 stage modules in a fixed order: call closure discovery, structural types,
domains, services, boundary declarations, signatures, per-body emission, providers, layouts,
quotients, scalar applications. Out: a `LoweredPsi` whose `TerminalModule` still has about 25 fields
empty (attached_unit.rs:17241-17299). `lower_terminal_selection` (machine_lowering.rs:1925-2072) then
runs a fixed list of retention steps that fill them.

**Stage 06, body emission** (`unit_composed_control.rs`, `unit_operation_frame.rs`,
`unit_structural_values.rs`, `unit_arguments.rs`). In: one checked body. Two body kinds exist, Ordinary
and Composed (attached_unit.rs:57). Composed bodies (a state graph with successors, guards, case
dispatch, ranked loops) go through `state_graph::emit` (unit_composed_control.rs:10126-11248);
ordinary ones through `ordinary_machine::emit` (attached_unit.rs:10514-11558). Both use
`OperationFrame` (unit_operation_frame.rs:7342) to lower each effect operation and
`structural_values::emit` (unit_structural_values.rs:4760) to build records, cases, arrays, views and
references. `unit_arguments.rs` holds the per-body `Evaluation` state, the argument schedule and the
cleanup control flow. Out: one `TerminalMachine` plus occurrence side tables.

**Stage 06, shared emitters** (`emission.rs`, `emission_cleanup.rs`, `operation_emission.rs`,
`shared.rs`). `OperationBuffer` (operation_emission.rs:1198) is the mutable emission state, about 30
fields, and derefs to `Vec<Operation>`. `emission.rs` is 33 inline modules for stores, claims,
boolean control, case payload dispatch and byte sequences. `emission_cleanup.rs` decides which owned
places are discarded on which edge (`affine_cleanup`), tracks move-out and restore windows on borrowed
storage (`borrowed_window`), and duplicates blocks per incoming custody state
(`continuation_specialization`). `shared.rs` holds `LoweringError`, id lookups and the direct-call
constructor.

**Stage 06, expressions and scalar computations** (`expression_bindings.rs`,
`expression_preparation.rs`, `scalar_computations.rs`, `scalar_lowering.rs`). In: checked scalar and
Boolean expressions and the scalar computation graph. `ScalarBindings` (expression_bindings.rs:4066)
carries every binding table and is cloned per callee frame. `Expansion`
(scalar_computations.rs:6788) turns each computation node into a chain of branch states. Out: lowered
expressions, branch-state graphs, scalar qualification catalogs.

**Stage 06, proofs and retention** (`proofs.rs`, `retention.rs`, `projections.rs`). In: contract
plans, crash plans, proof facts, borrow facts, and the already-built module. Out: `Proposition`s,
crash route buckets, evidence terms, closed reach and conformance applications, payload memberships,
suspension call plans, external provider declarations, task activation demands. Mostly
lookup-and-join code: find the lowered operation for a checked fact and attach the fact.

**Stage 07** (`07/psi_optimization.rs`, `07/control_flow_and_retention.rs`,
`07/scalar_and_value_passes.rs`, `07/lib.rs`). In: `LoweredPsi` and an ordered pass selection. Runs
branch folding with machine pruning, constant propagation, copy propagation, value numbering and dead
scalar removal (07/psi_optimization.rs:32-52). Out: `PsiOptimizationStageResult` with an execution
record.

**Stage 08** (`08/lib.rs`, `08/result_policy_verification.rs`). In: the stage 07 result.
`finalize_terminal_program` (08/lib.rs:104) validates activation storage over the whole module, then
builds the canonical artifact or the invocation carrier. Also prepares and rejoins the program-entry
receipt, and exports `verify_boundary_result_policies` (08/result_policy_verification.rs:277).

## How it decides

**Identity.** Most joins are on symbol handles, type-identity ids, place, block, edge and operation
ids, and claim identities. Examples: call closure membership by `SymbolHandle` with linear
`Vec::contains` (attached_unit.rs:899, 929, 1006, 1068); claim identity to dense claim id
(emission.rs:677-727); callee lookup by machine symbol (scalar_lowering.rs:905-972); stage 07 passes
by value, block, operation and edge ids (07/control_flow_and_retention.rs:132-164).

**Coordinate (position in the checked tables).** A checked fact is joined to its lowered operation by
`(state, statement index, call ordinal)`, usually demanding exactly one match: retention.rs:1381-1390,
4386-4396, 4888-4914; proofs.rs:6564-6570; scalar_computations.rs:580-590;
unit_operation_frame.rs:2809-3034; 08/result_policy_verification.rs:327-333. Parameters are matched
by authored position (expression_bindings.rs:202, 1629, 1815; attached_unit.rs:13825-13848). Dispatch
copies are found by statement ordinal plus arena index and generation
(unit_composed_control.rs:3564-3590).

**Place and path.** Overlap and prefix tests on structural paths decide cleanup, windows and
partial moves: `structural_paths_may_overlap` at emission.rs:6555, emission_cleanup.rs:1095, 880,
3282-3314; exact window identity `(source place, path, field)` at emission_cleanup.rs:2252-2290; case
test reuse keyed on `(place, path, type, case)` at unit_arguments.rs:240-293.

**Value.** Range obligations are "decided at source" only when the proven restriction entails the
required one (emission.rs:4001-4010, 8809-8839); domain representation by predicate values and
`scalar_field_entails` (emission.rs:233-240, 279); provider result compatibility by type, multiplicity
and qualification set equality (attached_unit.rs:7142-7188); contract identity by report fingerprint
and commitment (scalar_lowering.rs:211-214).

**Enum shape.** Routes are chosen by the variant of the checked plan: call route and result
(unit_operation_frame.rs:1082-1164), store leaf (unit_operation_frame.rs:5176-5270), argument source
ladder (attached_unit.rs:14306-14483), computation kind (scalar_computations.rs:7388-7794), transfer
source (unit_composed_control.rs:8767-9201). Unsupported shapes are refused by a fixed string
(`unsupported("...")`); the readers counted 259 such lines in unit_composed_control.rs, about 250 in
unit_operation_frame.rs and about 199 in unit_structural_values.rs (text-pattern counts).

**Source spelling or identity text (grouped).** I counted 92 distinct sites cited by the records: 91
in stage 06, 0 in stage 07, 1 in stage 08. A "site" is one cited line or line range; repeated mentions
are counted once. The groups below add up to 92 (36 + 2 + 1 + 15 + 5 + 18 + 5 + 7 + 3).

- Member (field or case) matched by interning its source spelling and comparing identity ids, 36
  sites: emission.rs:4203, 4297, 4323, 5498, 8554, 8579, 9042, 9074, 9119, 9217, 9244;
  emission_cleanup.rs:2390-2392; expression_bindings.rs:1383, 2092, 2229; proofs.rs:3544;
  scalar_computations.rs:1825, 1857, 3240; shared.rs:1744, 1770; attached_unit.rs:2892, 3743, 3821,
  3837; unit_operation_frame.rs:6191, 6701, 6724; unit_structural_values.rs:3302-3309, 3322-3328,
  3372, 3392, 8102, 8294, 8338; retention.rs:211-224.
- Member matched by direct string compare of spelling against identity text, 2 sites:
  unit_composed_control.rs:12259-12262 (with `unwrap_or_default()`), 12272-12275.
- Reverse direction, identity text turned back into a spelling, 1 site:
  scalar_computations.rs:3568-3572 (unknown text becomes the empty string).
- Type found or ordered by type-identity text, 15 sites: attached_unit.rs:3435-3436
  (`"named(name(u64))"`), 3567 (`"named(name(u8))"` constant from shared.rs:1810), 3635 (sort);
  emission.rs:838-843, 1709-1717; retention.rs:406-420, 832-853; machine_lowering.rs:1883-1886;
  unit.rs:476-482; scalar_computations.rs:3368-3371; unit_operation_frame.rs:893-900;
  proofs.rs:207, 346-348; expression_preparation.rs:3192-3210 (domain sort);
  attached_unit.rs:4189-4207 (domain name plus carrier text).
- Compiler-made types found by a literal identity string, 5 sites: emission.rs:6954
  (`"compiler(byte-sequence-literal-view)"`), 7158 and 8031
  (`format!("compiler(byte-sequence-owned:{capacity})")`), 7256 (`"compiler(shared-byte-descriptor)"`);
  shared.rs:18-20 (`format!("compiler(activation-byte-backing:{bound})")`). One sibling lookup is by
  shape, not identity (emission.rs:7305-7313).
- Callable, declaration or service identity as text, 18 sites: retention.rs:1074-1079, 1125,
  1681-1686, 1701-1709, 1728-1742, 2612-2621, 4001-4011; projections.rs:424, 602-609, 870-890,
  1117-1118, 1160-1161, 1948; proofs.rs:6207-6214, 6265-6270; attached_unit.rs:4698, 7199;
  08/result_policy_verification.rs:392-399.
- Display path emitted as the wire identity of a construction, field or case, 5 sites:
  scalar_lowering.rs:1995, 2000, 2040; unit.rs:647, 801.
- Named library items recognised by name, file or source bytes, 7 sites: projections.rs:2088-2101
  (trait `TaskRuntime`, file `task.omg`, members `start` and `try_start`); retention.rs:302-340 (file
  `layout.omg`, byte-equal to the bundled source, names `toolchain::ResourceProfileSupply` and
  `profile`); retention.rs:509, 543 (`"Extent"`, `"base"`, `"length"`); retention.rs:760-769 (accessor
  kinds `"read"`, `"take"`, `"write"`); proofs.rs:6494-6498 (proposition declaration by name string,
  while proofs.rs:4751 and 5005 match the same relation by symbol);
  unit_operation_frame.rs:4900, 4919 (`PrimitiveType::from_name(carrier.name())`).
- Dynamic family tuples as strings, 3 sites: unit.rs:274 and 417 (`Vec<String>` compared by
  equality); unit_arguments.rs:6640-6643 (integers parsed back out of `"named(integer-const(N))"`).
- Not counted as judgments: `spell_place` (emission_cleanup.rs:3582) builds display text for messages
  only, and the reader says matching there uses ids and paths (emission_cleanup.rs:2852-2861).

## The same job in more than one place

Within one record the reader read both sites unless marked. Across records nobody compared the code;
those are marked "suspected" unless I opened them.

1. **Walk a checked structural path (Case, Field, Referent, Index) against type declarations.**
   Six walkers in emission.rs, each with its own segment handling and error strings: 1574, 4265, 5473,
   8430, 8994, 9174 (reader read all six). Two in attached_unit.rs: 2550 and 2729 (read both). Others
   named by other records: expression_bindings.rs:2589 and 3166; proofs.rs:1504;
   scalar_computations.rs:467 and 3707-3786; unit_structural_values.rs:3351. Sameness across the four
   records is suspected, not compared.
2. **Find the one case or field by interned spelling.** The 36 sites in the spelling list above;
   expression_bindings.rs alone has three copies (1362-1383, 2090-2098, 2227-2233).
3. **Find the one declaration by id.** `unique_declaration` (shared.rs:1783),
   `structural_declaration_by_identity` (shared.rs:1890), `structural_type_by_identity`
   (shared.rs:28), and hand-rolled copies at expression_bindings.rs:499-503, 2075-2084, 2218-2225,
   3053-3061, 3084-3090, 3175-3184. They differ in error handling.
4. **Lower a contract predicate to a `Proposition`.** Four `PredicateTerms` impls:
   scalar_lowering.rs:2227, unit.rs:1368, shared.rs:1534, proofs.rs:3661 (checked: exactly four).
   Inside proofs.rs three polarity lowerings re-handle Constant, Not, And, Or: 1000, 1816, 3838, all
   calling `equality_from_polarities` (1205). Three Boolean-to-`ScalarTerm` translators: proofs.rs:1993,
   2473, 3695.
5. **Allowlist of expression forms a contract may mention.** scalar_lowering.rs:1487, unit.rs:159,
   unit.rs:1162. They differ: `ImplementationSelection` is accepted at scalar_lowering.rs:1574 and
   rejected at unit.rs:203. Record 070 counted proofs.rs:904 as a fourth; checked, it is not (see
   Disagreements).
6. **Service reach ceiling.** A "collect into a set" form (attached_unit.rs:4834, 4866, 4878) and a
   "lower to id list" form (attached_unit.rs:13999, 14078, 14090) carry the same three checks. Checked:
   same job; the first error string differs slightly (4843 vs 14006), the second is identical (4856 vs
   14018).
7. **Per-place liveness over the block graph.** Different encodings in each: `custody_flow` bool
   vectors (emission_cleanup.rs:245-568), `projected_transfers` moved-path sets
   (emission_cleanup.rs:569-1341), `RegionState` Absent/Live/Partial (emission.rs:6026),
   UNBORN/LIVE/RETIRED bits (emission.rs:2624-2710), `captured_loans`
   (unit_composed_control.rs:1613-2386), `loan_chains` with `retained_homes`
   (unit_composed_control.rs:3980-5909), `loaned_local_homes` (unit_composed_control.rs:5910-6359),
   `reference_reads` (unit_composed_control.rs:12536-12816), `outstanding_actions` (unit.rs:2001).
   Records 067 and 071 each describe their own group as parallel mechanisms; sameness across the two
   records is suspected.
8. **Edit cleanup rosters with one match arm per terminator kind.** Six sites in
   unit_composed_control.rs: 2335-2379, 3336-3348, 4327-4362, 5708-5768, 5931-5973, 12743-12815. Also
   the write-back in emission_cleanup.rs (ending at 2044) and `install` at emission.rs:2327. All of
   them leave Crash, InvokeNoNormal and ExternalCompletion untouched.
9. **Union-find.** unit_composed_control.rs:1641 (no path compression),
   unit_composed_control.rs:5605 and 5613 (path halving), proofs.rs:4643 and 4827-4874.
10. **Dominators and reverse postorder.** `block_dominators` (emission.rs:1841; users
    emission_cleanup.rs:1924 and unit_composed_control.rs:6165); retention.rs:6028 instead calls
    `terminal_semantics::control_graph::dominators`; stage 07 has its own at
    07/scalar_and_value_passes.rs:463 and 485. Record 072 marks the stage 07 pair "suspected"
    duplicate of control_graph (bodies not read).
11. **Region or reachability walks.** unit_composed_control.rs:2224-2308, 4366, 5650, 6142-6149;
    emission_cleanup.rs:1505-1541; 07/control_flow_and_retention.rs:94-110.
12. **Allocate a new id as max plus one.** Types: emission.rs:7171-7190, 7228-7248, 7270-7291;
    shared.rs:56-77. Services: retention.rs:1743-1753. Boundaries: retention.rs:4784-4793.
13. **Map one access enum to another.** attached_unit.rs:225 and 3704-3717 (the second rejects
    `Owned`); expression_bindings.rs:2535; retention.rs:569-573, 4245-4251, 4727-4732, 5619-5623,
    5720-5728.
14. **Widen or exact-cast an integer to a u64 coordinate.** expression_preparation.rs:1069,
    1121-1125, 2040-2061, 4068; expression_bindings.rs:975-980; operation_emission.rs:3951-3981;
    emission.rs:11468-11501. They differ: `view_subslice::coordinate` rejects address integers and
    marks the cast decided; `exact_u64_index` accepts any integer and leaves a signed cast undecided.
15. **Turn a runtime index selector into an expression.** expression_preparation.rs:1997-2037;
    expression_bindings.rs:1072-1090, 4662-4685, 5452-5548 (four copies, read); also
    unit_arguments.rs:5709-5768, unit_structural_values.rs:594, emission.rs:5652 (suspected).
16. **Symbol to terminal machine, or to owning machine.** shared.rs:1996; retention.rs:1573, 1689,
    5941; proofs.rs:4832-4836; expression_preparation.rs:2819-2822; machine_lowering.rs:1211-1214;
    unit.rs:468-472. All linear scans.
17. **Symbol to member identity by scanning every data definition.** proofs.rs:97-109, 5861-5873;
    scalar_computations.rs:2285-2306.
18. **Find an operation by scanning all machines, blocks and operations.** retention.rs:4916-4939,
    5977, 6071-6079; proofs.rs:5357-5364, 5628-5637.
19. **Emit `StructuralLeafCopy` by hand.** unit_operation_frame.rs:1001, 2078, 4185, 6658;
    unit_structural_values.rs:7318; emission.rs:11732; unit_composed_control.rs:9190. Record 072
    marked the last two "suspected". Checked: both build the same operation into a fresh Unrestricted
    result place; the emission.rs one always uses an empty path.
20. **Walk the capture frontier of a place.** unit_operation_frame.rs:1700-1992 and
    unit_structural_values.rs:431-567. The comment at unit_structural_values.rs:331-333 says "Value
    reads and dispatch captures share this evaluator"; the first site keeps its own loop.
21. **Build record or case field initializers.** unit_structural_values.rs:8041-8163 and 8231-8399
    (near-identical; erased fields skipped in record, refused in case);
    unit_operation_frame.rs:6668; attached_unit.rs:3742-3767 and 3816-3860 beside `lower_mixed_fields`
    and `lower_mixed_cases` (shared.rs:1724, 1752).
22. **Captured-store splice.** unit_operation_frame.rs:5414-5433, 5752-5778, 6168-6210.
23. **Parameter-kind code.** retention.rs:1085-1096 and proofs.rs:6877-6884. Checked: different
    numbers for the same enum (retention: Type 0, Const or Value 1, Proposition 2, Machine 3 or 4,
    Implementation 5; proofs: Type 1, Const 2, Value 3, Machine 4, Implementation 5, Proposition 6).
    The reader did not check whether both feed one consumer.
24. **Declaration identity text.** `declaration_identity` (retention.rs:1701, package-qualified) and
    plain `display_path` (proofs.rs:6207, retention.rs:2272).
25. **Zip specialization arguments by parameter kind.** retention.rs:1220-1361, 1584-1664, 2435-2489.
    They differ: a `Proposition` parameter is an error at 1350 and `Ok(None)` at 1648.
26. **Values a stage 07 pass must not touch.** 07/control_flow_and_retention.rs:290-365;
    07/scalar_and_value_passes.rs:139-193, 370-399, 600-611.
27. **Hand-built `Block { ... }` literals.** unit_arguments.rs:1263, 1729, 1806, 4253, 6233, 6486;
    13 lines in unit_composed_control.rs (grep count by reader).
28. **Proof-formal forwarding lists.** unit_arguments.rs:1402, 4293, 5623;
    unit_composed_control.rs:8497, 8804, 10002, 10628.
29. **Composed-graph helpers repeated in unit_composed_control.rs.** "Entry is re-entered": 10410,
    11333, 11441, 11600. "Incoming edges of a position": 11390, 11458, 11659. "Which parameter does
    this transfer forward whole": 1500-1512, 1670, 11523-11528, 11725. `EvaluateNoNormal`: 2500-2520,
    3004-3045, 8768-8811. `ReturnStructural` plus materialize: 6416-6489, 13289-13318. Frame build and
    evaluate by hand at about 20 sites while the macros at 571-594 exist only in one function.
30. **Smaller repeats, one record each.** Call binding emit (operation_emission.rs:824, 1965, 2158);
    slot-versus-tuple resolution (operation_emission.rs:2264, 2384); match on
    `CheckedScalarComputationKind` at 12 sites in scalar_computations.rs (reader count by reading);
    recursion over lowered Boolean and direct expressions four times in emission.rs (9457, 9631, 10856,
    11157); domain lookup by `(semantic_domain, carrier)` (expression_preparation.rs:2631, 3282, 3603,
    3620, 3639); place to single carrier type (expression_bindings.rs:34, 3970, 3992, 4480);
    mathematical certificate encoding (scalar_lowering.rs:1321, 1392, 1909, 1976); five scalar
    completion funnels (unit_arguments.rs:2926, 3007, 3121, 3185, 6009); case chains
    (unit_arguments.rs:1108 and 1621; 4005 and 4133); six float-source arms (proofs.rs:7175-7258);
    emission counters in two shapes (unit_composed_control.rs:138 and 303).
31. **`selected_memberships`** (emission_cleanup.rs:2369 and unit_operation_frame.rs:1995), marked
    "suspected related" by record 072. Checked: related, not copies. The first selects restorable
    memberships for a window field from the address's own qualifications (`select_restorable`); the
    second looks up a completed operand's supplying memberships and projects them to its path
    (`select`, `select_rows`).

## Fixed lists of shapes

- Body kinds: exactly Ordinary and Composed (attached_unit.rs:57). Unit plan kinds: three
  (unit.rs:3784). Plan family order in `lower_selected_machine`: six steps (machine_lowering.rs:1476-1552).
- Boundary execution plan: seven variants (attached_unit.rs:386-408); static interrupts: four
  operations (attached_unit.rs:779-791); descriptor tables: four operations
  (operation_emission.rs:2118-2145).
- Recursive types refused: "recursive structural type is outside the Unit terminal slice"
  (attached_unit.rs:3525). Schema ABI version must equal 4 (attached_unit.rs:9584).
- Structural argument source ladder, eight rungs, ending "neither a parameter nor a lowered literal"
  (attached_unit.rs:14306-14483).
- Nested selectors: `TERMINAL_NESTED_SELECTOR_LIMIT = 7` with `assert!(ordinal < 7)`
  (operation_emission.rs:2498, 2915); id namespaces carved at `1 << 61`, `1 << 62`, `1 << 63`
  (operation_emission.rs:2495-2499, shared.rs:1812).
- Depth and budget caps: 128 shells (attached_unit.rs:2699), depth 128 (emission.rs:8062), content
  term depth 256 (proofs.rs:714), predicate budget 4096 and depth 64 (proofs.rs:821, 904, 965),
  evidence renumbering at most 4 rounds (proofs.rs:6689-6730), eligibility lookups 4096
  (08/result_policy_verification.rs:48).
- `view_leaf_image`: exactly four carrier pairs (emission.rs:5582-5626). Saturating shift: widths
  "8 | 16 | 32 | 64" (expression_preparation.rs:4165-4172). Signed carrier widening table
  "0..=8 => 16, 9..=16 => 32, 17..=32 => 64" (expression_preparation.rs:986-994).
- Integer comparisons admitted: six operators mapped to three terminal kinds (emission.rs:11430-11454).
  Float handling only Binary32 and Binary64 (operation_emission.rs:416-421, 613-621;
  08/result_policy_verification.rs:187-224).
- Whole-parameter cleanup terminators: "scalar graph whole parameter cleanup requires scalar or Unit
  exits and ordinary edges" (emission_cleanup.rs:1882-1884).
- Borrowed windows: "borrowed-storage windows require a mutable-borrowed parameter or an established
  owned or exclusive structural local" (emission_cleanup.rs:3490-3496).
- Case payload dispatch binds only Scalar, BoundedInteger and RefinedScalar fields
  (emission.rs:10697-10707). Computed cases: "computed case requires a plain sum"
  (scalar_computations.rs:1804), "computed case field is not scalar" (scalar_computations.rs:1864).
- Outcome-specific guarantees: only payloadless scalar-case returns, "outcome-specific guarantees
  require guarded exit and caller-arm lowering" (machine_lowering.rs:1230-1232); callee side
  "outcome-specific selection from a callee with structural inputs or other contracts is
  unimplemented" (proofs.rs:5410).
- Static requirement dispatch: "outside the bounded runtime Unit, exact i32, or exact bool call"
  (proofs.rs:6204).
- Restored reborrow cohort size `1..=3`: "restored shared-freeze reborrow cohort is outside the
  retained bound" (retention.rs:4523-4526). Placement profile positions per operation: `[2]`, `[1]`,
  `[2, 3]` (retention.rs:271-275).
- Task start: only `Start` and `TryStart`, "must select exactly one static target machine"
  (projections.rs:2123).
- Activation storage: five actions; the outcome carrier must be a sum with exactly two cases,
  "activation outcome has no exact case roster" (unit_operation_frame.rs:79-249, 304-310). Register
  access: nine arities (unit_operation_frame.rs:4854-4896). Atomic events: six kinds
  (unit_operation_frame.rs:415-472).
- Composed graphs: `OrdinaryArm` seven variants (unit_composed_control.rs:2865-2885); a guarded jump
  chain needs at least two arms (unit_composed_control.rs:7195-7199); exclusive handoff promotion
  only for Record or FixedArray (unit_composed_control.rs:11619-11622); "Unit graph requires a closed
  checked body" (unit_composed_control.rs:1313).
- Dynamic family dispatch: nonzero arity, at least two tuples, integer binders
  (unit_arguments.rs:6440-6446, 6612-6617). `complete_straight_line` only for jump chains with three
  binding kinds, else it falls back (unit_arguments.rs:4322-4490).
- Stage 07: only `Conditional` on a `BooleanConstant` is folded (07/control_flow_and_retention.rs:26-48);
  constant propagation seeds only integer and Boolean constants (07/scalar_and_value_passes.rs:26-34);
  machines with a ranked cycle are skipped (07/control_flow_and_retention.rs:20,
  07/scalar_and_value_passes.rs:18); four selections are accepted and do nothing
  (07/psi_optimization.rs:48-51).
- Stage 08: the entry result must be Unit (08/lib.rs:74-76); the saturation-eligible expression must
  be exactly "byte one equals AND-tree" (08/result_policy_verification.rs:62-73).
- Exhaustive matches the readers noted (a new variant breaks the build): attached_unit.rs:3698-3882,
  4599-4643; emission_cleanup.rs:3967-4159; scalar_computations.rs:7388-7794, 6667;
  unit_composed_control.rs:8767-9201, 1450-1477.
- Panics instead of errors: operation_emission.rs:132 and 3167 (unchecked `parameters[*position]`),
  269-271, 294-297, 3188-3191, 3678-3736; emission.rs:3827, 10294, 10376-10379, 10523, 10788;
  attached_unit.rs:7236, 7311, 12093; unit_composed_control.rs:4894, 5834, 8019;
  07/scalar_and_value_passes.rs:226, 311-313.

## Accepts when it does not know

`lib.rs:14-16` says "any construct without an explicit checked plan fails closed immediately". The
entries below are where a reader saw a missing row, `None` or unmatched case lead to keeping or
accepting. A reader calling an entry "open" means no reason is given at the site and the reader did
not trace it; it does not mean a wrong program was shown to get through.

### Classed "open" (93 entries)

attached_unit.rs (record 066; the reader notes none of these has a comment at the site):
1. 541, 547: `lower_scalar_ensures` drops an ensures row, or an implication with an unavailable side,
   when the structural result is unavailable. Reader: a boundary ensures clause that names a
   structural result is not carried into the boundary declaration. Checked: matches.
2. 6290: authored `requires` are lowered only if every clause is `Some`; otherwise none are emitted.
   What `None` means was not traced. Checked: matches.
3. 7101: `boundary_candidates` returns no candidates, no error, for a boundary with no requirement
   catalogue entry.
4. 6711-6717: an invalid receiving handle becomes an empty default application.
5. 8375-8377, 8431-8434, 8467-8472: unreferenced recipes, evaluated constants and landed layout
   observations are skipped ("by reference set; open").
6. 8071-8073, 8087: a proof-scope description failure becomes `Unavailable`; an invalid symbol maps
   to the default id.
7. 9345: open (generic) target families are skipped.
8. 13701: returned-reference paths whose carrier is a byte or element view are skipped.
9. 15256-15268 with 15227: `prepare_immutable_local_borrow` returns `Ok(false)` and its one caller
   ignores it. Reader traced the single call site; the consequence is open.
10. 11984-11992: `ReleaseReference{defer_to_exit: true}` emits nothing here ("presumably emitted at
    exit", not traced).
11. 11666-11670: call literals are not pre-established after the first call with an `External`
    completion.
12. 8792-8802, 8827-8833: scoped lookup falls back to an unscoped entry.
13. 12242-12244: quotient types with no `type_ids` entry are skipped.
14. 4491-4531: an establishment is omitted on any missing link. Reader inferred the effect is
    conservative (fewer establishments).

emission.rs (record 067):
15. 1355-1357: an owned operand with no checked transfer rows contributes no claims. Reader: if the
    row were missing from `owned_transfer_paths` the claim would not appear in the result; producer
    not traced.
16. 436-444: `selections::destinations` skips non-matching and `selected == remaining` bindings.
17. 1714-1716: an unmatched formal type identity falls back to `EndpointUse::Storage`.
18. 1855-1858: `block_dominators` with no entry makes every block dominate every block. Checked: the
    branch exists, and both callers pass `Some(...)` (emission_cleanup.rs:1924,
    unit_composed_control.rs:6165), so it is not reached today.
19. 4546: unknown jump targets are dropped by `filter_map`.
20. 5248: with a runtime index only element 0 is probed after the disjointness check ("open for
    non-uniform arrays").
21. 8245-8253: `readable_byte_source_bound` ignores the capacity for a borrowed view. Reader: a
    borrowed view longer than the plan's capacity would be accepted here; a separate capacity
    obligation is emitted at 7922, not traced. Checked: matches.
22. 9646: `contains_short_circuit` does not look inside `IntegerComparison` operands (inferred).

emission_cleanup.rs (record 067):
23. 1194-1196: `restore_path` returns silently when the root is not in the frontier.
24. 872-895, 919: structural arguments on `StructuralCase` edges and `ExternalCompletion` are not
    run through `transfer`. Whether those lists can be non-empty there was not traced.
25. 2044: no discards are added on InvokeNoNormal, Crash or ExternalCompletion exits, even for live
    managed places.
26. 1439-1459: `scheduled_deaths` is machine-wide, so a descriptor arrival with a death on one branch
    gets no `DiscardRoot` on the others from this function.
27. 3089-3118, 3223-3227: a store with no open window and no required restoration is emitted as
    `Vacant` without this code proving the slot vacant.
28. 2274-2276: on ambiguous captures the unresolved carrier is used, so an overlap reached through
    two spellings would not be seen at 2852-2861.
29. 4903-4912: source-checked obligations that no instance renamed are kept verbatim.

expression_bindings.rs (record 068):
30. 2995: an invalid computation handle is skipped and the index stays a `ComputedIndex`.
31. 1732-1736: earlier payload rows with the same source and path are replaced.
32. 4032-4051: fewer captured places than runtime indexes leaves the rest as `RuntimeIndex`, no error.
33. 4434: `root == observed_root` is accepted without an access check (callers outside the file
    unread).

expression_preparation.rs (record 068):
34. 661-662, 743-747: five structural value kinds add no scalar dependencies to the closure.
35. 388-391: a missing call schedule is `unwrap_or(&[])`.
36. 2144-2148: a domain whose parent memberships are not well-formed loses its predicate instead of
    erroring.
37. 1668-1685: an "unresolved" Boolean is emitted when no structural fields exist. The emitter treats
    that variant as `unreachable!` (operation_emission.rs:269-271).
38. 3787: `ArithmeticDomain` constraints are ignored when collecting qualification atoms.
39. 3796-3813: three guard arms accept a `Domain` constraint with no atom when a predicate form
    exists; where the predicate is enforced was not traced.
40. 4449: other structural value kinds add no domains.
41. 4265-4268, 4281-4288: a missing root yields a default type reference.
42. 3326: `.ok()` turns an error into `None` (callers unread).

machine_lowering.rs (record 068):
43. 540: a callback request with no emitted registrar is dropped.
44. 1280-1282: a checked interrupt-install request with no emitted install operation is skipped.
45. 1618-1620, 1628-1630: `scalar_transport::complete` silently does nothing on two `None` paths;
    `lower_machine` (1798) ignores the outcome.

operation_emission.rs (record 068):
46. 1287-1319: `declared_source_claims` returns `Ok(None)` when nothing names the place.
47. 577-579: a call whose site is not an `Expression` gets no float result policy.

projections.rs (record 068):
48. 2133-2142: a `TaskRuntime` start whose target is a machine type parameter is skipped, yielding
    no activation demand.
49. 1835-1866: calls to unresolved or non-checked-supply targets are recorded as `unresolved_calls`
    instead of failing (consumers unread).

proofs.rs (record 069):
50. 2625-2626: the "policy" divisor check accepts a literal `-1`.
51. 298-308, 368-370: an entry claim with no content qualification or projection is dropped. Reader:
    a parameter claim whose qualifications do not resolve to a content projection is dropped rather
    than rejected.
52. 4452-4463: a crash bucket whose guards all vanish is dropped (the reader did not read
    `CrashRouteBucket::new`).
53. 4220, 4326: unlisted term and proposition kinds are left unsubstituted.
54. 4751-4766: an application naming an undeclared proposition is omitted. Traced for the lookups at
    5050 and 6543 only; open for other consumers.
55. 7186-7256: with no resolved direct source the float projection uses `TransitionalInput`.
    Reader: a program whose direct source could not be resolved still lowers.
56. 7319-7324, 7400-7402, 7417, 7449: four `Ok(None)` paths that lead to entry 55.
57. 7479-7487: `Ok(None)` when parameter scalar types differ.
58. 6954-6961: open generic requirements get an empty row list, so a lowered application with zero
    rows would pass the length check at 6330.

retention.rs (record 069):
59. 698-700: a view plan whose carrier type was not emitted is dropped.
60. 757-775: an accessor target matching no boundary machine yields an operations list without it.
61. 1183-1185, 1408-1410: a closed-reach call with no emitted occurrence gets no entry. Reader:
    fine only if the emitter legitimately elided the call; not traced.
62. 1898-1901, 1935-1945: conformance applications and static calls are skipped on owner or source
    mismatch.
63. 2517-2519: retired calls are skipped (what retires a call not traced).
64. 2652-2655, 2697-2699: provider candidates are silently omitted.
65. 3556: `operand_memberships::retained` returns `None` for any unlisted operation kind. Reader: a
    payload membership needed on an unlisted kind is lost silently.
66. 3742, 3783-3785: early returns leave results with fewer qualifications (inferred conservative).
67. 3474, 3490, 3814: `Err` from three membership sources is ignored.
68. 5369, 5393, 5547, 5647: a loan the facts cannot classify is emitted as `Unresolved`. Whether a
    later validator rejects `Unresolved` was not traced.
69. 4881-4886: crossings outside the machine and state-block filter are not planned.

scalar_computations.rs (record 069):
70. 712-778: `consume_pure_call_arguments` checks nothing for non-`Body` callees, calls with
    structural arguments, or non-`Value` operands.
71. 3104-3110, 3457-3459: no planned place loan means the store takes the non-loan path.
72. 3568-3572: unknown field identity text becomes an empty spelling.
73. 6060-6082: when both arms of a select are the same value, the false arm and its claim edge are
    skipped.
74. 7107-7117: only the `LocalInitializer` role is cross-checked.
75. 5847: `continuation = 0` used as a placeholder state index.
76. 4432-4441: an established case payload returns early, skipping later carrier checks.

scalar_lowering.rs, shared.rs, unit.rs, unit_arguments.rs (record 070):
77. scalar_lowering.rs:1761-1763: a closed `Boolean(_)` or `Integer(_)` clause becomes
    `Proposition::Truth` and the value is dropped. Reader: a closed Boolean `false` clause would be
    lowered as no requirement; producer not traced. Checked: matches.
78. scalar_lowering.rs:1783-1792: `None` and source-checked guarantees are filtered out of the
    projected contract.
79. scalar_lowering.rs:1827-1831, 2129-2167: a missing type identity becomes an empty string.
80. scalar_lowering.rs:2506-2508: carriers with domains or nonzero level arity are not published.
81. scalar_lowering.rs:2815-2818: a produced value without fresh storage gets no discard.
82. scalar_lowering.rs:793: `Ok(None)` snapshot completion.
83. shared.rs:308-321: a missing call row is treated as an ordinary operand call.
84. shared.rs:2065-2079: no qualification row means a plain scalar type with no qualifications.
85. unit.rs:98-100: no contract plan means no crash routes. Reader: a callee without a contract plan
    is treated as crash-free. Checked: matches.
86. unit.rs:625-628: tables not selected here get `None` rows and no realization check.
87. unit.rs:1074-1080, 1560-1561, 2061-2072: facts skipped, propositions returned unchanged, and a
    store re-opening a cleanup action.
88. unit.rs:3759-3761: a cleanup whose machine is not in the closure is skipped.
89. unit_arguments.rs:4293-4306: empty proof actuals are replaced by the caller's own formals.
90. unit_arguments.rs:1849-1855, 744-753, 1532-1535, 5191-5207: empty capture slots dropped;
    `CheckedPartition` actions dropped by `filter_map` (reader: guarded by construction at
    unit.rs:3703-3716, not confirmed for all producers); an owner with an invalid symbol silently
    adopted.

unit_composed_control.rs (record 071):
91. 365, 3231-3236: when every path crashes, or the completion is a crash, the window-closure check
    is skipped. 2265, 2378, 4359-4361, 5765-5767, 5972: crash-like exits get no storage disposal from
    any insertion pass. 1528, 1553: a borrowed target parameter fed by another source shape gets no
    owner. 3564-3571: no checked transition binding means no dispatch copies and no error; reader: an
    arm whose binding row is missing would read live storage instead of a saved copy (checked:
    matches). 4838-4861, 5018-5023: `Foreign` and `Other` arrivals are accepted in unseeded groups.
    4226-4235: a `DiscardRoot` disposition is synthesized when no checked roster exists. 7691-7699,
    8217-8222: a missing return value is appended; only scalar `Return` registers an occurrence.
    9333-9432: a loan from an unlisted source shape with no incoming resources is accepted with no
    retained home. 11325-11425: an entry borrowed alias is not checked for intermediate writes. 9498,
    11593: shared-to-mutable promotion is keyed on supplier access and type only. 11920: a cycle
    closed through a `StructuralCase` edge gets no rank edge. 12261: `unwrap_or_default()` on the
    declared case text. 12302-12319: atomic and non-scalar fields get no snapshot. 12629-12700: in
    three `continue` cases the deferred `ReleaseReference` is never emitted anywhere; reader: a
    program with an observed reference whose retirement point cannot be found would compile with no
    retirement. 13428, 13445-13475: fallback loan sources. (Counted as one entry here; 16 sites.)

unit_operation_frame.rs, unit_structural_values.rs, stages 07 and 08 (record 072):
92. unit_operation_frame.rs:2889, 2971-2979, 2996-2998, 3011-3017: a live value that crossed a
    suspension but has no located home is dropped from the frontier (consumer not traced).
    unit_operation_frame.rs:3178-3228: a call-ensured membership is not recorded on five conditions.
    unit_operation_frame.rs:8085-8091: cleanup skipped when already released.
    unit_structural_values.rs:3593-3634: the last dispatch arm is taken without testing its pattern;
    reader: a program whose final arm is not exhaustive would run that arm for non-matching subjects,
    resting on a checker guarantee not traced (checked: matches).
    unit_structural_values.rs:5442-5444: owners with an invalid symbol are always live (checked:
    matches). unit_structural_values.rs:2268-2293: an invalid handle means no content transformation
    ("inferred benign, open").
93. 07/psi_optimization.rs:48-62: four selections do nothing and are still recorded as executed
    (checked: matches). 07/control_flow_and_retention.rs:710, 959: debug sites of unlisted kinds are
    kept (debug data only). 08/result_policy_verification.rs:334-336: a float boundary call whose
    source site is not an `Expression` is skipped without adapter verification, and the expected
    count excludes those sites too, so a missing adapter there would not be caught; whether such a
    site can carry a policy was not traced (checked: matches).

The count of 93 is by numbered entry; entries 87, 90, 91, 92 and 93 bundle several sites, so the
number of cited line ranges is higher (over 100; I did not count them one by one).

### Classed "stated" (a comment, name or test at the site gives the reason)

- emission.rs:6113-6114: "Unknown or repeated identities leave the original lazy checks in charge."
- emission_cleanup.rs:5953-5958: "Missing claim mappings mean unchanged" (stated for claims; the same
  default at 4653 applies to values, places and obligations without a comment).
- retention.rs:3333-3339: an unknown membership id is kept; test name
  `domain_reachability_follows_children_and_conservatively_keeps_unknowns` (3362).
- unit_composed_control.rs:10971-10988: obligations marked source-checked for decided arrival edges;
  comment 10967-10970: "An arrival whose transition checking discharged in full needs no certificate,
  provided every row is a plain clause the call check judged". The reader did not read
  `call_requires_decided` (stage 05).
- unit_arguments.rs:172-188: doc comment 149-151, "Other cleanup regions keep their ordinary selected
  scope".
- Stated by name, enum or structure only (the readers' wording): emission.rs:1383-1384, 3156, 6702,
  8621; expression_preparation.rs:497-498, 2750-2752, 2811-2818; machine_lowering.rs:562-570;
  operation_emission.rs `bound_decided` and `formation_decided` (2979-2981, 3443-3446, 3495-3497);
  projections.rs `Unavailable(reason)` roots (455-459); shared.rs:1983-1994;
  unit_operation_frame.rs:1165-1167; unit_structural_values.rs:1879-1885, 7987-8024; 08/lib.rs:90-98.

### Classed "traced" (the reader followed it and says why it is or is not reachable)

- attached_unit.rs:1028-1032 (the target still enters the closure); 5656 (region kept as planned).
- emission.rs:81-285 and 337 (every unknown publishes more); 1948 (leads to a refusal); 2481-2524
  (cleanup direction only); 3499-3511 (already filtered); 4099-4104; 6279-6281; 7957-7973;
  10628; 11843-11848; 12033.
- emission_cleanup.rs:734-736; 1781-1783; 4653 for definitions (every id defined in a block is
  remapped at 5968-6057); 4834-4895 (dropped rows were attached to unreachable code).
- expression_bindings.rs:2194 and 2130 (closed at the caller); 4491-4493.
- projections.rs:1663.
- proofs.rs:4545-4549; 4847, 4882-4884, 4946-4948 and 4999-5001 (consumers error on `None` at
  6461-6471).
- retention.rs:90-92 (by inspection only); 3295-3300; 3997-4012; 4896-4898; 5141-5149 and 5524 (all
  five arms check validity).
- scalar_computations.rs:3153-3159 (error preserved and surfaced on use).
- unit.rs:147-149 (widens to `Truth`, the safe direction).
- unit_composed_control.rs:7123-7182 (unreachable with a guard); 2993-3002 (removes knowledge);
  4683, 4963-4965 (partly; "not traced further"); 11886, 11966.
- unit_operation_frame.rs:5081-5094; unit_structural_values.rs:1615-1647, 2837-2914.
- 07/control_flow_and_retention.rs:930-952 (valid for its three callers); 07/lib.rs:76 (guarded by
  the one caller); 07/scalar_and_value_passes.rs:293-322, 427-429, 594-617.
- 08/result_policy_verification.rs:400-411 and `saturation_eligibility::verify` (fail closed by
  count and on every unknown).

## Silent drops

Places where failing to plan or resolve something produces no row and no diagnostic. I counted 56
distinct sites or site groups in stage 06, 1 in stage 07 and 2 in stage 08, from the records.

- Contracts and declarations: attached_unit.rs:547 (ensures row), 6290 (all authored requires), 3112-3129
  (program-local root introductions), 4491-4531 (establishments), 7101 (provider candidates), 9345
  (open families), 13701 (returned reference paths), 12242 (quotient types), 16843-16848 (opaque
  representations), 2019 and 3054 (type roots for unlisted kinds), 11666 (call literals), 8375-8472
  (scalar applications); scalar_lowering.rs:1783-1792 (guarantees), 2506-2508 (carriers); unit.rs:98-100
  (crash routes), 625-628 (dispatch rows), 3759-3761 (cleanup subject).
- Claims, cleanup and custody: emission.rs:1355-1357 (claims), 10703 (payload parameters);
  emission_cleanup.rs:1194-1196, 2044, 4834-4895; scalar_lowering.rs:2815-2818 (discard);
  unit_arguments.rs:744-753 and 1532-1535 (partition actions), 1849-1855 (capture slots);
  unit_composed_control.rs:1528 and 1553 (loan owners), 3564-3571 (dispatch copies), 9333-9432
  (retained home), 11920 (rank edge), 12302-12319 (field snapshots), 12629-12700 (reference
  retirement); unit_structural_values.rs:2268-2293 (content transformation);
  scalar_computations.rs:3104-3110 (loan subject).
- Retention: machine_lowering.rs:540 (callback request), 1280-1282 (interrupt request), 1618-1630
  (scalar transport), 719-733 (debug spans); retention.rs:90-92, 698-700, 757-775, 1183-1185 and
  1408-1410, 1898-1901 and 1935-1945, 2652-2655 and 2697-2699, 3556, 3474/3490/3814, 4881-4886;
  projections.rs:602-612 (conformances), 863-866 (errors swallowed, names only), 2133-2142 (generic
  task start); unit_operation_frame.rs:2813-3017 (suspension frontier), 3178-3228 (call ensures).
- Proofs and qualifications: proofs.rs:298-308 and 368-370 (entry claims), 4452-4463 (crash buckets),
  4751-4766 (proposition applications); expression_preparation.rs:2144-2148 (domain predicate),
  2490-2520 (parameter facts).
- Stage 07: 07/psi_optimization.rs:48-51 (a selected pass that does nothing, no diagnostic).
- Stage 08: 08/result_policy_verification.rs:334-336 and 354-364 (calls skipped by the verifier).

## Unused or unwired

Counts are the readers' greps over `omega-rust` unless marked "checked" (my grep, `*.rs`).

- `verify_boundary_result_policies` (08/result_policy_verification.rs:277): 4 hits in 2 files, both
  inside its own crate. Checked: same result; also no hit in `.md`, `.toml` or `.sh` files of the
  checkout. Exported and not called.
- 08/lib.rs:39, 41, 42: variants `TerminalIdentity`, `ContextBridgeMultiplicity`,
  `ContextBridgeOccurrenceMismatch` appear only at their declaration. Checked: matches.
- 07/lib.rs:88 `PsiOptimizationStageError::InvalidDebugMap`: the reader saw no construction in the
  unit. Checked: no construction anywhere in omega-rust (the other hits are the different
  `LoweringError::InvalidDebugMap`).
- Four `PsiOptimization` selections are no-ops in stage 07 (07/psi_optimization.rs:48-51); the reader
  saw those variants referenced in 5 files and did not read the others.
- `lower_unit_services_including` (attached_unit.rs:4564): 2 hits (definition and one call passing
  `&[]`), so `additional_roots` is always empty. Checked: matches.
- `resolve_float_operation_result_source` (proofs.rs:7596) `call == false` arm: the only caller passes
  `true` (7414). Checked: 2 hits, matches.
- `continuation_identities` (emission_cleanup.rs:3717): 3 lines, one consumer. Checked: matches.
- Stage 07 retention helpers (`retained_machines_with_roots`, `machine_retention_roots`,
  `block_local_evidence`, `machine_evidence_bound`): all hits in one file, one consumer.
- `LoweredBooleanReturnExpression::UnresolvedStructuralParameterField`: produced at
  expression_preparation.rs:1668-1685; the emitter arm is `unreachable!`
  (operation_emission.rs:269-271).
- `TerminalModule` literal sets about 25 fields to empty at attached_unit.rs:17241-17299; filled by
  later retention steps (inferred by reader 066; record 068 lists those steps at
  machine_lowering.rs:1969-2070).
- Leftovers: `let _ = position;` (unit_composed_control.rs:1885); a `match scalar_type` with
  identical empty arms (unit.rs:1487-1490); values computed and discarded, only the error matters
  (emission_cleanup.rs:3125-3163).
- Pure wrappers: `edges::successors` (unit_composed_control.rs:12935);
  `lower_nominal_affine_unit_cleanup_machine` (unit.rs:2519).
- "Single way" comments checked by readers: "The one byte-sequence operand observation"
  (scalar_computations.rs:95) has 2 in-file callers and none outside; "Selected edges share one
  payload producer" (unit_composed_control.rs:3423) has 3 callers, all in the file; "Unified
  `OperationFrame` emitter" (unit_operation_frame.rs:1-5) has two outside users of `emit_call`
  (attached_unit.rs:12140, unit_composed_control.rs:846). The shared.rs source-guard tests
  (shared.rs:2256-2370) assert one direct-call constructor by scanning six named files as text; not
  run.
- Corrected: `LoweredCrashExit` (scalar_lowering.rs:471) was "suspected unused; not confirmed" in
  record 070. Checked: it is used, constructed at proofs.rs:4552 and returned from the function at
  4491. `PreparedNormalCleanupAction::discard_root` (unit.rs:3488), not grepped by record 070, has 1
  caller (scalar_computations.rs:2163).
- One-caller functions (readers' counts): `lower_float_meaning_equality` (machine_lowering.rs:1152);
  `lower_boundary_crash_routes` (attached_unit.rs:327); `access_lifecycles::storage_fields`
  (attached_unit.rs:15425); `forward_private_state_borrows` (unit_composed_control.rs);
  `descriptor_residences` (machine_lowering.rs); `atomic_source_range_decided`
  (unit_operation_frame.rs:476); `validate_short_circuit_expression` (scalar_lowering.rs:311);
  `lower_dynamic_dispatch_catalog` (machine_lowering.rs:2035); `whole_home_disposition`
  (unit_composed_control.rs:5983); `drop_removed_positions_across_edges`
  (07/scalar_and_value_passes.rs:282); the four composed-graph admission passes
  (unit_composed_control.rs:1346-1355).
- Debug hooks that read the environment inside product code (recorded, not unused):
  `OMEGA_COMPILER_CHECK_SITES` at machine_lowering.rs:2075, unit_arguments.rs:5420,
  unit_operation_frame.rs:7065; `OMEGA_TRACE_NATIVE_BOUNDARY` at machine_lowering.rs:1368;
  `OMEGA_STATE_REFERENCE_DIAGNOSTICS` at unit_composed_control.rs:8861.

## Stated limits

- lib.rs:14-16: "any construct without an explicit checked plan fails closed immediately".
- machine_lowering.rs:1516-1520: "machine signature is outside the current terminal-Psi source
  slice"; 1681: "physical layout applications require strict target closure before Terminal
  production".
- attached_unit.rs:5659-5661: "This consumer selected a smaller residual schedule. Keep its old
  strict history checks"; 5784-5787: "Source admission retains all authored exits. Only the checked
  control plan selects runtime returns"; 1124: "scalar helper is absent from the Unit machine
  closure".
- emission_cleanup.rs:4727-4730: "continuation specialization must precede source certificate
  retention"; 3906-3909: "continuation specialization must precede selected evidence retention";
  3444: "the root carries linear custody the window does not restore"; 1895-1899: "cyclic owned
  scalar graphs require exact structural forwarding custody".
- emission.rs:11565: "quotient invocation requires an unimplemented operand or contract lane".
- expression_bindings.rs:2749-2752: physical argument paths "must never implicitly dereference a
  descriptor value or a nested reference field"; 2613: "a runtime-index projection is lowered by the
  emitter that evaluates its index".
- expression_preparation.rs:3850: "qualified scalar references require their own transport
  judgment"; 1754: "structural equality is contract-only terminal vocabulary".
- proofs.rs:4322: "content conservation requires its own call correspondence"; 4516: "an explicit
  crash in the terminal-Psi source slice requires exactly one prechecked covering route bucket";
  5543: "outcome-specific evidence interface reference scopes are unimplemented"; 7898: "structural
  crash Exact shift requires explicit terminal count and overflow safety evidence".
- retention.rs:1352: "closed reach executable proposition argument is unavailable"; 3187: "operator
  crash contract requires an all-scalar operator signature"; 4682: "retained-borrow result is not
  the bounded nominal lifetime carrier". The Terminal vocabulary has explicit `Unresolved` states for
  suspension loans (retention.rs:5369, 5393, 5547, 5648).
- scalar_computations.rs:2725: "scalar dispatch has no independently covered terminal arm"; 3477:
  "byte record store requires separate semantic field-range evidence".
- unit.rs:2551-2552: "A later completion can select only part of an earlier cleanup schedule. Detach
  that partial schedule explicitly; it cannot close the full carrier."; 1688: "guarded contextual
  cleanup requires an exact edge cleanup plan".
- unit_arguments.rs:4711-4715: "Absence means that producer has not completed; a retired or
  out-of-frame name is an error, never permission to produce it again."; 6922-6927: a runtime window
  argument "evaluates its start and end exactly once".
- unit_composed_control.rs:10194-10196: "Reservation is not establishment"; 7806-7809:
  "nonreturning state lost its evaluated invocation"; 1544: "captured state loan has an unbounded
  parent lifetime"; 5755: "Unit graph state loan chain storage cannot escape as an owned return".
- unit_operation_frame.rs:3489: "mutable scalar value refinements require invariant-window custody";
  5816: "a local byte field element has no store route"; 1125: "boundary calls cannot carry a module
  no-normal result".
- unit_structural_values.rs:3629: "structural dispatch needs selected floating comparison lowering";
  5547: "selection pass-through of partially moved roots requires retained residual frontier
  correspondence"; 2004: "boundary body input needs an exact original projection correspondence,
  not only joint ancestry".
- 08/lib.rs:12-15: "Source-entry receipt preparation and boundary result-policy verification remain
  separate from canonical artifact sealing".
- Readers' notes on house rules: `HashMap`, `HashSet` or `BTreeMap` keyed by dense ids at
  emission_cleanup.rs:35, 99, 249, 1616-1643, 1914, 1931; retention.rs:1877, 1889, 2855-2866;
  operation_emission.rs:3860, 3886; 74 lines in unit_composed_control.rs (reader grep). None of the
  readers opened `tools/dense_id_maps.txt` or `tools/cache_state.txt`.

## Disagreements

No second-pass records exist for this area. Between first-pass records, and between a record and the
code:

1. **Literal type-spelling lookups in attached_unit.rs.** Record 066 says "The one literal-spelling
   lookup is `"named(name(u64))"` (3435)". Record 070 says `BYTE_VIEW_ELEMENT_TYPE_IDENTITY`
   (`"named(name(u8))"`, shared.rs:1810) is used at attached_unit.rs:3567 through
   `type_identity_by_text`. Code: both exist (attached_unit.rs:3436 and 3567). Record 070 is right;
   there are two.
2. **Is proofs.rs:904 a fourth namespace allowlist?** Record 070 lists `validate_namespace` at
   proofs.rs:904 beside the three allowlists ("grepped, not read"). Record 069 read it as
   `integer_math::validate_namespace`. Code: it walks integer-math terms with a 4096 budget and hands
   each embedded scalar to a caller-supplied check; the other three call it
   (scalar_lowering.rs:1502-1503, unit.rs:188-189, 1176-1177). It is a shared helper, not a fourth
   allowlist. Record 069 is right; there are three allowlists.
3. **`LoweredCrashExit`.** Record 070 suspects it unused; record 069 lists
   `lower_checked_crash_exit` (proofs.rs:4485) without naming its return type. Code: that function
   returns `LoweredCrashExit` (proofs.rs:4491, 4552). Used.
4. **Where the `OMEGA_COMPILER_CHECK_SITES` hook is.** Record 072 cites it in its
   unit_structural_values.rs section at 7065-7069. Code: unit_structural_values.rs:7060-7072 has no
   such read; the hook is at unit_operation_frame.rs:7065. Record 070 says the variable is used "in 3
   other stage files" besides unit_arguments.rs:5420; code shows three stage 06 files in total
   (machine_lowering.rs:2075, unit_arguments.rs:5420, unit_operation_frame.rs:7065), so two others.
   Record 072's "14 files" matches a grep over all file types (14), 7 of them `.rs`.
5. **Service-ceiling error strings.** Record 066 says the two forms have "the same error strings
   (e.g. 4841 vs 14005 ...)". Code: 4843 reads "Unit contract reach does not match the exact checked
   transitive reach", 14006 reads "Unit contract reach does not match checked transitive reach"; the
   ceiling string is identical (4856, 14018). Same checks, one string differs.
6. **Callers of `lower_unit_closure`.** Record 066 counts callers of the two wrappers only. Record
   068 says `lower_selected_machine` calls `lower_unit_closure` directly. Code: five call sites
   (attached_unit.rs:16733, 16753; machine_lowering.rs:1491, 1946; unit.rs:2198). Both records are
   right as far as they go; neither gives the full list.
7. **Deferred `ReleaseReference`.** Record 066 (ordinary bodies, attached_unit.rs:11984) says the
   release is "presumably emitted at exit". Record 071 (composed bodies,
   unit_composed_control.rs:782-790 and 12629-12700) read the later pass and found three cases where
   it is never emitted. These are two different emit paths, so not a contradiction; record 066's
   "presumably" stays untraced for the ordinary path.

## What I checked myself

Opened at commit `457adf4a11f`; no build or run.

| # | Claim (record) | Where | Verdict |
| --- | --- | --- | --- |
| 1 | Closed Boolean or Integer clause becomes `Truth` (070, open) | scalar_lowering.rs:1761-1763 | matches |
| 2 | Authored requires are all-or-nothing on `all(Option::is_some)` (066, open) | attached_unit.rs:6290 | matches |
| 3 | Ensures rows with an unavailable contract result are dropped (066, open) | attached_unit.rs:541, 547 | matches |
| 4 | Borrowed byte view skips the capacity comparison (067, open) | emission.rs:8245-8253 | matches |
| 5 | Last dispatch arm is not tested (072, open) | unit_structural_values.rs:3593, 3616, 3637 | matches |
| 6 | Invalid-symbol owner returns `Ok(true)` (072, open) | unit_structural_values.rs:5442-5444 | matches |
| 7 | No transition binding returns `Ok(Vec::new())` (071, open) | unit_composed_control.rs:3564-3571 | matches |
| 8 | No contract plan returns no crash routes (070, open) | unit.rs:98-100 | matches |
| 9 | Float call with a non-Expression site is skipped (072, open) | 08/result_policy_verification.rs:334-336 | matches |
| 10 | No-op selections are recorded as executed (072, open) | 07/psi_optimization.rs:25-31, 48-51, 60-62 | matches |
| 11 | `block_dominators` with no entry is fail-open (067, open, callers unread) | emission.rs:1855-1858 | partly: branch exists; both callers pass `Some` |
| 12 | `verify_boundary_result_policies` has no caller (072) | grep, 4 hits in 2 files | matches |
| 13 | Three 08 error variants are never constructed (072) | grep, declaration only | matches |
| 14 | `InvalidDebugMap` (07) is never constructed (072, unit only) | grep | matches, and holds across omega-rust |
| 15 | `LoweredCrashExit` suspected unused (070) | proofs.rs:4376, 4491, 4552 | does not match: used |
| 16 | `lower_unit_services_including` has 2 hits (066) | grep | matches |
| 17 | `continuation_identities` has 3 lines (067) | grep | matches |
| 18 | `call == false` arm has no caller (069, not grepped) | proofs.rs:7409-7414, 7596 | matches |
| 19 | Service ceiling written twice with the same strings (066) | attached_unit.rs:4834-4860, 13999-14024 | partly: same checks, one string differs |
| 20 | Parameter-kind codes differ between two sites (069) | retention.rs:1085-1096, proofs.rs:6877-6884 | matches |
| 21 | `StructuralLeafCopy` also built at two more sites (072, suspected) | emission.rs:11732, unit_composed_control.rs:9190 | matches |
| 22 | Two `selected_memberships` are related (072, suspected) | emission_cleanup.rs:2369, unit_operation_frame.rs:1995 | partly: related job, not copies |
| 23 | Four namespace allowlists (070) | proofs.rs:904-933 | does not match: three, plus a shared helper |
| 24 | Exactly four `PredicateTerms` impls (070) | grep | matches |
| 25 | One literal-spelling lookup in attached_unit.rs (066) | attached_unit.rs:3436, 3567 | does not match: two |
| 26 | Debug hook at unit_structural_values.rs:7065 (072) | unit_operation_frame.rs:7065 | does not match: wrong file cited |
| 27 | `discard_root` use unknown (070, not grepped) | scalar_computations.rs:2163 | 1 caller |
| 28 | `plan_case_payload_leaf` outside callers unknown (068) | grep, 2 hits in its own file | no outside caller |
| 29 | Every `.rs` line of the three crates is in a unit | `wc -l`: 126,063 = 123,583 + 1,812 + 668 | matches crates.json |

The digest text above already carries the corrections from rows 11, 15, 19, 23, 25 and 26.

## Not covered

- Records: none unread. All seven records were read in full. There are no `unit_NNN_gap_*.md` files
  for units 066 to 072, and no file in `gap_units` names a path in these three crates.
- Source: every `.rs` file under the three crates' `src/` is in a unit (19 + 4 + 2 files). Each
  record reports 0 lines skimmed and 0 not read. Not in any unit: each crate's `Cargo.toml` and
  `README.md`.
- What the readers said they did not do, which limits the lists above: callers outside each unit
  were not traced for any "open" entry; macro-expanded patterns in proofs.rs were not expanded
  (proofs.rs:1694, 4149, 4201-4216, 4236, 5368); the Terminal Psi validators and stage 05 producers
  (`call_requires_decided`, `authored_requires`, `owned_transfer_paths`) were not read; several
  use counts are per-file aggregates of combined grep patterns (record 067) or were cut by a result
  cap (record 066, `reference_results::` and `scalar_arrays::`); the shared.rs source-guard tests
  were not run.
- What I did not do: I did not compare the cross-record path walkers (job 1) or the liveness
  mechanisms (job 7) line by line, so those stay "suspected" as one job.
