# Area 14: Validation, second half

Entailment tail, declarations, domains, literals, places, program validation, quotients, custody, traits, terminal installation.

Source: nine first-pass records, `unit_097.md` to `unit_105.md` (2,874 lines, all read in full). There are no second-pass (`_gap_`) records for these units. Code checked at commit `457adf4a11f` in `.agents/worktree/worker-23`. Nothing was built or run.

File abbreviations used in citations (all under `omega-rust/psi/compilation/validation/src/` unless marked `ti-`):

| Abbr. | File | Abbr. | File |
| --- | --- | --- | --- |
| ce | contract_entailment.rs (lines 72849-91057 only) | pv | program_validation.rs |
| dc | declarations_core.rs | pcc | proof_contracts_core.rs |
| dp | domains_and_predicates.rs | q | quotients.rs |
| ei | effect_inference.rs | rc | recasts.rs |
| lib | lib.rs | rrc | reference_result_custody.rs |
| fz | frozen_program_and_queries.rs | scc | structural_call_custody.rs |
| fl | float_and_mathematical_contracts.rs | tf | trace_flags.rs |
| etf | expression_types_and_fields.rs | tr | traits.rs |
| lsl | literals_and_struct_literals.rs | trw | type_references_and_wire.rs |
| mcc | machine_calls_core.rs | vcc | value_custody_core.rs |
| mpb | machine_parameters_and_bindings.rs | ti-lib | terminal-installation/src/lib.rs |
| ops | operators.rs | ti-ev | terminal-installation/src/evaluation_and_synchronous_calls.rs |
| pl | places.rs | ti-prov | terminal-installation/src/provider_and_target_installation.rs |

"Record" below means one reader's notes file. Words such as "suspected", "inferred", "not traced" are the readers' own and are kept.

---

## What this area does

This area is the back half of the crate that decides whether a typed Omega program is acceptable before it becomes Checked Psi. It takes the typed program tables (expressions, statements, types, machines, traits, data definitions) through a read-only view called `CheckingInput`, runs a few dozen independent validators over them, and produces either diagnostics or a bundle of facts that later stages rely on (integer range obligations, operator selections, place and loan plans, effect tables, quotient judgments). A small second crate, `terminal-installation`, does a different job on the far side of the pipeline: given an already-built Terminal Psi artifact with no source, it decides whether a provider, a target layout, a private entry RAM image or a build-time evaluation may be installed. Almost all files are a "bag of modules": one file holds 10 to 30 unrelated `pub mod` blocks.

### Crate `psi/compilation/validation` (22 of its 29 source files are in this area)

- **Entailment tail** (ce:72849-91057). In: one machine, its `requires`/`ensures` facts, and the flow storage graph. Out: which `ensures` facts are proven (`ProvenContractExpressions`) or diagnostics. Holds the proof-term judge (`StructuralJudge`, ce:80570), its term type (`StructuralTerm`, ce:78886), the model of stored values across calls and writes, and the entry point `validate_machine_contract_entailment` (ce:90324).
- **Declarations** (dc). In: declarations and authored name selections. Out: diagnostics for visibility, package authority, state signatures, duplicate names, terminal calls, type aliases; a plan of definition-site facts (`build_definition_fact_plan`, dc:364).
- **Domains and predicates** (dp). In: operator expressions, data `where` facts, domain definitions. Out: yes/no answers on "does this operator mean the built-in one" (dp:430-1662), a bounded evaluator and small prover for predicate machines over integer collections (dp:1664-7276), the default-domain "invariant window" tracker that rejects reads of a value before its `where` facts are re-established (dp:7278-13766), and domain declaration checks plus runtime predicate builders (dp:13769-15927).
- **Effect inference** (ei). In: machine/state/call graph. Out: four fixed-point tables: which services a machine may invoke, may-suspend/may-block, and service reach (ei:330, 1126, 2319).
- **Crate root and frozen program** (lib, fz). `lib` is mostly re-exports but also holds build-time declaration checks, product entry selection and the native-binding vocabulary (lib:192-1430). `fz` defines `CheckingInput`, the about 55-slot memo table (fz:1198-1439, counted by eye by the reader), program projection (copy a typed program and keep a subset, fz:1667-2396) and shared expression walkers.
- **Floats and mathematical terms** (fl). In: operator contracts. Out: recognition of the sealed toolchain float operations and of proof-only mathematical applications.
- **Expression types** (etf). In: an expression. Out: its type reference (`result_type`, etf:4831), the central "does this argument fit this type" test (`argument_matches_type_reference_using`, etf:6712-7182), and mismatch diagnostics.
- **Literals and construction** (lsl). In: literals and struct literals. Out: landed numeric literals (this rewrites the typed tables at lsl:350 and lsl:2921), suffix checks, construction diagnostics, stored-integer-write rows and pending predicates.
- **Machine calls** (mcc). 18 unrelated modules: toolchain primitive recognisers, boundary call checks, contract identity bytes, "closed value call" decisions, effect ceilings, overload resolution by result type (rewrites `target_symbol`, mcc:7581).
- **Static machine arguments** (mpb), **operators** (ops), **places** (pl). `mpb` admits callables passed as compile-time machine parameters. `ops` resolves which operator a use means. `pl` turns expressions into root-plus-segments paths (`CanonicalPlace`, pl:5465) and answers overlap, type-along-path and member-symbol questions.
- **Program validation** (pv). The coordinator: `validate` (pv:4067) runs about 40 prologue validators (pv:301-587) then per-machine statement validation in parallel; `judge_contract_entailment` (pv:4668) runs entailment later and can reuse earlier judgments.
- **Proof contracts** (pcc). 21 unrelated modules: contract owners, domain weakening, erased-binding fences, properties (`copy`, `linear`), proposition entailment, embeddings.
- **Quotients and recasts** (q, rc). `q` verifies quotient declarations and builds a correspondence judgment for each `Quotient::lift`/`define` call. `rc` judges byte re-views (`&x as &T`) and exports the record layout inventory later stages use.
- **Custody and traits** (rrc, scc, tr, vcc, trw). `rrc`: which input storage a returned reference borrows from. `scc`: claim and loan plan for calls on structural values. `tr`: traits, conformances, dynamic dispatch. `vcc`: about 40 modules on what a type contains, claims, cleanup, atomics, locals. `trw`: type-reference validation and wire schema compatibility.

### Crate `psi/compilation/validation/terminal-installation` (3 of its 4 files are in this area)

- In: a `TerminalArtifactView`, target facts, provider selections. Out: `Admitted*` records or a `String`/enum error. `ti-lib`: private entry RAM recipes (ti-lib:210-400) and target layout selection (ti-lib:573-672). `ti-ev`: whether a build-time invocation may run (crash routes provably impossible, terminating closure, package custody; ti-ev:830-974) and the synchronous call graph. `ti-prov`: provider selection, assumed bindings, component providers, target machine family members.

---

## How it decides

**Identity (symbol handle, arena handle, normalised type identity).** Case membership meaning (dp:1073-1126), storage coordinate disjointness (ce:76895), selection authority by package identity (dc:988-1068), overload identity (mcc:827-883), nominal admission (mpb:2621-3268), operator candidate choice (ops:1728-1770), place overlap (pl:628-710), returned-reference origins (rrc: handles and `CheckedMemberIdentity`), claim plans (scc), partial-affine residuals (vcc:6349-7215, type identity ids), private entry RAM (ti-lib:217-389, "Keyed on identities and byte extents, not on names").

**Place (root plus path segments).** `current_value_valid` (ce:73351-73532), store preservation (ce:76735-76752), `canonical_place_*` overlap and agreement (pl:443-761), membership evidence (vcc:6145-6235), release boundaries (rrc:9285-9390).

**Value (intervals, closed constants, bit patterns).** Float bound worlds (ce:77459-78168), collection predicate exploration (dp:1685-2810), construction bounds (lsl:3003-3404), recast representation sets (rc:4583-4994), crash-guard evaluation (ti-ev:274-331), offset bounds (rc:1229-1397).

**Position (ordinal in a list).** Service reach pairs operational calls with its own work list by iteration order (ei:2550); entailment reuse re-materialises proven facts by ordinal among `Ensures` (pv:1399, 1413); contract binders by position (pcc:1672-1675); callable shape by positional zip (mpb:431-654); the range-endpoint recogniser needs exactly three parameters with start at 1 and end at 2 (pl:2002-2096).

**Value equality of whole tables as a cache key.** Effect plan reuse (ei:2220, 2235; fz:1442-1453); prologue obligation overlap (pv:632-641).

### Spelling-keyed judgments, grouped

What I counted: one site = one cited line or contiguous range where a record says the judgment compares a name string, display string, formatted identity string, toolchain file path or source text. Left out: duplicate-name diagnostics where the name is the thing being checked, diagnostic wording, and trace filters. Total for the validation crate files in this area: **247 sites**. Terminal-installation: **7 string compares** (4 on the target tag, 3 on identity text; none on source names).

| File | Sites | File | Sites |
| --- | --- | --- | --- |
| dp | 50 | ce (tail) | 11 |
| pcc | 27 | fl | 9 |
| vcc | 27 | dc | 8 |
| trw | 24 | mpb | 8 |
| mcc | 18 | q | 8 |
| pl | 15 | lib | 7 |
| ops | 7 | tr | 6 |
| etf | 5 | fz | 4 |
| pv | 4 | ei | 3 |
| lsl | 3 | scc | 2 |
| rc | 1 | rrc | 0 |

The groups:

1. **Compiler-owned declaration recognised by toolchain file path plus a name or `"toolchain::…"` identity string.** `vec.omg` + `"Vec"` (dc:1711-1712); `"Console"` trait for completion discovery (dc:1594); `core/external_binding.omg` + `ForeignBinding`/`DllImport` and their case, field and parameter names (lib:977-990, 1177, 1205-1210, 1286, 1332); `extent.omg` (lib:840-857; vcc:8830-8881 with `"Extent"`, `"Granted"`, `"Resident"`); sealed float sources and `"toolchain::FloatFormat::BINARY32"` (fl:717-727, 730-754, 1061-1090, 1191-1214); `assembly.omg` + `"toolchain::MachineControl"`/`"toolchain::PortIo"` (mcc:56-84, 1242-1248; mpb:4114-4122); `code_segment.omg`, `data_segment.omg`, `task_register.omg` with ten identity strings (mcc:1404-1429, 2251-2255); `static_interrupt.omg` plus byte equality with an embedded copy of `interrupt.omg` (mcc:1668-1701, 2016-2035); `<build-prelude>` + `"Build"` (mcc:2676-2692); `reflect.omg` + five function names (mpb:3397-3407); `assembly_model.omg` + `"port_written"`, `"model_specific_written"`, `"atomic_release_written"` (pcc:5717-5762); `relation.omg` + `"Equivalence"` + parent names `Reflexive/Symmetric/Transitive` (q:416-450); activation and atomic-fence identity strings (vcc:28-37, 100-109, 601-610, 616, 639); `layout.omg` (vcc:875-881).
2. **Hosted intrinsics by machine and data names.** `"ConsoleNativeProvider::exit_process"`, `"…::write_byte"`, `"…::read_byte"`, `"ProcessExitNativeProvider::exit_process"`, `"ByteRead"`/`"Eof"`/`"Byte"`/`"value"` (vcc:5107, 5172-5173, 5207-5219, 5262-5289).
3. **Call target or member spelled a certain way.** `BuildOperation::from_call_target` (mcc:936, 957, 1037; mpb:4770-4785); `"address_of"`, `"asm#"` prefix, byte-predicate names (mcc:2636-2655); `"encode"`/`"decode"` + schema name (mcc:2582-2593); `"embed"` (pcc:6201, 6368, 6411); `.length`/`.capacity`/`"len"` (dp:6479-6481, 11618-11639; pl:823, 2071); `"self"` (dp:8805-8811, 11794, 14021); `PreservingDecode`/`decode_preserving`, `FormatMigration`/`migrate`, `"compact_binary"` (trw:5633, 5777-5780, 5980-5983); `"from_f32"`-style conversion members and owner names `"I8"…"F64"` (mcc:8915-8975; ops:2147-2166); float catalog row names `"convert"`, `"divide…"`, `"add"`/`"subtract"`/`"multiply"` (fl:103, 445-451; pcc:10911-10915); `"Float"` namespace (fl:1392, 1419).
4. **Synthetic local names used as markers.** `"__destructure#"` and `"__arm_destructure#V="` carry the pattern for exhaustiveness (vcc:4262-4289, 4348); `"__value_index#"` (pv:3099); `"__transition_subject"` (pcc:9875).
5. **Place identity as a string.** The whole default-domain tracker: `TrackedPlace.spelling`, `"[*]"` wildcards, prefix and segment string matching, field values by field name (dp:7399, 7751, 7794, 7826, 7948, 8148, 8177-8194, 9383-9413, 9984, 10015, 10038-10084, 11607-11615, 13001-13011, 13225, 13282).
6. **Term or fact identity as a rendered string.** `observation_identity_key` is `format!("{:?}")` (ce:79641); `Opaque(display_name)` terms compare by text (ce:89539, 89638, 89738, 79934-79938); static type predicates as `Variable("$static-type-predicate:…")` parsed back (ce:89419, 89429); machine identity `"{target}<{args}>"` (ce:89179); contract refinement compares rendered `requires`/`ensures`/`crashes` strings with textual substitution and `$index` renaming (mpb:2181-2260, 2456-2558, 2103-2154); proposition entailment on `BTreeSet<String>` labels (pcc:8946, 8971-8977); quotient proof-fact identity is a `(rendered, trace)` string pair and relation pairs use `"$left"`/`"$right"`/`"$other"` text (q:286-322, 3462-3637); domain facts normalised to `"bool:…"`/`"membership:…"` strings (dp:15774-15789); atom identities minted with `format!` (dp:4885, 10830, 11165).
7. **Name fallback when a symbol is missing or as a second route.** Members, fields, locals, parameters, data and traits looked up by name: dc:642-668, 799-835, 1353-1376, 2926; dp:9511, 9647, 10150, 10278, 11065, 11387-11399, 11807-11812, 11939, 12439, 12473, 14719-14723; ei:746-777, 935-938; etf:7842, 7915-7936, 7261-7288; lsl:5951, 6016; mcc:7083-7105, 8496-8499; ops:941-943, 964-965, 1671; pl:4160-4216, 4482-4531, 5283-5443, 6124-6146, 6589-6603, 7369-7415; pcc:2863, 2872-2892, 4070-4076, 4168-4170, 4281-4297, 5042-5062, 5419-5425, 6901-6903, 7308-7313, 7390-7407, 8032-8045, 8103-8106, 8826-8832, 10671-10689; scc:897-899, 1359-1363; tr:349-364, 2723-2726, 2936-2951, 5356-5369; trw:2234-2243 and the repeated `!has_source_metadata() && name ==` fallback at trw:1488, 1554, 5195, 5205, 5231, 5374, 5422, 5454, 6752; vcc:989-993, 1699-1718, 2012, 3957, 5593-5638, 5653-5693, 7581-7582, 7931-7938, 8107-8119.
8. **A number read from a type name's text.** dp:11829-11835, 13841; lib:1407-1426; pl:1619-1623; tr:641-644; trw:2186, 2219; vcc:463-466, 3190.
9. **Primitive or domain recognised by name.** `PrimitiveType::from_name`: ops:1816; q:4740-4745, 4803, 5551-5553; rc:4669-4672; trw:2839-2880, 3921-3939; cast target by spelled name (etf:66-139); cast domain by name (ops:1620-1643; vcc:7931-7938); `CarryPermission::from_name` and `"Carry::"` (dp:15479; pcc:5855, 7382-7388, 9628-9635; trw:721-729); `"copy"`/`"linear"` (pcc:8306-8488); `"finite"` (trw:707); `"Rat"` (trw:2547); `"drop"` (vcc:2215).
10. **Source text or diagnostics text as a key.** Entailment reuse hashes the source text of statements and contract facts (pv:1216-1276) and finds priors by identity string (pv:749-753, 987-992); exact-overflow errors are picked out by message text (pv:4645-4649); const struct-literal diagnostics are kept only if the text contains `"evaluates FALSE"` (lsl:5543-5546); refinement fit depends on whether a keyword source span was recorded (tr:7490-7505); wire fields compare by `type_display` string (trw:5904-5920).

---

## The same job in more than one place

Each entry: job, sites, how the copies differ. "Opened" means I read both sites for this digest.

1. **Recognise "the" compiler-owned declaration in a toolchain file.** At least 16 hand-written recognisers (group 1 above). Three sub-pairs opened:
   - *Core `MachineControl` service of a machine*: mcc:1228-1255 and mpb:4098-4134 (neither record names the other). Same job. mcc requires the row to hold exactly one service and compares with the literal `"toolchain::MachineControl"`; mpb scans all services on the row, compares with the catalog's `required_service_identity()`, and errors on a repeat. A third selector over the same file is `AssemblyServices::new` (mcc:54-84).
   - *"Declared in toolchain `extent.omg`"*: lib:840-857 and vcc:8871-8881. Same test; lib also requires `DependencyScope::Product` and filters traits by name, vcc starts from a symbol.
   - *Plain boundary-declaration shape lists*: mcc:1185-1206 vs mcc:1708-1755 (same machine fields, two hand-written lists, record 100); vcc:39-65 vs vcc:5224-5249 ("plain data" list, record 105); vcc:28-37 vs vcc:601-610 (sealed identity).
2. **Unique data definition for a symbol.** fz:3989-4002 (own scan), etf:4428-4437 (inline scan), pl:8659-8664, typed-trees `declaration_tables_and_index.rs:1850` (not read). Record 99 "suspected identical". Opened: pl:8659 is a one-line forward to `program.unique_data_definition_by_symbol`; fz and etf are separate scans with the same "exactly one" rule. So two hand copies plus one forwarder of the typed-trees version.
3. **Type of a field of the machine's attached data.** etf:7842-7858 (by name path, `data_definition_by_name_str`), pl:8500-8514 (by field symbol through the attached application or unique definition), `calls.rs:41556-41566` (by symbol through `program.attached_data_definition`). Opened all three: same job, different key. Record 99 cites the places.rs copy at 8354; it is at 8500.
4. **Expression to type.** etf:4831 `result_type`; ops:755-906 `expression_type_reference_in_state` (partial, returns `None` for Boolean/Integer/Range/String/Unary, ops:900-904); pl:7457 (same name, built from canonical places); pl:1275, 4274, 7002; pcc:4101-4267 `infer_expression_type`; dp:381 and dp:1266 operand typing. Records 98, 101, 102.
5. **Walk a place-segment list over types.** Six cursors in pl (2784, 7123, 7632, 7856, 8218, 8387); three more path translators at rrc:6752-6817, rrc:8503-8636, scc:3256-3334. Records 101, 104.
6. **Field or member lookup by symbol or name.** pl:3443, 4195, 4482, 5369, 7369, 8666; etf:7705, 7884, 7915, 7936; dc:35; lsl:6010 (`data_declares_field`, also used at pl:1121); scc:897; pcc:4270-4311. Records 97, 99, 100, 101, 102, 104.
7. **Walk every call site / every expression.** Four full call walks in mpb (4461, 6174, 6220, 7570); three call finders in dp (8288 hand-written, 11990 `visit_pre_order`, 13163 via `children`); `scan_expression_use` (dp:8787-8967); the shared walker `CallSiteVisitor` (ei:14) has users only inside ei (12 line matches); three "visit once" walkers in fz (2698, 2721, 2841). Record 98 marks dp:8288 a "suspected duplicate of `expression_traversal::children`".
8. **"Where does a value land in a destination type."** lsl:442-528, lsl:2328-2478, lsl:2816-2910, mcc:8487-8611. Record 100.
9. **Reachable-machine closure.** mcc:4875, 4193-4268, 5144-5155, 1919; ti-ev:984-1033 vs ti-ev:1226 (proof scope vs Terminal machines). Records 100, 105.
10. **Strongly connected components.** mcc:6872; suspected duplicates at `omega/.../01_abstract-operations-to-abstract-operations/src/lib.rs:199`, `05_…/shared.rs:3686`, `05_…/checks_termination.rs:365` (not read by the reader, not opened here).
11. **Owner of a contract expression / owner environments, kept twice on purpose.** Raw scan pcc:3086-3359 vs index pcc:3483-3912, with a test asserting they agree (pcc:4862); memoised vs scan owner lookup (pcc:836-864 vs 1175-1197).
12. **Operator resolution for a binary use.** ops:1227, ops:2426, ops:1887-1967. F32/F64 policy-adapter mapping three times (ops:1124-1132, 2167-2181, 2600-2636).
13. **Product declaration selection by name within a package.** lib:601-685 vs fz:3512-3741 (same filter conditions, separately written, record 99 read both).
14. **Conformance signature comparison.** tr:863-1159 (top-level requirement) vs tr:2183-2542 (trait requirement): lifetime counts, kinds, `TraitTypeBinding` vectors built twice (tr:962-972, 2335-2363).
15. **Purity lists for an "admitted predicate machine".** dp:1868-1879 and dp:4829-4841 "the same purity list … repeated by hand"; two evaluators of the same bodies with budgets `1<<14` (dp:1685) and `8192` (dp:4499).
16. **Fixed-point over predecessors.** dp:12655-12707 and dp:12768-12881; the family/published/ceiling scaffold in all three effect inferences (ei:550, 1161, 1504, 2694, 2730).
17. **Type-shape walkers with their own unresolved-name rule.** vcc `has_contents` (9944) is the shared one; alongside it `append_linear_claim_frontier` (vcc:1354), `type_graph_requires_nominal_drop` (vcc:1959), `type_requires_establishment_inner` (vcc:3984), `carries_activation_claim` (vcc:521), `static_inline_storage::Search` (vcc:9546); rrc:6885, 6988, 7645; pcc:157 `first_forbidden_content`; scc:2754.
18. **Primitive identification.** q:4803-4814 (spelling only) vs rc:4512-4538 (builtin atom of the symbol and spelling). Opened: same job, different key.
19. **Byte-sequence carrier test.** etf:1768-1783 and etf:8646-8663. Opened: same body; the last line calls `program.primitive_type_reference` in one and `type_reference_table.primitive_type` in the other.
20. **Byte tags for `TypeParameterKind`.** mcc:2970-2977 (Type 1, Const 2, Machine 3, Proposition 4, Value 5, Implementation 6) vs mcc:3979-3986 (Type 1, Const 2, Value 3, Machine 4, Implementation 5, Proposition 6). Opened: as the record says.
21. **Smaller repeats the records list.** `integer_carrier` twice (dp:324, 1715); three string builders for one place key (dp:7751, 9984, 10015); float-format `match` five times (fl:92, 147, 402, 1261, 1300); "result `==` call" scan three times (fl:188, 349, 575); six comparison-operator lists (lsl:414, 610, 816, 2530, 2555); `symbol_key` twice (mcc:402, 4348); `LocalWriteSource {…}` literal six times (rrc:1288, 1373, 2019, 2120, 2810, 2911); three per-type `OnceLock` memo tables (vcc:1861, 10873, 10925); three i128 range checks (trw:2945, 2996, 3921); `declaration_parameters` twice with different return types (trw:191, 3067); `validate`/`admit_prepared_program_effects` repeat the effect-gated validators (pv:4839-4904 vs pv:349-532, 4146-4224); range-check sequence four times (pv:1810, 2186, 2493, 2907); family-head test three times (ti-prov:915-920, 991-993, 1089-1095); `align_up` helpers elsewhere "suspected duplicates of rc:3950; bodies not read"; `integer_literal_fits` (q:5065) vs `03_…/equality_and_exhaustiveness.rs:2416` "suspected"; trace-flag env reads bypass the cached functions at 38 lines outside tf.

---

## Fixed lists of shapes

Quoted comments and messages are from the code as the records cite them.

- **Entailment.** `unfold_application` needs a machine with exactly one state (ce:82192); `direct_literal_call_field` handles only a single-state, single-statement machine whose value is a struct literal (ce:81989-82042); `exact_store_coordinate` handles zero or one write destination (ce:76675-76714); stored integer widths 1..63 (ce:77127-77133); depth and budget caps of 32, 64, 128 throughout (ce:77459-77465, 78000, 81568, 86856); "Opaque and source-selected operations have no builtin arithmetic law" (ce:87058-87059).
- **Collection predicates.** Statement shapes: immutable local, ordinary transition, final expression, else `None` (dp:2235-2283, 4796, 6025); binary arithmetic only `Add`/`Subtract` (dp:2757-2809); budgets 4096 extent, 64 ranges, 64 clauses, 256 splits, depth 64.
- **Where-fact intervals.** Only `field <op> literal` with `<= < >= > ==`, or an exact predicate application (dp:12338-12390, 12205).
- **Native binding vocabulary.** Exactly 3 width parameters, 4 `DllImport` cases, 2 `ForeignBinding` cases, hard-coded unused-coordinate masks (lib:1064-1098, 1171, 1211, 1272-1280).
- **Projection roots.** `project_roots!` over 12 named tables plus machines; any other root table "is copied unfiltered" (fz:1936-1980).
- **Argument fit.** Final `match` on required type kind lists accepted expression node kinds per kind (etf:6971-7181). Messages: "arrays do not support `==` / `!=` yet" (etf:2070-2071); "match structural, range, and binding patterns are not supported by value dispatch yet" (etf:1233).
- **Literals.** Closed primitive arrays: Bool, I8-I64, U8-U64, F32, F64 only (lsl:994-1004); `construction_conditions` only `<= < >= > ==` (lsl:3861-3868).
- **Toolchain primitives.** `plain_primitive` U8/U16/U64 (mcc:1267-1272); eight selector identities (mcc:1404-1429); "descriptor must be exactly [u8; 10]" (mcc:2763-2779); static interrupt Install needs exactly six parameters (mcc:1788-1806); fact projection "sum, mixed, empty, and opaque result shapes are not admitted" (mcc:6171-6180); ten primitive conversion owners (mcc:8961-8975).
- **Static arguments.** "has a lifetime, machine, or proposition telescope whose explicit application is not yet supported" (ops:87-89); `closed_static_call_type_bindings` only Type/Machine parameters and a short literal list (mpb:1652-1831).
- **Places.** `observable_scalar` 13 primitives (pl:3396-3414); element steps only on literal-length arrays with constant index (pl:3276-3322); `zeroed_structural_parts` walks only the first variant (pl:5169-5194); `mutable_byte_slice_parameter_element` only `&mut [u8]` (pl:2398-2423).
- **Program validation.** Atomic fetch operators `FetchAdd/Sub/Xor/Or/And` (pv:1680-1691); `integer_establishment` only assignment or atomic-once local (pv:2554-2568).
- **Proof contracts.** Erased data: "erased-stripped runtime support for {shape} is not implemented yet" (pcc:10204); "this slice requires a closed monomorphized instance" (pcc:10137); slice ranking accepts only `s[k..]` under `s.length > b` or `>= b` (pcc:11323-11371); scalar boundary guarantees recognise only float add/subtract/multiply/divide with operands `[Format, Meaning, Meaning]` (pcc:10911-10952).
- **Quotients.** Role collection: Define `[Congruence]`; Lift one or two theorems (q:7503-7528); `ClosedLiftLiteral` 13 variants and a long per-shape list (q:4492-4775); single-state machines, empty static telescope, immutable parameters (q:2508-2536).
- **Recasts.** "deeper shapes land with the byte-view rung" (rc:1871); "record sources and temporaries land with the byte-view rung" (rc:1896); budgets 4096 nodes, 16384 edges, depth 256 (rc:2355-2357, 2772-2773); phantom lifetime shell depth 2 (rc:2357); offset algebra only Add/Subtract/Multiply/ShiftLeft (rc:428-496).
- **Reference custody.** `select` handles a listed set of structural value kinds, else `None` (rrc:67-349); `NativeCodeLayout` only the second declared field (rrc:152-170); limits 4096 leaves, 8192 visits, depth 128 (rrc:6827-6829).
- **Structural custody.** `carrier_bounds` I8-I64/U8-U64 (scc:770-782); "projected qualification array length is not literal" (scc:2898).
- **Traits.** "named-witness conformance currently requires a concrete non-generic trait, requirement, and satisfier" (tr:1608); "this first conformance rung is concrete only" (tr:1729); "the first evaluated-binding rung requires its body-bearing, non-generic, zero-parameter entry state" (tr:341).
- **Type references and wire.** "only direct const binders combined with `+`, `-`, `*`, or `/` are in the proof-static algebra fragment" (trw:3894); const predicates: integer/boolean fragment only (trw:305-504).
- **Value custody.** Five activation operations (vcc:100-112); three fence cases in fixed order (vcc:649-668); compare-exchange recognised as the tree `prior + (prior == expected) * (replacement - prior)` (vcc:1112-1133); plan-laid `abi_version == 4`, at most 32 fields and 32 retired identities, stored width 1..=64 (vcc:7486-7501, 7619-7642); 13 `ContentsRequirement` kinds with per-kind `matches!` lists (vcc:9928-10450); logical record match = single arm, record, no guard (vcc:5892-5908).
- **Terminal installation.** "private entry RAM currently constructs plain scalar records" (ti-lib:281); fields only Boolean/Integer, "requires ordinary whole-byte integer fields" (ti-lib:292-300, 337); byte count 1..=65536 (ti-lib:262); executable entry only `(Machine, CheckedBody|Boundary)` or `(Boundary, non-CheckedBody)` (ti-prov:926-942, 1036-1051); "component result requires projected membership transfer" (ti-prov:673-676).

---

## Accepts when it does not know

Classes are the readers' own and are kept apart. "Open" means the reader found no reason at the site and did not trace callers. No reader in this area showed a wrong program reaching any of these sites; where a record names the wrong program it thought could get through, it is given after the dash.

### Open (185 bullets; each bullet is one record entry, some covering two or three cited lines)

**ce (10)**
- ce:73357-73359 `current_value_valid` returns `true` with no storage graph — every stored observation treated as current. Sibling at ce:75288 passes a version in graph mode; "Not traced beyond that."
- ce:76996 `entry_storage_observations_preserved` true when the judge is not a value-preserving one — a non-runtime judge never refuses on staleness.
- ce:83286-83287 `!P` is `Proven` when `hypotheses_contradictory` — any negated proposition; where the flag is set "not traced".
- ce:85364-85368 `visit_non_contradictory_arm` does not check `integer_premises_contradictory` — an arm with contradictory integer premises reaches the visitor.
- ce:88447-88449 `scalar_operand_is_observed` true for non-reference declared types — an `Opaque`, `Identity`, `LogicalNot` or `MathematicalBinary` term as a scalar operand.
- ce:89539, 89638, 89738 unsupported names, calls, members become `Opaque(display_name)` — "two distinct places or calls with identical display text inside one frame" compare equal.
- ce:90209-90216 `premises_prove` — contradictory hypotheses prove any transparent application.
- ce:90730-90736 with ce:91002-91007 an `ensures` equal to a `requires` is proven if its calls are "declared" — a call whose target resolves by name only (see check 8).
- ce:90957-90963 engine `Unknown` outside the arithmetic language on an empty-bodied machine yields neither proven nor diagnostic; "whether another stage rejects it is not traced".
- ce:90781-90784 contradictory hypotheses with a "mention" fact are recorded proven.

**dc (6)**
- dc:133-135 public-interface selection with an unresolved target skipped ("partially covered" by dc:929-932, 1009-1014).
- dc:998-1000, 1020-1031, 1064 no authority obligation for builtin and toolchain selections — any selection of a toolchain-origin declaration without a dependency edge.
- dc:933 `excluded` rows skipped — selections inside descriptor-skipped occurrences or pruned machines.
- dc:1106 `allows` returns `None` for `Absent`; "Not traced."
- dc:1646 `is_none_or` — generated or provenance-less traits count as product declarations for completion discovery.
- dc:3151, 3161 terminal result types outside Unit/primitive/data are not compared — reference, slice, array, generic or trait results.

**dp (22)**
- dp:1478 `constant_subtree_meaning(..).unwrap_or(true)` — a non-constant arithmetic operand is not checked by this helper.
- dp:582-588 `And/Or/Bitwise*/Shift* => true` with no operand-type check — operands whose type has a user operator for that spelling.
- dp:683 any leaf node kind counts as meaningful.
- dp:877-892 `.all` over zero authored selection occurrences passes.
- dp:1035 returns `Some(selected)` when the subject type is not found.
- dp:5043 and dp:6250-6253 a comparison with no complement is "negated" to itself (reader: "not reachable from what I read").
- dp:6605-6611 a literal with no landing is accepted as the declared primitive.
- dp:7502-7504 no call-frame resolver: origin-based field stores skipped silently.
- dp:7976-7987 store to a field no fact names returns before any check.
- dp:8005-8049 requirement filter drops requirements judged disjoint from the store.
- dp:8982-9019 `validate_qualified_place_read` six silent returns.
- dp:8966 `_ => {}`: reads inside unlisted node kinds are not scanned.
- dp:9939-9941 every state parameter counts as established whatever its type.
- dp:11400 unresolved transition targets add no edge — a missing predecessor makes the target look more established.
- dp:11807-11812 — a case path that resolves to a different variant with the same spelling evaluates true.
- dp:12902-12904 with dp:13231-13233 — "a callee that leaves an invariant window open on its own parameter, called once with a tracked place that the re-walk proves and once with an untracked place".
- dp:13689 — a state with any crash transition skips the end-of-state window check "even if the crash is guarded and the state can also fall through".
- dp:13685 other statement kinds neither clear nor poison tracked places.
- dp:12681-12683 with dp:11373 dropped edges enlarge a state's intersection.
- dp:14154-14195 membership carrier check accepts an unknown subject type and generic carriers.
- dp:14879 runtime float comparison builder does not call the meaning check ("open observation, not a confirmed gap").
- dp:8515-8516 `case_position` drops the owner index — variants of two data types with the same position compare equal ("Not traced").

**ei (6)**
- ei:976-984 (with 405-408, 849) — a caller passes an argument with no nameable origin to a callee that invokes that parameter's service; the caller's inferred set omits the service.
- ei:1450, 1473 no row means "no suspend, no block" — a call to a signature kind not in the row list whose signature says `suspends`.
- ei:1285 exact embed calls contribute no effect.
- ei:72 assembly statements contribute no calls.
- ei:2972-2982 — an assembly intrinsic whose service cannot be resolved contributes no reach.
- ei:3186 other target kinds yield empty reach (traced for ordinary machines only).

**lib, fz, fl, etf (26)**
- lib:172-174 invalid value handle returns no error.
- lib:773 `_ => {}` for other `ProgramEntryVisibleParameters` variants (enum not read).
- lib:940-951 parameters with an access-view plan never count as visible arrival parameters.
- lib:421-444, 493 incomplete recursive component skipped.
- lib:1056-1098 `None` on any failure; caller meaning not traced.
- fz:1751-1754 symbols with no provenance are always retained in a Build projection.
- fz:2090-2091 only "materialized" occurrences are removed from the descriptor set.
- fz:2450-2452 an invalid sub-expression counts as inert.
- fz:3334-3336 `layout_family_is_complete` is `true` when realizations are not `TargetInputs`.
- fz:4458 `_ => true` for unpaired type shapes — no region constraint and the match succeeds.
- fz:4537-4539 required domain obligation with no actual is skipped.
- fz:4147-4149, 4165-4167 `Ok(None)` means no region constraint at that slot.
- fl:491-498 any literal counts as an "input-only" term.
- fl:557-609 first matching equality accepted, later ones ignored.
- fl:1451-1459, 1523, 1525 equality with one non-projection side is ignored.
- fl:2381-2383 `logical_body` returns `true` on a cycle.
- fl:2633 `logical_expression` `_ => true` — a `Name` is admitted without checking it is a logical parameter or local.
- etf:305-312, 354-356 — a cast whose operand type cannot be computed gets no float-denotation check.
- etf:36-38 recasts return immediately from `validate_cast_types`.
- etf:484-492, 2117-2127 unknown operand or source type gives no error (no class given by the reader).
- etf:1444-1462 values with no declared type and no value class are compatible.
- etf:1270-1275 returns before compatibility checks when arms share a static type.
- etf:3078-3340 conflict reporters return "no conflict" on unreachable context, unlisted expression kinds, failed substitution, any `Reference`, unresolved carriers.
- etf:7030-7062, 7168-7178 — required `Slice`, `Generic`, `DynamicTrait`, plain `Named` accept by expression node kind alone, "e.g. a `Call` whose `constructed_value_type_reference` is unset".
- etf:685-704 any value pattern counts as total.
- fz:1097-1119 after `program_mut()` every `has_active_*` cycle cut is false ("Suspected unbounded recursion risk"; not an accept).

**lsl, mcc (17)**
- lsl:4229-4231 — a generic data literal with a valid applied type skips field obligations.
- lsl:5509-5547 — "a `const` whose struct literal has an out-of-range field, a duplicate field, or an unknown field, unless another pass reports it"; nothing is scanned when the program has no machine.
- lsl:3557-3559 no data definition: all `requires` facts skipped (reader infers unreachable).
- lsl:2761-2765, 2776-2778 — a suffixed literal landing in a non-primitive destination is not checked.
- lsl:350 unmatched float destinations are left unlanded.
- lsl:5247-5262 window validation returns silently when the shape is `None`.
- mcc:607-634 — a call whose flow row lost its boundary edge and whose target is a concrete implementation.
- mcc:366-400 "approved" boundary trait means unique name in module and package; no provider table read here.
- mcc:123-215 a generic-data method never selected is never bound-checked.
- mcc:1931-1933 an invalid dependency symbol is treated as closed.
- mcc:3158-3167 `.ok()` swallows the diagnostic.
- mcc:4767-4784 memo keyed on `(machine, state)` only though the result depends on the plans passed.
- mcc:5688-5712, 5594 — "a hosted-build program that places a port-I/O or IDT asm intrinsic in a `let` initializer or return expression".
- mcc:6359-6369 a call under Indexed/Binary/Unary/Match inside a member receiver is not seen.
- mcc:6611-6613 — "a runtime call to a proof machine written with a namespace receiver".
- mcc:7083-7105 no name match means no refinement check.
- mcc:8911-8913 — a synthesized cast with no span stays an unbound raw numeric cast.

**mpb, ops, pl (19)**
- mpb:4770-4785 unresolved target whose name string is a Build operation returns unchecked.
- mpb:4941-4943 an argument that names the demanded parameter itself skips validation.
- mpb:4028-4033 silent return for an invalid registrar application with open parameters.
- mpb:7679, 7923 incomplete tuples are not conformance-checked.
- mpb:7396-7401 `require_declared_application_operands(...).is_err()` returns `true`.
- mpb:6092-6094 skip when the applied signature cannot be rebuilt.
- mpb:6912-6947 `continue` after limited checks.
- ops:84-86, 107-109 `Ok(None)`: no closed application recorded, no error.
- ops:1753-1756 with ops:1816 — "A user type spelled like a primitive name with no declared candidate" takes the builtin fallback ("not traced whether `Named` names can be shadowed").
- ops:1939-1943 `continue` when no trait meaning and an operand is non-primitive.
- ops:2075 no fact when all operand types are `None`.
- pl:448-455 a write path longer than the observed path does not overlap the case tag.
- pl:708 `_ => false`: segments of different kinds count as disjoint.
- pl:6026-6029, 6585 other expressions become their own root, so `Disjoint` from everything.
- pl:1131-1139 with pl:4058-4121 unresolved receivers produce no "no field" diagnostic.
- pl:94-103, 220-230 chains longer than 128 judged not to write ("practically unreachable").
- pl:5169-5194 — "a data type whose first variant is zero-representable and later variants are not would be judged zeroable".
- pl:6124-6128, 6142-6146 carrier match falls back to name equality.
- pl:1618-1626 a `Named` reference with invalid symbol is read as a number from its spelling.

**pv, pcc (20)**
- pv:74-79 a state with a return type and no finite order is not checked for a missing return value.
- pv:4927-4929 cleanup target missing from the operational plan is skipped.
- pv:4555-4570 — "A program whose only errors match the message text would pass `validate` if a caller ignores the field" (`deferred_overflow`).
- pv:860-887 body reuse — semantic inputs outside the hashed source text "would not invalidate it"; which are missing "was not traced".
- pcc:217-221, 245-255 — "a quotient over a carrier whose type name resolves to something other than data or a data type parameter".
- pcc:795-801 unresolved or opaque proposition returns `true`.
- pcc:2056-2058 no atoms means no weakening check; pcc:2036-2043 early return on unresolved local initializer.
- pcc:3014, 3100-3103 members resolved against a partial owner set.
- pcc:5565-5572, 5686-5695 — "a name resolved to something that is neither a state local nor a state parameter … is treated as an immutable bound".
- pcc:5934-5953, 5959 — "a bound using a user-defined `&` or `<<` on a non-integer type would be treated as the builtin when this function is the only gate".
- pcc:6085-6144 `None` from the candidate check: neither admitted nor an error here.
- pcc:7515-7519 — "a fact whose expression is an integer-valued name or call" passes the boolean-shape test.
- pcc:7156-7160 argument/parameter zip does not report arity mismatch.
- pcc:8189-8195 — "a boundary-opaque data declared `[copy]` is accepted in the pending phase".
- pcc:8971-8977 — "two different expressions with identical rendering (… same spelling of shadowed locals) would be treated as the same fact".
- pcc:9095, 9109 — "a call whose target symbol resolves to none of these categories keeps its qualification facts".
- pcc:9684-9686 return type match ignores generic arguments.
- pcc:10571-10582, 10680-10682, 10714-10718 — an erased field reached through an unresolvable receiver is not reported.
- pcc:10464-10469 struct literal with no data definition: no erased-field check.
- pcc:10996-10999 emitted conclusion is `result == result`; consumption "not traced".

**q, rc (12)**
- q:477-479 — a dependency that resolves to no machine is skipped, "i.e. treated as checked".
- q:3586-3588, 3616 trace key omits Binary/Unary operators; sound if the rendered half includes them ("inferred, not traced").
- q:5551-5556 argument with no data definition accepted if spelled like a primitive.
- q:7540-7549 — "a quotient type whose definition is not in `data_definitions()` would be treated as an ordinary type".
- q:424-426 authority is path text `"relation.omg"`; whether a non-toolchain symbol can return it "was not traced".
- rc:1483-1490 returns with no diagnostic while layout geometry is pending; the re-run pass was not traced.
- rc:1795-1800 pending view accepted and checked at consumption; coverage of consumption paths untraced.
- rc:731-770 — "a reference to a pending view received from another machine is not charged here".
- rc:1902-1907 silent return if a scalar byte size is `None`.
- rc:2307-2321 only scalar-vs-scalar puns are reported.
- rc:4818-4820 no storage index means "no atomic storage", which disables the qualification-erasure check.
- rc:1028-1050 with rc:4660 non-`Domain` constraints give `None`, read as "no qualification issue"; an `ArithmeticDomain` constraint is ignored.

**rrc, scc, tr (13)**
- rrc:3354-3356 with rrc:4516-4535 `Image` sources count as not present so the leaf needs no loan; reader: "none identified for ordinary programs".
- rrc:8795-8799 invocation-rooted sources accepted without comparing any argument.
- rrc:7020-7022 — "a boundary/opaque data value that stores a reference would then not be a 'reference record'".
- rrc:7685-7687, 7742-7747 `NumericOwned` contents and activation byte carriers count as holding no references.
- scc:642-656 — "a data type with a `where` fact the spelling cannot express would get predicate rows silently missing for that fact"; the fence function has no production caller.
- scc:438 `is_none_or` returns `true` for a carrier with no closed data definition.
- scc:2953-2958, 2980-2987, 3017 — "an unresolved named type contributes no qualifications".
- tr:1517-1531 `AppliedTraitScope::new` returning `None` skips signature, evidence and law checks.
- tr:1718-1724 — "named evidence rows backed by expression/membership facts get no equality check".
- tr:3681-3697, 7907-7909 silent return on shape mismatch (open for data/conformance paths).
- tr:2912, 2933-2941 — "an implementation whose reference lifetime is unconstrained by the requirement gets no region check".
- tr:7397-7399, 7490-7505 — "a refinement whose clause signature loses `suspends` without a recorded keyword span would not reject a suspending realization".
- tr:362-363, 5366-5368 fall back to the first state on a name miss.

**trw, vcc (28)**
- trw:5633 — a demand requiring canonicality over schemas with no declared codec is satisfied.
- trw:2153-2154, 2186-2217 — "an atom with the right type name and a malformed/out-of-range encoding for an ineligible-but-index-legal type".
- trw:2114-2116 pending range endpoint accepted without checks here.
- trw:3688-3698 `Ok(None)` passes without error.
- trw:3462-3485 normalisation silently does nothing for unlisted operators or owners.
- trw:6794-6802 a field whose type changes with the same number is not flagged between eras.
- vcc:526-541, 584 — "a claim hidden behind a reference/slice/dyn type is not seen".
- vcc:1521 — a `[Linear; N]` with const-parameter length has no claim frontier at this layer.
- vcc:1460-1462, 1491-1495 unresolved data definition contributes no claims.
- vcc:2025-2033 generic with missing definition or wrong argument count judged not to need a drop machine.
- vcc:2142-2149 a reserved cleanup selected by an unresolved target is not diagnosed.
- vcc:2399-2403 delivery check returns silently without a source cleanup selection.
- vcc:2838-2839, 2848-2850 regions that cannot be compared pass.
- vcc:2785-2787, 2815 missing machine skipped.
- vcc:2971-2973, 2997 length mismatch returns; a `Value` argument with no carrier is skipped.
- vcc:3418-3445 array literal against a non-array carrier: nothing pushed.
- vcc:4052 — "a `Named` type with no resolvable data definition is treated as needing no establishment" (the `Generic` arm at vcc:4068 says the opposite).
- vcc:3938-3963 non-membership default-domain facts skipped.
- vcc:4402-4408 arm pattern on an unresolvable type is not exhaustiveness-checked; the non-arm path errors.
- vcc:4442-4451 arm naming a missing case returns silently.
- vcc:5630-5638 — "a free identifier spelled like some unrelated enum case name … is not reported as out-of-scope by this check".
- vcc:5591 conformance roots accepted without further check.
- vcc:7303 mixed permission origins reported as no provenance, same as a constant.
- vcc:7878-7880, 7900-7905 — a cast to a domain over a non-primitive carrier is not predicate-checked here.
- vcc:8454-8456 any projected path of a reference slot counts as writable.
- vcc:8618-8636 an assignment inside a guarded transition establishes the local for all later statements ("inferred from the loop").
- vcc:9551 budget of 4096 steps exhausted returns "no runtime extent found".
- vcc:1705, 1720 unknown name defaults to `Affine`, not `Linear` (no class given by the reader).

**terminal-installation (6)**
- ti-lib:593-600 — "a Terminal artifact whose target-layout application lacks a member for this target is admitted with that record left unresolved".
- ti-lib:653 duplicate-boundary check compares only with the last kept plan; correct only if sorted.
- ti-ev:910 — "a build-time invocation across packages without any authority check, if a caller passes `None`". Callers not traced.
- ti-ev:928-933 edge custody check skipped when either header lookup fails.
- ti-prov:709-715 — "an unmatched source membership theory that is not in `source_required` passes" (structure only; "open whether that is sufficient").
- ti-prov:1141-1146 families with no member are omitted; consumers that iterate `selections()` "would not see the gap (not traced)".

### Stated (the reader found a reason at the site; several are "by name" or "by structure" rather than a comment)

- ce:75288 `storage_value_transfer_is_current`: comment ce:75395-75403 "A versioned judge asks the storage graph at each read."
- ce:90785-90803 `AdmissionClaim` machines: unproven facts dropped, only `Refuted` errors; message "grants notwithstanding (chapter 10 engine veto)".
- ce:90037-90039 an `= true` proposition proves vacuously.
- dc:1433-1490 `authored_data_member` returns its input on any mismatch ("stated by name only; not traced").
- lsl:2757-2759 statically unreachable code skipped ("stated by the query name only").
- mcc:5202-5212 call through a machine parameter accepted on its declared contract: "This consumes the declared promise; it never inspects a selected private implementation" (mcc:4696-4699).
- mpb:4766-4768 mathematical declarations return early: "Constant and layout preparation can arrive before mathematical terms are checked." (mpb:4763-4765).
- mpb:6132 `Unknown if entry_proof_follows => {}` (by parameter name); mpb:6085-6088 (by name `closed_non_member`); mpb:1991-1994, 2017 and mpb:2195-2205 (by function names and structure).
- ops:269-274, 300-312; ops:1234-1261 (by helper names).
- pl:1169-1171 (by variable name `writes_borrowed_storage`); pl:2436; pl:5339-5351.
- pcc:2277-2298 `is_deferred_index_compatibility` (by name only); pcc:2636-2684; pcc:6811-6827 (by structure).
- rrc:8438-8440 "Signature lifetime dependencies remain in the borrow facts. They describe retained inputs, not an executable descriptor".
- scc:286-303 const/erased parameters and write-only references skipped ("stated by position only (no comment)").
- tr:541-546 non-generic requirement needs no closure (by structure).
- tr:2706-2708, 2730-2736 unbound trait type parameter binds to the actual and returns `true`. The reader wrote `"stated": no comment`; the code has no comment there (see Disagreements).
- trw:2128-2132 open const expression with a matching carrier defers to formation obligations (by structure).
- vcc:9728-9737 generic, proof, indexed-logical and content-projection machines skipped (by structure).
- ti-ev:1070 `(Toolchain, _) | (_, Toolchain) => Ok(())` (by the match); ti-ev:1210 (by field name).

### Traced (the reader followed callers or the same file)

- ce:73244-73253 `graph_place_invalidated` false with no graph ("inferred").
- mcc:6586-6588 unresolved static machine argument "terminates": refused anyway by `static_machine_argument_is_eligible` (mcc:6556-6558, 6648-6654).
- mcc:8338-8340, 8370 overload selection `None`: traced in part to the duplicate-identity check (mcc:819-883).
- mpb:4822-4840; mpb:8085, 8101-8103 (a closed wrong tuple still errors at 8104-8110); mpb:5527-5529.
- pl:1141-1143; pl:3258; pl:7246-7248.
- pv:2150-2284 `validate_once_resident` silent returns: callers establish the inputs (pv:2778-2804).
- rrc:4343-4345 recursion answered with an empty list, then checked by monotonicity and frontier tests (rrc:3755-3760, 3860-3915); rrc:425-514 defaults make every path "residual" and the one caller then demands loans (scc:3598-3611).
- scc:3762-3773; scc:3613-3621; scc:3828-3897.
- tr:5563-5565 (kind check precedes); tr:792-802 lifetime ordinal already errors.
- trw:153.
- vcc:756-762; vcc:6177; vcc:8205-8251; vcc:10529.
- ti-ev:155 unlisted proposition shapes are not formation-checked but `closed_guard` returns `None` for them, which is an error (ti-ev:329); ti-ev:1215; ti-prov:116-118; ti-prov:912-914; ti-prov:1141-1146 for consumers that call `member()` (ti-prov:1029).

---

## Silent drops

Counted from the records: about 100 sites in the validation files and 4 in terminal-installation where a failed lookup or unlisted shape produces no row and no diagnostic. Most appear in the lists above; the ones where something already computed is thrown away, or a whole check is skipped, are:

- dp:12587-12592 the crash-site list is built and discarded by the only caller (`&mut Vec::new()`).
- dp:12902-12904 a machine's own diagnostics dropped when every recorded call attempt proved; dp:13231-13233 calls with untracked actuals record no attempt.
- lsl:5543-5546 every const struct-literal diagnostic not containing `"evaluates FALSE"`.
- ce:90785-90803 unproven facts of admission-claim machines; ce:90957-90963 unknown facts outside the arithmetic language; ce:81436 facts that are not `And`/`Equal`/`NotEqual` are not intaken (fewer facts, conservative).
- ce:85781-85785, 86073-86079 type equalities from incomplete walks.
- dc:1372-1377 a late-bound target with zero or several name matches.
- dp:11400 state edges to unresolved targets; dp:8371 and dp:8966 unlisted node kinds; dp:15794 fact payload kinds left out of the agreement comparison; dp:11002 non-expression hypotheses.
- ei:405-408, 849 invoked parameters with no nameable origin; ei:1450 symbols with no row; ei:2982 unresolved asm service.
- scc:642-656 `where` facts the spelling cannot express get no predicate row.
- mcc:3158-3167 a `Diagnostic` turned into `None`; mcc:8370 more than one overload match; mcc:8911-8913 spanless casts.
- ops:1939-1943, 2075 operator uses with no fact.
- pv:1259-1272 machines without authored spans are never reused (cost, not acceptance); pv:994-1026 all prior judgments for an identity dropped if two disagree.
- pcc:11165-11293 `collect_parameter_bound_comparisons`: "Every failed step returns without pushing."
- rc:757-759 recasts issued in other machines; rc:1483-1490 judgment skipped while geometry is pending.
- tr:1718-1724 named evidence rows not of the one-proposition shape.
- trw:3462-3485 index expressions that get no operation selection.
- vcc:1460-1462, 1491-1495, 1521 types that yield no claim.
- ti-lib:593-600 application with no realization row for the target; ti-prov:1141-1146 families with no member; ti-ev:928-933 edges with a failed header lookup; ti-prov:709-715 unmatched source domains.

---

## Unused or unwired

Counts are the readers' greps over `omega-rust`; "re-checked" means I ran the grep again.

- **Documented as the one shared answer, no outside user.** `PlaceRelation` / `place_relation` / `place_segments_relation` (pl:571-618): 22 hits, all in places.rs (re-checked: 22 in 1 file). The comment says "every keep-or-kill decision gives the same answer for the same two places" (pl:560-569); the wide consumers call `canonical_place_segments_may_overlap`, `contents_overlap`, `binding_overwrite`, `case_tag_segments_may_overlap`, `canonical_places_agree` directly.
- **Fence with no production caller.** `declaration_facts_carried` (scc:361): 7 hits = definition, 2 re-exports, 4 test uses (re-checked).
- **Never filled.** `BoundMaps.machine_self_parameters` (pcc:4992, 5011, 5139): 3 hits, no insert (re-checked), so `machine_self_parameter` always returns `None`.
- **Definition only.** `shared_place_borrowed_parts` (rrc:9047), `close_conformance_declaration` (tr:4372), `name_path_last_member` (fz:2792): 1 hit each (re-checked).
- **Produced and dropped.** `OpenInvariantCrashSite` (dp:12557): 8 lines in 2 files, `.open_data()` has no outside use; the only caller discards the list (re-checked).
- **Re-export only, no outside call.** `select_named_evidence_implementation` (pcc:8819; 3 hits, re-checked); `machine_specialization_matches_template_identity` wrapper (mcc:7323; the one consumer calls typed-trees directly, re-checked); `invocations::boundary_requirement_symbol` (mcc:7210; 0 outside); `closed_constant_array_elements` (lsl:1034; in-file use plus `lib.rs` re-export, re-checked); `recast_scalar_path_projection` (rc:3101; 1 internal call, re-checked); `derive_checked_operator_realization_contracts` (ops:534; 2 hits both in operators.rs, re-checked); `derive_checked_operator_crash_contracts` (ops:572; re-export plus in-file use); `completed_descriptor_load_slot` (rrc; definition plus re-export).
- **Used only inside their unit.** `record_call_reception` (4 total, 0 outside), `transient_call_record_loans` (2, 0), `terminal_call_sources` (2, 0), `call_result_paths` (3, 0), `borrowed_initializer_source` (3, 0), `shared_borrowed_parts` (4, 0) in rrc; `type_argument_identity`, `substituted_type_identity`, `type_parameter_kinds_identical/bindable/callable`, `trait_type_bindings`, `dynamic_receiver_call_error` in tr; `fact_is_exact_relation_pair`, `checked_proof_dependency`, `quotient_for_type`, `base_data_symbol` in q; `type_is_primitive`, `arithmetic_policy_adapter` in ops; `visit_state_calls`/`CallSiteVisitor` (12 lines, all in ei).
- **Dead branches.** `theorem_correspondence` `retain_legality = false` branch has no caller (q:2200, 2299); `static_argument_matches_type` passes `_substitutions` through and never reads it (q:5383, 5421); `if field.relevance.is_erased() { continue; }` at the end of a loop body (trw:6650-6653); `#[allow(clippy::too_many_arguments)]` on a struct (pv:3588-3589) and on a four-parameter function (rc:2003); `data_fact_applies_to_field` returns `Option<bool>` but is always `Some` (dp:7107-7113).
- **Test-only.** `paths_overlap` (rrc:5521); `own_fact_reuse::original_definition_storage` kept as a test oracle (scc:1785).
- **One outside caller** (selection): `validate_default_domain_writes` (pv:4675), `validate_quotients` (pv:502), `validate_recasts` (pv:573), `validate_boundary_calls` (`checking.rs:1394`), `admit_checked_quotient_requests` (`checking.rs:1102`), `derive_scalar_boundary_guarantees` (`execution_terminal_unit_control.rs:436`), `reconstruct_case_selected_*` (`execution_terminal_unit_cleanup.rs`), `normalize_terminal_transition_calls` (`lowerer.rs:1902`).
- **Not grepped by the readers** (use unknown): `observe_private_arrivals_batch`, `AdmittedArm`, `SiteArm`, `visit_non_contradictory_arm`, `term_spelling`, `unfold_constant_applications`, `RecordedDeclarationAuthority`, `CallerSiteCaches` (record 97); most remaining `pub` functions in pl and several vcc modules (`cleanup_selection`, `claim_frontier`, `membership_evidence`, `plan_laid::validate_plans`, `qualification_casts::validate`, `source_liveness`); `admit_compiler_evaluation`, `is_subset_of` and the three `admit_provider_*_from_artifact` functions in terminal-installation.

---

## Stated limits

- lib:7-8 "Successful validation is a source judgment, not a portable proof certificate."
- lib:27-28 "Checking only reads typed tables: preparation may rewrite them, checking may not".
- dp:1326 "Unknown operands cannot borrow another operand's carrier."
- fl:1581-1584 "A mathematical definition owns a checked dependent telescope and transparent term; it is never an executable machine, a runtime result occurrence or a purity claim."
- fl:1693-1696 "This view forms a statement; it grants neither a witness nor a value law."
- etf:2070-2071 "arrays do not support `==` / `!=` yet"; etf:1233 "match structural, range, and binding patterns are not supported by value dispatch yet".
- lsl:5874 "constructor obligations have no exact prepared application ... unresolved generic members cannot waive field or case checking".
- mcc:4547-4551 "Checked mathematical signature formation owns the dependent application. It has no operational closure to classify."
- mcc:6086 "fact-position call projection must be one direct plain-record field; nested or adapted projection is not admitted".
- mpb:7084-7087 "generic callers must declare an explicit operation requirement"; mpb:4856-4857 "erased evidence cannot eliminate into runtime computation".
- ops:88-89 "explicit application is not yet supported"; ops:2779-2780 "domain memberships and proposition applications are proof facts, not runtime-refinable crash routes".
- pl:560-569 "A range that covers an index is `MayOverlap`, which a caller must treat as not contained."; pl:4961-4964 "Reused declarations can retain different symbol handles for the same nominal owner."; pl:1526 "array extent still requires selected constant evaluation"; pl:7778 "unsupported collection window".
- pcc:6243-6246 "This census grants no fact and runtime uses below remain executable demands"; pcc:10204 "erased-stripped runtime support for {shape} is not implemented yet"; pcc:4216-4217 "A separate call/binary recognizer here lost proof Int".
- q:869, 904 "executable quotient operations are not admitted until ... are independently checked" (every quotient operation is rejected at validation time, q:622); q:6828 "does not yet substitute a generic quotient owner"; q:6749 "has no closed application boundary yet"; q:2451 "the proof-only bridge excludes generic quotient declarations"; q:571-577 "the initial quotient surface cannot identify affine or linear occurrences".
- rc:1871 "deeper shapes land with the byte-view rung"; rc:1896 "record sources and temporaries land with the byte-view rung"; rc:5406 "proof-only fact positions (`requires`/`ensures`) produce no runtime borrow and stay fenced".
- rrc:3757 "captured return alternatives are not monotone"; rrc:3953 "recursive return has no independently covered value boundary".
- scc:2658 "structural result custody cannot manufacture an owned reference"; scc:2920 "projected qualification array length is a constant recipe fixed at target closure".
- tr:1608, 1729, 341 (quoted under Fixed lists); tr:5807-5811 "boundary-machine requirements are not local dynamic calls".
- trw:907-910 "an explicit `Packed` parameter is not implemented yet"; trw:924-928 "the packed grammar of an unnumbered schema is not implemented yet"; trw:2900 "default-domain facts whose index-site proof is not implemented"; trw:2881 "does not have canonical structural index identity".
- vcc:2155 "reserved cleanup machine ... is compiler-selected and cannot be selected by source"; vcc:3402 "constant type must permit copying and contain no cleanup, shared ownership, or interior mutability".
- tf:7-9 the switches "only add `eprintln!` output and nothing sets them mid-process".
- ti-lib:281 "private entry RAM currently constructs plain scalar records"; ti-lib:273 "does not establish schema qualification predicates"; ti-lib:287 "does not establish declaration predicates"; ti-lib:234 "does not supply qualifications or linear custody".
- ti-ev:870 "assumed binding contracts are runtime-only and cannot supply compiler evaluation"; ti-ev:874-877 "constant invocation retains a pending target proof condition in its demanded closure; it cannot establish its own pending target condition".

---

## Disagreements

No second-pass record exists for these units, so there is no second-pass against first-pass conflict. Between and inside the nine records I found these; I opened the code for each.

1. **How the files are mounted.** Record 101 says its three files are "`include!`-style module fragments"; record 103 says its files are "plain `.rs` files mounted by `validation/src/lib.rs:60-61`". The code: `lib.rs:40-67` declares every file with `mod name;` (for example `pub(crate) mod operators;` at lib:56, `pub(crate) mod places;` at lib:57). Record 103 is right; there is no `include!`.
2. **Module count in proof_contracts_core.rs.** Record 102 says "19 top-level `pub mod` blocks" and then lists 21, and later says "a bag of 21 unrelated modules". The code has 21 top-level `pub mod` lines. 21 is right.
3. **Module count in machine_calls_core.rs.** Record 100 says "17 independent `pub mod` blocks" and lists 18. The code has 18. 18 is right.
4. **Where the attached-data sibling is.** Record 99 cites `attached_data_field_type_reference` at "places.rs:8354". pl:8354 is a `return false;` inside another function; the function is at pl:8500-8514. Record 101 (which covers places.rs) does not list it.
5. **Are the four "unique data definition" sites identical?** Record 99 says "suspected identical" for pl:8659 and the typed-trees one. pl:8659-8664 is a forwarder to `program.unique_data_definition_by_symbol`, so those two are the same code; fz:3989 and etf:4428 are separate scans.
6. **`fact_call_targets` rule.** Record 97 says a call is "declared" if `target_symbol` is valid or any machine has the same bare name (ce:91002-91007). The code has a third way between them: `typed_trees::operator::resolve_named_expression_call(program, call).is_some()` (ce:91003). The bare-name compare is there as described.
7. **Class label against evidence.** Record 104 classes tr:2706-2736 as `"stated": no comment`. The code at tr:2706-2737 has no comment; by the brief's definition that entry is "open". Left in the stated list with this note.
8. **Who calls the default-domain validator.** Record 98 gives the only caller as pv:4675. Record 102 describes `judge_contract_entailment` (pv:4668) without mentioning it and places default-domain inputs in the prologue (pv:221-248). The code: pv:4672-4679 inside `judge_contract_entailment` builds `CallFrameResolver::new(program)` and calls `validate_default_domain_writes`; grep finds no other caller. Record 98 is right; record 102 omits it rather than contradicts it.
9. **Citation without a file.** Record 102's "maps to `(&[], true)` (550)" sits in its proof_contracts_core section; the line is pv:550.

---

## What I checked myself

Opened at commit `457adf4a11f`. 39 claims; verdicts are matches / partly / does not match.

| # | Claim (record) | Where | Verdict |
| --- | --- | --- | --- |
| 1 | `PlaceRelation` family has no user outside places.rs though documented as the shared answer (101) | grep count: 22 hits, 1 file; comment pl:560-569 | matches |
| 2 | `declaration_facts_carried` has no production caller (104) | grep: scc:361, 1600, 2021, 2115, 2152, 2349; lib:1518 | matches |
| 3 | `machine_self_parameters` is never inserted into (102) | grep: pcc:4992, 5011, 5139 only | matches |
| 4 | `shared_place_borrowed_parts` definition only (104) | grep: rrc:9047 only | matches |
| 5 | `close_conformance_declaration` definition only (104) | grep: tr:4372 only | matches |
| 6 | `name_path_last_member` 0 call sites (99) | grep: fz:2792 only | matches |
| 7 | Const struct-literal diagnostics kept only if text contains "evaluates FALSE" (100) | lsl:5509-5547; filter at lsl:5542-5545 | matches |
| 8 | Callee diagnostics dropped when all recorded attempts proved (98) | dp:12896-12906 | matches |
| 9 | No attempt recorded when no argument is a tracked place (98) | dp:13231-13233 | matches |
| 10 | Any crash transition skips the end-of-state window check (98) | dp:13607-13611, 13688 | matches |
| 11 | Crash-site list discarded by the only caller (98) | dp:12587-12592; grep shows one caller, pv:4675 | matches |
| 12 | Any variant name of any data definition is a known value root (105) | vcc:5630-5638 | matches |
| 13 | `fact_call_targets` treats a bare machine-name match as declared (97) | ce:91002-91007 | partly: a third route at ce:91003 is not in the record |
| 14 | Emitted conclusion is `result == result` (102) | pcc:10996-10999 | matches |
| 15 | `current_value_valid` returns `true` with no graph (97) | ce:73357-73359 | matches |
| 16 | `checked_value_call_candidate` memo keyed on `(machine, state)` only (100) | mcc:4767-4784 | matches |
| 17 | Four identical "unique data definition" sites (99, suspected) | fz:3989-4002; pl:8659-8664; etf:4428-4437 | partly: pl is a forwarder; two separate scans |
| 18 | Attached-data sibling at places.rs:8354 (99, not read) | pl:8500-8514; calls.rs:41556-41566; etf:7842-7858 | does not match the line; same job, keyed by symbol there and by name in etf |
| 19 | Text-carrier and byte-sequence-carrier tests have the same body (99) | etf:1768-1783, 8646-8663 | matches (last line uses a different primitive lookup) |
| 20 | Two different byte-tag tables for `TypeParameterKind` (100) | mcc:2970-2977, 3979-3986 | matches |
| 21 | Service-reach inclusion check is a `debug_assert!` (100) | mcc:5671-5675 | matches |
| 22 | `exact_primitive_type`: spelling only in q, atom plus spelling in rc (103) | q:4803-4814; rc:4512-4538 | matches |
| 23 | Custody checks skipped when `authority` is `None` (105) | ti-ev:910, 927 | matches |
| 24 | Edge custody skipped when a header lookup is `Err` (105) | ti-ev:928-931 | matches |
| 25 | Missing realization row is `continue`d (105) | ti-lib:593-600 | matches |
| 26 | `TargetLayoutSelectionError::MissingMember` has no construction site in ti-lib (105) | grep: ti-lib:531, 555 only; the constructions at ti-prov:1029, 1098 are a different enum | matches |
| 27 | proof_contracts_core.rs has 19 / 21 modules (102) | `^pub mod` count = 21 | "19" does not match; "21" matches |
| 28 | machine_calls_core.rs has 17 modules (100) | `^pub mod` count = 18 | does not match |
| 29 | Files are `include!`-style fragments (101) | lib:40-67 | does not match: plain `mod` |
| 30 | Only caller of `validate_default_domain_writes` is pv:4675 with an `Option` resolver (98) | pv:4672-4679; grep | matches |
| 31 | "duplicate member" and "duplicate owned data" fire on the same condition (97) | dc:2423, 2455-2488; `member_verdicts` has 3 lines | matches |
| 32 | `select_named_evidence_implementation` 0 outside callers (102) | grep: pcc:8794, 8819; lib:1828 | matches |
| 33 | `derive_checked_operator_realization_contracts` 0 outside (101) | grep: ops:529, 534 | matches |
| 34 | Wrapper `machine_specialization_matches_template_identity` has no caller (100) | grep: mcc:7323, 7329; lib:1798; consumer calls typed-trees | matches |
| 35 | `recast_scalar_path_projection` 0 outside calls (103) | grep: rc:3101, 4406, 5038; lib:1882 | matches |
| 36 | `closed_constant_array_elements` no external call (100) | grep: lsl:1034, 1178, 2041; lib:1862 | matches |
| 37 | Unresolved `Named` needs no establishment, unresolved `Generic` does (105) | vcc:4052, 4060-4074 | matches |
| 38 | Two `MachineControl` service recognisers (found while merging 100 and 101) | mcc:1228-1255; mpb:4098-4134 | same job; differ as described under job 1 |
| 39 | Two "in toolchain `extent.omg`" tests (found while merging 99 and 105) | lib:840-857; vcc:8871-8881 | same job; lib adds Product scope |

Corrections made in this digest because of the checks: items 13, 17, 18, 27, 28, 29 are written with what the code shows.

---

## Not covered

- **Records not read:** none. All nine (`unit_097.md` to `unit_105.md`) were read in full. No `unit_NNN_gap_*.md` exists for 097-105, and no file in `gap_units/` lists a range in these source files.
- **Ranges the nine records say they did not read:** none ("Skimmed: none. Not read: none." in every record).
- **Same crates, outside this area's units:**
  - `validation/src/contract_entailment.rs` lines 1-72848 (the enclosing `mod` headers of the tail are there; record 97 says the module names for its range "are inferred").
  - `validation/src/calls.rs` (44,215 lines), `arithmetic_domains.rs` (26,886), `completed_storage_and_conservation.rs` (6,993), `borrow_and_reflection.rs` (6,376), `call_cycles.rs` (4,256), `access_views_and_borrows.rs` (3,931). Many "open" entries above depend on callers in these files.
  - `terminal-installation/src/scalar_applications.rs` (2,704 lines), in unit 106.
- **Things the readers flagged as unknown and I did not resolve:** whether callers of `prepare_specialized_program` surface `deferred_overflow` (102); which inputs the entailment reuse hash misses (102); whether `render_proof_expression` includes operators (103); what `machine_call_dependency_symbols` collects (103); whether the two callers of `state_declaration_predicates` pre-check unspellable facts (104); whether callers of `admit_compiler_evaluation` ever pass `authority = None` (105); the count of memo slots in `CheckingMemos` (about 55, by eye, 99).
- **Counts in this digest that are mine, not a command's:** the spelling-keyed site table (247 + 7), the open-entry count (185 bullets) and the silent-drop estimate (about 100 + 4) were tallied by hand from the records under the rules stated in each section.
