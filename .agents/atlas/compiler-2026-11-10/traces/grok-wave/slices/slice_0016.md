slice_0016

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0117 | flow.rs:6609-6641 | REACHABLE | `qualification_is_content_independent` can be true without checking constrained target-type content the way `domain_predicate_theory_is_complete` does (expression_types_and_fields.rs:8390-8410). |
| A0118 | flow.rs:6913-6920 | UNDECIDED | `matching_mutation_for_fact_place` returns no match for `Unknown`/invalid roots so facts are not write-invalidated (read), but I did not trace every semantic fact publisher to see whether such places affect acceptance. |
| A0119 | flow.rs:8110-8138; 8896-8904 | CLOSED | Unresolved `Name` omits receiver-read rows but incomplete footprints fall back to whole-receiver reads (flow.rs:7655-7657, read). |
| A0120 | flow.rs:9282 | CLOSED | `Unknown` and borrow-resource constraints are intentionally retained when projecting constraints to active contexts (flow.rs:9274-9283, read). |
| A0121 | flow.rs:9426-9482; 9732-9753 | CLOSED | Failed lookups `continue` without dependency rows; exposure defaults to private and rows feed portable-reference metadata, not a lone acceptance gate (checking.rs:645-648, read). |
| A0122 | flow.rs:10617-10619; 10869-10874; 10759-10768; 10799-10804 | CLOSED | Documented eligibility skips (`zero_image`, non-checked-body, `DynamicTrait` shell, missing target machine) are intentional boundaries on cleanup-leaf/dependency derivation (read). |
| A0123 | flow.rs:11515 | CLOSED | Selector projection cycle yields empty origins and `derive` omits that capture, conservatively under-approximating (flow.rs:11437-11438, 11475-11476, read). |
| A0124 | flow_call_phases.rs:836-857 | CLOSED | Non-parameter or non-resolvable contract occurrences are treated as surviving ensures filtering; `Err(())` on parameter lookup fails closed (flow_call_phases.rs:854-855, read). |

### A0117

1. Read in full: `qualification_is_content_independent` and inner `visit` (flow.rs:6590-6646, read).
2. Search `rg '\bqualification_is_content_independent\b' omega-rust` → 6 matches in 4 files (read grep output).
3. Read callers: `preserves_owner_qualification` (flow_mutation.rs:2344-2412, read), operand constructed path (flow_expression.rs:5389-5408, read), call-write invalidation (flow_call_phases.rs:1273-1306, read).
4. Compared with `domain_predicate_theory_is_complete` (expression_types_and_fields.rs:8390-8410, read), which rejects `Constrained` target types; `qualification_is_content_independent` does not inspect `definition.target_type` (flow.rs:6609-6616, read).
5. **Input to accepting branch:** A `DomainMembership`/`ContractDomainMembership` on a `NonresidentContent` write region whose `domain_symbol` passes the empty-shell checks at flow.rs:6609-6636 (read).
6. **Verdict: REACHABLE.** A domain whose carrier type still carries constraints but the definition record has no classification, roles, operators, or present predicate can be marked content-independent, so owner qualifications on nonresident content are preserved across content-affecting writes and call-write invalidation is skipped when `preserves_owner_qualification` returns true (flow_mutation.rs:2371-2375, flow_call_phases.rs:1293-1294, read).
7. **Unsure:** Whether typed-tree domain admission always rejects constrained carriers without full predicate theory before stage 05 (not read).

### A0118

1. Read in full: `matching_mutation_for_fact_place` (flow.rs:6901-6970, read) and `visit_context_invalidations` / `filter_contexts_after_place_mutations` (flow.rs:7043-7107, read).
2. Search `rg '\bmatching_mutation_for_fact_place\b' omega-rust` → 8 matches in 2 files (read grep output).
3. Read callers at flow.rs:7030, 7097 and flow_call_phases.rs:1296 (read); read operand-context filter (flow_operand_contexts.rs:440-452, read).
4. **Input to accepting branch:** A semantic `Fact` with `fact_place` whose stored root is `PlaceRoot::Unknown`, invalid `Symbol`, or invalid `Expression` (flow.rs:6912-6920, read).
5. **Verdict: UNDECIDED.** For those roots the matcher returns `None`, so `visit_context_invalidations` never marks the fact invalidated (flow.rs:7067-7070, read)—matching the digest’s “survives every write” wording—but I did not read every publisher of semantic fact places to determine whether such rows appear on programs that reach final checking acceptance.
6. **Unsure:** Production paths that attach `DomainMembership` (or other checked obligations) to `Unknown`/invalid place handles.

### A0119

1. Read in full: `DirectReads::place` and `DirectReads::value` (flow.rs:8100-8139, 8027-8039, read); read `preserve_statement_effects` when `flow` is absent (flow.rs:8884-8905, read).
2. Read `call_receiver_read_accesses` incomplete-footprint fallback (flow.rs:7628-7657, read) and footprint `complete` collection (flow.rs:7724-7796, read).
3. **Input to accepting branch:** A `Name` expression where `place_at` does not yield `PlaceRoot::Symbol` (flow.rs:8110-8122, 8137-8138, read), or origin tracing runs with `flow: None` (flow.rs:8884-8905, read).
4. **Verdict: CLOSED.** Missing per-read rows set `captured_home`/`complete` false (flow.rs:8129, 7788-7796, read); borrow checking then uses the whole receiver place (flow.rs:7655-7657, read), which is conservative for overlap. The no-flow origin path checks only call `may_write` and statement value-write frames (flow.rs:8884-8905, read), consistent with the comment that missing published rows do not substitute resolver frames (flow.rs:8834-8839, read).
5. **Unsure:** Whether any acceptance check relies solely on precise receiver-read lists without the whole-receiver fallback (not fully traced).

### A0120

1. Read in full: `project_constraint_refs_to_active_contexts` (flow.rs:9257-9284, read).
2. Search `rg '\bproject_constraint_refs_to_active_contexts\b' omega-rust` → 17 matches in 3 files (read grep output); read one call site in call exit build (flow_call_phases.rs:2300-2304, read).
3. Read parallel pattern `filter_expired_borrow_loans` keeping `Unknown` (flow.rs:6062-6074, read).
4. **Input to accepting branch:** Constraint refs including `FlowConstraintKind::Unknown` or borrow-resource kinds when active semantic contexts are projected away (flow.rs:9277-9282, read).
5. **Verdict: CLOSED.** The keep predicate always retains `Unknown` and borrow-resource constraints while dropping stale `SemanticContext` links (flow.rs:9277-9282, read); loan expiry filtering uses the same retention rule for non-loan kinds (flow.rs:6072-6074, read).
6. **Unsure:** None material for this site.

### A0121

1. Read in full: `derive_checked_semantic_dependencies` (flow.rs:9419-9497, read), `event_exposure` (flow.rs:9726-9753, read), `append_type_dependencies` (flow.rs:9545-9582, read).
2. **Input to accepting branch:** Machines with no entry state (`continue` flow.rs:9426-9427), calls whose return type lookup fails (9452-9455), permission events with missing machine or `event_type_reference` failure (9468-9481), roots failing `event_exposure` predicates defaulting to `PrivateImplementation` (9732-9753, read).
3. Read consumer `validate_portable_references` (checking.rs:645-648, read): missing rows only shrink portable reference closure.
4. **Verdict: CLOSED.** Skips omit dependency metadata rather than bypassing type or borrow checking; private exposure is the conservative default.
5. **Unsure:** Whether package review treats missing automatic-cleanup rows as silently equivalent to “no dependency” (not read outside checking.rs).

### A0122

1. Read `selected_cleanup_leaves_in` (flow.rs:10600-10683, read), `derive_cleanup_effect_dependencies` loop (flow.rs:10727-10814, read), `visit` (flow.rs:10853-10973, read).
2. **Input to accepting branches:** `disposition.zero_image` (10617-10618), machines not `CheckedBody`/open/proof-only (10739-10743, 10759-10767), `TypeReferenceNode::DynamicTrait` at visit (10964-10968), missing target machine or entry state when recording edges (10799-10803).
3. Verdict: CLOSED.** Each skip matches stated comments (e.g. cleanup eligibility vs proof of hook requirements at 10686-10691, read); missing target machine `continue` drops an edge only when the symbol does not resolve (10799-10800, read), inside dependency derivation already guarded by disposition validity from published facts (not fully traced).
4. **Unsure:** Whether an ill-formed disposition with a dangling machine symbol can reach this loop without an earlier diagnostic (not traced).

### A0123

1. Read `selected_implementations` cycle guard (flow.rs:11500-11516, read) and `dynamic_effect_dependencies::derive` (flow.rs:11449-11497, read).
2. **Input to accepting branch:** Recursive selector projection revisiting the same `(operand, projection)` pair in `active` (flow.rs:11511-11515, read).
3. **Verdict: CLOSED.** Cycle returns `Some(Vec::new())`; derive skips empty origins (11475-11476, read), under-approximating captured implementations per module comment (11437-11438, read).
4. **Unsure:** None material.

### A0124

1. Read in full: `copied_guarantees::retain_guarantees` (flow_call_phases.rs:814-893, read), especially the `all` closure (835-857, read).
2. Read caller `build_call_exit_contexts` (flow_call_phases.rs:2284-2297, read).
3. **Input to accepting branch:** Ensure-clause contract facts whose occurrences do not resolve to a symbol-rooted place, are not parameters, or have `contract_parameter_position` `Ok(None)` (flow_call_phases.rs:836-855, read).
4. **Verdict: CLOSED.** Those occurrences are treated as surviving (branches return `true`), retaining guarantees rather than dropping checks; lookup failure returns `false` (854-855, read).
5. **Unsure:** None material.

### What I did not read

- Full bodies of all 17 `project_constraint_refs_to_active_contexts` call sites beyond samples.
- Typed-tree / validation domain admission for constrained carriers without predicates.
- Every semantic-fact publication path that sets `PlaceRoot::Unknown` or invalid roots.
- `checks_borrows` and related borrow gates end-to-end from receiver-read footprints.
- Remainder of `selected_implementations` after the cycle guard (flow.rs:11517+, partial).
- `validation::infer_operational_may_for_dependencies` and downstream effect of skipped cleanup edges.

SLICE COMPLETE
