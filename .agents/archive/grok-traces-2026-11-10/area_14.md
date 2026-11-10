# Area 14 open accepts — trace

Read-only. No build, no compiler run. Working tree `omega-rust/psi/compilation/validation/` and `terminal-installation/` have an empty diff against atlas commit `457adf4a11f` (`git diff --stat 457adf4a11f HEAD` produced no lines). Citations below are the lines I read in that tree. “Read” means the Read tool returned those lines. “Inferred” is marked.

Open bullets are numbered in the digest’s order. Entries 17–185 were not opened.

## Summary

| # | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| 1 | ce:73357-73359 | CLOSED | Every caller that can reach `current_value_valid` already has a storage graph, so the no-graph `return true` does not run. |
| 2 | ce:76996 | UNDECIDED | A non-runtime judge reports storage preserved, and I did not follow that into a recorded proof. |
| 3 | ce:83286-83287 | CLOSED | A negation is proved only after the judge has already recorded a real hypothesis contradiction. |
| 4 | ce:85364-85368 | CLOSED | Every caller drops the arm on `integer_premises_contradictory` before the visit, and passes empty additions. |
| 5 | ce:88447-88449 | UNDECIDED | Non-reference types treat opaque and proof terms as observed scalars; I did not follow that into a false proof. |
| 6 | ce:89539, 89638, 89738 | REACHABLE | Two different calls that print the same and are stored as opaque text compare equal and the equation judge returns proven. |
| 7 | ce:90209-90216 | REACHABLE | Contradictory integer requires make any fully unfolded transparent proposition count as proved, so the missing-ensures error is skipped. |
| 8 | ce:90730-90736 with ce:91002-91007 | CLOSED | The bare-name call check only keeps a shortcut for an ensures that is already the same expression as a requires. |
| 9 | ce:90957-90963 | UNDECIDED | An empty machine’s out-of-language ensures gets neither a proof nor an error here; I did not see whether the exit checker still rejects it. |
| 10 | ce:90781-90784 | REACHABLE | The same function that refuses to certify an unproved proof-only ensures records it proven when integer requires contradict. |
| 11 | dc:133-135 | CLOSED | An unresolved public selection has no visibility symbol; a late-bound one is an error later unless the row is excluded (entry 13). |
| 12 | dc:998-1000, 1020-1031, 1064 | UNDECIDED | A selection written in a toolchain-origin file skips the dependency obligation; I did not see who can set that origin. |
| 13 | dc:933 | UNDECIDED | Excluded selection rows skip the dependency diagnostic, and I did not see whether an excluded row is still used. |
| 14 | dc:1106 | CLOSED | The only caller turns `None` from an absent authority record into a reuse miss. |
| 15 | dc:1646 | CLOSED | Missing provenance counts as a product file, and the callers still require the product package identity. |
| 16 | dc:3151, 3161 | UNDECIDED | A terminal call whose result is not a unit, primitive, or data value is not compared; I did not see another type check. |

## 1. ce:73357-73359 — CLOSED

`current_value_valid` (read in full, ce:73351-73532) returns true when `storage_graph()` is `None` (ce:73357-73359). `storage_graph` (read, ce:72974-72976) is `program.storage_graph()`. The default `CheckingInput` implementation returns `None` (read, frozen_program_and_queries.rs:649-650).

Search `current_value_valid(` in `omega-rust`: 10 matches, all in `contract_entailment.rs` (1 definition, 9 calls). I read every call.

- Test calls at ce:72942 and ce:72949 sit on a judge whose context was given a graph at ce:72920-72922 (read).
- `rejoin_followed_path` (read, ce:73190-73237) returns at ce:73191-73192 when the graph is missing, before the call at ce:73209.
- ce:74576 is inside `graph_storage_mode() && …`. `graph_storage_mode` (read, ce:73239-73241) is `storage_version.is_some()`.
- ce:74586-74588 calls only in the `storage_graph().is_some()` arm.
- ce:75294 calls only when the version is `Some`. The `None` arm returns `storage_graph().is_none()` without calling (read, ce:75288-75296).
- ce:75827 calls only when `graph_storage_mode()` is true and a version is present (read, ce:75820-75828).
- ce:75940-75942 same guard (read, ce:75933-75943).
- ce:76750-76752 calls only inside `storage_version.is_some_and` (read, ce:76740-76752).

Search `StorageVersion {` and `StorageVersion::` in this file: the production constructors are `incoming` via `flow_state` (ce:72986-72987, and `flow_state` uses `storage_graph()?` at ce:72978-72979), `statement_storage_version` (ce:72995-73004), `call_storage_version` (ce:73021-73035), `transition_call_storage_version` (ce:73038-73060), and `read_storage_version` (ce:73097-73110). Each uses `storage_graph()?` before building a version. The test constructor at ce:72929 is the one that also installs the graph. ce:67481 writes a different field, `returned_value_source.storage_version`, on a test judge; it is not a call to `current_value_valid`.

Versions are copied at ce:75279 (`self.storage_version = returned.storage_version` inside `leave_storage_scope`, read ce:75226-75279) and ce:56630 (`advance_storage_version(completion_version)`, read ce:56598-56631). `returned` is `caller.clone()` (ce:75233). `completion_version` is `caller.call_storage_version` or `caller.transition_call_storage_version` (read, ce:46340-46347), both of which are `None` unless the caller’s graph exists. `normal_judge` is `outcome.judge.clone()` (read the assignment at ce:44355) or `arm_judge` from `structural_arm_judge`, which clones the judge when the added lists are empty (read, ce:46981-46989). Those clones keep the same program reference. I did not read `versions::operand_snapshot` or the other `versions::` helpers; the `?` on `storage_graph()` is before they run.

The digest is right that the line returns true with no graph. That line is not how a program is accepted: no caller evaluates it unless a version exists, and every version on that judge was built from a graph on the same program.

## 2. ce:76996 — UNDECIDED

`entry_storage_observations_preserved` (read in full, ce:76992-77008) returns true when `requires_value_preservation()` is false. That predicate (read, ce:81087-81088) is `runtime_body_values && !case_facts_only`. `from_declaration_requires` sets `runtime_body_values: false` (read, ce:81176). Proof machines take `from_requires` (read, ce:90384-90385), which uses that constructor (read, ce:81108-81124). Runtime machines set the flag true (read, ce:81036).

Search `entry_storage_observations_preserved(` : 3 matches, all in this file. Both calls are in `judge_proposition` (read, ce:83312-83318 and ce:83394-83398). True means the caller keeps judging instead of returning `Unknown`.

I did not read `judge_observed_predicate` or `closed_float_meaning_equality`. I therefore cannot say whether a proof machine whose body assigns storage named by a mathematical or float ensures is recorded proven. What would decide it: those two callees returning `Proven` for a fact that names storage the body assigns, and `judge_structural` (the no-arm path at ce:90610-90611, which I did read) or the filter at ce:90767-90775 storing that result.

## 3. ce:83286-83287 — CLOSED

In `judge_proposition` (read from ce:83272 through the negation arm at ce:83280-83290), a logical-not fact returns `Proven` when `hypotheses_contradictory` is set, without using the operand’s judgment.

Search `hypotheses_contradictory = true`: 5 stores, all read.

- ce:51417: a case judgment is `Refuted`. I did not read `judge_case`; this store is the one I cannot re-derive. The other four do not depend on it.
- ce:51993: two constructors of the same carrier with different identities, inside `intake_equation_fact_with_purpose` (read ce:51979-52081).
- ce:52078: two different integer terms after `resolve_with_at` (read ce:51855-51867 and the variable substitution at ce:81606-81614).
- ce:52105: two type-atom substitutions that disagree and are both true (read ce:52084-52106).
- ce:52336: the same distinct-constructor test on owned subjects (read ce:52316-52337).

A sixth store, `hypotheses_contradictory |= vacuous` at ce:47016, is set when an arm’s requires fact judges `Refuted` (read, ce:47005-47017). `from_declaration_requires` starts the flag false (read, ce:81185) and intakes each requires fact (read, ce:81195-81197) through `intake` (read, ce:81400-81437), which sends identity equalities to `intake_equation`.

With no case arms, `judge_structural` returns that proposition judgment (read, ce:90610-90611), and a proved fact with no proof-only mention is recorded (read, ce:90764-90775). The negation is proved only because the requires already contradict. A machine whose requires cannot hold, and whose ensures is a negation, is accepted as a vacuous contract. I do not have a program that should be rejected: whenever the requires hold, the ensures holds.

The digest is right that the operand is not judged. The flag is the check in front of the branch.

## 4. ce:85364-85368 — CLOSED

`visit_non_contradictory_arm` (read in full, ce:85350-85374) returns on `hypotheses_contradictory` and does not call `integer_premises_contradictory`.

Search `visit_non_contradictory_arm(`: 4 matches (1 definition, 3 calls), all in this file. I read all three calls.

- ce:44843-44849 removes the path when `hypotheses_contradictory || integer_premises_contradictory()` before the call at ce:44871. If the revision already matches, the path was kept only after that same test.
- ce:45248-45258 calls only when `checked_contradiction_state` already equals the current revision.
- ce:45260-45263 skips the call when the integer or hypothesis test is true; the call at ce:45266 follows that test.

`integer_premises_contradictory` was read (ce:53351-53362). All three calls pass empty hypothesis and equation slices and `CitationFacts::default()`. `CitationFacts` derives `Default` (read, ce:9259-9262). `intake` on an empty value adds nothing (read, ce:9317-9323). `structural_arm_judge` returns the clone immediately when both slices are empty (read, ce:46988-46989), so the arm judge’s integer premises are the path judge’s premises, which the caller already tested.

The digest is right about the function body. The callers are the check.

## 5. ce:88447-88449 — UNDECIDED

`scalar_operand_is_observed` (read in full, ce:88424-88512) returns true when `reference_shell` of the declared type is `None`, before the match that returns false for `Identity`, `LogicalNot`, `ContentValue`, `Opaque`, `RepeatedArray`, `IntegerEmbedding`, and `MathematicalBinary` (ce:88504-88510). The digest is right.

Search `scalar_operand_is_observed(` in this file returned a long list (the tool output showed on the order of sixty lines, including the definition). I did not read most of them. I did read `boolean_equality_operand` (ce:77499-77533), its use inside equation intake (ce:51957-51962), `term_is_unobserved_descriptor`’s scalar-binary arm (ce:88593-88601), and `judge_equation`’s equal-term return (ce:83473-83490). `PartialEq` for `StructuralTerm` is semantic match (read, ce:79975-79978), and the opaque arm compares display strings (read, ce:79934-79938).

True lets a non-reference boolean comparison whose operand is `Opaque` or a proof term into the rewrite at ce:51961. I did not find a path from that rewrite to a recorded ensures that is false when the requires hold, other than the display collision in entry 6. What would decide it: one caller that records `Proven`, or skips an error, for a `ScalarBinary` whose operand is `Identity` or `MathematicalBinary` while that operand’s proposition is false.

## 6. ce:89539, ce:89638, ce:89738 — REACHABLE

The name fallback (read, ce:89529-89541), the call fallback (read, ce:89580-89640), and the member fallback (read, ce:89719-89738) each build `Opaque` from `expression_table(program).display_name(expression)`.

Call printing (read, typed-trees `values.rs:653-696`) writes the receiver, the target string, machine arguments, and value arguments. It does not write the target symbol or evidence arguments. A call with evidence arguments, or whose receiver is not a static receiver for the target, fails the application conditions (read, ce:89588-89591) and becomes that opaque text.

`judge_equation` (read, ce:83482-83490) returns `Unknown` for an unobserved descriptor and otherwise returns `Proven` when the two semantic terms are equal. `Opaque` is not an unobserved descriptor: `term_is_unobserved_descriptor`’s scalar and constructor arms do not match it, and the final arm returns false (read, ce:88609). Equality of opaque terms is equality of the display string (read, ce:79934-79938 and ce:79975-79978).

Smallest program that should be rejected and is accepted: a machine whose ensures says two calls are equal, where both calls print the same receiver, the same target name, and the same argument text, but they are different calls because they name different target symbols or carry different evidence. Both become the same opaque text, the equation judge returns proven, and with no case arms `judge_structural` returns that judgment (ce:90610-90611) and the filter records it (ce:90767-90775).

The digest’s “identical display text” description matches what I read. Struct-literal printing is only the type name (read, `values.rs:450-452`); that node does not go through these three opaque arms.

## 7. ce:90209-90216 — REACHABLE

`premises_prove` (read in full, ce:90180-90217) returns false when any conjunct is missing (ce:90187-90190). Otherwise it returns true if `hypotheses_contradictory` is set, without judging the predicates.

Search `premises_prove(`: 3 matches (1 definition, 2 calls), both calls in this file (ce:90142 inside `transparent_proposition_application_entailed`, which I read from ce:90089, and ce:90171 inside `transparent_call_requirement_entailed`, read from ce:90145).

`transparent_proposition_application_entailed` is called from `validate_proposition_entailment` (read, proof_contracts_core.rs:8686-8705). If it returns true, the error “cannot establish proposition ensure” is not pushed. That function is registered in the declaration prologue (read, program_validation.rs:376-377) and `declaration_prologue::run` invokes every registered validator (read, program_validation.rs:589-606), from `validate` (read, program_validation.rs:4095-4108).

The judge is `from_requires(..., true)` (ce:90201-90202), so `resolve_applications` is true and the early return in `resolve_uncached_with_budget` (read, ce:81563-81566) does not apply. Integer parameters substitute (ce:81606-81614). A second identity equality of the same parameter to a different integer sets the flag at ce:52078. Identity equality of non-float scalars is recognized (read, ce:88094-88139) and `intake` sends those equalities to `intake_equation` (read, ce:81426-81434). Integer literals become `StructuralTerm::Integer` (read, ce:89493-89495).

The digest overstates “any transparent application”: a conjunct that does not unfold makes the function return false before the flag is read.

Smallest program: a checked machine whose requires say one integer parameter equals one integer literal and also equals a different integer literal, and whose ensures is a proposition whose unfolded body is entirely present as predicates the machine does not otherwise prove. `premises_prove` returns true on the flag, and the prologue does not emit the missing-ensures error.

The call-site twin at proof_contracts_core.rs:8891-8908 (read) likewise treats the callee’s proposition requirement as established. I did not read every caller of `transparent_requirement_proves` beyond the return at pipeline `checks_contracts.rs:9110` and its use as one disjunct of exit satisfaction (`checks_contracts_exits.rs:9800-9802`, read). The prologue path above is enough for acceptance.

## 8. ce:90730-90736 with ce:91002-91007 — CLOSED

The filter (read, ce:90730-90735) records the ensures and drops it only when some requires expression is `expressions_structurally_equal` to it and `fact_call_targets(...).all_declared` is true. `fact_call_targets` (read in full, ce:90974-91047) sets `all_declared` false when a call’s symbol is invalid, `resolve_named_expression_call` is `None`, and no machine’s bare name equals `call.target`. Search `fact_call_targets(`: 3 matches, the definition, the recursive call, and this one use.

A bare-name hit can only keep this shortcut. It cannot prove an ensures that is not the same expression as a requires. Restating a requires as an ensures does not depend on the call’s symbol. The digest’s “a call whose target resolves by name only” is the third disjunct (ce:91004-91007), and the digest’s disagreement note about `resolve_named_expression_call` at ce:91003 matches what I read. That disjunct does not accept a different fact.

## 9. ce:90957-90963 — UNDECIDED

The engine loop (read, ce:90934-90964) on `Judgment::Unknown { goal_in_language }` records the fact only if collection premises prove it, and emits “cannot prove ensures” only when `goal_in_language && requires_fully_visible`. Otherwise it neither records nor diagnoses. This loop runs only for an empty body: a non-empty body returns at ce:90873-90902 (read).

`judge_machines` (read, program_validation.rs:4756-4774) keeps the diagnostics and, for a checked body with an empty diagnostic list, keeps the proven set. `finish_diagnostics` (read, program_validation.rs:4819-4826) succeeds when there is no error. An ensures left out of `proven.entry` is therefore not an error by itself.

The exit checker emits “cannot prove ensures contract for exit” when `satisfied` is false (read, checks_contracts_exits.rs:9883-9908). I did not read whether an empty machine has an exit row, or whether `is_proved` (read only its closure head at checks_contracts_exits.rs:9726-9754) is false for an ensures the arithmetic engine called out-of-language. What would decide it: that empty machine’s flow exits, and `satisfied` for this fact.

## 10. ce:90781-90784 — REACHABLE

Inside the same filter (read, ce:90713-90783), a fact that mentions a zero value or proof-only data is removed from the unproved list and pushed to `proven.entry` when `structural.hypotheses_contradictory` is set. No diagnostic is emitted. The other arm, for a mention the judge does not prove, says accepting it would certify an unproven structural claim and emits an error (read, ce:90845-90855).

`mention` is built from `fact_mentions_zero_value` and `fact_mentions_proof_only_data` (read the calls at ce:90713-90719). I did not read those two function bodies; the error text at ce:90846-90850 is what I am using for what “mention” denotes.

The flag is set for a proof machine by the integer path in entry 7: `from_requires` intakes the requires (ce:90384-90385, ce:81195-81197), and two different integer equalities of one parameter set `hypotheses_contradictory` at ce:52078.

Smallest program: a proof machine whose requires say one integer parameter equals one integer literal and also equals a different integer literal, and whose ensures mentions proof-only data. The ensures is recorded proven at ce:90782. The same function’s unknown arm would have rejected an unproved proof-only ensures. The integer contradiction is not a proof of that claim; the function itself treats certifying the claim as an error.

## 11. dc:133-135 — CLOSED

`collect_declaration_visibility_diagnostics` (read, declarations_core.rs:119-135) skips a public-interface selection whose target is not `as_resolved()`. Search `collect_declaration_visibility_diagnostics(`: 2 matches besides the definition, program_validation.rs:365 (the prologue, invoked as in entry 7) and declarations_core.rs:234. I read the prologue registration and did not read the function at dc:234.

There is no symbol to test for visibility. A late-bound target is an error in `normalize_checked_selection` (read, dc:1009-1014), stored on the row and reported by `validate_authored_declaration_selections` (read, dc:936-946) unless the row is excluded. That call is checking.rs:996 (search `validate_authored_declaration_selections(`: the definition and that one call). An intrinsic target returns `Ok(None)` (read, dc:1008) and is not a package visibility subject. Excluded late-bound rows are entry 13, not this skip.

## 12. dc:998-1000, dc:1020-1031, dc:1064 — UNDECIDED

`normalize_checked_selection` (read in full, dc:988-1074) returns `Ok(None)` when the selection’s own source file has toolchain origin (dc:998-999), when the target is intrinsic (dc:1008), when the symbol is a builtin atom, builtin function, builtin member, or fixed-core declaration (dc:1020-1030), and when the symbol’s origin is toolchain (dc:1063-1064). `Ok(None)` hits `_ => {}` in the validator (read, dc:936-947), so no dependency diagnostic. A user origin with no package identity is an error (dc:1065-1068). No origin at all is an error (dc:1069-1072).

The toolchain-file return at dc:998-999 happens before the selected symbol is inspected, so a toolchain-origin file selecting a user-package declaration also skips the obligation. I did not read the code that assigns `SourceOrigin::Toolchain`. What would decide it: whether a source file the user can put in a program can receive that origin. If only the compiler’s own files can, the skip does not accept a user program.

## 13. dc:933 — UNDECIDED

`validate_authored_declaration_selections` (read, dc:921-949) continues when `excluded` is true, so neither a failed request nor a missing dependency is reported. `excluded` is set (read, dc:956-980) when the occurrence is in `descriptor_selection_occurrences` or the owning machine is not retained. `owning_machine_is_retained` (read, typed-trees declarations.rs:2682-2691) is false when a machine ancestor is a machine symbol that is not in the program. `descriptor_selection_occurrences` (read, frozen_program_and_queries.rs:2079-2092) adds selections for which `checks_authored_selection` is false, then removes materialized product operands. `checks_authored_selection` (read, typed-trees lib.rs:1561-1575) is false for an id in `excluded_authored_selection_occurrences`, and, when a product frontier is set, for a file that is neither toolchain nor build scope.

I did not read who fills `excluded_authored_selection_occurrences`, or whether a selection in that set is still applied. What would decide it: that set’s producer, and whether an excluded occurrence can still select a declaration from a package the program has no dependency on.

## 14. dc:1106 — CLOSED

`RecordedDeclarationAuthority::allows` (read, dc:1100-1110) returns `None` for `Absent`. Search `.allows(requester`: 1 match, psi-compiler `source_compilation.rs:108`. I read that use (source_compilation.rs:92-117). `None` becomes `SourceReuseMiss::DeclarationAuthority` through `.ok_or(...)?` at line 114. That refuses to reuse a previous compilation. It does not accept a program.

## 15. dc:1646 — CLOSED

`is_product_declaration` (read, dc:1641-1647) is true when the provenance span or the source file is missing (`is_none_or`), and otherwise when the file’s dependency scope is `Product`. Search `is_product_declaration(` in `declarations_core.rs`: the definition and three calls, all read.

- dc:1568, inside `prepare_source_completion` (read dc:1564-1576): the trait must also have the admission’s package identity and display path, and the resulting list must be exactly one trait or the function returns an error (dc:1573-1576).
- dc:1593, inside `nominate_source_completion` (read dc:1588-1601): the trait must also be named `Console` and have a package identity.
- dc:1625, inside `has_external_source_candidate` (read dc:1614-1637): the machine must also share the declaration’s package identity.

A missing file cannot admit another package, because those callers compare package identity. A same-package trait with no file is a product trait. I did not read a generator that mints a provenance-less `Console`; the digest’s “generated” word is not something I saw. The package comparison is what makes the `is_none_or` harmless.

## 16. dc:3151, dc:3161 — UNDECIDED

`terminal_results::validate` (read, dc:3142-3181) returns with no diagnostic when `static_transition_type_context_is_unreachable` is true, or when either `shape` is `None` (dc:3156-3160). `shape` (read, dc:3184-3212) returns `None` for any type node that is not unit, a primitive, a constrained type that reduces to one of those, or a named type with a data definition. Reference, slice, array, generic, and trait results take that `None`.

Search `terminal_results::validate(`: 2 calls, both in `validate_transition_target_node` (read, dc:3302-3310 and dc:3326-3338), and only for a resolved terminal target whose owner is not the current machine. Search `validate_transition_target_node(`: 2 calls, program_validation.rs:3232 and :3245. I did not read those call sites beyond the line hits, and I did not read another result-type comparison for terminal calls. Search `terminal machine call` in `omega-rust` found this diagnostic and two comments in the state-graph file, which I did not read. What would decide it: whether the transition’s result type is already required to match the caller’s result for a reference, slice, array, generic, or trait, before this function runs.

## Counts

| verdict | count |
| --- | --- |
| REACHABLE | 3 (entries 6, 7, 10) |
| CLOSED | 7 (entries 1, 3, 4, 8, 11, 14, 15) |
| UNDECIDED | 6 (entries 2, 5, 9, 12, 13, 16) |
| not reached | 169 (entries 17–185) |

## Entries not reached

17. dp:1478
18. dp:582-588
19. dp:683
20. dp:877-892
21. dp:1035
22. dp:5043 and dp:6250-6253
23. dp:6605-6611
24. dp:7502-7504
25. dp:7976-7987
26. dp:8005-8049
27. dp:8982-9019
28. dp:8966
29. dp:9939-9941
30. dp:11400
31. dp:11807-11812
32. dp:12902-12904 with dp:13231-13233
33. dp:13689
34. dp:13685
35. dp:12681-12683 with dp:11373
36. dp:14154-14195
37. dp:14879
38. dp:8515-8516
39. ei:976-984
40. ei:1450, ei:1473
41. ei:1285
42. ei:72
43. ei:2972-2982
44. ei:3186
45. lib:172-174
46. lib:773
47. lib:940-951
48. lib:421-444, lib:493
49. lib:1056-1098
50. fz:1751-1754
51. fz:2090-2091
52. fz:2450-2452
53. fz:3334-3336
54. fz:4458
55. fz:4537-4539
56. fz:4147-4149, fz:4165-4167
57. fl:491-498
58. fl:557-609
59. fl:1451-1459, fl:1523, fl:1525
60. fl:2381-2383
61. fl:2633
62. etf:305-312, etf:354-356
63. etf:36-38
64. etf:484-492, etf:2117-2127
65. etf:1444-1462
66. etf:1270-1275
67. etf:3078-3340
68. etf:7030-7062, etf:7168-7178
69. etf:685-704
70. fz:1097-1119
71. lsl:4229-4231
72. lsl:5509-5547
73. lsl:3557-3559
74. lsl:2761-2765, lsl:2776-2778
75. lsl:350
76. lsl:5247-5262
77. mcc:607-634
78. mcc:366-400
79. mcc:123-215
80. mcc:1931-1933
81. mcc:3158-3167
82. mcc:4767-4784
83. mcc:5688-5712, mcc:5594
84. mcc:6359-6369
85. mcc:6611-6613
86. mcc:7083-7105
87. mcc:8911-8913
88. mpb:4770-4785
89. mpb:4941-4943
90. mpb:4028-4033
91. mpb:7679, mpb:7923
92. mpb:7396-7401
93. mpb:6092-6094
94. mpb:6912-6947
95. ops:84-86, ops:107-109
96. ops:1753-1756 with ops:1816
97. ops:1939-1943
98. ops:2075
99. pl:448-455
100. pl:708
101. pl:6026-6029, pl:6585
102. pl:1131-1139 with pl:4058-4121
103. pl:94-103, pl:220-230
104. pl:5169-5194
105. pl:6124-6128, pl:6142-6146
106. pl:1618-1626
107. pv:74-79
108. pv:4927-4929
109. pv:4555-4570
110. pv:860-887
111. pcc:217-221, pcc:245-255
112. pcc:795-801
113. pcc:2056-2058, pcc:2036-2043
114. pcc:3014, pcc:3100-3103
115. pcc:5565-5572, pcc:5686-5695
116. pcc:5934-5953, pcc:5959
117. pcc:6085-6144
118. pcc:7515-7519
119. pcc:7156-7160
120. pcc:8189-8195
121. pcc:8971-8977
122. pcc:9095, pcc:9109
123. pcc:9684-9686
124. pcc:10571-10582, pcc:10680-10682, pcc:10714-10718
125. pcc:10464-10469
126. pcc:10996-10999
127. q:477-479
128. q:3586-3588, q:3616
129. q:5551-5556
130. q:7540-7549
131. q:424-426
132. rc:1483-1490
133. rc:1795-1800
134. rc:731-770
135. rc:1902-1907
136. rc:2307-2321
137. rc:4818-4820
138. rc:1028-1050 with rc:4660
139. rrc:3354-3356 with rrc:4516-4535
140. rrc:8795-8799
141. rrc:7020-7022
142. rrc:7685-7687, rrc:7742-7747
143. scc:642-656
144. scc:438
145. scc:2953-2958, scc:2980-2987, scc:3017
146. tr:1517-1531
147. tr:1718-1724
148. tr:3681-3697, tr:7907-7909
149. tr:2912, tr:2933-2941
150. tr:7397-7399, tr:7490-7505
151. tr:362-363, tr:5366-5368
152. trw:5633
153. trw:2153-2154, trw:2186-2217
154. trw:2114-2116
155. trw:3688-3698
156. trw:3462-3485
157. trw:6794-6802
158. vcc:526-541, vcc:584
159. vcc:1521
160. vcc:1460-1462, vcc:1491-1495
161. vcc:2025-2033
162. vcc:2142-2149
163. vcc:2399-2403
164. vcc:2838-2839, vcc:2848-2850
165. vcc:2785-2787, vcc:2815
166. vcc:2971-2973, vcc:2997
167. vcc:3418-3445
168. vcc:4052
169. vcc:3938-3963
170. vcc:4402-4408
171. vcc:4442-4451
172. vcc:5630-5638
173. vcc:5591
174. vcc:7303
175. vcc:7878-7880, vcc:7900-7905
176. vcc:8454-8456
177. vcc:8618-8636
178. vcc:9551
179. vcc:1705, vcc:1720
180. ti-lib:593-600
181. ti-lib:653
182. ti-ev:910
183. ti-ev:928-933
184. ti-prov:709-715
185. ti-prov:1141-1146

## What I did not read

- Entries 17–185: the cited lines, their enclosing functions, and their callers. Nothing in those sections is a finding.
- `judge_case`, so the store of `hypotheses_contradictory` at ce:51417 is unread beyond the call.
- `judge_observed_predicate`, `closed_float_meaning_equality`, `fact_mentions_zero_value`, `fact_mentions_proof_only_data` (bodies), `versions::operand_snapshot` and the other `versions::` helpers, `resolve_case_subject`, `expressions_structurally_equal`.
- Most call sites of `scalar_operand_is_observed`.
- The producer of `SourceOrigin::Toolchain` and the producer of `excluded_authored_selection_occurrences`.
- The function at declarations_core.rs:234, and the bodies of the two `validate_transition_target_node` call sites in program_validation.rs.
- Whether an empty machine has exit rows in `checks_contracts_exits.rs`, beyond the `is_proved` closure head and the unsatisfied-exit diagnostic.
- `static_transition_type_context_is_unreachable`.
- Unit records `unit_097.md` through `unit_105.md`. The digest was the list of entries; the verdicts come from the source lines named above.
