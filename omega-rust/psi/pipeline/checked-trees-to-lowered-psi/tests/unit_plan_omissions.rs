//! A missing checked Unit plan reports why the checked stage omitted it: the
//! lowering error carries the omission chain from the requested machine to
//! the machine whose own body failed local construction.

use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(typed)
        .unwrap_or_else(|errors| panic!("{source}: {errors:#?}"))
}

const UNSUPPORTED_BODY: &str = r#"
    data Token { observed: bool; other: bool; }
    machine Token::drop(&mut self) {}
    data Main {}
    machine Main::main(token: Token, input: u64, enabled: bool) -> bool {
        let staged: bool = token.observed && ((input < 1u64) || enabled);
        staged
    }
"#;

#[test]
fn a_root_without_an_admitted_body_names_its_own_local_construction() {
    let checked = checked(UNSUPPORTED_BODY);
    let error = checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main")
        .expect_err("the mixed member/comparison body has no Unit plan");
    let checked_trees_to_lowered_psi::LoweringError::InvalidUnitMachinePlan {
        machine,
        reason,
        omission,
    } = error
    else {
        panic!("unexpected error: {error:?}");
    };
    assert_eq!(machine, "Main::main");
    assert_eq!(
        reason,
        "attached Unit closure is missing a checked transitive machine plan"
    );
    assert_eq!(
        omission.as_deref(),
        Some("`Main::main` has no admitted body (local construction stopped at completion)")
    );
}

#[test]
fn a_root_whose_callee_lacks_a_body_names_the_callee_chain() {
    let checked = checked(
        r#"
        data Token { observed: bool; other: bool; }
        machine Token::drop(&mut self) {}
        data Main {}
        machine Main::main(token: Token, input: u64, enabled: bool) {
            Main::relay(token, input, enabled);
        }
        machine Main::relay(token: Token, input: u64, enabled: bool) {
            Main::leaf(token, input, enabled);
        }
        machine Main::leaf(token: Token, input: u64, enabled: bool) {
            let staged: bool = token.observed && ((input < 1u64) || enabled);
        }
    "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let named = |name: &str| {
        checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} is declared"))
            .symbol
    };
    let omission = |name: &str| {
        plans
            .omission_for_machine(named(name))
            .unwrap_or_else(|| panic!("{name} has an omission row"))
            .stage
    };
    assert!(matches!(
        omission("Main::leaf"),
        checked_trees::CheckedUnitPlanOmissionStage::LocalConstruction { .. }
    ));
    assert_eq!(
        omission("Main::relay"),
        checked_trees::CheckedUnitPlanOmissionStage::UnavailableCallee {
            target: named("Main::leaf")
        }
    );
    assert_eq!(
        omission("Main::main"),
        checked_trees::CheckedUnitPlanOmissionStage::UnavailableCallee {
            target: named("Main::relay")
        }
    );
    let error = checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main")
        .expect_err("the root's transitive callee has no Unit plan");
    let checked_trees_to_lowered_psi::LoweringError::InvalidUnitMachinePlan {
        machine,
        omission,
        ..
    } = error
    else {
        panic!("unexpected error: {error:?}");
    };
    assert_eq!(machine, "Main::main");
    assert_eq!(
        omission.as_deref(),
        Some(
            "`Main::main` calls `Main::relay`, which has no plan; \
             `Main::relay` calls `Main::leaf`, which has no plan; \
             `Main::leaf` has no admitted body (local construction stopped at completion)"
        )
    );
}

// TR3-TR8 pin: routed `TaskRuntime::start<Worker::run>` establishment and the
// ordinary generic-receiver consumer behind it. The source below is the
// smallest program that calls the generic boundary requirement with a
// concrete target machine through a runtime capability carried on `self`,
// then settles the returned `Task<Token>` through the generic attached
// `Task::settle<T>(self)`. Checking retains the `start` call's admitted
// specialization as a checked fact, and monomorphization retargets the
// `settle` edge at a receiver-specialized `Task::settle$specialized` clone
// whose retained owner application is the concrete `Task<Token>`: the
// specialized machine's signature, self argument, and claim path all replay
// that application instead of the open `Task<T>`. Both calls produce checked
// operations, and the routed receiver now clears provider attachment
// requirements: the `token` parameter stays an ordinary argument while the
// `self.runtime` receiver supplies the `start` requirement row. The static
// boundary roster then resolves the `machine Target` binder from the
// retained specialization row — `T`/`Arguments` derive `Token` and `Target`
// selects `Worker::run`'s entry — so `TaskRuntime::start` carries a
// substituted boundary plan and `probe` clears the boundary target check.
// An owned `self` on `[linear]` attached data then establishes its Unit entry
// claim directly — the checker owns the consumption judgment and records no
// `StateEntry` event for it, so `entry_claims` mints the established identity
// — and both `settle` bodies plus `probe` carry full Unit plans. In
// emission the `start` boundary mints `task`'s caller claim binding
// from the `Establish` event its binding statement recorded — the minted
// `ClaimId` is that claim's only binding — and the completed result carries
// it. Ordinary call preparation now hands that completed-result custody to
// transfer-shape validation: `settle`'s `self` formal admits the move only
// when the result's single carried claim is the minted statement-established
// binding the formal's whole-value entry claim names, and the emitted call
// records the consume as a transfer of that exact claim. The remaining
// frontier is module validation: the ordinary-call argument source policy
// still admits only affine claim-free operation results.

const ROUTED_TASK_START_DECLS: &str = r#"
    data Task<T> [linear] {
        provider: u64;
        activation: u64;
    }

    boundary trait TaskRuntime {
        machine start<T, Arguments, machine Target>(
            &self,
            arguments: Arguments
        ) -> Task<T>
        where machine Target(arguments: Arguments) -> T suspends; blocks;
        ensures true;
    }

    data CanaryTaskRuntime { }

    machine CanaryTaskRuntime::start<T, Arguments, machine Target>(
        &self,
        arguments: Arguments
    ) -> Task<T>
    where machine Target(arguments: Arguments) -> T suspends; blocks;
    satisfies TaskRuntime::start
    via Binding::CompilerIntrinsic;

    data Token {
        id: u64;
    }

    data Worker { }
    machine Worker::run(token: Token) -> Token suspends; {
        token
    }

    machine Task::settle<T>(self) { }
"#;

#[test]
fn a_routed_task_start_call_plans_and_owned_settle_stops_at_lowered_custody() {
    let checked = checked(&format!(
        "{ROUTED_TASK_START_DECLS}
         data Main {{
             runtime: TaskRuntime;
         }}
         machine Main::probe(&mut self, token: Token) reaches TaskRuntime {{
             let task: Task<Token> = self.runtime.start<Worker::run>(token);
             Task::settle(task);
         }}
         machine Main::main(&mut self) {{ }}"
    ));
    let probe = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::probe")
        .expect("Main::probe is declared")
        .symbol;
    // The routed call survived checking with its exact static target: the
    // requirement symbol and the selected `Worker::run` entry are both
    // retained on the checked call node and the flow call fact.
    let start_requirement = checked
        .traits()
        .iter()
        .flat_map(|definition| checked.trait_machine_signatures(definition))
        .find(|signature| signature.name.as_str() == "start")
        .expect("TaskRuntime::start requirement is retained")
        .symbol;
    let worker_entry = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Worker::run")
        .and_then(|machine| checked.typed.machine_states(machine).first())
        .expect("Worker::run entry is retained")
        .symbol;
    let call = checked
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| {
            let checked_trees::expression::ExpressionNode::Call(call) = expression else {
                return None;
            };
            (call.target_symbol == start_requirement).then_some(call)
        })
        .expect("the start call is retained in the checked expression table");
    let [target] = call.machine_arguments.as_ref() else {
        panic!("start<Worker::run> retains exactly one static machine argument");
    };
    assert_eq!(target.symbol, worker_entry);
    let probe_state = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, state)| (state.machine_symbol == probe).then_some(state))
        .expect("Main::probe has a checked flow state");
    assert!(
        checked
            .facts
            .flow
            .control
            .calls
            .span_or_empty(probe_state.calls)
            .iter()
            .any(|call| call.target_symbol == start_requirement),
        "the start call is retained as a checked flow call fact"
    );
    // The retained specialization row for `start<Worker::run>` resolves the
    // requirement's whole telescope — `T` and `Arguments` derive `Token`, and
    // the `machine Target` binder's selection is `Worker::run`'s entry — so
    // `build_static_boundary_requirements` plans `TaskRuntime::start` with the
    // substituted envelope instead of declining an untelescoped binder.
    let specialized = checked
        .typed
        .machines()
        .iter()
        .find(|machine| {
            machine
                .name
                .as_str()
                .starts_with("Task::settle$specialized$")
        })
        .expect("the Task<Token> receiver specialization is emitted");
    let specialized_symbol = specialized.symbol;
    let task_token_identity = checked
        .typed
        .normalized_type_identity(specialized.attached_data_application)
        .into_string();
    let token_identity = {
        let probe_machine = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.symbol == probe)
            .expect("Main::probe is declared");
        let probe_typed_state = checked
            .typed
            .machine_states(probe_machine)
            .first()
            .expect("probe has a state");
        let token_parameter = checked
            .typed
            .state_parameters(probe_typed_state)
            .iter()
            .find(|parameter| parameter.name.as_str() == "token")
            .expect("probe declares token");
        checked
            .typed
            .normalized_type_identity(token_parameter.type_reference)
            .into_string()
    };
    let boundary = checked
        .facts
        .flow
        .terminal_unit_effects
        .boundary_machines
        .iter()
        .find(|plan| plan.machine == start_requirement)
        .expect("TaskRuntime::start has a boundary plan");
    assert!(
        boundary
            .structural_parameters
            .iter()
            .any(|parameter| parameter.type_identity == token_identity),
        "the substituted `arguments: Token` parameter is planned"
    );
    match &boundary.result {
        checked_trees::CheckedBoundaryMachineResultPlan::Structural {
            type_identity,
            multiplicity,
            ..
        } => {
            assert_eq!(type_identity, &task_token_identity);
            assert_eq!(*multiplicity, language_semantics::Multiplicity::Linear);
        }
        result => panic!("unexpected start result plan: {result:?}"),
    }
    // An owned `self` on `[linear]` attached data IS the custody the unit
    // consumes: `entry_claims` now mints the receiver's `StateEntry` claim
    // itself rather than declining the body. Both `settle` bodies — the
    // generic and the `Task<Token>` receiver specialization — carry one
    // whole-value claim on parameter 0, and `probe` retains a full plan
    // because its specialized `settle` callee is no longer unavailable.
    let settle_machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Task::settle")
        .expect("Task::settle is declared")
        .symbol;
    let settle_state = |machine: symbols::SymbolHandle| {
        let machine = checked
            .typed
            .machines()
            .iter()
            .find(|candidate| candidate.symbol == machine)
            .expect("settle machine is declared");
        checked
            .typed
            .machine_states(machine)
            .first()
            .expect("settle has a state")
            .symbol
    };
    for machine in [settle_machine, specialized_symbol] {
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine)
            .unwrap_or_else(|| panic!("{machine:?} has a Unit plan"));
        let [claim] = plan.entry_claims.as_slice() else {
            panic!("an owned linear receiver holds exactly one entry claim: {plan:?}");
        };
        assert_eq!(claim.parameter_index, 0);
        assert!(claim.path.is_empty());
        assert_eq!(claim.carry, language_semantics::CarryPolicy::STRICT);
        assert_eq!(
            claim.claim_identity,
            language_semantics::PermissionClaimIdentity::Established {
                machine_symbol: machine,
                state_symbol: settle_state(machine),
                source: language_semantics::PermissionEventSource::StateEntry,
                ordinal: 0,
            }
        );
    }
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .omission_for_machine(probe)
            .is_none(),
        "Main::probe plans once its specialized settle callee is admitted"
    );
    // The named frontier moves one edge deeper: `settle` now emits with its
    // completed-result custody joined — transfer-shape validation admits the
    // move only because `task`'s result carries the exact whole-value claim
    // `settle`'s `self` entry claim names, minted statement-established, and
    // the emitted `CallUnit` records that consume as a claim transfer naming
    // the minted caller binding. Emission then stops inside module
    // validation: the ordinary-call source policy still classifies every
    // operation-result argument place as affine and claim-free, so the
    // claim-carrying boundary result place has no argument source to join.
    let error = checked_trees_to_lowered_psi::lower_machine(&checked, "Main::probe")
        .expect_err("the settled self argument has no ordinary-call source yet");
    assert!(
        matches!(
            error,
            checked_trees_to_lowered_psi::LoweringError::InvalidTerminalModule(
                terminal_verifier::ModuleError::UnknownStructuralArgument {
                    operation,
                    argument_index: 0,
                    place,
                }
            ) if operation.get() == 2 && place.get() == 5
        ),
        "unexpected error: {error:?}"
    );
}

// The claim-carrying admission is evidence, not a bypass: each corruption
// below must still be rejected by the consumer's claimed-custody join rather
// than by an unrelated later check. Every mutation is applied to the same
// routed `start`/`settle` fixture the pinning test above establishes.
#[test]
fn a_routed_task_result_into_self_rejects_claim_custody_corruption() {
    let baseline = checked(&format!(
        "{ROUTED_TASK_START_DECLS}
         data Main {{
             runtime: TaskRuntime;
         }}
         machine Main::probe(&mut self, token: Token) reaches TaskRuntime {{
             let task: Task<Token> = self.runtime.start<Worker::run>(token);
             Task::settle(task);
         }}
         machine Main::main(&mut self) {{ }}"
    ));
    let probe = baseline
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::probe")
        .expect("Main::probe is declared")
        .symbol;
    let specialized = baseline
        .typed
        .machines()
        .iter()
        .find(|machine| {
            machine
                .name
                .as_str()
                .starts_with("Task::settle$specialized$")
        })
        .expect("the Task<Token> receiver specialization is emitted")
        .symbol;
    let consume = baseline
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .find_map(|(handle, event)| {
            (event.machine_symbol == probe
                && event.kind == language_semantics::PermissionEventKind::Consume
                && event.multiplicity == language_semantics::Multiplicity::Linear
                && event.access == language_semantics::PermissionAccess::Owned)
                .then_some(handle)
        })
        .expect("probe consumes the task claim at the settle call");
    let establish = baseline
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .find_map(|(handle, event)| {
            (event.machine_symbol == probe
                && event.kind == language_semantics::PermissionEventKind::Establish
                && event.multiplicity == language_semantics::Multiplicity::Linear
                && event.access == language_semantics::PermissionAccess::Owned
                && event.claim_identity
                    == baseline
                        .facts
                        .flow
                        .ownership
                        .permissions
                        .get(consume)
                        .claim_identity)
                .then_some(handle)
        })
        .expect("probe establishes the task claim at the start statement");
    fn settle_plan_mut(
        checked: &mut checked_trees::CheckedTrees,
        machine: symbols::SymbolHandle,
    ) -> &mut checked_trees::CheckedUnitEffectMachinePlan {
        checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| plan.machine == machine)
            .expect("the specialized settle has a Unit plan")
    }
    let rejects = |checked: &checked_trees::CheckedTrees, expected: &str, corruption: &str| {
        let error = checked_trees_to_lowered_psi::lower_machine(checked, "Main::probe")
            .expect_err(corruption);
        assert!(
            matches!(
                error,
                checked_trees_to_lowered_psi::LoweringError::Unsupported(message)
                    if message == expected
            ),
            "{corruption}: unexpected error: {error:?}"
        );
    };

    // A `self` formal without its entry claim is not claim-carrying custody.
    let mut missing_claim = baseline.clone();
    settle_plan_mut(&mut missing_claim, specialized)
        .entry_claims
        .clear();
    rejects(
        &missing_claim,
        "Unit structural result claim custody has no exact callee entry claim",
        "missing entry claim",
    );

    // The entry claim must sit on the consumed parameter itself.
    let mut wrong_parameter = baseline.clone();
    settle_plan_mut(&mut wrong_parameter, specialized).entry_claims[0].parameter_index = 1;
    rejects(
        &wrong_parameter,
        "Unit structural result claim custody has no exact callee entry claim",
        "claim on another parameter",
    );

    // The entry claim must carry the whole value, not a projection below it.
    let mut projected_claim = baseline.clone();
    settle_plan_mut(&mut projected_claim, specialized).entry_claims[0]
        .path
        .push(checked_trees::CheckedUnitStructuralPathSegment::Field(
            "provider".to_string(),
        ));
    rejects(
        &projected_claim,
        "Unit structural result claim custody has no exact callee entry claim",
        "projected claim path",
    );

    // The entry claim must belong to the exact callee state it names.
    let mut foreign_claim = baseline.clone();
    let language_semantics::PermissionClaimIdentity::Established { state_symbol, .. } =
        &mut settle_plan_mut(&mut foreign_claim, specialized).entry_claims[0].claim_identity
    else {
        unreachable!()
    };
    *state_symbol = probe;
    rejects(
        &foreign_claim,
        "Unit structural result claim custody has no exact callee entry claim",
        "claim belonging to another state",
    );

    // An affine consume does not discharge linear result custody.
    let mut affine_consume = baseline.clone();
    affine_consume
        .facts
        .flow
        .ownership
        .permissions
        .get_mut(consume)
        .multiplicity = language_semantics::Multiplicity::Affine;
    rejects(
        &affine_consume,
        "Unit structural result claim custody lost its exact caller consume event",
        "non-linear consume",
    );

    // The consumed claim's ordinal is the join: the producer's established
    // identity must match what the call consumed exactly.
    let mut drifted_ordinal = baseline.clone();
    let language_semantics::PermissionClaimIdentity::Established { ordinal, .. } =
        &mut drifted_ordinal
            .facts
            .flow
            .ownership
            .permissions
            .get_mut(consume)
            .claim_identity
    else {
        unreachable!()
    };
    *ordinal += 1;
    rejects(
        &drifted_ordinal,
        "Unit structural result claim custody was not established and consumed exactly once",
        "mismatched claim ordinal",
    );

    // One call may not consume the same claim custody twice.
    let mut second_consume = baseline.clone();
    let duplicate = second_consume
        .facts
        .flow
        .ownership
        .permissions
        .get_mut(consume)
        .clone();
    second_consume
        .facts
        .flow
        .ownership
        .permissions
        .insert(duplicate);
    rejects(
        &second_consume,
        "Unit structural result claim custody lost its exact caller consume event",
        "consumed twice",
    );

    // A stray transfer row on the same argument is not an entry-claim proof.
    let mut stray_transfer = baseline.clone();
    let transfer = checked_trees::CheckedUnitClaimTransferPlan {
        claim_identity: baseline
            .facts
            .flow
            .ownership
            .permissions
            .get(consume)
            .claim_identity,
        argument_index: 0,
    };
    let probe_plan = stray_transfer
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|plan| plan.machine == probe)
        .expect("Main::probe has a Unit plan");
    let checked_trees::CheckedUnitEffectOperationPlan::CallUnit {
        claim_transfers, ..
    } = probe_plan
        .operations
        .iter_mut()
        .find(|operation| {
            matches!(
                operation,
                checked_trees::CheckedUnitEffectOperationPlan::CallUnit { .. }
            )
        })
        .expect("probe retains its settle call")
    else {
        unreachable!()
    };
    claim_transfers.push(transfer);
    rejects(
        &stray_transfer,
        "Unit structural result argument has invalid claim-carrying custody",
        "stray claim transfer",
    );

    // Two claims minted at one coordinate mint sibling custody the consume
    // never retires.
    let mut over_minted = baseline.clone();
    let mut sibling = over_minted
        .facts
        .flow
        .ownership
        .permissions
        .get(establish)
        .clone();
    let language_semantics::PermissionClaimIdentity::Established { ordinal, .. } =
        &mut sibling.claim_identity
    else {
        unreachable!()
    };
    *ordinal += 1;
    over_minted.facts.flow.ownership.permissions.insert(sibling);
    rejects(
        &over_minted,
        "Unit structural result claim custody was not established and consumed exactly once",
        "second claim minted at the same coordinate",
    );

    // An establish event cannot re-mint an identity established elsewhere:
    // the claim the consume retires must be minted at this statement.
    let mut reminted = baseline.clone();
    let language_semantics::PermissionClaimIdentity::Established { source, .. } = &mut reminted
        .facts
        .flow
        .ownership
        .permissions
        .get_mut(establish)
        .claim_identity
    else {
        unreachable!()
    };
    *source = language_semantics::PermissionEventSource::StateEntry;
    rejects(
        &reminted,
        "Unit structural result claim custody was not established and consumed exactly once",
        "claim re-minted from an existing binding",
    );

    // A claim the consume never actually retires dies unclaimed.
    let mut unclaimed = baseline.clone();
    unclaimed
        .facts
        .flow
        .ownership
        .permissions
        .get_mut(consume)
        .obligation_live = false;
    rejects(
        &unclaimed,
        "Unit structural result claim custody lost its exact caller consume event",
        "claim dies unclaimed",
    );
}

#[test]
fn a_provider_carrying_argument_still_stops_at_provider_attachment_requirements() {
    // `token: Token` is an ordinary routed argument, but a structural
    // parameter whose own carrier holds a provider-backed field is a second
    // provider surface: the requirement roster names `self.<field>`
    // receivers only, so `carrier` cannot cross as an unspecialized
    // argument and the receiver still stops at provider attachment
    // requirements.
    let checked = checked(&format!(
        "{ROUTED_TASK_START_DECLS}
         data Carrier {{
             runtime: TaskRuntime;
         }}
         data Main {{
             runtime: TaskRuntime;
         }}
         machine Main::probe(&mut self, carrier: Carrier) {{ }}
         machine Main::main(&mut self) {{ }}"
    ));
    let probe = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::probe")
        .expect("Main::probe is declared")
        .symbol;
    let omission = checked
        .facts
        .flow
        .terminal_unit_effects
        .omission_for_machine(probe)
        .expect("Main::probe has an omission row");
    assert!(matches!(
        omission.stage,
        checked_trees::CheckedUnitPlanOmissionStage::LocalConstruction {
            phase: "provider attachment requirements",
            ..
        }
    ));
}

#[test]
fn a_shared_task_runtime_place_stops_at_signature_construction() {
    // The authored canary shape carries `&TaskRuntime`: a shared borrow of a
    // selected provider capability. Checked structural signatures admit an
    // owned boundary-trait field but not a borrowed one, so construction
    // stops one stage earlier, before any call planning is reached.
    let checked = checked(&format!(
        "{ROUTED_TASK_START_DECLS}
         data Main {{
             runtime: &TaskRuntime;
         }}
         machine Main::probe(&mut self, token: Token) reaches TaskRuntime {{
             let task: Task<Token> = self.runtime.start<Worker::run>(token);
             Task::settle(task);
         }}
         machine Main::main(&mut self) {{ }}"
    ));
    let probe = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::probe")
        .expect("Main::probe is declared")
        .symbol;
    let omission = checked
        .facts
        .flow
        .terminal_unit_effects
        .omission_for_machine(probe)
        .expect("Main::probe has an omission row");
    assert!(matches!(
        omission.stage,
        checked_trees::CheckedUnitPlanOmissionStage::LocalConstruction {
            phase: "signature",
            ..
        }
    ));
    let error = checked_trees_to_lowered_psi::lower_machine(&checked, "Main::probe")
        .expect_err("a shared TaskRuntime place has no admitted checked signature");
    let checked_trees_to_lowered_psi::LoweringError::InvalidUnitMachinePlan {
        machine,
        omission,
        ..
    } = error
    else {
        panic!("unexpected error: {error:?}");
    };
    assert_eq!(machine, "Main::probe");
    assert_eq!(
        omission.as_deref(),
        Some("`Main::probe` has no admitted body (local construction stopped at signature)")
    );
}
