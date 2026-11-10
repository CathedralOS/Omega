# Area 9 digest: Psi stage 05, execution plans, flow, published facts, values

Source: 12 records (units 056 to 065, plus `unit_059_gap_016` and `unit_059_gap_017`), all read in full. Checkout `worker-23` at `457adf4a11f`. Nothing here was compiled or run.

All files are in one crate, `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/`. The area is 36 of that crate's 62 source files, 178,231 of its 337,429 lines (unit tables summed). Short names used below:

| Short | File | Lines |
|---|---|---|
| ESC | execution_scalar_computations.rs | 11126 |
| ETU | execution_terminal_unit.rs | 8373 |
| CALLS / CLEAN / CTRL | execution_terminal_unit_calls.rs / _cleanup.rs / _control.rs | 9432 / 5292 / 6926 |
| SEQ / SG | execution_terminal_unit_control_statement_sequence.rs / _state_graph.rs | 9704 / 10120 |
| SSS / TYPES | execution_terminal_unit_structural_scalar_store.rs / _types.rs | 3826 / 5353 |
| F | flow.rs | 11937 |
| FCP / FC / FCX / FX / FE | flow_call_phases.rs / flow_calls.rs / flow_context.rs / flow_exits.rs / flow_expression.rs | 2370 / 1071 / 1316 / 1681 / 11742 |
| FM / FO / FOC / FOV | flow_mutation.rs / flow_observations.rs / flow_operand_contexts.rs / flow_operand_values.rs | 4762 / 1690 / 1136 / 1193 |
| FOW / FP / FRP / FRB / FSV / FT | flow_ownership.rs / flow_place.rs / flow_reference_places.rs / flow_retained_borrows.rs / flow_state_values.rs / flow_transfers.rs | 1989 / 1837 / 1108 / 1394 / 3815 / 4593 |
| PUB / PF / PBP / PCC / PEV | published.rs / published_facts.rs / published_facts_boolean_predicates.rs / _crash_calls.rs / _crash_entry_values.rs | 10658 / 11563 / 2576 / 5493 / 2546 |
| PROD / SH / VA / VS | products.rs / shared.rs / values.rs / values_scalar.rs | 1414 / 4805 / 3589 / 11585 |

Words kept from the records: "open" means the reader saw no reason at the site and did not trace it; "stated" means a comment at the site gives the reason; "traced" means the reader followed callers. "Suspected" means the reader did not read the other site.

## What this area does

Stage 05 takes a typed program that it may only read and publishes "check facts" beside it. This area holds four parts of that stage. The **values** part turns typed scalar and Boolean expressions into a closed "checked" expression form and records what is known about values. The **flow** part walks every state, statement, call and exit in execution order and records which facts hold there, which storage each statement may write, which loans are live, and what each operand captured. The **published facts** part builds the proof, contract, crash, cleanup and capability tables that the checks and later stages read. The **execution plans** part builds, per machine body, the ordered plan of operations (calls, stores, cleanups, terminators) that stage 06 lowers. In the planners almost every builder returns `Option`; a `None` means "no plan row", recorded as a plan gap or not recorded at all (ESC:9561-9598, CTRL:5251-5445, unit 57 "five things" 1).

Per file group (one crate, so groups stand in for crates):

- **Values (VS, VA).** In: typed expressions, operator facts, parameter and local rosters. Out: `CheckedScalarExpression` / `CheckedBooleanExpression` trees, the located plan rows `CheckedScalarExpressionPlans` (VS:5370), `CheckedValueFacts` per occurrence (VA:3510), interval and scalar readers over the fact plan (VA:346, 1346, 2531), reference-capture plans (VA:1793).
- **Flow (F and the 15 `flow_*.rs` files).** In: typed program, `BorrowFacts`, `ProofFacts`, the semantic `FactPlan`, call-frame resolver. Out: `FlowFacts` (state, statement, call, exit, operand, snapshot, invalidation and ownership rows) built by a per-machine-group fixed point (F:291, 532), plus query code over finished facts: write footprints (FM), receiver read footprints (F:7533), value origins (F:8167), cleanup leaves and dependencies (F:10515), selector narrowing (F:11435).
- **Published facts (PUB, PF, PBP, PCC, PEV).** In: typed program, proof plan, borrow facts, operator facts, flow facts. Out: `ProofFacts` (PUB:6332), the semantic `FactPlan` (PUB:9861), contract plans, crash capsules and per-call crash sites (PF:2571, 3735, PCC:5072), cleanup entry requirements (PF:871), capability, termination, blocking and suspension rows (PF:65, 7630-7815), and the driver `build_check_facts` / `finish_check_facts` (PF:10963, 11148).
- **Execution plans (ESC, ETU, CALLS, CLEAN, CTRL, SEQ, SG, SSS, TYPES).** In: `CheckFacts` and the typed program. Out: scalar computation, structural value and completion plans (ESC:10508); one `CheckedUnitEffectMachinePlan` or composed control-graph plan per surviving body, cleanup actions, boundary machine plans, the structural type catalog (`ShapeCollector`, TYPES:2574), and one gap row per dropped machine (ETU:7079-7996).
- **Shared and products (SH, PROD, lib.rs, immutable_checking_input.rs).** SH finalizes late-bound authored selections into checked targets (SH:973), binds boundary operator applications (SH:3829), and holds a call-cycle wave scheduler (SH:3540). PROD holds read-only review wrappers, product pruning by reachable machine (PROD:1119) and a quotient eligibility check (PROD:1266). `immutable_checking_input.rs` is one test that scans source text for forbidden strings.

## How it decides

**Keyed on identity (the common case).** Symbol handles, expression handles, arena handle plus generation, `(statement_index, call_ordinal)` coordinates, type identities. Examples: call lookup requires exactly one match on `(statement_index, target_symbol, authored_expression)` (ESC:10484-10504); claim transfer joins permission events by source call coordinate and ends with a count equality (CALLS:7418, 7403, 7707); retirement of loans keys on loan handles, binding ordinals and statement order "never on names" (SEQ:5496-5676); case-subject rejoin is a chain of identity gates with a named refusal each (FO:248-478); one shape per `TypeIdentityId` (TYPES:2692-2703).

**Keyed on place (root plus projected segments).** Invalidation after a write compares canonical places with same root and overlapping segments (F:6901-7008, FCP:1262-1306); stores resolve a destination root, authority and carrier path (SSS:956, 1209); value stability checks compare recorded write rows against a place (FOC:641, 1113-1135); custody accounting requires exactly one disposition per structural result (SG:5132); cleanup leaves are found by walking the type under the removed paths (F:10853-11181).

**Keyed on value.** Index segments narrow to fixed indexes when the facts pin one value (FP:52-88); crash routes are dropped only when a guard folds to a decided `false` (PCC:2793-2896, 3308-3318, 3624-3637); interval joins widen after 4 growths and hold after 8 shrinks (FSV:716, 1103-1104); `unsatisfiable` fires only on a contradiction between established truths (PBP:2020-2025).

**Keyed on position.** Ordered ladders where the first success wins: `build_expression` (ESC:9932-10390), `structural_value` (ESC:8945-9381), `plan_assignment` (SSS:3275-3471), `local_data::plan_inner` (SEQ:1318-1924), `terminator::build` (SG:8082), `lower_scalar_expression` (VS:9089-9356), `checked_call_target` and `checked_operator_target` (SH:338, 2174). Call targets are also matched by source span equality (SH:412-426). Invocations are matched by site and visit order, not ordinal (FE:9742-9746). Statement position windows decide whether a binding still holds its entry value (PEV:989-1004).

**Keyed on source spelling: 96 distinct sites cited by the records** (I counted each cited line or line range once; one is test-only). Grouped:

- *Execution plans, 14 sites.* Generic or named types match when base names are equal strings even if symbols differ (ESC:8329-8330, 8348-8349); sealed accessor by `call.target.as_str()` (ESC:519-521); member symbol resolved by member name (ETU:3191-3198); builtin intrinsics by `builtin.name()` (CALLS:1982-1990, 2054-2062); callback telescope binder name match (CALLS:8142-8148); wire codec keyed on the spelling `"encode"`, schema name and field name (SEQ:8609, 8698-8701, 8759); variant and field name compare in computed dispatch payload (SG:766-769, 774); primitive-vs-data by `PrimitiveType::from_name` (TYPES:3638, 1437).
- *Flow, 18 sites.* Operator resolution by receiver path members and target name (F:3932-3953; FOW:1083-1090, 1398-1405, 1360-1366); contract place root by `parameter.name == head` (F:4566-4570); member lookup by name for domain dependencies (F:6405-6428); boundary edge by `signature.name == target_state.name` (F:5908); operand labels from `display_name` feeding label instantiation (F:4054); write frames as string paths (F:3210, 3278); carry-fact transfer by label equality (FC:420-424); ensured parameter by name (FC:1066-1069); reserved `result` by name (FC:947-953; FCX:1120, 1127); `"{subject_label}.{field.name}"` substitution strings (FX:502-517); write-frame report paths split on `.` and matched to parameter and local names, `"[*]"` suffix stripped, fields by member name (FM:1928-1975, 1914-1924, 1956).
- *Published facts, 44 sites.* Label and display-name equality for Boolean facts (PUB:132, 109-121; PF:8155-8176, 8185-8187); parameter by name (PUB:412-414, 7677, 10211, 10486; PF:449; PCC:4983-4991); `self` by spelling (PUB:10508; PF:442); a state named `"entry"` (PUB:202); evidence terms by `(owner, kind, name)` (PUB:2772-2784, 2353-2370, 5438-5460, 5937-5976); projection member name and `format!("{}.{}")` label (PUB:2282, 2301-2313); `"boolean:..."` and other rendered label sets (PUB:4962, 6153-6159); literal `"result"` (PUB:4831, 4956, 3129, 3525); `CarryPermission::from_name` (PUB:6797, 7271, 9487); cycle guard keyed on `data.name.as_str()` (PUB:9158-9162); boundary capability by `signature.name == state.name` (PF:631); closed index argument by parsing a type-reference name (PF:5103-5104); invocation target encoded as `parameter:{index}` / `service:{name}` strings (PF:3856-3865, 7826-7834); index-compatibility dedupe compares labels (PF:6725-6733); crash predicate identity retains member strings, call target text, `Opaque(display_name)`, `format!("{case}::{field_name}")` (PCC:4992-4997, 5017, 5025-5028, 5492); integer and float literal text parsing (PCC:3203-3218, 3172-3176); const identity `type_name` from a primitive name (PCC:4714-4717); builtin `len` by authored spelling (PEV:1651-1654); place-to-path by rendered field name, then re-authenticated by identity (PEV:582-588, 619-624); the source-text scan test (immutable_checking_input.rs:13-128).
- *Shared, 10 sites.* `"BuildBindingOperations"`, `"<build-prelude>"`, eight operation names, `"Build"`, `"ProductEntryRef"` (SH:57, 66, 138-251, 242, 256); `"Equatable"` / `"equals"` (SH:2375-2377); callee by name and `"::"` (SH:1506-1509, 1578-1588); field by name taking the first variant that has it (SH:1795-1808); operator parameter by name (SH:2585-2597).
- *Values, 10 sites.* Assembly intrinsic and carrier by name (VS:1267-1275, 1292); primitive by name cross-checked with a symbol atom (VS:3943-3953); parameter by spelling and symbol (VS:3606, 3872); spelling fallback when no symbol is attached (VS:4383-4389, 4417, 5619-5622); data definition by name, first match (VS:10938-10941); `self` by spelling (VS:11042-11048).

Not counted as spelling: structural expression equality used to match dispatch-copy subjects (FE:1646-1668) and repeated subjects (PUB:6590-6612).

## The same job in more than one place

Entries marked *(merged)* come from separate records that each described one site; I did not open those sites side by side unless the "checked" note says so.

1. **Walk a Boolean expression through not / and / or / equal / comparison.** Four typed-to-checked lowerers, each with its own arms and its own way of naming parameters: VS:385, VS:3387, VS:5581, VS:7602. Checked-expression walkers: PBP:2249, 587, 2201; PCC:2395, 2353, 3492, 4136. Guard decomposition over typed expressions *(merged)*: F:3365-3438, FP:407-483, FX:563-596, FX:598, FCX:1153, PUB:7095, CLEAN:1750-1795.
2. **Which parameter does this name mean.** VS:3861 (symbol and spelling), VS:4370 (symbol, else spelling), VS:5610-5622 (symbol or text), VS:6128 (symbol only), VS:7052; *(merged)* FC:1050-1069, F:4551-4570, PUB:412-414, 7676-7679, 10211-10214, 10485-10486, PCC:4983-4991, PF:449, SH:2585-2597, FM:1928.
3. **Dense scalar position of a parameter.** VS:3802-3811, VS:4360, VS:8078-8084; TYPES:1826; SSS:3219-3229. `scalar_parameter_roster` (VS:4340) is documented as the one source (VS:4335-4339) but 10 other sites rebuild the filter: values.rs:117, PCC:2207, 3937, 4059, PBP:161, 170, 1464, TYPES:1836, SSS:3224, ESC:10534 (unit 65 found these by grep line text and did not read them). Dense erased index: VS:4157, 6166, 7073-7082.
4. **Find the one flow call for a statement, ordinal or expression** *(merged)*. ESC:10484-10504, 10448-10482; ETU:3828-3835; CLEAN:1236-1248, 2723-2738, 4466-4472; SEQ:8193-8204; SG:1732-1755, 7692-7702, 7790-7815; VS:70-97, 1030-1098, 11333; FRP:967; F:11623-11691. The keys differ: authored expression plus target (ESC), coordinate (ETU), statement only (SEQ:8193).
5. **Pick a scalar value source: computation root, else bound pure expression.** CALLS:230-294, 587-610, 3807-3827, 8898-8930; CTRL:5533-5620; SEQ:1020-1051, 8542-8563, 9662-9695. Unit 57 counted `bound_expression_at(` 21 times in 7 files.
6. **Admit a structural call argument.** The ordinary path (CALLS:6374, arms 6410-6848) and the boundary path (CALLS:7987, arms 8168-8338) each carry their own arm sequence; the first has view-local, trivial-affine-local and retained-temporary arms the second lacks. The scalar-or-structural parameter classification loop is written four times (CALLS:2200-2219, 3661-3677, 8168-8183, 6103-6195) although `parameter_signatures` is documented as the one classifier (CALLS:6084-6092). The callback telescope check exists twice with different strictness (CTRL:657-678 vs CALLS:8101-8164).
7. **Walk a place path through a type.** VS:5868 and VS:10634 (the first skips the erased and symbol-validity checks the second makes); *(merged)* SSS:1306, SSS:552-563, CALLS:911, F:11282, TYPES:920, PEV:404-433. Unit 59 suspected SSS:552-563 duplicates CALLS:911 and did not read CALLS; I did not open the pair.
8. **Do two place paths overlap, or does one cover the other** *(merged)*. ETU:712-729; FM:2326, 4479, 651; PEV:841 (its comment at PEV:834 says it mirrors validation `places.rs:665`, not read by the reader); FCP:246, 1625, 1644, 1658; PF:1483; F:7264, 8826; four inline copies in F:6165-6302.
9. **Join integer intervals.** FSV:745-802, FSV:1311-1427, and hand-written min/max joins at FSV:619-629, 3078-3097, 1546-1553, with two separate widening constants both 4 (FSV:716, 1103). Hull code inline at least eight times in VA (VA:295-300, 319-325, 504-510, 625-631, 2699-2705 and others); *(merged)* FT:2798-2803, FE:5220-5225.
10. **Read assigned values at a place.** Three readers with the same completed-call exclusion: VA:2275, VA:2286, VA:2763.
11. **Per-state footprint fixed point over calls.** Reads: F:7827-7883. Writes: FM:2725 onward. Gap 017 suspected the second mirrors the first without reading it; unit 61 read FM. Checked: both instantiate callee footprints into callers using the shared `jump_loans` helper (F:7295) and a per-state `complete` flag with whole and held runs; the read version is a serial loop until nothing grows, the write version runs on rayon with dirty tracking. Same shape for two access kinds, two implementations.
12. **Append one semantic context to the context list and the constraint list.** Checked, three functions: F:9229 (creates the context first), FCX:462 (takes an existing context), FX:35 (re-retains both spans, then appends).
13. **Is the callee a boundary trait signature** *(merged)*. FM:4065-4070 and FOW:720-733 (same test, unit 61 read both); VS:1214-1241 (wider: also bodyless machines and boundary operators); by name at F:5908 and PF:631.
14. **Crash predicate identity.** PF:4413-4435 and PF:4475-4497 have identical bodies (checked); the comment at PF:4472-4473 says they are separate namespaces. The invocation-target string encoding is written twice (PF:3856-3865, 7826-7834). `build_blocking_facts` and `build_suspension_facts` are parallel (PF:7746, 7781).
15. **Result and parameter domain vocabularies.** Whole-program scan and symbol-indexed variants of the same rows: FC:749/781, 810/849, 981/1004; the scan variants are reached only from checks_content.rs:1912 and 2267. Owned-result and reference-result memberships are published by two functions from the same rows (FC:459 vs FE:2776).
16. **Find where a local's value or reference came from** *(merged)*. FRP:789, 834, 362-410 (three scans, read together by unit 61); F:8167 (backward origin trace); F:3136; PEV:2010; VS:11495.
17. **Enumerate composed terminators.** ETU:4326, ETU:2538-2638, ETU:3580-3649. Arm-return logic three times (SG:462-531, 8496-8647, 8987-9097). The scalar return-retention test four times (SG:473-482, 8588-8597, 9058-9067; SEQ:9576-9592).
18. **Reference retirement.** Per-statement and exit loops are near-parallel (SEQ:6753 vs 7191); the 17-field `Retirement` literal is written out at SEQ:3800-3819, 6797-6816, 7360-7379.
19. **Move a fact payload to a written place.** Three lanes with different payload lists: FT:704 (list at 744-750), FT:1427 (1494-1522), FT:520 (644-647).
20. **Contract plumbing.** Owner to machine: PROD:595, 928. Clause facts with separate lane counting: PUB:1831-1879 vs 1425-1479 (comment PUB:1748-1753 says the lane position "must always agree"). Contract expression to place: PUB:7475-7592 vs 10026-10127. Cycle guards keyed two ways: `(symbol, generic_instance)` at PUB:8966-8970, `data.name` at PUB:9158-9162 (checked).
21. **Named target to state index.** SG:1161-1173 and validation `call_cycles.rs:3285`. Checked: the same rule (a state symbol resolves directly, a machine head resolves to its entry state) over different rosters (several fused owners vs one machine's states).
22. **Refuse image, recast or representation sources.** FE:3024-3030, 3550-3553, 4454-4461; *(merged)* FM:951-956, 995-1000; FRP:670-684; FE:6101-6106.
23. **Small copies.** `symbol_key` has 9 definitions in omega-rust (ETU:7998 and 8 others; checked by grep). `push_unique*` has 12 other definitions besides SH:3437, whose comment calls it "one owner" (SH:3412-3431). Member-by-name resolution *(merged)*: F:6405, ETU:3191, FM:1956, SH:1795, PUB:10311. Operand type reconstruction three times in SH (647, 2551, 1887). Statement and expression forms of operator operand binding and mutated places (F:3993/4077, 4273/4236). Scalar-binding exactness in two variants (SSS:2226 vs 2267).

## Fixed lists of shapes

Counts and ranges:
- Affine array construction prefix only for length `3..=26` (CALLS:48-51).
- `ArgumentPathShape::Static { fields: 0, fixed_indexes: 1..=3 }` (CALLS:4082-4086).
- "exact two- or three-child shared-freeze cohort", `call_ordinal == 0`, callee body with zero statements (CALLS:7023-7056).
- One linear input and a Linear result for carry transfer: "Conditional aggregates and every n-ary shape wait for P1c path mappings." (FC:263-270, 373).
- Dispatch payload copies only on the primary target arm (FE:1550); returned-referent capture only for an empty `result_path` (FE:7212-7214).
- Exactly two arms per transition, target and continuation (FX:1593).
- Descriptor-table calls: `ActivateTask` 1 scalar and 3 structural, `ReloadSegments` 1 and 2, `ReadTask` / `ReadSegment` 0 and 0 (ESC:387-404); SGDT exactly one operand (FM:4738).
- Proof-output calls "currently limited to a one-state Unit- or scalar-result checked machine or exact generic requirement" (PUB:5217-5221); scalar means `I32` or `Bool` (PUB:5629-5647); "outside the first closed dispatch rung" (PUB:5687).
- Build selection requests: closed variant list with expected argument counts 1, 2 or 3 (SH:121-204).

Hard caps: 64 referent crossings (ESC:7670); 8 reference shells, then proceeds (ESC:7119-7123); 16 alias rebases, then returns the partial place (ETU:1212); 128 depth (SSS:391; FM:2021); 8 hops (SG:9286-9309; FE:3804; PCC:2032; VS:1976, 2210, 2350, 2375); 16 (FRP:202; F:8199); 32 place segments (F:7598); 64 (F:1759; PEV:1324; PCC:4137, 4194; many in VS such as 6320); 4096 work units (CTRL:74; PUB:8225; VA:583; VS:3039).

Ladders and allow-lists:
- Single-state body tails: three accepted shapes, else "unsupported statement kind" (SEQ:9077-9094). Composed tails: a match on the reachable transition prefix with ten named unsupported shapes (SG:8373-8921, names at 8878-8915).
- Successor transfer kinds tried in fixed order, else `None` (SG:6393-6901). Unit-completion dispatch only in four scalar and two structural `(pattern, comparison)` pairs (SG:9482-9535).
- `body_route` decides Sequence vs Graph from authored shape (ETU:8140-8186). Admitted operation kinds in a graph: fixed list, others named in the trace and refused (SG:3639-3762). Composed control drops unlisted kinds as "composed control vocabulary" (ETU:2615-2621).
- Store leaves: widths only 1, 2, 4, 8 bytes (SSS:2904-2912); "call results, payload sums, records and affine carriers keep declining." (SSS:3493-3498); path segments only Field, Case, FixedIndex, Index (SSS:1306-1458).
- Wire codec: only `Schema::encode`, byte-slice fields with constant exclusive ranges (SEQ:8584-8824).
- Nominal cleanup hook machine: one state, empty body, Unit return, owned non-generic parameters (CLEAN:5075-5129). `requires` on a hook only as `x.f`, `!x.f`, `x.f == true/false`, `x.f != true/false` (CLEAN:1758-1795).
- Cleanup type walk: references, slices, `DynamicTrait`, unit and const expressions give no leaves; open type parameters give `Symbolic` leaves that become an error (F:10869-10874, 10964-10968, 10793-10798).
- Reference projection and owned case subjects only through `Field | Case | FixedIndex` (FE:2328-2337, 843-854); same subset in FP:161-179, 1832; FT:1274-1281, 4490-4494; PEV:404-433.
- Entry operand expressions: a fixed list of node kinds and 18 binary operators, else `None` (PEV:1781-1962).
- Guard decomposition and stable-predicate node whitelists (F:3375-3434, 3448-3468); statement-kind whitelist for saved decisions (F:3175-3199).
- Checked-expression forms refused by the rebinding walkers: `ErasedParameter`, IEEE comparison, structural identity, byte-sequence forms (PBP:2319-2323, 2406); crash-lane allow-list (PCC:3492-3557); evaluator refusal lists (VA:791-792, 925-928, 1061-1066).
- Scalar capture across calls only for receiverless calls into checked-body machines with no owned data and primitive parameters (FT:2280-2407); conversions only same-carrier retag or total widening (FT:3105-3108).
- Seven binary operators are builtin whatever the operand type; `Unary` is never a candidate for an authored operator (SH:2181-2198, 2648, 2734).
- `numeric_term` admits only Add, Subtract, Multiply (VS:7197-7202); integer comparison "establishes no totality for arithmetic, calls, casts, result values, or current body storage." (VS:3636-3639).
- Index intersection handles Index/OpaqueRange vs FixedIndex/FixedRange and range pairs only: "An unrepresentable intersection withholds refinement instead of an effect." (FCP:241-302).
- Product pruning is an explicit list of about 40 machine-keyed tables (PROD:659-920); a new table not on the list would be left unpruned (inferred by unit 62).

## Accepts when it does not know

### Classed "open" by the readers (124 entries: 123 classed open and 1 given no class; none traced to a wrong program that reaches the site)

Execution plans:
- ESC:8329-8330, 8348-8349. Types match on equal base name when symbols differ. Reader's wrong program: two different generic data types with the same spelled name treated as matching carriers at a call result (ESC:9274), case construction (ESC:9299) or struct case (ESC:7514). Upstream type checks not traced.
- ESC:7119-7129. After 8 reference shells the walk continues with the unpeeled reference. Reader: "probably benign", not traced.
- ESC:10538-10543, 10590, 10597. A state with a non-primitive scalar parameter is skipped whole; `record_completion` results discarded. A state is left with no computation plans.
- ESC:6697-6733. Empty cleanup span on any failed precondition: a fresh reference receiver whose retirement lookup fails would skip normal cleanup actions.
- ETU:1212-1240. Alias resolution returns the partly rebased place after 16 steps or a failed lookup: a chain of 17 or more reference-copy aliases would resolve to an intermediate local.
- ETU:2359-2364. `Dynamic | ActivationStorage | DescriptorTable` call routes get no dependency edge and no check: a body calling through such a route is kept even if the called implementation has no plan.
- ETU:3374-3380 with ETU:500-501, and SSS:1006-1009. A failed receiver-alias lookup becomes an empty alias list; an unresolved alias keeps the authored place. An atomic place or a store through an erased `&mut` alias would be planned against the alias local.
- ETU:6403. Value kinds not listed contribute no transfers: a `Scalar`, `QuotientCall`, `QuotientAdaptation`, `ZeroedStructural` or `ByteLiteralView` value whose subtree moves an owned source would record no transfer. The reader notes ESC:4995-4996 requires Unrestricted input for the quotient adaptation arm.
- ETU:5116-5164. Operation and binding kinds not listed contribute no transfers. No wrong program named.
- CALLS:5678-5691. A projected mutable borrow rooted at an expression returns `StructuralResult` without the exact access check.
- CALLS:6172-6174. Qualification rows with an unknown domain are removed from the plan rather than rejected.
- CLEAN:929-991. `consumed_claims` returns an empty list when no event matches: a hook on a path whose linear claim was never recorded is not refused here.
- CTRL:398-402. Reference outputs skipped for checked families and descriptor-preserving interrupts.
- CTRL:2546-2563; CTRL:1670. A byte-sequence predicate with no flow call, and an absent Guard root, record nothing and succeed.
- CTRL:1881-1898. Remaining statement calls are accepted if retired, builtin with an empty write frame, or on another arm: a call wrongly marked retired would go unscheduled.
- SEQ:1350; SEQ:5502-5506; SEQ:7230-7232; SEQ:7208-7221 and 6770-6783; SEQ:9224-9228. Nothing planned for an erased dispatch subject; a loan not yet formed counts as retired; no releases when there is no normal completion or no loans; fact-only citations skipped. No wrong program identified for any.
- SG:2000-2002, 2566-2568, 3021-3023. Image storage roots return "no local home" or `true`.
- SG:3568-3570; SG:3571-3581. Unrestricted results, and no-code completion owners, pass with no cleanup check or disposition row.
- SG:3895-3897, 3905-3909. No implementation witness and no runtime components means no rank; relies on termination being decided elsewhere.
- SG:8482-8487, 8704-8713, 9255-9282. A Unit state whose transitions are all on erased dispatch subjects (or `Always`) returns Unit without planning any arm. "Stated" for intent ("the arms are not executed", SG:9251-9254); "open" for whether an arm can carry runtime effects.
- SG:7362-7372. When `add_reference_type` fails the carrier identity becomes the default value and the plan proceeds.
- SSS:3061-3088; SSS:3051-3056. Assignments that fail planning but have a call or structural value, and atomic or array assignments, are skipped on the assumption another builder takes them.
- TYPES:4786-4872, 1093, 4653-4659, 4760-4762. Memberships skipped when the root, domain definition or routing is missing; whether the scalar lane catches them was not traced.

Flow:
- F:742-748. A borrow call whose caller or callee does not resolve adds no grouping edge: dependent machines could land in separate groups.
- F:1273-1353. A state or call with no reach summary gets the default summary: a state missing from the operational plan that does suspend. The `Default` impls were not read.
- F:1469, 1506; F:1524-1539. A store to a destination without static identity is not recorded, so the older snapshot stands; a join of zero predecessors makes an empty join node.
- F:1647-1678, 1618-1621, 1780-1784. An unresolved descriptor returns `Some(empty)`, not `None`; in `address_before_statement` the address then vanishes from the output.
- F:2741-2775. For exit contexts a fact whose root has no valid target origin is still published with its unrebased root.
- F:3687-3689, 3729-3734. `possible` is false for an unresolvable selection; callers skip on false (checks_multiplicity.rs:6568, 6592; checks_contracts.rs:5760).
- F:4146-4165; F:4166-4174. The expression-form operator effect returns early when operands cannot be rebuilt, and drops a mutable operand with no place, where the statement form reports unknown (F:4292-4296): facts about that operand would not be dropped.
- F:4432-4449. An `ensures` with no operand place is published at `FactPlace::Unknown` and no write ever invalidates it (traced at F:7056-7071); open whether expressions with only unplaceable operands end up here.
- F:4652-4656. A state with no borrow state fact gets no flow state row.
- F:6609-6641. A domain with no predicate, aliases or memberships is content independent, so content writes would not invalidate its qualification.
- F:6913-6920. A fact whose place root is `Unknown` or invalid survives every write.
- F:8110-8138; F:8896-8904. A `Name` that does not resolve to a symbol-rooted place records no receiver read; without published flow only the value write frame is checked.
- F:9282. `Unknown` and every borrow-resource constraint are kept regardless of the active contexts.
- F:9426-9482, 9732-9753. Machines, calls and events that fail a lookup add no dependency row; exposure defaults to private.
- F:10617-10619; F:10869-10874, 10964-10968; F:10759-10768; F:10799-10804. `zero_image` gives no cleanup leaves; a `DynamicTrait` value with an owner cleanup gets no edge; dispositions of non-checked-body, open or proof machines and leaves whose machine has no entry state are skipped.
- F:11515. A selector cycle contributes no origins.
- FCP:836-857. Contract facts about non-parameter occurrences (no place, non-symbol root, no position) are never invalidated by this filter.
- FCP:775-777; FCP:1394-1399; FCP:2068-2069, 2088. A source rooted in a temporary survives writes; a `ParameterReferent` incoming reference always survives a call; an unmatched mutation "does not affect the fact".
- FC:420-424. A carry fact transfers when its rendered label equals the source's label even if the places differ.
- FX:1433-1436. An exit with zero or several outgoing rows is recorded with invalid `returned_value` and `completed_storage`.
- FE:601-604, 720. A projected pointer with no stored capture is kept with its referents dropped.
- FM:1491-1500, 1512. A failed declaration lookup makes a place count as declared storage: a write through a reference-typed local would be recorded as a write to the local itself.
- FM:4612. `.or(Some(places))` keeps the unclosed places when alias closure fails, though the doc comment says "An unresolved origin requires full invalidation" (FM:4571): facts would be retired only at the directly written place.
- FM:4593; FM:3879, 3897, 4047-4051; FM:1283-1289. Non-assignment statements write nothing here; fact-only, byte-predicate and empty-frame calls write nothing; image-storage origins add no alias.
- FO:1257-1269. A context with zero descriptor observation rows agrees.
- FOC:111-114. A case path counts as not inactive when evidence cannot be prepared.
- FOW:67-69; FOW:869-874, 909-911. An unresolved callee records no move events; a place-like expression with no ownership place is neither moved nor observed.
- FP:1012 (source mutability below an immutable root not checked); FP:1241 (any valid expression root is a stable subject).
- FRP:526-531, 858-860. A symbol-rooted place not explained as a local or parameter is accepted as its own storage.
- FRB:1271-1281, 1368-1391. No handbacks when argument sources or ending inputs are missing; a loan not recognized as owing is handed back.
- FSV:518-541. A lexical-region delivery that is skipped leaves the state with no row, and "A missing row is unreachable" (FSV:2764).
- FT:3599-3602. Values are kept across a Unit call when `observation_places` is empty; the comment says "complete storage footprint is empty" (FT:2275-2278). A callee writing through a path not in `observation_places`.
- FT:1085-1115. For a non-exact target a candidate keeps a membership if any one source context proves it (`any`, not `all`): a write through a reference local with several possible referents.

Published facts, shared, values:
- PROD:809, 819. Nominal-use and interrupt rows with an expression site are kept whatever the owning machine.
- PUB:8387-8586. A call-site proposition fact whose instantiation failed is still published with `instantiated` invalid.
- PUB:6796-6806. A parameter constraint whose name is neither a domain nor a carry permission adds no obligation.
- PUB:10387-10391. Invalid member symbols are skipped, giving a shorter place.
- PUB:9732, 9780, 9822. An unknown owner is published at global scope.
- PUB:4465-4521. An outcome arm with no selector that cannot be classified is skipped without error.
- PUB:2483-2485. A machine with no entry state is skipped in definite-assignment validation.
- PUB:790-792. A call with no retained signature owns every non-signature contract. The reader gave no class, only "No comment."
- PCC:5205-5207. A target that is not a checked body and has no published routes gets no crash causes for that call.
- PCC:815-870. Asm intrinsics and atomic fences get no crash edge.
- PCC:750, 788-789. The summary fixed point returns its last rows if it has not converged within `equations.len()` rounds.
- PF:6637-6643, 5356-5377. Index compatibility is skipped for an invalid `semantic_id`, empty arguments, or an expression kind with no instances.
- PF:412-455. A receiver head that is not `self` or a parameter gives `Unknown` provenance, so capability verbs may be under-reported.
- PF:7755, 7789-7791, 7835-7840. A machine with no operational row defaults to not blocking, not suspending, no invocations.
- PF:8155-8187. A named operator crash route is dropped on label or display-name equality: two distinct expressions with the same display name.
- PF:8290-8292, 8394-8399. Operator uses with no selected candidate or no declaration are skipped.
- PF:9551-9569. Default (empty) qualification evidence for a missing domain definition or a non-ensures fact.
- PEV:1014. An `Assembly` statement never overwrites a binding: an assembly statement that writes a local between the binding and the read.
- PEV:2211-2221. Float-typed bindings skip the write scan, and the type is peeled through references first: a write through `&mut f64`.
- PEV:1296-1299; PEV:1246-1251. Arguments are paired with parameters by position without selector alignment; only `VALUES` child edges are scanned.
- SH:1028-1030. Product-operand selections are left unfinalized and the final unresolved check does not name them.
- SH:2253-2292, 2648, 1967; SH:2181-2198. Unary operators and seven binary operators settle as builtin: a user-declared overload would not be selected. Whether the language has such overloads was not checked.
- SH:489-491; SH:2912-2923; SH:4685-4687. The resolver's direct symbol is returned when no contextual root exists; assembly-fact roots feed the proof-view collector; a body candidate with no matching declaration is dropped.
- VA:1016-1018. `StructuralIdentity` evaluates true when both sides are equal trees: two reads of the same mutable storage at different times.
- VA:2164-2170. Storage other than `Declared | View` is treated as needing no source qualification.
- VS:231-254. The atomic operand lowerer always returns `Some`, with fewer or zero rows, and its callers stop planning that statement.
- VS:1328, 1394. Erased formals of boundary calls are skipped.
- VS:2226-2234; VS:2032-2041, 2073-2093. Proof symbols correspond if either derives from the other within 8 hops; a projection is kept if any call anywhere passes the expression as an argument.
- VS:4480-4489. No recorded operator use counts as builtin: an expression whose user-defined operator resolution was never recorded. Lowerers at VS:488, 496, 544, 9333 rely on it alone.
- VS:4388, 4417; VS:5619-5622. A name with no symbol resolves to a parameter or local of the same spelling.
- VS:10041-10060 with VS:5442-5443. A cast whose operand type cannot be resolved counts as not needing custody and keeps its plain plan row.
- VS:8119-8135. A non-interval domain constraint adds no slot and does not mark the roster incomplete.
- VS:6880-6882; VS:7552. An empty predicate list yields `Constant(true)`.
- VS:10938-10941. A type reference with no symbol resolves to the first data definition with that name.
- VS:5429 and the planners. Failed lowerings leave a statement with no scalar row.

### Classed "stated" (a comment at the site gives the reason)

- CALLS:1311-1320, 627-629: slice index and runtime index carry no bound, "the hop stays unproven"; Terminal re-proves it (not traced).
- CALLS:6823-6825 restored alias takes the target access by certificate (CALLS:6962-6966). CALLS:4837-4839 unrestricted owners skip the event join (stated for the design at 4726-4735, not at the early return).
- CLEAN:1998-1999 "Unknown source outcomes remain potentially normal". CTRL:109-114, 366-378 pending recipe rows "grant no proof". CTRL:5695-5699 stated indirectly.
- SEQ:1318, 1424, 2154-2171, 2992-3004, 708-724. SG:3802-3807 "The call sequencer already joined each operand to its loan or consumption."; SG:5132 "A domain-ending or all-crash exit abandons custody"; SG:9117-9121; SG:10015.
- SSS:2011 no numeric receipt for refined scalars. TYPES:90-97 "it introduces no membership"; TYPES:308-312 stated implicitly.
- F:2823-2848, 2086-2112, 5941-6016, 6945, 7211-7213, 7221, 7637-7646, 8110-8116, 8807-8824, 8753, 8533-8535, 10614-10616; F:10896-10916 by code shape.
- FCP:762-764 live views describe live referents. FC:546-561 "absence denotes a crossing or bodyless provider and adds no fence". FE:2980-2981 geometry deferred to "Checked return sources"; FE:13-14 an empty referent span is unresolved evidence.
- FM:159-180 builtins and fact-only citations write nothing. FO:1473, 763. FOC:361-363. FOV:188-190, 971-973. FOW:52-64, 1074-1078. FP:1012 for the immutable-copy rule. FSV:3022-3044.
- FT:2805-2811 the declared type bounds an unsnapshotted argument. PROD:945, 904-905, 344-352. PUB:8382-8384 "identity alone never establishes membership".
- PBP:46-49, 830-833. PCC:5143-5236 absence means "unexamined" (consumers not read).
- SH:1385-1398 toolchain-source selections stay late-bound (SH:921-924). SH:2951-2988 receiverless untargeted calls in contract position are proof views (SH:2867-2884). VA:1731-1735.
- VS:7850; VS:11512-11519 "Source type/loan/scope/precondition validation remains the admission owner."; VS:10839-10845.

### Classed "traced" (the reader followed callers)

- ETU:8144 reachable only if two supply-mode tests disagree. CALLS:7249-7251, 7542-7544. CLEAN:2549-2551 (scalar parameters only), 1803-1811. CTRL:6767-6768 partly.
- SEQ:5385-5390, 5406 the `true` fallbacks only delay retirement. SG:1898-1903, 3436-3438, 7864-7866, 10002-10004.
- TYPES:277-281 a missing definition is marked incomplete, not complete (partly).
- F:3271 children are examined separately; F:6530-6538 an unresolved dependency falls back to the fact's own place only (F:6988-6995); F:7747-7770; F:8646; F:10463 partly.
- FCP:1245-1251 within the file only. FE:5109-5123, 9512 unknown completion counts as possible. FE:11002-11004 an `Err` argument order returns before `invoke` (traced locally).
- FOC:771, 95-104. FOV:758-782. FP:1163. FRP:1090 fail-closed through FM:211-225. FRB:113 traced to `value_provable_domain` at checks_contracts_exits.rs:4379-4384.
- FT:2798-2803. PROD:397, 434. PUB:7890-7895, 1116-1175, 5206-5208 (unreachable).
- PCC route-drop sites (each needs a decided `false`). PF:2176-2178 backstopped by PF:1396-1425. PF:8301, 8703.
- SH:2455-2458 with 2444 (unknown operand types only widen candidates, and exactly one is required). VA:1527, 1641, 1490-1492, 642 (carrier range is an over-approximation), 2807-2822, 1860-1872. VS:3454.

One entry that is a possible panic, not an accept: `negate` indexes the arena without a validity guard while "false" is the invalid handle (F:3757, 3592); open, `Arena::get` not read.

## Silent drops

Places where failing to plan or resolve leaves no row and no diagnostic. 62 numbered entries from the records; an entry is one cited line range, except entries 13, 61 and 62, which group several ranges of one function family:

1. ESC:9561-9598 root not recorded when the expression does not plan. 2. ESC:10538-10543 whole state skipped. 3. ESC:10590, 10597 completion result discarded. 4. ESC:6697-6733 empty cleanup span.
5. ETU:5116-5164 and 6. ETU:6403 unlisted kinds add no transfer.
7. CALLS:6172-6174 qualification rows removed. 8. CLEAN:2549-2551 disposition skipped. 9. CTRL:109-114 membership skipped. 10. CTRL:2546-2563 predicate with no call. 11. CTRL:1670 no guard root. 12. CTRL:6767-6768 storage operand skipped.
13. SEQ:1318, 1350, 1424 nothing planned. 14. SEQ:9220-9228 assembly and fact-only statements skipped.
15. SSS:3084-3086 and 16. SSS:3051-3056 assignments skipped.
17. TYPES:277-281. 18. TYPES:4820-4847. 19. TYPES:1093. 20. TYPES:4653-4659. 21. TYPES:4760-4762. 22. TYPES:4932-4958. 23. TYPES:90-97 (domain predicates and memberships not retained).
24. F:742-748 no grouping edge. 25. F:1173-1209 return predicate facts dropped. 26. F:1469, 1506 store not recorded. 27. F:4146-4174 operator effects not applied. 28. F:4378-4392 operator ensures not published. 29. F:4652-4656 no flow state row. 30. F:9426-9482 dependency rows skipped. 31. F:10759-10768 and 32. F:10799-10804 cleanup dependencies skipped. 33. F:11465-11477 selector argument skipped. 34. F:2823-2848 no exit origin row.
35. FC:373 no carry transfer. 36. FC:686-691 parameter facts skipped.
37. FX:1213-1217 no observation fact. 38. FX:1433-1436 exit recorded with invalid handles.
39. FE:9747 a statement or transition call with no matching invocation produces no call fact. 40. FE:11002-11004 call never emitted on an `Err` order. 41. FE:9538-9542 an `Err` order yields no operands. 42. FE:3804 selectors deeper than 8 not recorded. 43. FE:4884-4939 no call stores. 44. FE:6395, 6502 descriptor permission left unavailable. 45. FE:8996-9002 non-fixed-index leaf dropped (inferred by the reader).
46. FOW:67-69 no move events. 47. FOW:869-911 neither moved nor observed. 48. FRB:1271-1281 no handbacks. 49. FSV:518-541 no delivery. 50. FSV:329 propositional `requires` facts ignored. 51. FT:3867-3906 no transfer facts on any refusal.
52. PUB:6796-6806. 53. PUB:10387-10391. 54. PUB:7890-7895. 55. PUB:4465-4521. 56. PUB:2483-2485.
57. PF:6637-6643 with 5356-5377 index compatibility not checked. 58. PF:8290-8292, 8394-8399 operator uses skipped. 59. PCC:5143-5236 no crash-call record. 60. PCC:815-870 no crash edge.
61. SH:1028-1030 and 4685-4687.
62. VS: rows dropped at 78-107, 231-252, 979-1001, 1328-1399, 4585-4601, 4657-4672, 5174-5191, 5429, 8119-8135 (counted as one family; unit 65 "five things" 3 says consumers outside the file must detect the gap).

Backstopped by a later diagnostic according to a record: PF:895-914 (by PF:1396-1425). Reported as a plan gap rather than silently: dropped machines (ETU:7384-7419) and construction traces (CTRL:5251-5445).

## Unused or unwired

With the readers' grep counts ("occ" includes definitions and re-exports). "(checked)" means I re-ran the grep.

- `run_caller_first` SH:3637: 2 occ, the definition and an in-file test; `#[allow(dead_code)]`; "No phase uses this order yet" (SH:3633). (checked)
- `MachineCallComponents` SH:3566: documented for termination, mutation, crash and reach summaries (SH:3541-3563); one outside user, checks_termination_progress.rs:370, 387.
- `scalar_parameter_roster` VS:4340: 12 occ, all in VS, though documented as the roster every lowerer takes (VS:4335-4339). (checked)
- `lower_scalar_parameter_range_requirements` VS:8049: one call, VS:8275; not re-exported. (checked)
- `family_attached_data_symbol` FM:2177: `pub(crate)`, 2 occ, definition and one in-file use. (checked) `rebase_descriptor_slot_regions` FM:1671: 2 occ in-file.
- `expression_is_contextual_statement_primitive`, `expression_is_intrinsic_primitive_without_origin` SH:1835, 1887: 11 occ, all in SH. (checked) `checked_statement_call_target`, `checked_call_target`, `checked_name_path_segment_target`, `checked_operator_conformance_targets`, `implementation_call_selection`: `pub(crate)`, no uses outside SH. `GenericOperatorValueOrigins` SH:2082: only inside SH.
- `append_writes` F:1397, `captured_before_snapshot` F:1597, `before_snapshot` F:1582: `pub(crate)`, each one call, in-file. (first two checked)
- `CallValues.fields` FT:3654: only ever built as `Vec::new()` (FT:2195, 2456), because receiver calls return `None` at FT:2347; its readers search an empty list. (checked)
- `prune_checked_tree_product` PROD:1119: only caller is `select_checked_tree_product` (PROD:1107), which has one external caller (psi-compiler `source_checking.rs:834`); re-exported at lib.rs:79. (checked)
- `receiver_call_writes` PEV:1262: its `find` predicate is the constant `false`, so it returns `true` on every input. (checked)
- Unused parameters: `_call_frames` SEQ:9009 (SG:9855 still passes it); `_exact_integer_casts` VS:3029; `_before_statement` PEV:2266; `_borrow`, `_proof`, `_semantic` FCX:412-414; `machine_symbol` ignored at FT:4554; `_receiver` computed and unused at PUB:6080-6083.
- Dead or inert code: both branches `continue` at VS:5169-5172 (checked); `SuccessorGuard` has 16 variants used only to spell a trace phase (SG:6193-6247); macro `keep` only forwards (VA:1156-1160); an orphaned comment at PEV:2118-2125; `build_carry_facts` fills only `data` and leaves the other `CarryFacts` fields empty (PF:643-661).
- Used only inside their own file: `fixed_byte_array_view_is_admitted` and `fixed_array_slice_view_is_admitted` (10 hits, all CALLS); `CapturedCaseSubjectFailure` (9 occ, FO only); `crash_route_expressions_by_identity` and `solve_private_summary_fixed_point` (3 each, PCC only); `bind_checked_body_call_source_spans` (2), `retain_selected_receiver_loans` (3), `derive_authored_operator_crash_buckets` (4) in PF; `append_statement_ownership_events` (2), `is_private_result_place` (2) in FOW; `lexical_scope_covers_call` (2) in FSV.
- Corrected: unit 57 marked `checked_copied_result_type` (CTRL:4726) "suspected unused or used only by path from another file". Grep gives 3 occ: the definition, an import at SEQ:65 and a call at SEQ:273. It has one caller.
- Not counted by the readers: `calling_signatures::collect`, `ScalarCalleeClosure`, `PreparedCalls::state_schedules` (ETU); `SelectedCopies::for_destinations`, `DeferredFailure`, `ViewSubslice::transfer`, `ComputationCallArguments::subslice_argument` (unit 57); `add_affine_array_construction_type` TYPES:3693, a pure alias of `add_type`; five of the `package_review` wrappers in PROD.

## Stated limits

- "their current coverage differs, so a failed candidate is not permission to omit its unsupported statements." (ETU:26-27)
- "What remains open is a contract that names an erased binding" (ETU:95-100).
- "One known boundary: Psi 06's `parameter_declaration` omits the `[erased]` predicate" (ESC:8898-8901).
- "This is intentionally not a general local-alias resolver." (CALLS:6965)
- "Runtime endpoints need separate extent evidence; this route retains only canonical fixed bounds" (CALLS:1357-1358).
- "A forwarded in-scope binder or a missing row leaves the requirement unplanned." (CTRL:1160-1162)
- "The trace is diagnostic only: it never changes which bodies are admitted." (CTRL:5240-5241)
- "does not reconstruct a second permission partition or dispose a selected live linear obligation" (CLEAN:1007-1009).
- "An open selector or a field this walker cannot size fails the plan." (SEQ:8595)
- "A decision may retain a constant obligation. This permits planning only; machine lowering refuses pending obligations" (SG:1196-1197).
- "What remains refused is a custody or proof condition, not an arrangement" (SSS:36-38).
- "The planner proves no bound for a runtime element: Terminal re-proves `index < extent`" (SSS:2370-2371).
- "Multiple reference leaves within one parameter currently share its footprint" (F:7317-7318).
- "This is a may-effect set" (F:10523-10524); "This is not proof of the selected hooks' requirements or guarantees" (F:10687-10692).
- "Unknown predecessors retain the public envelope; every known alternative contributes" (F:11437-11438).
- "An opaque, unresolved, generic or dispatched callee ... retains the signature/authority ceiling" (FM:26-28).
- "Conditional aggregates and every n-ary shape wait for P1c path mappings." (FC:263-270); unit 60 notes this comment sits above `memoized_call_target_return_type` (FC:271), not the carry function it describes (FC:317).
- "membership and proposition-application clauses still wait for their carriers." (FX:439-441)
- "Unknown (no resolved operator) preserves the legacy behavior" (FOW:1237-1238); "a future copy/drop policy" (FOW:1606-1609).
- "Ordinary machine applications contribute unknown inputs to callable non-entry states" (FSV:2-4).
- "Runtime-indexed segments still stop the transport" (FT:1351-1352); "A cycle requiring induction or a missing selected operation withholds the whole capture" (FT:1890-1892).
- "Arena-resident rows keyed by non-machine identities ... remain as retained dead evidence" (PROD:270-274); "the phase does not re-extract the edges of its own output to confirm it." (PROD:276-278)
- "Transitional exact typed-expression custody for source forms whose artifact-reconstructible Terminal coordinate has not landed yet." (PUB:2873-2876)
- "outcome-guarded and crash contract rows remain unsupported" (PUB:5893).
- "Mutable bindings and non-binding value transformations remain fail-closed." (PUB:4435-4436)
- "missing evidence leaves the possible crash in place" (PCC:6-8); "a dependency outside the recognized local/capsule graph keeps its caller unexamined" (PCC:5066-5068).
- "Unsupported clauses fail closed; a missing Boolean recognizer cannot erase a requirement" (PF:830-831).
- "The scan is deliberately syntactic and conservative: anything it cannot rule out dirties the snapshot" (PEV:724-726); "This is source provenance, not a Terminal certificate." (PEV:31)
- "symbolic statement boundary applications are not yet supported" (SH:4013).
- "Implicit match-equality comparisons carry no authored operator token of their own, so they stay out of this cohort for now." (SH:4142-4144)
- "Legacy floating ranges retain `None` because their separate endpoint roster has not joined this predicate owner" (VS:8020).
- "A generic operator has no closed telescope yet" (VS:3285-3286); "its native replay is still owed a Boolean re-view form" (VS:9718-9719).
- Performance notes inside comments: "the product program asks 466 times about 231 distinct types" (ESC:1509-1516); "std's 193 literals made one loop counter take 183 passes" (FSV:1189-1190).
- No iteration cap is visible in the `build_check_facts` loop (PF:11059-11129; a record's observation, not a comment). Panics by `expect` or `assert`: F:5769, 5773, 6095, 6104; FX:1497, 1501; FM:3963; FSV:3659, 3727; PF:11505-11514; PCC:59 to 5378 (list in unit 63).

## Disagreements

1. **Do `flow_stored_values.rs` and `flow_stored_views.rs` exist?** Unit 60 named them as suspected sibling files, from import paths. Gap 016 records `stored_values` and `stored_views` as inline modules at F:1360-1558 and F:1559-1793. The directory has no such files (checked by listing). Gap 016 is right.
2. **Is `checked_copied_result_type` unused?** Unit 57 suspected it; unit 58 did not mention it. It is called at SEQ:273 (checked). Not unused.
3. **Where is `canonical_places_agree` defined?** Units 56 and 61 attribute it to flow.rs. The definition is validation `places.rs:546`; F:11818 re-exports it (checked).
4. **How many functions append a semantic context?** Unit 60 named two (FX:35, FCX:462); gap 017 named a third (F:9229) without the other two. All three exist (checked).
5. **How many modules in shared.rs?** Unit 64 says "six" in two places and names five. The file has five top-level modules and its header lists five (SH:9, 3387, 3412, 3540, 3777; checked).
6. **`package_review` wrapper callers.** Unit 62 left seven wrappers as "suspected"; gap 017, unit 63 and unit 64 give callers for three (`derive_checked_semantic_dependencies`: checking.rs:1183 and review-evidence; `infer_checked_crash_causes`: review-evidence `policy.rs` and checks.rs; `derive_checked_nominal_call_target`: review-evidence). Not a contradiction; the later records fill the gap. Not re-checked by me.
7. **Receiver aliases.** Unit 59 flagged SSS:1006-1009 without reading `receiver_aliases`; unit 56 read it. The code agrees with both: `aliases` returns `None` when the flow or borrow state lookup fails (ETU:3374-3377) and SSS turns that into an empty list (checked). Still open in both records.
8. **First pass vs second pass on flow.rs.** No contradiction: unit 59 read F:1-1200 only and claimed nothing beyond; both gap records say they found none.
9. **Code vs comment, reported by one record each:** ETU:9-11 calls `build_checked_unit_effect_plans` "its test-only form" of itself; grep finds one definition (ETU:7079) and one caller (checked). FT:2275-2278 says "complete storage footprint is empty" while the code checks `observation_places` (FT:3599-3602; not re-checked).

## What I checked myself

30 claims opened in the checkout. 27 match, 2 partly, 1 does not match.

| # | Claim | Site | Result |
|---|---|---|---|
| 1 | Types match on equal base name when symbols differ | ESC:8329-8330, 8348-8349 | matches |
| 2 | Three call routes accepted with no check | ETU:2359-2364 | matches |
| 3 | `value_kind` ends in `_ => {}` | ETU:6403 | matches |
| 4 | `receiver_call_writes` always true (constant-false `find`) | PEV:1262-1273 | matches |
| 5 | `Assembly` statement never overwrites | PEV:1014 | matches |
| 6 | No recorded operator use counts as builtin | VS:4480-4489 | matches |
| 7 | `.or(Some(places))` after failed alias closure | FM:4604-4612 | matches |
| 8 | `lower_carrier` only returns `Some` | VS:208-254 | matches |
| 9 | `run_caller_first` has no production caller | SH:3637, 3769 | matches (2 occ) |
| 10 | `family_attached_data_symbol` used only in its file | FM:2177, 2210 | matches (2 occ) |
| 11 | `scalar_parameter_roster` has no outside user | VS | matches (12 occ, one file) |
| 12 | `checked_copied_result_type` suspected unused | CTRL:4726 | does not match: one caller at SEQ:273 |
| 13 | `lower_scalar_parameter_range_requirements` one call | VS:8049, 8275 | matches |
| 14 | `prune_checked_tree_product` only called in PROD | PROD:1107, lib.rs:79 | matches |
| 15 | `append_writes`, `captured_before_snapshot` in-file only | F:1397, 1447, 1590, 1597 | matches |
| 16 | Two SH predicates have no outside users | SH | matches (11 occ, one file) |
| 17 | `CallValues.fields` only built empty | FT:2195, 2456, 3654 | matches |
| 18 | Two crash predicate functions are identical | PF:4413-4435, 4475-4497 | matches |
| 19 | Semantic-context appenders duplicate each other | F:9229, FCX:462, FX:35 | matches; three, not two |
| 20 | Unknown or invalid root is never invalidated | F:6913-6920 | matches |
| 21 | Zero descriptor rows agree | FO:1257-1269 | matches |
| 22 | Cycle guards keyed by symbol in one walker, name in the other | PUB:8966-8970, 9158-9162 | matches |
| 23 | Carry transfer accepts label equality | FC:420-424 | matches |
| 24 | `symbol_key` defined 9 times | omega-rust | matches |
| 25 | Read and write footprint fixed points are the same job | F:7827-7883, FM:2725 on | partly: same shape and shared helper, different access kind and scheduling |
| 26 | `fused_target_index` repeats `activation_target_state_index` | SG:1161-1173, call_cycles.rs:3285 | partly: same rule, different roster |
| 27 | Dead branch | VS:5169-5172 | matches |
| 28 | Non-interval domain constraint adds no slot | VS:8119-8135 | matches |
| 29 | Header names a "test-only form" that is the same function | ETU:9-11, 7079 | matches |
| 30 | `stored_values` / `stored_views` are inline modules, not files | F:1360-1793 | matches gap 016; unit 60's suspicion does not |

I did not re-check the readers' "wrong program" examples; none of them was traced to a reachable program by a reader or by me.

## Not covered

- Records not read: none. All 12 were read in full.
- Source ranges with no record: none. F:1-1200 is in unit 59, F:1201-7200 in gap 016, F:7201-11937 in gap 017; every other file in the ten unit tables was reported read in full.
- Depth limits the readers stated: unit 57 read its longest functions in one pass and did not diff the duplicated arm sequences line by line; unit 60 summarised the largest FE functions "by their decision points, not line-by-line" (FE:4198-4758, 2989-3276, 6534-6729, 5536-5814).
- Outside this area and not described here: the other 26 files of the same crate (`checks*.rs`, `borrow.rs`, `execution.rs`, `checking.rs` and others) and the `validation` crate, which many "suspected" entries point into (`CallFrameResolver`, `completed_storage`, `storage_path_relations`, `places.rs:665`).
- The 96 spelling sites, 124 open entries and 62 silent-drop entries are counts of what the records cite, not of what the code contains.
