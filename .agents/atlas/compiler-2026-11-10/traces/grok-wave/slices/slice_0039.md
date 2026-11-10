slice_0039

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0293 | tp/build_bindings.rs:32-41 | CLOSED | Same `produce` path runs `select_terminal_machine`, which errors on zero or ambiguous `Name` matches (read). |
| A0294 | tp/lib.rs:173-177 | CLOSED | `Err` `instances` still allow terminal emission, but `prepare_boundary_calling_plans` fails on that `Err` before provider settlement (read). |
| A0295 | tp/lib.rs:188-198 | UNDECIDED | `continue` on `None` is read; full `source_boundary_declarations` roster and whether a skipped row is required later were not traced. |
| A0296 | fa/lib.rs:1859-1867 | UNDECIDED | Label fallback is read; whether distinct symbols with identical `symbols.name` strings can meet at the same proof site was not determined. |
| A0297 | fa/lib.rs:1466,1479 | UNDECIDED | Invalid field symbols collapse `PlaceSegment` equality (read); whether unresolved member paths still reach domain-membership proofs was not traced. |
| A0298 | fa/predicates.rs:121-125 | UNDECIDED | Prefix `take(maximum_length)` is read; callers pass geometry caps (read) but whether `maximum_length < bytes.len()` occurs on obligation vs assumption paths was not determined. |
| A0299 | c&o:109-114 | CLOSED | `retain` returning `None` drops to contract proof failure diagnostics in `checks_contracts_calls` (read). |
| A0300 | c&o:189 | CLOSED | `discharge_all` has no production callers in `omega-rust` (grep); `&|_, _| true` is not on any acceptance path found. |

### A0293

1. Read `native_formations::capture` in `omega-rust/psi/compilation/psi-compiler/terminal-production/src/build_bindings.rs:18-41 — `TerminalMachineSelection::Name` filters `checked.machines()` by `name.as_str()` and sets `root` to `None` when there are zero or more than one matches (read `:32-41`; digest cited `:33-41`, one line low).
2. Search `capture_native_formations` in `/agent/repos/omega`: **2 hits** (definition `build_bindings.rs:283`, caller `terminal-production/src/lib.rs:440`).
3. Read `TerminalProductionRequest::produce` at `lib.rs:439-452` — `capture_native_formations` runs, then `lower_machine` with the same `self.machine` (read).
4. Read `select_terminal_machine` at `machine_lowering.rs:1445-1461` — zero matches → `MachineNotFound`; two or more → `AmbiguousMachineName` (read).
5. Input on the accepting branch: a `TerminalMachineSelection::Name` where machine-name resolution in `capture` yields `root = None` (ambiguous or zero hits).
6. **Verdict: CLOSED** — `tp/lib.rs:442-452` + `machine_lowering.rs:1454-1460` reject before terminal production completes; `native_binding_formation` never publishes a completed artifact on those paths.

### A0294

1. Read `calling_signatures::capture` in `terminal-production/src/lib.rs:46-181 — per-instance `policy`, `policy_declaration`, and `applications` are `Result` values pushed even when inner steps fail (read `:135-169`); `instances` from the closure may be `Err(Vec<String>)` (read `:73-172`).
2. Search `calling_signatures::capture` in `/agent/repos/omega`: **1 production caller** — `terminal-production/src/lib.rs:455` (read).
3. Read push site `lib.rs:173-177` — `CallingPolicyRequest { instances, .. }` is appended regardless of `instances` being `Err` (read).
4. Read `CallingPolicyRequest` at `terminal-psi/src/lib.rs:461-464` — `instances: Result<Vec<CallingPolicyInstance>, Vec<String>>` (read).
5. Read consumer `plan_computation.rs:116-119` — `request.instances.as_ref().map_err(|reasons| diagnostic(reasons.join("; ")))?` (read).
6. Read `execution_settlement.rs:401-405` — `prepare_boundary_calling_plans(...)?` on the product compile path (read).
7. Input on the accepting branch: a calling-policy instance whose `abi_refusal`, policy callable lookup, or declaration match fails inside the closure but is stored as `Err` fields rather than aborting `capture`.
8. **Verdict: CLOSED** — terminal emission may record the `Err` (read `:173-177`), but full product compilation fails at `execution_settlement.rs:401-405` via `plan_computation.rs:118-119`. Did not read every psi-only path that skips `prepare_boundary_calling_plans`.

### A0295

1. Read `boundary_application` at `terminal-production/src/lib.rs:183-198` — after `abi_for_boundary`, non-`ExternalRealization` machines return `None` without error (read `:193-197`).
2. Read caller `calling_signatures::capture` at `lib.rs:56-63` — `None` → `continue`, no `CallingSignatureDeclaration` pushed (read).
3. Read remainder of `boundary_application` `:199-313` for `ExternalRealization` paths returning `Some(Err(..))` or `Some(Ok(..))` (partial read).
4. Input on the accepting branch: a `source` in `boundaries` with no `abi_for_boundary` and `supply_mode` not `ExternalRealization`.
5. **Verdict: UNDECIDED** — whether such a `source` appears in `lowered.source_boundary_declarations` and whether absence of a captured row is benign or leaves a hole in later calling/ABI planning was not determined; `source_boundary_ids` construction in `machine_lowering.rs` was not read in full.

### A0296

1. Read `places_match` at `facts/src/lib.rs:1819-1867` — structural `places_equal` first; otherwise canonical labels via `write_canonical_place_label` and string equality (read `:1859-1864`).
2. Read `write_canonical_place_label` / `push_symbol_label` at `contexts_and_places.rs:1272-1340` — valid symbols use `program.symbols.name(symbol)` (read).
3. Search `\bplaces_match\b` in `/agent/repos/omega/omega-rust`: **12 hits** in **5 files** (read counts from grep).
4. Read `proves_place_domain_membership_in_program` at `contexts_and_places.rs:1040-1063` — uses `places_match`, not `fact_place_equals` (read `:1062`; compare `proves_place_domain_membership` at `:1034` using `fact_place_equals`).
5. Partial caller read: `checks_contracts_prover.rs:6160-6178`, `checks_contracts_calls.rs:4015`, `flow_transfers.rs:1100` — not all six Psi 05 callers read in full.
6. Input on the accepting branch: two `PlaceHandle` values with different roots or segments but equal canonical label strings.
7. **Verdict: UNDECIDED** — label fallback behavior is confirmed (read); whether the compiler can place two distinct storage bindings with the same bare `symbols.name` spelling at a site where `proves_place_domain_membership_in_program` decides acceptance was not determined.

### A0297

1. Read `append_place_from_expression` for `ExpressionNode::Name` at `facts/src/lib.rs:1458-1468` and `Member` at `:1473-1482` — unresolved members become `SymbolHandle::invalid()` on field segments (read `:1466`, `:1479`).
2. Read `places_equal` at `:1819-1825` — segment slice equality uses `PlaceSegment` `PartialEq` (read).
3. Inferred: two different member spellings that both fail resolution yield identical `PlaceSegment::Field { symbol: invalid }` chains, so `a.x` and `a.y` can have equal segment lists.
4. Input on the accepting branch: place construction with invalid member symbols on two differently spelled members of the same receiver.
5. **Verdict: UNDECIDED** — segment collapse is read; whether such places participate in `proves_place_domain_membership_in_program` / contract proof acceptance without an earlier diagnostic was not traced through Psi 05 callers.

### A0298

1. Read `byte_literal_equality_in_extent` at `facts/src/predicates.rs:100-148` — length compares full `bytes.len()` (`:116-119`); byte comparisons use `.take(maximum_length.unwrap_or(bytes.len()))` (`:121-125`) (read).
2. Search `byte_literal_equality_in_extent` in `/agent/repos/omega/omega-rust`: **3 hits** — definition, `values_scalar.rs:622`, `scalar_computations.rs:348` (read).
3. Read `values_scalar.rs:614-627` — `maximum_length` from fixed-array length or bounded sequence capacity (read).
4. Input on the accepting branch: call with `maximum_length` strictly less than `bytes.len()`.
5. **Verdict: UNDECIDED** — truncation logic is read; whether that call shape arises on a proof obligation vs assumption, and whether evaluation would accept a carrier matching only the compared prefix, was not determined; obligation discharge paths beyond construction were not read.

### A0299

1. Read `ConstantPredicateObligation::retain` at `contract_and_obligation_facts.rs:48-126` — unsupported expression shapes hit `_ => return None` at `:109` (read).
2. Read caller `checks_contracts_calls.rs:3447-3523` — when `retain` returns `None`, falls through to `Diagnostic::error("cannot prove requires contract for call ...")` at `:3507-3523` (read).
3. Search `ConstantPredicateObligation::retain` in `/agent/repos/omega/omega-rust`: **1 production call site** — `checks_contracts_calls.rs:3450` (read).
4. Input on the accepting branch: target-dependent contract boolean whose expression shape is not in the `retain` walk (e.g. unsupported node at `:109`).
5. **Verdict: CLOSED** — no pending obligation is retained; Psi 05 emits an error instead (`checks_contracts_calls.rs:3507-3523`).

### A0300

1. Read `ConstantPredicateObligation::discharge_all` at `contract_and_obligation_facts.rs:173-217` — `scalar_predicates::evaluate_nodes` with `comparison_is_admitted` `&|_, _| true` at `:189` (read).
2. Read `retain` at `:86-89` — non-`builtin` binaries return `None` before capture (read).
3. Read `capture_predicate` at `:256-258` — uses `scalar_predicates::source_node` without the `builtin` closure (read).
4. Search `discharge_all` in `/agent/repos/omega/omega-rust`: **2 hits**, both in `contract_and_obligation_facts.rs` (definitions at `:173` and `:462`; read grep). No `ConstantPredicateObligation::discharge_all` call sites found.
5. Read `attached_unit.rs:7496-7531` — pending `constant_predicate_obligations` lowered without calling `discharge_all` (read).
6. Input on the accepting branch: binary predicate nodes evaluated under `discharge_all` with all comparisons admitted regardless of builtin policy.
7. **Verdict: CLOSED** — no production caller reaches `:189`; pending obligations flow through lowering (`attached_unit.rs:7496-7531`) while `discharged_target` is only set inside unused `discharge_all` (`:210`, read `discharged_target` grep).

### What I did not read

- Full `boundary_application` (`lib.rs:199-313`) and full construction of `source_boundary_declarations` / `source_boundary_ids` in `machine_lowering.rs`.
- Entire `calling_signatures::capture` closure and all `foundation_validation` branches beyond `calling_policy_requests` (`foundation_validation.rs:1198-1259`).
- All six external `places_match` / `proves_place_domain_membership_in_program` caller chains in Psi 05 (only partial reads in `checks_contracts_prover.rs`, `checks_contracts_calls.rs`, `flow_transfers.rs`, `flow_calls.rs`).
- Byte predicate evaluation semantics after `CheckedBooleanExpression` construction in lowering.
- Symbol-uniqueness / homonym policy across machines and states in symbol resolution.

SLICE COMPLETE
