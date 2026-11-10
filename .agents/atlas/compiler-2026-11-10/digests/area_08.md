# Area 8: Psi stage 05: borrow ledger and the checks

Source: 12 reader records (units 046 to 055, plus second-pass records `unit_051_gap_014` and
`unit_051_gap_015`), all read in full. Checkout `worker-23` at `457adf4a11f`.

File names below are basenames. `preparation.rs` and `selection_and_reflection.rs` are in
`omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/`. Every other file is in
`omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/`.

Words kept from the records: "open" = the reader saw no reason at the site and did not trace it;
"stated" = a comment at the site gives the reason; "traced" = the reader followed it; "suspected" and
"inferred" = the reader did not read the other site or did not follow it to the end.

## What this area does

This area takes a program whose types are already known and decides whether it is allowed to run. First
the tail of stage 04 finishes the typed program (fills in generic arguments, expands the compile-time
reflection queries, freezes it). Then stage 05 builds a ledger of every borrow ("loan") in every state
of every machine, and runs a fixed sequence of checks over that ledger and over the flow facts: borrow
conflicts, enum case access, contracts (`requires` at calls, `ensures` at exits), ownership of linear
values, crash-route coverage, array index bounds, and termination. Each check either pushes
diagnostics or records rows that later stages (lowering, native) consume. This area's units cover 28
files (2 in stage 04, 26 in stage 05) of 1,400 to 12,300 lines, most made of nested inline modules.

**Stage 04, `preparation.rs` (2,137 lines).** In: a mutable `TypedTrees` and a `PreparationRequest`.
Out: a frozen typed program plus `TypedPreparation`. `prepare_typed_trees` (`preparation.rs:1941`)
runs one-shot passes around a specialisation loop that stops when a round materialises nothing
(`preparation.rs:1985-2018`).

**Stage 04, `selection_and_reflection.rs` (4,260 lines).** In: call sites with static arguments, the
toolchain's `reflect.omg` declarations, inferred-type locals. Out: `CallSelection` rows, generated
machines and statements for reflection, refreshed local types (`selection_and_reflection.rs:1-5`,
`:1551`, `:2518`, `:4103`).

**Stage 05, entrance and order: `checking.rs` (1,408), `checks.rs` (12,259).** `lower_typed_trees`
(`checking.rs:793`) runs validation, fact building, the checks, and publishes `CheckedTrees`
(`checking.rs:1012-1408`). `check_checked_facts_recording` (`checks.rs:12022`, one caller
`checking.rs:1139`) runs the checks in a fixed order and also writes facts (crash plans, carry facts,
proven indexes, qualifications; `checks.rs:12080-12220`). `checks.rs` itself holds about 12 inline
checks (atomic fences, case access, runtime indices, stored integer calls, suspension carry,
ownership occurrence order, cleanup admission, crashes, indexed writes).

**Stage 05, the borrow ledger: `borrow.rs` (9,506).** In: the typed program and call frames. Out:
`BorrowFacts`: per state, the loans each statement forms, call argument accesses, each loan's last
use (`borrow.rs:9437`, built per machine in parallel `:9452-9471`).

**Stage 05, borrow checks: `checks_borrows.rs` (11,526), `_calls.rs` (1,742), `_overlap.rs` (3,377),
`_resources.rs` (7,148).** In: `BorrowFacts`, flow facts. Out: diagnostics, and resource rows written
into `facts.borrow` (`checks_borrows_resources.rs:7073-7111`) that lowering reads.
`captured_place_compatibility` (`checks_borrows_overlap.rs:2924`) is the one spatial judgment under
reads, moves, call arguments and aggregates.

**Stage 05, contracts: `checks_contracts.rs` (11,876), `_calls.rs` (4,723), `_exits.rs` (9,918),
`_prover.rs` (8,071), `_writes.rs` (2,313), `checks_current_scalar_values.rs` (6,740),
`checks_content.rs` (7,295).** In: flow contexts, contract facts. Out: diagnostics, deferred
obligations, qualification rows. A call's `requires` is proved by about twenty OR'd routes
(`checks_contracts_calls.rs:3270-3372`, `:99-186`); an exit's `ensures` likewise
(`checks_contracts_exits.rs:9777-9852`).

**Stage 05, ownership: `checks_multiplicity.rs` (10,930) and five `checks_multiplicity_*.rs` files
(15,340 together).** In: flow ownership events. Out: permission events, claim-outcome maps, cleanup
dispositions, diagnostics. `checks_multiplicity_linear_validation.rs` both produces the events and
replays them (`:2643`, `:4193`).

**Stage 05, operators, ranges, termination, execution plans: `checks_operators.rs` (2,945),
`checks_ranges.rs` (11,251), `checks_ranges_facts.rs` (5,101), `checks_termination.rs` (8,421),
`checks_termination_progress.rs` (4,429), `execution.rs` (2,879).** Out: operator diagnostics, proven
index handles (`checks_ranges.rs:10964`), one `CheckedRankBodyJudgment` per machine
(`checks_termination.rs:8218`), progress summaries (`checks_termination_progress.rs:4348`), terminal
plans (`execution.rs:268`).

## How it decides

**Identity (symbol, handle).** Parent of a reborrow by loan handle (`checks_borrows_resources.rs:2812-2836`);
case access by recorded formation rows (`checks.rs:495-764`); claim joins by
`PermissionClaimIdentity` (`checks_content.rs:3232-3250`,
`checks_multiplicity_claim_outcomes.rs:1771-1856`); callee summary choice
(`checks_termination_progress.rs:1558-1633`); exit `ensures` skips by contract and expression handle
(`checks_contracts_exits.rs:9676-9715`).

**Place (root plus path segments).** Loan overlap and retirement (`borrow.rs:3513-3622`);
`captured_place_compatibility` (`checks_borrows_overlap.rs:2924-3015`); linear place tracking
(`checks_multiplicity_linear_validation.rs:491, 576, 681`); borrowed-storage windows
(`checks_multiplicity_borrowed_windows.rs:1721-1762`); remaining custody
(`checks_multiplicity_cleanup_dispositions.rs:826, 1225, 1294`).

**Value (integers, intervals, scalar equality).** Index bounds (`checks_ranges.rs:6592-6716`); runtime
index descriptors (`checks.rs:2877-2881, 2990-2996`); call bounds (`checks_contracts.rs:4941-5175`);
rank decrease by `proves_values_relation` (`checks_termination.rs:1716-1847`).

**Position (statement index, call ordinal, parameter position).** Call acknowledgements by
`(statement_index, call_ordinal)` (`checking.rs:329-332`); lifecycle order by
`(statement_index, phase)` (`checks_borrows_resources.rs:2464-2468`); family summary substitution by
position (`checks_termination_progress.rs:1279-1283`); permission rule order: the first of thirteen
rules that answers decides (`checks_borrows_resources.rs:3617-4022`).

**Source spelling (names, labels, `display_name`, `format!` of debug values).** By my count from the
records there are **17 cited sites in stage 04 and 131 in stage 05** (a site is one cited line or line
range; diagnostic-only uses are marked "d"). On top of those, the whole range-fact store is keyed by
rendered labels: `display_name(` appears 85 times in `checks_ranges.rs` and 9 in
`checks_ranges_facts.rs` (unit 054 grep count), with a typed read table deciding which labels survive
a write (`checks_ranges_facts.rs:2634, 3531-3603`).

Stage 04 (17):
- generic binder matched by name: `selection_and_reflection.rs:1082-1087`, `:1099-1103`, `:891-919`;
  domain family by name or last `::` segment `:1333-1342`; const literal found by printed value
  `:88-91`, `:164-167`, `:1272-1278`; evidence conflict by symbol and `display_name` `:851-852`; const
  identity by text `:1352-1364`.
- reflection: file path equals `reflect.omg` `:2506-2516`; declaration names `"schema"`,
  `"inspect_record_fields"`, `"visit_active_fields"` etc. `:2537-2552`; data names `"CaseInfo"`
  `:1820`, `"FieldInfo"` `:2663`; info fields by name `:1895`, `:1939-1966`, `:2409`, `:2971-3004`.

Stage 05 (131), by file:
- `borrow.rs` (5): `uses_local_name` compares `member.as_str() == local_name` `:1995-1998`, `:2036`,
  `:2077`, `:2138`; case selection by name `:4408`.
- `checks.rs` (13): carry by type name `:5620-5623`, `:5640`, `:5656-5662`; claims grouped by
  `place_label` `:5419-5421`; `fallback_name` `:5947-5953`; claim policy by label vs value name `:7267`,
  `:7334-7345`, `:7726`; crash root "sensitive" by name `:9367-9371`, `:9377-9381`; crash predicate
  identity encodes parameter names `:10401-10402`, `:10639-10645`; `starts_with("__transition_subject#")`
  `:10967`.
- `checks_borrows.rs` (4): subject by `state.name == "entry"` `:301-307` (d), `:609-613` (d); scope
  compare by name `:267`; persistent paths compared to frame strings `:1462-1518`.
  `checks_borrows_calls.rs` (2): `target.as_str() == "prepare"` `:1605`, `:1613`.
  `checks_borrows_overlap.rs` (3): field names in `where` facts `:3345`, `:3351`; attached field by
  symbol name `:3140-3145`.
- `checks_content.rs` (11): `:3821-3838`, `:3890-3914`, `:5286`, `:5308-5317`, `:5174`, `:6258`; sort
  keys by debug text `:4367-4372`, `:4395-4401`, `:4403-4408`; lifetime text `:6418-6420`, `:6459`.
- `checks_contracts.rs` (17): `:1126`, `:1370`, `:1384`, `:2617`, `:2647`, `:2719`, `:2771`,
  `:2774-2781`, `:3157-3168`, `:3734`, `:9769-9777` (`"is_finite"` under `F32|F64|Float`), `:10293`,
  `:10442-10443`, `:10814-10816`, `:11020-11021`, `:11305`, `:11575-11580`.
- `checks_contracts_calls.rs` (10): `:1963`, `:1993-2013`, `:2218`, `:2244-2247`, `:2368`,
  `:2709-2710` (splits on `" && "`), `:2915`, `:2968-2975`, `:3146-3149`, `:3534` (d). Stated in the
  code: "Contract comparison is currently label-based" (`:1987-1988`).
- `checks_contracts_exits.rs` (6): atom identity is a `format!` of debug handles `:1368`, `:2237`,
  `:2877`, `:3315`; machine name prefix `@const-qualification-` `:3690-3692` (d); field name fallback
  `:8394-8398`. `checks_contracts_writes.rs` (1): field and case name strings `:1999-2000`.
- `checks_contracts_prover.rs` (16): atom identity strings `:1918`, `:2022`, `:2087`, `:2121`,
  `:2374`, `:3475`, `:4105`, `:6969`, `:7104`, `:7321`; const decode by name `:3548`, `:3615`,
  `:3634-3635`, `:3645-3650`; proposition label compare `:6366-6373`; carrier type by name `:7867`.
- `checks_current_scalar_values.rs` (3): `:608`, `:630-640`, `:1986-1995`.
  `checks_multiplicity.rs` (4): `:5221-5227`, `:10840`, `:10908`, `:10911`.
- `checks_multiplicity_claim_outcomes.rs` (3): `:3946-3955`, `:4273-4276`, `:4311-4312`.
  `checks_multiplicity_linear_validation.rs` (1): `:1672-1676`.
  `checks_multiplicity_permission_events.rs` (2): `:1961`, `:1972`.
  `checks_operators.rs` (4): `:1813`, `:2200-2212`, `:2295-2310`, `:2352-2354`.
- `checks_ranges.rs` (7): guard paths as strings `:944/1004-1007`; parameters paired by name `:1096`,
  `:1262`; `len` spelling `:5897`, `:5907`; `starts_with("__hoist_")` `:6517-6530`;
  `PrimitiveType::from_name` `:10600`.
  `checks_ranges_facts.rs` (6): `:1378-1396`, `:4098-4109`, `:4478-4482`, `:4500-4512`, `:4821-4833`,
  `:4960-4967`.
- `checks_termination.rs` (13): view choice from the `view_path` string `:2659-2711`, `:2737-2741`,
  `:2745-2762`; catalog authenticity by identity string and names `:3091`, `:3109-3121`; countdown tier
  matches parameters by symbol or name `:4152`, `:4297`, `:4438-4447`, `:4488-4491`; static value from
  spelling `:7817-7850`; `:2867` (d), `:2889-2892` (d), `:3028` (d).
- `checks_termination_progress.rs`, `execution.rs`, `checking.rs`,
  `checks_borrows_resources.rs`, `checks_multiplicity_borrowed_windows.rs`,
  `checks_multiplicity_cleanup_dispositions.rs`: none for decisions, per their records.

## The same job in more than one place

Sites that readers read on both sides unless marked "suspected". Pairs I opened are marked "checked".

1. **Path equality (`same_path`), 4 copies.** `borrow.rs:2825`, `checks_borrows.rs:9894`,
   `checks_borrows_resources.rs:6172`, `flow_retained_borrows.rs:1181`. Unit 046 "suspected"; checked:
   the four bodies are the same; three call `validation::storage_path_relations::canonical_place_segments_equal`
   and `checks_borrows.rs:9894` calls the `crate::flow` re-export of the same function (single
   definition `validation/src/places.rs:505`, unit 049).
2. **`owner_path_matches`, not the same job.** Unit 046 "suspected" `borrow.rs:3557` and
   `checks_borrows.rs:4121` were duplicates. Checked: the first is a prefix match that lets a dynamic
   index match any index segment; the second is exact equality of a loan's owner path with a path.
3. **"Definite slot replacement" test, at least 5 copies in `borrow.rs`.** `:1591-1603`,
   `:1670-1677`, `:4905-4915`, `:4953-4960`, `:9094-9103` (unit 046).
4. **`append_distinct`, 3 copies plus one inline.** `checks_borrows.rs:7258`, `:7460`, `:9905`,
   inline `:7547-7551`. Checked `:7258` and `:9905`: same comparison (place and lineage); `:9905` takes
   an iterator.
5. **`CapturedPlace` four-field construction, 9 hand-written copies** in
   `checks_borrows_resources.rs`: `:323`, `:1547`, `:3012`, `:3181`, `:3637`, `:4225`, `:4798`,
   `:4873`, `:4954` (counted by eye, unit 049).
6. **"Selected alternatives" handling, 3 copies.** `checks_borrows_resources.rs:141-178`,
   `:3758-3777`, `:5568-5597`.
7. **Weakening phase order encoded twice.** `checks_borrows_resources.rs:2640` and `:2747`.
8. **Single-name expression to immutable local or parameter symbol, 4 ways.**
   `checks_borrows.rs:992`, `:1288`, `:1309`, `:1616`.
9. **Segment overlap or equality, re-implemented for smaller vocabularies.**
   `checks_borrows_overlap.rs:2665-2829` vs `checks_borrows.rs:1352-1394` and `:1586`;
   `checks_multiplicity.rs:4435` vs `validation/src/places.rs:628` (suspected) vs
   `validation/src/arithmetic_domains.rs:22096` (string-typed, suspected).
10. **"Is this place behind a reference prefix", 5 loops.**
    `checks_multiplicity_borrowed_windows.rs:1932-1945`, `:1215-1222`; `checks_multiplicity.rs:8626-8640`,
    `:2303-2314`, `:8839-8855`.
11. **Does this guard or premise establish the required Boolean, 4 matchers** all reached from one
    `proves_case` chain: `checks_contracts.rs:2374` (structural), `checks_contracts_calls.rs:1652`
    (builtin-gated with copy aliases), `checks_contracts_calls.rs:2910` (label string),
    `checks_contracts.rs:10353` (interval over a spelled place). A fifth and sixth:
    `checks_contracts_prover.rs:5193` (`predicates_match`) and `published::facts::boolean_predicates::proves`
    (suspected, called at `checks_contracts_prover.rs:4415, 5982`).
12. **Does a call requirement hold from integer facts, 4 routes.** `checks_contracts.rs:40-207`,
    `:3173-3924`, `:3934-5391`, `:8673-8747` (through `CurrentScalarValues`). Comparison from low/high
    bounds written at `:88-99`, `:3241-3323`, `:5164-5174`.
13. **Label proof by string compare, 3 places.** `checks_contracts.rs:2617-2651/2719-2781`,
    `:3071-3115`, `:11005-11039`. Two label toolkits: `checks_contracts_calls.rs:1889-2036` (rewrites
    label text) and `checks_contracts.rs:10681-11281` (renders from trees); a third hand renderer
    `checks_contracts_calls.rs:2818-2890`.
14. **Domain membership proved in 3 places.** `checks_contracts_prover.rs:35-592`,
    `checks_contracts_exits.rs:2628-4762`, `checks_contracts_writes.rs:337-1548`; each with its own
    recursion guard (unit 051). `exact_scalar_membership` (`checks_contracts_exits.rs:3906`) vs
    `carried_membership_at` (`checks_contracts_prover.rs:171`): same scan, different match rule.
15. **Opaque arithmetic atoms, 5 identity formats.** `checks_contracts_prover.rs:1918/2087`
    (`\0call-guarantee`), `:2022` (`\0call-result`), `:2374/3475/4105` (`\0static-parameter`),
    `:6969/7104/7321` (`\0instruction-*`), and `checks_contracts_exits.rs:2237` (`\0{scope}`).
16. **Did a write disturb a captured actual, 2 ways.** `checks_contracts_prover.rs:4557`
    (`capture_preserved`, write ceiling) vs `:7360` (`post_binding_preserved`, binding agreement).
17. **Bounded backward demand transport, not the same job.** Unit 049 "suspected"
    `checks_contracts_exits.rs:1633` re-implements `checks_content.rs:54`. Checked: the first
    substitutes scoped arithmetic expressions for loop-header invariants (`TRANSPORT_WORK = 16_384`,
    `:1649`); the second rebases content-conservation plans across callee states
    (`TRANSPORT_WORK = 65_536`, `:80`). Same pattern (work counter, substitution), different objects.
    Their `same_type` helpers (`checks_content.rs:475`, `checks_contracts_exits.rs:179`) do the same
    job with different identity calls (`normalized_type_identity` vs `type_identity(ordinary)`); more
    `same_type` at `checks_contracts.rs:8364`, `published.rs:7840` (suspected).
18. **Recursive walks over the two-variant `ConservationTerm`, 6 or 7.** `checks_content.rs:437`,
    `:1015`, `:1107`, `:2937`, `:3579`, `:4316`, `:5604`, `:866-878`. Two claim-lineage walkers
    `:1348-1368`, `:1570-1592`.
19. **"Callee map has no entry for this path, so the claim is inactive", 3 copies with 2 lookups.**
    `checks_multiplicity_claim_outcomes.rs:3516-3525`, `:4015-4019`,
    `checks_multiplicity_permission_events.rs:2036-2082`. Checked: the last uses
    `program.state_by_symbol` (`:2060`), the first `validation::call_target_state` (`:3511`). Unit 053
    did not trace whether they can differ.
20. **Carry-claim policy fold, 3 copies in `checks.rs`.** `:5423-5433`, `:7271-7285`, `:7663-7677`.
21. **Full statement and expression walks, 4 in `checks.rs`.** `:5516/5575`, `:5780/5843`, `:8618`,
    `:9208`; `checking.rs:255-314` and `borrow.rs:1909-1963` repeat the child walk.
22. **Entry-provenance test for crash predicates, 2 traversals.** `checks.rs:9329-9344`,
    `:10239-10253`. `boolean_structure` decomposition redone over a second representation
    `checks.rs:9423-9465`.
23. **Event source to statement index, 3 functions.** `checks_multiplicity_linear_validation.rs:688-699`,
    `:4071-4078`, `flow.rs:9585` (suspected). `LinearPlace` 11-field literal written 6 times
    `:1123-1281`.
24. **Hash index with equality recheck, 3 flavours.** `checks_multiplicity_claim_outcomes.rs:15-18`,
    `:2216`, `checks_multiplicity_cleanup_dispositions.rs:36`.
25. **Ranges.** Edge builders twice (`checks_ranges.rs:1035-1060`, `:1683-1735`); `subject == true`
    unwrapping 4 times (`:1180-1194`, `:1579-1598`, `:3187-3205`, `:9409-9417`); `range_bound_is_proven`
    twice (`:8546`, `:10443`); three candidate-expression scans (`checks_ranges_facts.rs:676`, `:1043`,
    `:2394`).
26. **Termination.** "Does this state graph have a cycle" 3 ways (`checks_termination.rs:331`, `:428`,
    `:7511`); descending/distance/increasing arithmetic twice (`:1771-1846`, `:4038-4084`); Tarjan SCC
    here (`:365`) and suspected at `validation/src/machine_calls_core.rs:6872`, `shared.rs:3686`,
    `omega/.../01_abstract-operations-to-abstract-operations/src/lib.rs:199`; "statement write frame
    overlaps this place" 5 ways in `checks_termination_progress.rs:2581`, `:3567`, `:3716`,
    `:3380-3396`, `:3859-3875`.
27. **Execution.** Variants of a sum type listed 4 times (`execution.rs:459-484`, `:2145-2169`,
    `:2175-2194`, `:2565-2580`); guard-tail terminator selection twice here (`:2417`, `:2643`) and a
    third in `execution_terminal_unit_state_graph.rs:8473` (suspected, other area).
28. **Stage 04.** Name-substituting tree copy (`preparation.rs:1098-1142`,
    `selection_and_reflection.rs:3420-3462`); generated-local minting at 3 sites
    (`preparation.rs:1307-1313`, `selection_and_reflection.rs:2889-2897`, `:3844-3850`); the
    `native_code_layout` redirect twice (`selection_and_reflection.rs:545-561`, `:604-632`).

## Fixed lists of shapes

- Shared-cohort observation: only cohorts of 2 or 3 (`checks_borrows_resources.rs:814`); restored-call
  binding: "Unsupported shapes stay unbound rather than receiving inferred authority"
  (`:829-830`), cohort `1..=3` (`:868`). Failure scratch keeps 4 steps and 8 path segments (`:3279`,
  `:3287`).
- Reflection call operands must be `[context]` or `[subject, context]`
  (`selection_and_reflection.rs:2684-2688`); operand capture attenuation only for 8 integer primitives
  (`:3763-3771`).
- `align_of` refusals: 8 listed shapes (`preparation.rs:1608`, `:1682`, `:1691`, `:1695`, `:1705`,
  `:1712`, `:1735`, `:1743`).
- Atomic fence: exactly one argument and one operand (`checks.rs:109-120`). `arithmetic_spelling`:
  `Add`, `Subtract`, `Multiply` only (`checks.rs:12255`). Crash `boolean_structure`: "every other form
  -- including a disjunction -- is an atom" (`checks.rs:11030-11031`).
- Persistent borrow sources: string literal, cast, binary, borrow of a reference shell, static-returning
  call, immutable local with static initializer; all else false (`checks_borrows.rs:1656-1724`).
  Incoming backing: only `Field`, `Case`, `FixedIndex` segments (`checks_borrows.rs:2933-2946`).
- Recursive callable lifetime frontier: "unimplemented: recursive callable lifetime frontier requires
  cycle-aware applied telescopes" (`checks_borrows.rs:366-368`).
- Premise decomposition: six integer comparisons; non-negated `Or` and negated `And` contribute nothing
  (`checks_borrows_overlap.rs:2087-2094`, `:2193`).
- Retained cell: one variant with exactly two payload fields; "Shape alone is not authority"
  (`checks_content.rs:2053-2091`). Retained loan: "the bounded retained-loan rung requires exactly one
  content-bearing result domain" (`checks_content.rs:6415`); "only a whole direct reference is
  admitted" (`:6443`).
- Call bounds operands: integer literal, single-segment parameter name, member; else `None`
  (`checks_contracts.rs:5076-5152`). Guard implication: only `<=`/`>=` from `<`/`>`
  (`checks_contracts.rs:4773-4783`). Contradictory edge: exactly `When` transition then `Always`
  transition (`checks_contracts_calls.rs:1403-1417`). Case-requirement peel: `for _ in 0..8`
  (`checks_contracts_calls.rs:2734`).
- Reconstruction: pattern `(dividend / divisor) * divisor` only (`checks_contracts_exits.rs:6796`,
  `:6820`); cyclic headers: "Saturating and Trapping stay outside the language" (`:594-595`).
- `semantic_contexts_prove_contract_fact`: the explicit `false` arms list `InstructionEffect` and
  `CallCompletion` (`checks_contracts_prover.rs:6146`) and 14 more payload kinds (`:6229-6242`); none
  can be proved by this entry. Instruction effects: only `Register(op)` and
  `AtomicReleaseStore`, the latter hard-coded to arity 2 (`:7670-7681`). Const projections: records,
  arrays, integers only (`:3629-3650`).
- Match alternatives: "arms.is_empty() || arms.len() > 8 => Vec::new()"
  (`checks_current_scalar_values.rs:5746`). Arithmetic capture only in Exact or Trapping domain
  (`:5632-5636`).
- Construction frontier takes fields plus the first variant's payload only
  (`checks_multiplicity.rs:1934-1941`). Reachable arms: "Only literal equality and complete
  Boolean/wildcard coverage remove ownership alternatives here; no predicate theorem is invented"
  (`checks_multiplicity.rs:4320-4323`). `settle`: exactly one candidate (`:4796-4822`).
- Borrowed windows: projection walk bound 32, storage rebase bound 16
  (`checks_multiplicity_borrowed_windows.rs:1599`, `:1729`).
- Positive case tests: `And`, `Equal`, `CaseMembership` only; "deliberately not infer through negation
  or disjunction here" (`checks_multiplicity_permission_events.rs:1826-1829`). Parallel walk only at 64
  or more states (`:335`).
- Operator `requires` clause shapes for `[]`/`[..]`: exactly `p1 < len`, `p1 <= p2`, `p2 <= len`,
  `0 <= p1`, `0 <= p2`; else "has an unsupported selected `requires` clause; bounds alone cannot
  discharge it" (`checks_ranges.rs:5840-5867`, `:5774-5776`). Loop counter: only `self.f = self.f - c`
  or `+ c`; "A literal `c` is deliberately required." (`checks_ranges.rs:3133-3181`). State-argument
  propagation: 64 passes, publishes nothing if unsettled (`:4349`, `:4459`).
- Ranking: only one or two subjects (`checks_termination.rs:2700-2718`); `Lexicographic` returns `None`
  (`:5992-6002`); joint ranks need one-state members (`:730-732`); countdown argument must be literally
  `param - 1` (`:4130-4133`); positive guard only against an integer literal (`:4225-4271`).
- Progress: call-result recursion depth 16 as a literal at 5 sites
  (`checks_termination_progress.rs:1920`, `:1961`, `:3062`, `:3079`, `:4153`) and as a constant
  (`:3640`). `call_result_place`: nongeneric checked body with positional arguments only
  (`:2084-2121`).
- `checked_terminator` accepts 8 shapes, `_ => return None` (`execution.rs:2643-2798`); tuple coverage
  extent at most 4096 (`:2202-2203`).
- Budgets used as "no proof at the cap": 4096 nodes and depth 64 (`checks_contracts.rs:1889`, `:1900`),
  65,536 visits and depth 128 (`checks_content.rs:80`, `:1297`, `:1674`;
  `checks_multiplicity.rs:491`, `:512`), 1024 steps (`checks_operators.rs:544`, `:602`, `:722`),
  depth 128 (`checks_ranges_facts.rs:2631`).

## Accepts when it does not know

### Classed "open" by the readers

By my count 103 record entries (4 in stage 04, 99 in stage 05); some bullets below join two entries.
None was traced to a wrong program; where a reader named the program they thought could get through,
it is given in the reader's words.

Stage 04:
- `preparation.rs:47-61` array length left unset on every failure; later rejection not traced.
- `preparation.rs:138` recipe positions other than two get no context entry.
- `preparation.rs:1452` an unresolved selection stays late-bound.
- `selection_and_reflection.rs:1087`, `:1103` name-only match: "a required parameter whose spelling
  equals a candidate binder binds to it even if the symbols differ".

`borrow.rs`, `checking.rs`:
- `borrow.rs:4765-4775`, `:4891-4893`, `:7764`, `:7801-7803` template-dependent stored-slot relation
  yields no loans for that call.
- `borrow.rs:1475-1479` no custody rows when call frames are missing.
- `borrow.rs:1058`, `:1174` unknown formal: "A `&mut`-typed formal of an unresolved target would be
  recorded as read".
- `borrow.rs:2452-2519` `source_type` returns `None`; consumers not read.
- `checking.rs:329-334`, `:47-54`: "A call whose acknowledgement mismatches its envelope would pass if
  the plan lacks a row."
- `checking.rs:995-997` no declaration authority at 15 of 16 construction sites: stated for toolchain
  selections (`:790-792`), open for the rest.
- `checking.rs:408-412` pending opaque Copy receipts: whether the strict check re-runs is open.

`checks.rs`:
- `:2602-2603` region depth over 16 returns `Inferred`, so the comparison is skipped.
- `:4167-4169` no check when the type context is called unreachable.
- `:4205-4210`, `:4294-4296`, `:4297-4305` a call with no parameter roster or call site gets no
  runtime-index check.
- `:4269` assignment value unchecked when `actual` cannot name the target.
- `:3328-3330` invalid symbol is a stable index; `:1902`, `:1926-1935` invalid symbol is const.
- `:5750-5753` "not used after" when the statement is absent.
- `:6972-6977` a loan with a transport supplier accepted without one of the three resource kinds.
- `:6299-6301` empty required path makes the value view-free and permissive: "a value whose type
  `borrow_carrying_owner_paths` reports as carrying no borrow paths but does hold one".
- `:7448-7457`, `:7460-7469`, `:7708-7714` suspending call skipped when machine, state, call row or
  call site is missing.
- `:7289-7291` a call argument whose display name differs from the claim's label falls back to the
  structural policy, which "may be more permissive than the claim".
- `:8633-8639` an expression whose saved subject belongs elsewhere produces no ordered steps.
- `:8959-8961` a named-transition call inside a cleanup machine is not examined; `:8940-8946` vacuous
  when the target has no flow states.
- `:10374-10381` machine skipped when its contract plan row is missing; `:10798-10800` constant entry
  skipped when no crash summary exists.

Borrow checks:
- `checks_borrows.rs:439`, `:446` incomplete frontier is not deferred; `:555` const-parameter root
  accepted (stated only for by-reference parameters, `:508-512`).
- `checks_borrows.rs:1447-1449` a call with an empty may-write frame keeps all static provenance.
- `checks_borrows.rs:5393-5408` no unique completion and no copied borrow-carrying operand: call
  skipped.
- `checks_borrows.rs:5502-5509` state skipped when machine or state lookup fails.
- `checks_borrows.rs:11434-11435` and `checks_borrows_calls.rs:1497`: a flow state with no borrow state
  gets no statement, aggregate or call check.
- `checks_borrows_calls.rs:1079-1085` later-operand check skipped unless the receiver is a `Borrow` or
  non-symbol root.
- `checks_borrows_overlap.rs:2828` unlisted segment pairs are "do not overlap" (inferred that they
  cannot occur at one depth).
- `checks_borrows_overlap.rs:2989-2992` `independent_input_permissions`: no reason at the site.
- `checks_borrows_resources.rs:100` "a call argument whose backing root disagrees with its supplier's
  parent lifetime root would carry no call-loan demand"; `:129-131` invalid operand returns silently.
- `checks_borrows_resources.rs:2940-2953` un-activated call-result or descriptor-load loans get no
  resource rows.
- `checks_borrows_resources.rs:5773`: "a saved `UnretainedDerived` pointer without retained call
  inputs".
- `checks_borrows_resources.rs:4001-4013` last-chance retry: "a plain-observation capture whose
  specific rule ... failed, but where some stored loan owned by the same symbol is live at that
  statement, is accepted".

Content:
- `checks_content.rs:2283-2285` checked-body target accepted without a result-claim check.
- `:2286-2288` "a result qualification on a call through a non-boundary public requirement signature".
- `:2307-2317` boundary trait requirement with no matching route accepted for results with no
  projection or a nested path.
- `:1258` body-sourced replacements are not judged.
- `:5395-5409` "accepts a bodyless boundary signature that returns qualified nested owned outputs
  although no input supplies owned qualified content".
- `:6270` one owned source accepted on `owned_source_can_supply` alone.
- `:6005`, `:6011-6019`: "owned qualified content nested inside a type form not in the match is
  invisible to the borrowed-source and partition checks".
- `:3834`, `:3897-3910`: "a contract membership whose place text spells the same as the structural
  subject but resolves to a different symbol".

Contracts:
- `checks_contracts.rs:1201-1209` evidence binding skipped when the call site is not found.
- `checks_contracts.rs:244-251`, `:255-261` assembly fact not checked when machine, state or statement
  flow is missing.
- `checks_contracts.rs:1566-1587` nominal field check skipped; `:690-696` cleanup check skipped when
  `target_machine` is `None`.
- `checks_contracts.rs:2873`, `:2876` third closure `&|_, _| true`; what it gates was not traced.
- `checks_contracts_calls.rs:3407-3474` an unproved requirement becomes a deferred obligation with no
  diagnostic; stated (`checks_contracts.rs:104-106`), downstream discharge not traced.
- `checks_contracts_calls.rs:4542-4560`, `:4617-4621`, `:4638-4652`: "a program that lends a state
  PARAMETER's storage (not a local) with a declared domain to a `&mut` callee that breaks the domain
  would not be checked here" (inferred).
- `checks_contracts_calls.rs:4175-4178` evidence term matched against any arm; cross-arm sharing open.
- `checks_contracts_exits.rs:9696`, `:9713` facts skipped on entailment or evidence term set elsewhere.
- `checks_contracts_exits.rs:9655-9659`: "an exit the type context calls unreachable".
- `checks_contracts_exits.rs:4037`, `:4042` result field domains unchecked when the returned
  expression is invalid or has no paths.
- `checks_contracts_exits.rs:2079-2088`, `:2096`: "a transition into the header whose site lookup
  fails" is not counted as an arrival.
- `checks_contracts_writes.rs:1590-1594` no write checks when the state lookup fails; `:205-214` check
  skipped when a formation membership was already published.
- `checks_contracts_prover.rs:2836-2845` any `And`/`Or` accepted for requirements and propositions
  without checking operand meaning.
- `checks_contracts_prover.rs:4667-4676` preserved when the write ceiling is empty; ceiling
  completeness not traced.
- `checks_contracts_prover.rs:5557` type test skipped for unlisted program points (reader: low).
- `checks_contracts_prover.rs:7703` default contract handle as placeholder; consumers not traced.
- `checks_contracts_prover.rs:7615-7623`: "one instruction event used to discharge two propositions
  when one proof routes through a helper's guarantee and the other uses the event directly".
- `checks_contracts_prover.rs:7384-7386` fixed incoming reference returns `true` outright.
- `checks_contracts_prover.rs:7863-7886` vacuous match if an operation has zero input carriers; whether
  any does was not checked.

Scalar values and ownership:
- `checks_current_scalar_values.rs:5073` inline assembly retires no tracked entry; `:4820-4822` arm
  pattern markers skipped.
- `checks_current_scalar_values.rs:604-610` "a type reference named like a number is treated as that
  integer"; `:1994` "two fields of different structs with the same name would match".
- `checks_multiplicity.rs:7218`, `:7518` other event sources use statement index 0.
- `checks_multiplicity.rs:9802-9937`: "a temporary the checker does not recognise gets no
  cleanup/disposal obligation recorded here".
- `checks_multiplicity.rs:10872` final `true` for unlisted expression shapes (see my check 1: the one
  call site uses it to mark an obligation live).
- `checks_multiplicity_borrowed_windows.rs:1498-1513`: "`owner = other_value` of any type replaces the
  whole enclosing owner" with no type test here.
- `checks_multiplicity_borrowed_windows.rs:565-567`, `:1368-1370`, `:1433-1435`, `:1480-1482` uses
  outside a frame or rooted at an expression never see windows; `:960-962` invocation skipped on
  flow flags computed elsewhere.
- `checks_multiplicity_claim_outcomes.rs:2167-2187` an event with Unknown provenance and no receipt
  passes; `:1678-1686`, `:1704-1706`, `:1839`, `:1851-1855` missing lookups make no receipt.
- `checks_multiplicity_cleanup_dispositions.rs:822-833` a root with no frontier is present everywhere;
  `:722-727` join drops a one-sided frontier (reader: over-disposal direction).
- `checks_multiplicity_linear_validation.rs:489-494`, `:574-579`, `:679-684` a recorded event with no
  tracked place is skipped; `:3654` `let _ =` discards the commit result; `:4472-4525` a result counts
  as mapped on identity alone.
- `checks_multiplicity_permission_events.rs:1961`, `:1972`: "Two different places with the same
  spelling would pass as one subject".
- `checks_operators.rs:1856` an Index/Range-spelled use whose expression is not `Indexed` is skipped;
  `:190-192`, `:251-255`, `:332` a float-format mismatch through an unsupported shape is not reported.

Ranges, termination, execution:
- `checks_ranges.rs:6074-6079`, `:6091`: "a custom-operator index whose operand type could not be
  resolved (a `requires` that nobody discharges)".
- `checks_ranges_facts.rs:4830-4833`: "A shadowed or same-named binding with a different symbol could
  return another binding's value or length."
- `checks_ranges.rs:11163-11230` the gate walker and the checker may not enumerate the same nodes.
- `checks_ranges_facts.rs:3717` field integers on an assignment through an aliased path not traced.
- `checks_termination.rs:6729-6731` a machine with no states is `Proven(None)`; `:7903-7918` proof
  machines get no ranking judgment here.
- `execution.rs:817-821`: "an affine field projection (partial move) whose path contains an
  invalid-symbol field segment would be treated as a harmless read".
- `execution.rs:547-550` unresolved callable maps to `StateArrival`.
- `execution.rs:2565-2580` exhaustiveness checked against the first case's parent definition, not the
  subject's type.
- `execution.rs:292-301` a trait signature with no reach is omitted.

### Classed "stated" by the readers

- `checks.rs:2849` `(Some(_), Some(_)) => true` for inferred regions; stated `:1858-1862`,
  `:2574-2576` ("so no obligation compares it here"); left to the borrow checker, not traced.
- `checks.rs:9310-9328` crash leaf with unknown or non-sensitive root holds; stated `:9084-9085`,
  `:9350-9353`. `checks.rs:10207-10209` literals; stated `:10182-10183`.
- `checks.rs:8436-8442` unclaimed moves appended conditional and unordered; stated `:8238-8242`.
- `checks.rs:7942` borrowed-storage result ignored when preservation is not required; stated partly
  `:7405-7407`.
- `borrow.rs:3525` invalid field symbol overlaps every owner field; stated `:3589-3594`.
- `checking.rs:706-719` provider defaults, target-context machines, conformances and build-scope files
  not scanned; stated `:607-616`. `checking.rs:579-597`; stated `:482-487`.
- `checks_borrows.rs:2075-2077` two lifetime-dependency leaves never compared; stated partly
  `:1863-1867`. `:3471-3487`; stated `:3453-3457`. `:7754-7760`; stated `:7583`. `:4236-4239`.
- `checks_borrows_calls.rs:331-353`, `:528-557`; stated `:602-607`. `:1715-1730`; stated `:1706-1708`.
- `checks_borrows_overlap.rs:3003-3013` two reads never interfere; stated by access semantics.
- `checks_borrows_resources.rs:72` missing provenance; stated `:17-18`. `:4819-4886`; stated
  `:4813-4818`. `:883-1096`; stated `:829-830`. `:2318-2320`; stated `:2305-2308`. `:5340-5348`,
  `:5390-5394`; stated `:5353-5360`.
- `checks_content.rs:2335-2398`; stated `:2810-2817`. `:2493-2526`; stated `:3013-3014`.
  `:6286-6288`; stated `:5041-5045`. `:114-116`, `:180`, `:206`; stated `:78-79`.
- `checks_contracts.rs:3912-3914`; stated `:3898-3900` ("Unknown never proves a contract and never
  falsely rejects one"). `:9990-9999`; stated `:9879-9886`, `:9979-9983`.
- `checks_contracts_calls.rs:3699-3743` declared domain of a shared parameter; stated `:3683-3698`,
  not traced.
- `checks_contracts_exits.rs:9687-9689`; stated header `:14-16`. `:3759-3815`; stated `:3744-3751`.
  `:3592-3596` and `checks_contracts_prover.rs:487-490` vacuous domain; stated by the helper name.
- `checks_contracts_writes.rs:2176-2184`; stated `:2155-2157`.
- `checks_contracts_prover.rs:3079-3085` memo soundness.
- `checks_current_scalar_values.rs:5317`; stated `:4486`. `:5277-5278`; stated `:4429-4437`.
- `checks_multiplicity.rs:2110-2112`; stated `:2068-2069`. `:2139`; stated `:2252-2255`.
- `checks_multiplicity_borrowed_windows.rs:801-809`; stated `:215-220`.
- `checks_multiplicity_permission_events.rs:1670-1681`; stated `:1575-1576` ("an underapproximation
  by design").
- `checks_operators.rs:1921-1926`; stated `:1819-1823`. `:2139-2144`; stated `:2428-2435`.
- `checks_ranges.rs:889-907`; stated `:883-888`. `:6271`, `:6285` `Unsupported` with no diagnostic;
  stated `:6133-6136` ("Silence is not bounds evidence") but not traced to the other checker.
  `checks_ranges_facts.rs:1378-1396`; stated `:1356-1363`.
- `checks_termination.rs:2021-2026`; stated `:2000-2012`. `:3083-3085` builtin view with no
  declaration keeps the builtin reading; stated `:3074-3077`.
- `checks_termination_progress.rs:1563-1590` closed-value callees terminate; stated by name only.
  `:3521-3526`; stated `:3501-3505`.
- `execution.rs:169-177`; stated `:121-123`. `:2456-2470`, `:2839-2851`; stated `:536-537`.

### Classed "traced" by the readers

- `checks.rs:3104-3173` three erased-evidence proofs are positive proofs. `:4538-4574` stored-integer
  guards each mean the call cannot write an encoded field. `:7140` fails closed at `:6819-6821`.
  `:8878`. `:9684-9686`. `:10492-10499` a same-cause bucket with a constant-true alternative covers
  every site of that cause.
- `preparation.rs:454-466` original receipt stays. `selection_and_reflection.rs:3148` errors later at
  `:3464-3523`.
- `checks_borrows.rs:2624-2628` immutable backing. `:6011-6016`. `:8577-8603`. `:9195-9203`.
  `checks_borrows_calls.rs:162-164` reported at `:298-304`.
  `checks_borrows_overlap.rs:2993-2996` different roots disjoint, for callers that resolve backings.
- `checks_borrows_resources.rs:3215-3217` three lineages handled by other modules.
- `checks_content.rs:1743-1751` a fresh root is rejected at `:1453-1455`. `:6939-7062` fixed point with
  no cap; termination inferred, not proved.
- `checks_contracts.rs:7068-7170` default membership evidence supplies an empty roster; then only
  context-free routes can pass (`checks_contracts_calls.rs:3247-3248`); `:7131-7135` refused at
  `checks_contracts_calls.rs:3381-3387`. `checks_contracts.rs:769`.
- `checks_contracts_writes.rs:1190-1192`, `:493-502`.
- `checks_contracts_prover.rs:5263-5269`, `:5438`, `:4610`, `:3926`, `:4046-4048`, `:6257-6271`
  (all "traced locally").
- `checks_multiplicity.rs:1628`, `:479`, `:6348`, `:6369-6380`.
- `checks_multiplicity_claim_outcomes.rs:4375`, `:4015-4019`, `:3516-3525` (rest on map completeness
  `:1415-1418`), `:4468-4476`.
- `checks_multiplicity_cleanup_dispositions.rs:1349-1351`, `:1440-1447`, `:1311-1318`.
- `checks_multiplicity_permission_events.rs:2036-2082`. `checks_operators.rs:1522-1526`.
- `checks_ranges.rs:158-163`, `:6197-6203`, `:4880-4888`, `:5941-5955`, `:5684-5696`, `:4742-4772`;
  `checks_ranges_facts.rs:3862-3879` (to the call boundary).
- `checks_termination.rs:180-188`, `:478-480`, `:7817-7850`, `:8045-8057` (source-natural path only).
- `checks_termination_progress.rs:1088-1093`, `:4208-4210`, `:1761-1765`, `:4378-4383`, `:3422-3424`.

### Classed "inferred" only (no class of the three given)

`preparation.rs:527-529`, `:218`; `selection_and_reflection.rs:215`, `:237`, `:86-100`;
`borrow.rs:1336-1341`, `:1944-1951`, `:2425-2427`; `checks.rs:3032`, `:4456-4464`;
`checks_contracts.rs:9772-9777`; `checks_contracts_calls.rs:2244-2247`; `checks_content.rs:7218-7224`;
`checks_contracts_prover.rs:8063`; `checks_multiplicity_linear_validation.rs:4207-4209`;
`checks_multiplicity_permission_events.rs:178-186`.

## Silent drops

Places where a failed lookup or plan yields no row and no diagnostic. By my count 6 sites in stage 04
and 67 in stage 05 (a site is one entry in the list below).

- Stage 04: `preparation.rs:47-61`, `:138`, `:454-466`, `:1452`;
  `selection_and_reflection.rs:215/237`, `:86-100`.
- `borrow.rs:4765-4775`, `:4891-4893`, `:7764/7801-7803`, `:1475-1479`, `:9089-9093` (`.ok()`),
  `:326-372`. `checking.rs:329-334`, `:47-54`.
- `checks.rs:4205-4210`, `:4294-4296`, `:4297-4305`, `:7448-7457`, `:7460-7469`, `:7708-7714`,
  `:8633-8639`, `:10374-10381`, `:10798-10800`.
- `checks_borrows.rs:5502-5509`, `:5393-5408`, `:11434-11435`; `checks_borrows_calls.rs:1497`,
  `:162-164`.
- `checks_borrows_resources.rs:72` (`let _ =`), `:129-286`, `:2940-2953`, `:5773` (`Err(_) => {}`),
  `:883-1096` (event kept without the restored-call join).
- `checks_content.rs:114-206`, `:588-590/664-666/3441-3443`.
- `checks_contracts.rs:1201-1209`, `:244-261`, `:1566-1587`, `:690-696`;
  `checks_contracts_calls.rs:4542-4652`; `checks_contracts_exits.rs:2086/2096`, `:4037/4042`;
  `checks_contracts_writes.rs:1590-1594`, `:2172-2184`.
- `checks_current_scalar_values.rs:5073`.
- `checks_multiplicity.rs:2110-2112`, `:4796-4822`, `:8430-8482`, `:8754-8759`, `:8839-8855`,
  `:9802-9937`. `checks_multiplicity_borrowed_windows.rs:565-567`, `:1368/1433/1480`.
- `checks_multiplicity_claim_outcomes.rs:1678-1855`;
  `checks_multiplicity_cleanup_dispositions.rs:1311-1318`, `:1349-1351`;
  `checks_multiplicity_linear_validation.rs:489-494`, `:574-579`, `:679-684`, `:3654`;
  `checks_multiplicity_permission_events.rs:178-186`.
- `checks_operators.rs:1835-1870`, `:190-332`.
- `checks_ranges.rs:6271/6285`, `:11163-11230`, `:4459`, `:1077-1079`;
  `checks_ranges_facts.rs:2639` (incomplete read set stored as `None`).
- `checks_termination_progress.rs:1088-1093`, `:4378-4383`.
- `execution.rs:166-168`, `:292-301`, `:594-596` (whole plan omitted).

## Unused or unwired

With the readers' grep counts over `omega-rust`.

- `collect_state_argument_facts_whole_pass`: named in a comment as the function tests compare against
  (`checks_ranges.rs:3524-3525`); 1 match, the comment itself. No such function exists (checked).
- `tests/depth.rs`, cited by `checks_ranges_facts.rs:2628-2629`: no such file (checked across
  `omega-rust`). A `/* Tests. */` comment at `checks_ranges.rs:10950` is followed by no tests; 0 test
  attributes in both ranges files. `checks_ranges_facts.rs:5098-5101` ends with a comment and no item.
- `initial_linear_places`: `let owned_receiver = false && ...`
  (`checks_multiplicity_linear_validation.rs:1074-1075`), so the owned-receiver branch `:1076-1103`
  never runs (checked).
- `SimultaneousHoldConflict::DependentReads` (`checks_borrows.rs:11340-11341`, text `:11363-11365`):
  inferred to have no producing path, because two reads are always non-interfering
  (`checks_borrows_overlap.rs:3003-3013`) (checked: matches).
- `collect_positive_case_tests` (`checks_multiplicity_permission_events.rs:1830`, `pub(crate)`): 0
  outside callers (checked: 4 lines, all in that file).
- `select_static_case_alternative` (`checks_multiplicity_linear_validation.rs:4022`, `pub(crate)`): 0
  outside, 1 in-file call `:2993` (checked: 2 lines).
- `call_declares_direct_view_source` (`borrow.rs:6841`): comment `:3082` says resource lineage replay
  uses it; 3 lines, all in `borrow.rs`, one use `:7280` (checked).
- `exact_loan_lifecycle_rows` (`checks_borrows_resources.rs:2561`): comment `:2554-2560` says every
  reconstruction joins the ledger through it; 10 occurrences, all in that file, 0 outside (checked).
- `live_contexts_prove_scalar_postcondition` (`checks_contracts_exits.rs:9582`): doc `:9577-9581` says
  package exit evidence shares it; grep found one use (`:9732`).
- `WholeContractCache` (`checks_contracts_calls.rs:3183`) only holds `contexts` and `answers`
  (`:3236-3269`). `fact_requirement_mentions` ignores its `_facts` argument
  (`checks_contracts.rs:10089-10091`). `owned_destination` has an unused `_machine`
  (`checks.rs:4861`). `storage_place` ignores program, machine and state
  (`checks_multiplicity.rs:2271-2281`).
- Every `pub(crate)` item in `checks_termination_progress.rs`'s inline modules is used only in that
  file: 70 matches in 1 file. Its header (`:1-7`) names `machine_summaries.rs` and `fact_subjects.rs`,
  which exist only as inline modules; `checks_content.rs:14-19` names four files the same way.
- `pub(crate)` wider than reach: crash checks inside private `mod crashes`, 1 caller each
  (`checks.rs:12115`, `:12120`, `:12133`, `:12139`); `reborrow_resource_drift`,
  `reborrow_restored_call_use_drift`, `invalid_reborrow_attenuation_diagnostic` internal only
  (`checks_borrows_resources.rs:2832`, `:852`, `:2964`).
- `settle_value_destinations` 3, `normalize_open_index_identities` 2, `specialize_machine_calls` 4:
  all in `preparation.rs`.
- Debug paths left in: `trace_missing_frontier` reading `OMEGA_TRACE_CLAIM_FRONTIER`
  (`checks_multiplicity.rs:6704-6814`) and two more reads at
  `checks_multiplicity_linear_validation.rs:4119`, `:4528` (5 occurrences in 2 files); `eprintln!`
  under trace flags at `checks.rs:6361-6378`, `:6979-6983`,
  `checks_borrows_resources.rs:2422`, `:3994`, `:5768`.
- `checks_ranges.rs:10983` discards the result of `immutable_binding_type(invalid)`; purpose not read.
- `finalize_execution` runs part of what the header says (`execution.rs:6-13`): `terminal_debug` and
  `build_checked_guarded_exit_plans` are called from `published_facts.rs:11255`, `:11249`.
- `checks.rs` header (`:1-37`) omits five checks that run: `runtime_indices`, `atomic_fences`,
  `stored_integer_calls`, `indexed_writes`, `zero_value_qualifications`.

## Stated limits

- "`align_of(value)` in an `ensures` contract is not yet supported" (`preparation.rs:1608`).
- "unknown source relationships remain unretained" (`borrow.rs:3076`); "The early borrow producer
  cannot yet know the reaching stored operand graph" (`borrow.rs:292`).
- "unimplemented: portable target-family contract" (`checking.rs:667`).
- "This is deliberately incomplete but sound" (`checks.rs:10999-11000`); "Portable products still
  need their own invocation/certificate representation." (`checks.rs:10319-10324`).
- "Non-static sources remain fenced until the flow plan propagates a persistent owner's loan through
  every outgoing transition." (`checks_borrows.rs:616-628`).
- "unknown bounds remain conservative" (`checks_borrows_overlap.rs:1-7`).
- "Missing provenance produces no resource: carry must still establish complete leaf coverage."
  (`checks_borrows_resources.rs:17-18`).
- "checking alternative case frontiers is not implemented" (`checks_content.rs:5472`); "Recursive
  expansion and unresolved extents remain explicit analysis limits" (`checks_content.rs:5061-5063`).
- "Contract comparison is currently label-based" (`checks_contracts_calls.rs:1987`); "Mutable locals
  need an argument-evaluation snapshot, not this call-entry storage query."
  (`checks_contracts.rs:5279-5280`); "A strict `>` is not claimed" (`checks_contracts.rs:9340-9346`).
- "Candidate discovery remains incomplete" (`checks_contracts_exits.rs:577-578`); "Longer member paths
  stay unproven." (`checks_contracts_exits.rs:7604-7605`).
- "Evidence and deferred payloads are never proved here." (`checks_contracts_prover.rs:22-23`).
- "Unknown effects retire current bindings; they cannot revive an entry fact about new storage."
  (`checks_current_scalar_values.rs:8-9`); "Never expand pointer-type graphs" (`:2506-2507`).
- "Temporary bounded diagnostic. Root removes this with its call before landing."
  (`checks_multiplicity.rs:6703`); "Temporary bounded source-producer diagnostic; remove before
  landing." (`checks_multiplicity_linear_validation.rs:4111`).
- "unimplemented: conditional temporary owner cleanup needs selected establishment custody"
  (`checks_multiplicity.rs:9209`).
- "Shared and write-only links keep the existing plain rejection."
  (`checks_multiplicity_borrowed_windows.rs:22`).
- "Floats are excluded because NaN is not equal to itself" (`checks_operators.rs:2433-2435`).
- "A defensive bound, not proof of convergence. No inferred argument facts are published if any
  contribution remains unsettled" (`checks_ranges.rs:4345-4348`); "None is an incomplete read set, not
  a storage-free computation." (`checks_ranges_facts.rs:2639`).
- "Distances have no recognizer here" (`checks_termination.rs:4408-4409`); "declared measure is never
  selected implicitly" (`:2800-2801`); declared measures land "with the rest of TPR3" (`:6922-6924`).
- "A structural bound on finite premise transport, not an iteration budget."
  (`checks_termination_progress.rs:35`).
- "Anything that needs projected, nominal, or claim-bearing cleanup omits the complete state plan
  rather than publishing partial evidence." (`execution.rs:594-596`); "Resolve a complete static
  prefix, never speculate past an unknown guard." (`execution.rs:501`).

## Disagreements

1. **Where `evaluator` lives.** `unit_051` lists `evaluator` among the unread contents of
   `checks_contracts_prover.rs`. `unit_051_gap_014` says it is a sibling module imported at
   `checks_contracts_prover.rs:5861`; `unit_050` describes it at `checks_contracts.rs:3173-3924`. The
   code agrees with the second pass: `checks_contracts.rs:3173` is `mod evaluator {` and
   `checks_contracts_prover.rs:5861` is the `use`.
2. **Does the atomic-store route test position and order.** `unit_051_gap_015` says route 1 of
   `evidence_inner` (`checks_contracts_prover.rs:7689-7712`) tests neither and leaves it "open" because
   `atomic::matches_store` was not read. `unit_051_gap_014` read it and says it does. The code agrees
   with `gap_014`: `store_matches` requires a `Statement` point in the same machine and state
   (`:6661-6671`) and that the event precedes the query statement in admitted order or the query is the
   state's end (`:6672-6680`). That "open" item is closed.
3. **`InstructionEffectEvidence` counts.** `gap_014` says 12 occurrences in 2 files, `gap_015` says 16
   in 3 files. Not a contradiction: `gap_015` counted a three-name pattern that also matches the two
   re-export aliases (`:8067`, `:8069`) and `checks_contracts.rs:540`.
4. **Is the shared guard index used by crash coverage.** `unit_054` could not find the "crash
   coverage" consumer named at `checks_ranges.rs:94-97`, while its own grep list has
   `checks.rs:10366`. `unit_047` records that line as inside `mod crashes`. The code agrees:
   `checks.rs:10366` is the `incoming_guards` parameter of `infer_path_conditioned_guard_coverage`.
5. **Who calls `build_borrow_facts`.** `unit_046` found the header's `facts::build_check_facts` only
   as a caller in `published_facts.rs`; `unit_055` was unsure the two are the same. They are:
   `published.rs:9-10` mounts `published_facts.rs` as `mod facts`, and `build_check_facts`
   (`published_facts.rs:10963`) calls `build_borrow_facts` at `:10983`.
6. **Suspected duplicates across units** (`same_path`, `owner_path_matches`, `terminal_demands` vs
   `demands`): see items 1, 2 and 17 of "The same job in more than one place".

No two first-pass records contradict each other on a statement about the code that I could find.

## What I checked myself

30 claims opened in the checkout, all in stage 05.

| # | Claim (record) | Cite | Result |
|---|---|---|---|
| 1 | `expression_establishes_obligation` ends in `true` for unlisted shapes; two callers untraced (052) | `checks_multiplicity.rs:10872`; `checks_multiplicity_linear_validation.rs:1558` | partly: the final `true` is there; there is one call site (plus an import), and it sets `obligation_live` on a written target, so the default marks the obligation live. Consumers of that flag not read. |
| 2 | invalid field symbol returns `Some(())` before any type check (055) | `execution.rs:817-821` | matches |
| 3 | `owned_receiver = false && ...` is dead (053) | `checks_multiplicity_linear_validation.rs:1074-1075` | matches |
| 4 | `calls_complete` returns `true` for `TransitionNamed` (047) | `checks.rs:8959-8961` | matches |
| 5 | missing operand type makes ranges own the index; operators then skip it (054, 053) | `checks_ranges.rs:6057-6092`; `checks_operators.rs:1849-1858` | matches on both sides |
| 6 | `same_path` duplicated in 4 files (046 suspected) | `borrow.rs:2825`, `checks_borrows.rs:9894`, `checks_borrows_resources.rs:6172`, `flow_retained_borrows.rs:1181` | matches: same body |
| 7 | `owner_path_matches` duplicated (046 suspected) | `borrow.rs:3557`, `checks_borrows.rs:4121` | does not match: prefix match with dynamic-index tolerance vs exact equality |
| 8 | `demands` re-implements `terminal_demands` (049 suspected) | `checks_contracts_exits.rs:1633-1660`, `checks_content.rs:54-82` | does not match: arithmetic expressions vs content plans; only the budget pattern is shared |
| 9 | `same_type` twice (049) | `checks_content.rs:475`, `checks_contracts_exits.rs:179` | partly: same purpose, different identity call |
| 10 | atomic store checks point and order (gap_014 vs gap_015) | `checks_contracts_prover.rs:6661-6680` | matches gap_014 |
| 11 | `evaluator` is not in the prover file (gap_014 vs 051) | `checks_contracts_prover.rs:5861`, `checks_contracts.rs:3173` | matches gap_014 |
| 12 | leaf receipt and receipt with children never overlap (gap_014) | `checks_contracts_prover.rs:7615-7623` | matches |
| 13 | `DependentReads` has no producing path (048 inferred) | `checks_borrows.rs:11306-11341`, `checks_borrows_overlap.rs:3003-3013` | matches: one return in `captured_place_compatibility`, and two reads set `non_interfering` |
| 14 | `collect_state_argument_facts_whole_pass` does not exist (054) | `checks_ranges.rs:3524` | matches: 1 line, the comment |
| 15 | `collect_positive_case_tests` 0 outside (053) | `checks_multiplicity_permission_events.rs:1830` | matches: 4 lines in one file |
| 16 | `select_static_case_alternative` 0 outside (053) | `checks_multiplicity_linear_validation.rs:4022` | matches: 2 lines in one file |
| 17 | `call_declares_direct_view_source` no outside caller (046) | `borrow.rs:6841` | matches: 3 lines in one file |
| 18 | `exact_loan_lifecycle_rows` 0 outside (049) | `checks_borrows_resources.rs:2561` | matches: 10 lines in one file |
| 19 | `removed_regions` 0 outside callers (053) | `checks_multiplicity_cleanup_dispositions.rs:886` | does not match: one outside call at `checks_multiplicity.rs:9080` |
| 20 | crash leaf returns `true` on unknown or non-sensitive root (047) | `checks.rs:9310-9328` | matches |
| 21 | region depth over 16 returns `Inferred`; `(Some, Some) => true` (047) | `checks.rs:2602-2603`, `:2849` | matches |
| 22 | last-chance `collect_binding_observation` retry (049) | `checks_borrows_resources.rs:4001-4013` | matches |
| 23 | `Err(_) => {}` drops stored call inputs (049) | `checks_borrows_resources.rs:5766-5773` | matches |
| 24 | `validate_call` returns when no row matches (046) | `checking.rs:329-334` | matches |
| 25 | `roster_lookup` falls back to name text (054) | `checks_ranges_facts.rs:4816-4834` | matches |
| 26 | countdown tier matches parameter by symbol or name (055) | `checks_termination.rs:4488-4491` | matches |
| 27 | `published_facts.rs` is `facts`; it calls `build_borrow_facts` (046, 055 unsure) | `published.rs:9-10`, `published_facts.rs:10963`, `:10983` | matches the header |
| 28 | crash coverage consumes the guard index (054 unsure) | `checks.rs:10366` | consumer exists |
| 29 | inactive-map rule uses two different target lookups (053) | `checks_multiplicity_permission_events.rs:2060`, `checks_multiplicity_claim_outcomes.rs:3511` | matches |
| 30 | `append_distinct` copies (048); `tests/depth.rs` missing (054) | `checks_borrows.rs:7258`, `:9905`; whole `omega-rust` | matches both |

Corrections applied above from these checks: 7, 8 (not duplicates), 10 (open item closed), 19
(`removed_regions` has an outside caller), 1 (one call site, and what the default does there).

## Not covered

- Records: none unread. All 12 files were read in full.
- Source ranges: every range in the ten unit files is covered by a record. `unit_051` left
  `checks_contracts_prover.rs:1701-8071` unread; `gap_014` (1701-7700) and `gap_015` (7701-8071) cover
  it. `unit_048` says three long modules of `checks_borrows.rs` (staged values, aliases, completed
  backings, about `:4972-9918`) are recorded at module level, not per function, and that not every
  field-equality check in `:9061-9170` is described.
- Not in this area's units, so not digested here although they sit in the same two crates: the other
  eight files of stage 04 (`assembly_and_operators.rs`, `body_cloning_and_rewriting.rs`,
  `conformance.rs`, `constructed_results.rs`, `contextual_and_requirement_calls.rs`,
  `data_and_product_operands.rs`, `lib.rs`, `monomorphization.rs`) and, in stage 05, every `flow*.rs`,
  `published*.rs`, `values*.rs`, `execution_scalar_computations.rs`, `execution_terminal_unit*.rs`,
  `shared.rs`, `products.rs`, `immutable_checking_input.rs`, `lib.rs`.
- My own checks read only the cited lines and their immediate callers. I did not build or run
  anything, and I did not trace any "open" entry to a concrete program.
