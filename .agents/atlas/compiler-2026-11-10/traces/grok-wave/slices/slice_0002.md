slice_0002

| entry id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0009 | checking.rs:47-54, 329-334 | UNDECIDED | Missing operational state/call row skips acknowledgement check; operational completeness vs typed calls not traced to acceptance gate. |
| A0010 | checking.rs:790-792, 995-997 | CLOSED | Package checking always supplies declaration authority in source_checking; other 15 `CheckingRequest::new()` sites are documented toolchain/preliminary paths. |
| A0011 | checking.rs:408-412 | CLOSED | `admits_pending_inputs` only defers opaque Copy at preliminary checkpoint; settled preparation requires receipts before acceptance. |
| A0012 | checks.rs:2602-2603, 2846-2850 | REACHABLE | Lifetime depth cap returns `Inferred`, and `compare` treats two non-`Known` regions as matching. |
| A0013 | checks.rs:4167-4169 | CLOSED | Early return only on expressions already marked unreachable in the static type context. |
| A0014 | checks.rs:4205-4210, 4294-4305 | UNDECIDED | Flow call loop `continue`s without runtime-index checks when parameter roster or call site lookup fails; callers to acceptance not fully traced. |
| A0015 | checks.rs:4269 | UNDECIDED | Assignment runs `scope.check` on the value only when `actual(target)` is `Some`; full assignment-target shapes not read. |
| A0016 | checks.rs:1902, 1926-1935, 3328-3330 | CLOSED | Invalid `SymbolHandle` is treated as an internal sentinel in proof-index const/stable checks, not as an unresolved user name at this stage. |

### A0009

1. Read `validate_call_acknowledgements` in full (`checking.rs:39-77`) and `validate_call` in full (`checking.rs:318-365`).
2. Search `rg 'validate_call_acknowledgements' /agent/repos/omega/omega-rust`: 2 file hits (definition + 1 call at `published_facts.rs:11230`).
3. Search `rg '\bvalidate_call\(' .../checking.rs`: 4 hits (1 def, 3 call sites inside `validate_statement`/`validate_child`).
4. Upward trace: `published_facts.rs:11230` sits in `finish_check_facts` after `admit_prepared_program_effects` (`published_facts.rs:11228`); failures return `Err` and abort `check_program` (`checking.rs:1077-1088`).
5. Accepting branches: (a) no operational state with matching `summary.symbol` → `continue` (`checking.rs:47-54`); (b) no `CallOperational` for `(statement_index, call_ordinal)` → `return` without diagnostic (`checking.rs:329-334`). Envelope vs acknowledgement is only checked when a row is found (`checking.rs:347-358`).
6. Verdict: **UNDECIDED** — entry matches read code; whether a program that should fail acknowledgement validation can reach these branches with a missing row/state while still passing earlier effect admission was not established.
7. Unsure: whether operational call rows are always present for every typed call on every path that yields `Ok(())` from `lower_typed_trees`.

### A0010

1. Read `CheckingRequest` (`checking.rs:755-776`), comment at `checking.rs:788-792`, and `admit_request` in full (`checking.rs:991-999`).
2. Search `rg 'CheckingRequest::new\(\)' /agent/repos/omega/omega-rust`: 16 construction sites (1 `source_checking.rs`, 2 `const_initializers.rs`, 7 `machine_execution.rs`, 5 `const_evaluation.rs`, 1 example).
3. Search `rg 'with_declaration_authority' /agent/repos/omega/omega-rust`: 2 hits — method def (`checking.rs:770`) and one production use (`source_checking.rs:703`).
4. Read `validate_authored_declaration_selections` header and body start (`declarations_core.rs:905-951`).
5. Input to accepting branch: `request.declaration_authority` is `None` at 15 sites → `admit_request` skips selection validation (`checking.rs:995-997`).
6. Verdict: **CLOSED** — shippable package checking uses `source_checking.rs:702-704` with `self.admission.declarations`, and reuse paths still call `admit_request` (`checking.rs:923`, `979`). Comments at `checking.rs:788-792` and `README.md:79-81` (read) state toolchain/preliminary checking intentionally omits authority until package reconciliation.
7. Unsure: none for the package-compile path; did not read every build-time consumer of `CheckedTrees` for non-package artifacts.

### A0011

1. Read `prepare_typed_program` in full at cited branch (`checking.rs:399-419`) and `check_program` call site (`checking.rs:1047-1055`).
2. Search `rg 'allow_pending_opaque_copy|PendingBuildSelection' .../05_typed-trees-to-checked-trees`: hits only in `checking.rs` and validation `program_validation.rs:547-555`.
3. Read `validate_opaque_data_properties` (`proof_contracts_core.rs:8182-8200`).
4. Read `PreparationRequest::settled` / `preliminary` (`preparation.rs:1836-1850`).
5. Input: `allow_pending_opaque_copy == true` when `admits_pending_inputs` propagates from preliminary preparation → Copy receipt not required for boundary opaque data (`proof_contracts_core.rs:8190-8199`).
6. Verdict: **CLOSED** — deferral is checkpoint-gated; package source checking uses `PreparationRequest::settled()` (`source_checking.rs:695`) with `admits_pending_inputs: false` (`preparation.rs:1838`), so strict `Required` path runs (`checking.rs:410-411`).
7. Unsure: none.

### A0012

1. Read `region_operand` in full (`checks.rs:2578-2637`) and `compare` Generic lifetime branch (`checks.rs:2839-2854`).
2. Search `rg 'region_operand\(' checks.rs`: definition + recursive use + `compare` call sites (read `2843-2844`).
3. Caller trace: `compare` is used in runtime-index checking in the same inline module (read through `check_completed_value` / binding checks; did not read every `compare` caller).
4. Input: nested lifetime inference with `depth > 16` → `Some(Region::Inferred)` (`checks.rs:2602-2603`); pair `(Some(_), Some(_))` where either side is not both `Known` → treated as match (`checks.rs:2849`).
5. Verdict: **REACHABLE** — a program whose lifetime evidence requires more than 16 levels of rowless-call inference could have disagreeing regions reported as equal because both operands are `Inferred` and the comparison arm returns true.
6. Unsure: exact minimal source shape; only the depth gate and comparison semantics were read.

### A0013

1. Read `check_completed_value` in full (`checks.rs:4161-4183`).
2. Search `rg 'static_type_context_is_unreachable' .../05_typed-trees-to-checked-trees`: multiple skip sites including `checks.rs:4167`.
3. Accepting input: expressions marked unreachable by static typing before runtime indexed-value equality runs.
4. Verdict: **CLOSED** — skip is intentional for dead code; unreachable arms are not part of the executable program contract (inferred from validation usage at `effect_inference.rs:199`, read one site only).
5. Unsure: whether any reachable-at-runtime expression can carry the unreachable flag incorrectly (not traced).

### A0014

1. Read `check` function sections at `checks.rs:4190-4210` and `4293-4305` (read through `4319`; did not read entire `check`).
2. Accepting input: `call_target_parameters` returns `None` or `find_call_site` returns `None` → `continue` with no runtime-index argument checking for that flow call.
3. Verdict: **UNDECIDED** — code matches entry; whether flow-published calls without resolvable sites/parameters can appear on programs that still pass `check_checked_facts_recording` was not traced.
4. Unsure: relationship between `facts.flow.control.calls` and guaranteed `find_call_site` success on accepted programs.

### A0015

1. Read `StatementNode::Assignment` arm (`checks.rs:4263-4271`) and `check_descriptor_store` (`checks.rs:3073-3081`); read `actual` through `checks.rs:2409-2444` only (not full `actual`).
2. Accepting input: `scope.actual(assignment.target)` is `None` → only descriptor-store checking runs, not `scope.check` on the value.
3. Verdict: **UNDECIDED** — entry matches read code; which assignment targets leave `actual` absent while still allowing runtime-indexed values was not enumerated.
4. Unsure: full `actual` implementation and assignment-target shapes.

### A0016

1. Read `expression_is_const` in full (`checks.rs:1896-1938`), `static_index` helper closure (`checks.rs:1950-1961`), and `stable_index_symbol` (`checks.rs:3327-3362`).
2. Search `rg 'stable_index_symbol|expression_is_const' checks.rs`: production uses inside same proof-index module (read call sites at `3397`, `3468`, `3499`, `1966`).
3. Accepting input: `!symbol.is_valid()` → `true` for stable-index and const eligibility (`checks.rs:3328-3329`, `1901-1902`, `1927`).
4. Verdict: **CLOSED** — at Psi 05 the typed program is preparation-closed; invalid handles function as sentinels in proof-index stability logic, not as late-bound user identifiers (inferred from stage contract in `checking.rs:1-5`, read header only).
5. Unsure: did not prove no user-visible spelling produces invalid named type references at this stage.

### What I did not read

- Full `check_program` after `build_check_facts` beyond `validate_call_acknowledgements` wiring.
- Operational plan construction in `effect_inference.rs` (beyond sampled lines).
- Entire `actual`, entire runtime-index `check`, and all `compare` callers.
- `find_call_site` / `call_target_parameters` implementations.
- Build-time paths that retain `CheckedTrees` without going through `source_checking.rs`.

SLICE COMPLETE
