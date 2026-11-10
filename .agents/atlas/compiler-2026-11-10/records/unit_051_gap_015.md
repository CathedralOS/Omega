Second pass for unit 51

Ranges covered (path relative to `<checkout>/`):
- `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_contracts_prover.rs` 7701-8071 (371 lines), read in full.

Context lines read above the range (not counted): 7565-7700. The range starts inside `fn evidence_inner` (7646-7890), inside `mod instruction_effects` (6523-8066). Line 7701 is the first line of a `let receipt = InstructionEffectEvidence { ... }` literal in the `AtomicReleaseStore` branch. The earlier notes (unit_051.md) say nothing about this range (they stop at prover.rs:1700), so there is no contradiction to report.

All file:line citations below are `checks_contracts_prover.rs` unless stated.

## 7701-7890: tail of `evidence_inner` (the instruction-effect evidence search)

### Purpose
Given a proposition application that names an instruction-effect contract (a "this register operation / atomic release store was performed" premise), it searches the fact contexts at a statement for an event that witnesses it. It returns a receipt (`InstructionEffectEvidence`) naming the witnessing fact, or `None`. A receipt may not "overlap" one already spent, so one physical event is not used twice. (read; header of the function 7646-7660, doc at 7565-7567 says overlap means "any execution alternative would spend the same physical event".)

### Shape
- Enclosing function `evidence_inner(program, facts, state_symbol, statement, contexts, application, spent, active)` 7646-7890, returns `Option<InstructionEffectEvidence>`. `active: &mut Vec<SymbolHandle>` is a recursion guard of machine symbols.
- Struct `InstructionEffectEvidence` 7569 (declared before the range): `event`, `contract` (a `ContractProofFact` handle, `Default::default()` for direct events), `children: Vec<..>`, `observation: ProgramPoint`. Built at 7701, 7846, 7877 (all in range).
- Three witness routes, tried per `(context, fact reference)` pair in the double loop 7682-7888:
  1. Atomic release-store event (7689-7712): payload `InstructionEffect { operation: AtomicReleaseStore }` while `operation` is `AtomicReleaseStore`; accepted when `atomic::matches_store(...)` (7697, defined 6634). Receipt has no contract and no children (7701-7706). `continue` after the branch whether or not it matched (7711), so such an event never falls into route 2 or 3.
  2. Call-completion with a callee guarantee (7738-7858): event payload `CallCompletion { target }` equal to `call.target_symbol`. For every `Ensures` contract fact of the call (7740-7742) whose proposition (7745-7749) `matches` the requested operation (`operands::guarantee_matches` 7752 for `Register`, `atomic::guarantee_matches` 7767 for `AtomicReleaseStore`), it finds the callee's flow (7776-7785) and machine (7786-7789), skips a callee already in `active` (7790), collects the callee's `exits` (7793-7801), skips when none (7802), pushes the callee onto `active` (7805), and recursively calls `evidence_inner` once per exit (7828-7837) with the exit's entry-constraint contexts (7809-7812). `collect::<Option<Vec<_>>>()` (7843) makes it all-or-nothing: every exit of the callee must itself produce evidence. `active.pop()` (7844). On success the receipt carries `contract: ensured.fact`, the child receipts and the call's observation (7846-7851).
  3. Direct instruction event (7859-7886): payload `InstructionEffect { operation: recorded }` with `recorded == operation`; the call's operand rows (7863) must match `required` one by one under `operand_matches` (7870, defined 7147) using each carrier's primitive type (7867).
- Return `None` at 7889 when nothing matched.

### Inputs and outputs
Reads `facts.semantic.{refs,contexts,facts}` (7683-7688), `facts.flow.control.{states,exits,operands}` (7776-7863), `facts.proof.{contract_fact_refs,contract_facts}` (7740-7741), `program.proof_facts`, `program.machine_by_symbol`, `program.statement_table.admitted_execution_order` (7660-7662, above range), `call_flows(...)` (7730). `spent` is read only (7707, 7822, 7852, 7883). `active` is pushed and popped around the recursion (7805, 7844). Output: a fresh receipt value; nothing is mutated except `active`.

### How it decides (keys)
- Event identity: the fact handle `reference.fact` (7702, 7848, 7878); events are tied to a place in the program by `ProgramPoint::CallEnsures { machine_symbol, state_symbol, statement_index, call_ordinal }` (7713-7721). Same machine and state symbol are compared (7722-7723); the call is found by `(statement_index, call_ordinal)` (7730-7734). Keyed on symbols and indices, not names.
- Ordering: the event's statement must precede the query statement in the admitted execution order, or the query statement is the state's end (`statement == state_statements(...).len()`) and the event's statement is in the order (7724-7727).
- Callee flow lookup: scans all flow states for `flow.state_symbol == call.target_symbol` (7776-7782), first match wins (symbol identity).
- Operand-count check at 7864: `operands.len() == required.len()`, then zipped with `carriers` (the operation's input carriers, 7670-7673).
- Carrier typing by name: `PrimitiveType::from_name(carrier.name())` at 7867; a carrier whose name is not a primitive makes the route return `false` (7868). This is a spelling lookup of the carrier's type name (carrier type is outside the range; `name()` is not read).
- Spent test: `spent.iter().any(|other| receipt.overlaps(facts, other))` at 7707, 7852, 7883. `overlaps` (7605-7624, read as context) compares the event's `ProgramPoint`, then children by `observation` equality.
- `spent_here` (7819-7827): for the recursive child call, the spent children whose parent event shares `event.point` and whose `observation` equals this exit's observation.

### Cases by shape
- `arity` is `2` for `AtomicReleaseStore` and `carriers.len()` otherwise (7674-7678); `required.len() != arity` returns `None` (7679-7681). Atomic store is hard-coded to two arguments. `carriers` is `&[]` for the atomic store (7672).
- Only two operation kinds are handled: `Register(_)` and `AtomicReleaseStore` (7670-7673, 7750-7772 match is exhaustive over those two variants).
- Only the first `Ensures` guarantee that matches, has a callee flow, has exits, and whose children all succeed and are not spent, is returned (7845-7855).

### Same job elsewhere
- `atomic::matches_store` / `atomic::guarantee_matches` (6634, 6743) and `operands::guarantee_matches` / `operand_matches` (7189, 7147) are the per-operation matchers; they are called here but defined before the range (not read by me).
- `evidence` (7626) is a thin wrapper that supplies an empty `active`. Outside callers of the evidence entry: `checks_multiplicity.rs:4839` (grepped; only one). `instruction_effect_proven` (alias of `proves` at 7542) has one outside use, `checks_contracts.rs:540` (grepped). `InstructionEffectEvidence` is named in `checks_multiplicity.rs:4639` (field `spent`) and in this file only (Grep count over `omega-rust` for `instruction_effect_evidence|instruction_effect_proven|InstructionEffectEvidence`: 16 occurrences in 3 files: prover.rs 13, multiplicity.rs 2, contracts.rs 1).
- The three receipt-construction blocks (7701-7706, 7846-7851, 7877-7882) repeat the same spent-then-return pattern (read).

### Unused or one-caller code
- `evidence_inner`: private; called from `evidence` (7635) and from itself (7828). Not a one-caller finding beyond that.
- `evidence` has one external caller (`checks_multiplicity.rs:4839`) as grepped above.

### Stated limits
- None in range 7701-7890. (The doc above the struct at 7565-7567, outside the range, speaks of "mutually exclusive exits may select different instructions".)

### Fail-open risks
- Route 1 (7689-7712) does not test the event's `ProgramPoint`, the statement order, or the call. Any `AtomicReleaseStore` fact in the active contexts for which `atomic::matches_store` returns true yields a receipt. Whether `matches_store` itself checks position (it takes `state`, `statement`, `contexts`) was not read by me: open. The other two routes explicitly require a `CallEnsures` point and ordering (7713-7729), so this asymmetry is only visible by reading.
- Route 3 (7863-7886): `operands.iter().zip(&required).zip(carriers).all(...)` is vacuously true when `required.len() == 0 == carriers.len()`. An instruction operation with zero input carriers would then match any recorded event of that operation at an ordered call. Whether any `Register` operation has no input carriers: not checked, open.
- `exits.is_empty()` (7802) and missing callee flow/machine (7776-7789) `continue`: these reject (closed), not accept.
- `active.contains` (7790) rejects recursion; a cycle yields no evidence (closed).
- `Default::default()` contract handle (7703, 7878) is a placeholder for events with no contract; it is published into `FlowInstructionEffectReceiptFact.contract` (7599, context lines). Stated by the struct doc? not stated; "open" whether consumers treat the default handle as "no contract".
- Otherwise none seen in 7701-7890.

## 7892-8015: `QueryArgument`, `QueryFrame`, `QueryArguments::new`, `scoped_value`

### Purpose
Builds the chain of proposition-application "frames" for an instruction-effect contract, so an argument written as a parameter of an inner proposition can be traced back to the source expression written at the outermost application. Then evaluates a frame argument as a scoped arithmetic value, projecting through struct places.

### Shape
- `enum QueryArgument` 7892-7899: `Source(ExpressionHandle)` or `Parameter { frame: usize, expression }`.
- `struct QueryFrame<'program>` 7900-7904: `declaration: &PropositionDefinition`, `application: &PropositionApplication`, `arguments: Vec<QueryArgument>`.
- `struct QueryArguments<'program> { frames: Vec<QueryFrame> }` 7905-7907.
- `QueryArguments::new(program, contract)` 7909-7950: for each application in `contract.applications` (7914), look up the declaration by `application.proposition` symbol (7915-7916), check actual count equals declared parameter count (7918-7920, else `None`), then for each actual expression: if there is a previous frame, try to read the expression as a name path (7924-7926) matching one of the previous declaration's parameters by `parameter.symbol == name.symbol` (7928-7931) and take the previous frame's argument at that position (7932-7933); if that does not resolve, record `Parameter { frame: frames.len()-1, expression }` (7934-7937). For the first frame every argument is `Source` (7939).
- `scoped_value` 7951-8014: `Source` calls the caller-supplied `source` closure (7964-7968). `Parameter`: finds the frame (7970), tries `structural_parameter_place` + `resolve_structural_parameter_path` (7972-7978) to project the expression to `(position, segments)`; on success appends the outer `projection` and recurses into the frame's argument at that position (7979-7988). Otherwise delegates to `call_guarantees::arithmetic::scalar_term` with a `Callable::Proposition` (7990-7994) whose leaf resolver either projects again (7998-8006) or takes `program.closed_integer_value_in(occurrence, frame.declaration.symbol)` as an `Integer` value (8008-8010).
- `unreachable!()` at 7966 is reached only if `QueryArgument` gains a third variant (the two `let-else`s are exhaustive today).

### Inputs and outputs
Reads `validation::InstructionEffectContract` (`applications`), `program` proposition tables (`proposition_definition_by_symbol`, `proposition_parameters`, `proposition_binders`), `expressions(program)`. Produces `QueryArguments` or `ScopedArithmeticValue` options.

### How it decides (keys)
- Argument forwarding is keyed on symbol identity: `parameter.symbol == name.symbol` (7928-7931), position within the previous declaration's parameter list.
- A forwarded argument is recognised only when the actual is a name path (`name_path_node`, 7925). Any other expression (e.g. `a + 1`) becomes `Parameter{frame: previous, expression}` (7934-7937), evaluated later by `scalar_term`.
- Frame count mismatch returns `None` (7918-7920), which in `evidence_inner` ends the whole query as `None` via `?` at 7668.

### Cases by shape
- Name-path forwarding only, with struct projection handled through `structural_parameter_place` (defined outside the range, not read).
- `scoped_value` handles `Integer` values from closed proposition-parameter constants only (8008-8010).

### Same job elsewhere
- `QueryArguments::source_projection` (8018-8041) walks the same frame chain as `scoped_value`, with the same `structural_parameter_place` + `resolve_structural_parameter_path` calls (8029-8032 vs 7973-7976); `binder_symbol` (8042-8064) walks the chain for type binders. Three recursions over one frame chain in this file (read).

### Unused or one-caller code
- Not grepped for `scoped_value`, `source_projection`, `binder_symbol`; these are private to `instruction_effects::QueryArguments`. Not individually grepped; call sites are in the unread-by-me range 6523-7700 (suspected).

### Stated limits
- None in range.

### Fail-open risks
- `QueryArguments::new` returns `None` on any unresolved declaration or count mismatch, which rejects. `scoped_value` returns `None` on missing frame or position (7970, 7984, 8001). None seen.

## 8017-8065: `source_projection`, `binder_symbol`

- `source_projection(program, argument, suffix)` 8018-8041: returns `(ExpressionHandle, Vec<PlaceSegment>)`; `Source(expr)` returns `(expr, suffix)` (8025). `Parameter` recurses to the next frame with segments extended by the suffix (8033-8038). `None` when the frame, place or position cannot be resolved (8027-8032, 8036).
- `binder_symbol(program, frame, position)` 8042-8064: returns the type-binder symbol for position `position` of frame `frame`'s `binder_arguments` (8049). Only `PropositionBinderArgumentKind::Type` arguments are accepted (8050-8052, else `None`). If a previous frame exists and its declaration has a binder with `binder.symbol == argument.symbol` (8055-8059), recurse into the previous frame at that binder's position (8060). Otherwise returns `argument.symbol` when `symbol.is_valid()` (8063).
  - Judgment keyed on symbol identity. Fail-open: an argument symbol that is not a binder of the previous frame is returned as a concrete symbol (8063); "stated" nowhere; it means the binder was instantiated with a concrete type symbol (inferred).
- Callers of these two not grepped (suspected callers in `operands`/`atomic` submodules, 6540-7700).

## 8066-8071: closing of `mod instruction_effects` and re-exports

- 8066 closes `mod instruction_effects`.
- 8067: `pub(super) use instruction_effects::proves as instruction_effect_proven;` (one outside use: `checks_contracts.rs:540`, grepped).
- 8069: `pub(in crate::checks) use instruction_effects::evidence as instruction_effect_evidence;` (one outside use: `checks_multiplicity.rs:4839`, grepped).
- 8071: `pub(in crate::checks) use instruction_effects::InstructionEffectEvidence;` (one outside file reference: `checks_multiplicity.rs:4639`, grepped).
- This is the end of the file (8071 lines).

## Coverage
- Read in full: `checks_contracts_prover.rs` 7701-8071 = 371 lines.
- Skimmed: 0. Not read: 0.
- Context-only lines read (not counted): 7565-7700.
- Grep commands run (Grep tool, over `omega-rust`): `mod instruction_effects|fn evidence_inner|fn evidence\b|fn proves\b|struct InstructionEffectEvidence|fn overlaps|fn operand_matches|mod atomic|mod operands|fn matches_store|fn guarantee_matches` (prover.rs only, content mode); `instruction_effect_evidence|instruction_effect_proven|InstructionEffectEvidence` (count mode: 16 in 3 files; content mode for the listing).

## Five things a maintainer most needs to know about this range
1. `evidence_inner` (7646) finds a witnessing fact for an instruction-effect premise by three routes: atomic release store (7689), a callee's `Ensures` that recursively needs evidence at every callee exit (7738-7858), and a directly recorded instruction event with matching operands (7859-7886).
2. The recursive route is all-or-nothing across exits (`collect::<Option<Vec<_>>>`, 7843) and guarded against recursion by machine symbol (`active`, 7790, 7805, 7844). A callee with no exits yields nothing (7802).
3. The atomic-store route (7689-7712) does not test event point or statement order in this function, unlike the other two (7713-7729); whether `atomic::matches_store` (6634) does it was not read.
4. Argument forwarding for nested proposition applications is by parameter symbol identity along a frame chain (`QueryArguments::new` 7909, `scoped_value` 7951, `source_projection` 8018). Anything other than a bare name forwards as `Parameter{..}` and is evaluated by `scalar_term`.
5. Single-use exports: `evidence` is used from `checks_multiplicity.rs:4839` only, `proves` from `checks_contracts.rs:540` only (grepped).

## What I am unsure about
- Whether `atomic::matches_store` checks the event's program point and ordering (route 1).
- Whether any `Register` operation has zero input carriers, which would make route 3's `.all` vacuous (7865).
- What `PrimitiveType::from_name(carrier.name())` (7867) does with non-primitive carrier names beyond returning `None`; defined outside range.
- Whether consumers of `FlowInstructionEffectReceiptFact.contract` treat `Default::default()` as "no contract".
- The call sites of `scoped_value`, `source_projection`, `binder_symbol` (in 6523-7700, not in my unit).
