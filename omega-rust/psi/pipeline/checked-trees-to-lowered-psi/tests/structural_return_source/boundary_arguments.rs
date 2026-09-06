use super::*;
use checked_trees::{
    CheckedScalarComputationKind, CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan,
};
use typed_trees::{expression::ExpressionNode, statement::StatementNode};

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).unwrap_or_else(|errors| panic!("{source}: {errors:#?}"))
}

fn artifact(checked: &checked_trees::CheckedTrees) -> (Vec<u8>, Vec<u8>) {
    let root = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::enter")
        .unwrap();
    let [state] = checked.typed.machine_states(root) else {
        panic!("one authored returned-result state");
    };
    let statements = checked
        .typed
        .statement_table
        .statements(state.statement_nodes);
    assert_eq!(statements.len(), 2, "no synthetic source statements");
    let StatementNode::LocalData(local) = &statements[0] else {
        panic!("authored boundary result binding");
    };
    assert_eq!(local.name.as_str(), "accepted");
    assert!(!local.is_mutable);
    let lowered = checked_trees_to_lowered_psi::lower_machine(checked, "Root::enter")
        .expect("returned boundary scalar with computed operand lowers");
    let semantic = encode_module(&lowered.semantic_module).unwrap();
    let evidence = encode_proof_bundle(&lowered.proof_bundle).unwrap();
    let module = decode_module(&semantic).unwrap();
    let proof = terminal_codec::decode_proof_bundle(&evidence).unwrap();
    assert_eq!(module, lowered.semantic_module);
    assert_eq!(proof, lowered.proof_bundle);
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();
    (semantic, evidence)
}

fn start(artifact: &(Vec<u8>, Vec<u8>)) -> TerminalExecution {
    let module = decode_module(&artifact.0).unwrap();
    let root = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let [parameter] = root.structural_parameters.as_slice() else {
        panic!("one owned receipt");
    };
    TerminalExecution::start_artifact_with_structural_arguments(
        &artifact.0,
        &artifact.1,
        &AdmissionProfile::default(),
        &[],
        &[TerminalStructuralValue {
            opaque_identity: 700,
            structural_type: parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        }],
    )
    .unwrap()
}

#[test]
fn returned_boundary_scalar_accepts_computed_argument_before_linear_settlement() {
    let source = format!(
        "machine identity(value: bool) -> bool {{ value }}\n{}",
        RESULT_BOUNDARY_CUSTODY_SOURCE
    )
    .replace(
        "Receipt::settle(self)",
        "Receipt::settle(self, value: bool)",
    )
    .replace("receipt.settle()", "receipt.settle(identity(true))");
    let checked = checked(&source);
    let artifact = artifact(&checked);
    let mut execution = start(&artifact);
    assert_eq!(execution.live_claim_frontier().count(), 1);
    assert_eq!(
        execution
            .resume_with_effect_handler(
                &mut TerminalFuelMeter::unbounded(),
                &mut ResultBoundaryHandler { reject: false }
            )
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Boolean(true)
        ))
    );
    assert_eq!(execution.effects().len(), 1);
    assert_eq!(execution.live_claim_frontier().count(), 0);
}

const BOOLEAN_HELPERS: &str = r#"
    machine identity(value: bool) -> bool
    requires true == true
    ensures true == true
    { value }
    data Scalar {}
    machine Scalar::identity(value: bool) -> bool
    requires true == true
    ensures true == true
    { value }
"#;

const INTEGER_HELPERS: &str = r#"
    machine identity(value: u8) -> u8
    requires 0u8 == 0u8
    ensures 0u8 == 0u8
    { value }
    data Scalar {}
    machine Scalar::identity(value: u8) -> u8
    requires 0u8 == 0u8
    ensures 0u8 == 0u8
    { value }
"#;

fn source(carrier: &str, arguments: &str, helpers: &str, crashes: bool) -> String {
    let crash_contract = if crashes {
        "crashes Abort\ncrashes Trap"
    } else {
        ""
    };
    format!(
        r#"
        {helpers}
        boundary trait PortIo {{}}
        pub data Receipt [linear] {{ value: u64; }}
        boundary machine Receipt::settle(self, first: {carrier}, second: {carrier}) -> {carrier}
        reaches PortIo
        ensures true;
        data Root {{}}
        machine Root::enter(receipt: Receipt) -> {carrier}
        reaches PortIo
        {crash_contract}
        {{
            let accepted: {carrier} = receipt.settle({arguments});
            accepted
        }}
    "#
    )
}

fn unsigned(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: semantic_vocabulary::IntegerType::new(
            semantic_vocabulary::IntegerSign::Unsigned,
            16,
        )
        .unwrap(),
        value: semantic_vocabulary::IntegerValue::Unsigned(value),
    }
}

#[derive(Default)]
struct ObserveSettlement {
    calls: Vec<Vec<TerminalScalarValue>>,
    receipts: Vec<TerminalStructuralValue>,
    reject: bool,
}

impl TerminalEffectHandler for ObserveSettlement {
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        panic!("result-bearing handler must be invoked");
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<terminal_interpreter::TerminalEffectResult, TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall {
            arguments,
            structural_arguments,
            ..
        } = effect
        else {
            panic!("boundary settlement");
        };
        let [receipt] = structural_arguments.as_slice() else {
            panic!("one whole-root receipt");
        };
        assert_eq!(receipt.opaque_identity, 700);
        assert!(receipt.path.is_empty());
        self.calls.push(arguments.clone());
        self.receipts.push(receipt.clone());
        if self.reject {
            return Err(TerminalEffectRejection::new("settlement refused"));
        }
        Ok(terminal_interpreter::TerminalEffectResult::Scalar(
            *arguments.last().unwrap(),
        ))
    }
}

#[test]
fn returned_boolean_and_integer_boundary_values_preserve_computed_and_pure_operands() {
    for (source, expected) in [
        (
            source(
                "bool",
                "identity(false), Scalar::identity(identity(true))",
                BOOLEAN_HELPERS,
                false,
            ),
            vec![
                TerminalScalarValue::Boolean(false),
                TerminalScalarValue::Boolean(true),
            ],
        ),
        (
            source(
                "u16",
                "(Scalar::identity(identity(255u8)) as u16) + 1u16, identity(7u8) as u16",
                INTEGER_HELPERS,
                false,
            ),
            vec![unsigned(256), unsigned(7)],
        ),
        (
            source("bool", "false, true", "", false),
            vec![
                TerminalScalarValue::Boolean(false),
                TerminalScalarValue::Boolean(true),
            ],
        ),
    ] {
        let artifact = artifact(&checked(&source));
        let mut execution = start(&artifact);
        let mut observer = ObserveSettlement::default();
        assert_eq!(
            execution
                .resume_with_effect_handler(&mut TerminalFuelMeter::unbounded(), &mut observer)
                .unwrap(),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
                *expected.last().unwrap()
            ))
        );
        assert_eq!(observer.calls, vec![expected]);
        assert_eq!(execution.effects().len(), 1);
        assert_eq!(execution.live_claim_frontier().count(), 0);
    }
}

#[test]
fn returned_boundary_provider_rejection_preserves_receipt_until_successful_retry() {
    for (source, expected) in [
        (
            source(
                "bool",
                "identity(false), Scalar::identity(identity(true))",
                BOOLEAN_HELPERS,
                false,
            ),
            vec![
                TerminalScalarValue::Boolean(false),
                TerminalScalarValue::Boolean(true),
            ],
        ),
        (
            source(
                "u16",
                "identity(17u8) as u16, Scalar::identity(identity(23u8)) as u16",
                INTEGER_HELPERS,
                false,
            ),
            vec![unsigned(17), unsigned(23)],
        ),
    ] {
        let artifact = artifact(&checked(&source));
        let mut execution = start(&artifact);
        let claims = execution.live_claim_frontier().collect::<Vec<_>>();
        assert_eq!(claims.len(), 1);
        let mut observer = ObserveSettlement {
            reject: true,
            ..ObserveSettlement::default()
        };
        assert!(
            matches!(execution.resume_with_effect_handler(&mut TerminalFuelMeter::unbounded(), &mut observer), Err(TerminalInterpretError::EffectRejected { rejection, .. }) if rejection.reason == "settlement refused")
        );
        assert_eq!(execution.live_claim_frontier().collect::<Vec<_>>(), claims);
        assert!(execution.effects().is_empty());
        assert_eq!(observer.calls, vec![expected.clone()]);
        observer.reject = false;
        assert_eq!(
            execution
                .resume_with_effect_handler(&mut TerminalFuelMeter::unbounded(), &mut observer)
                .unwrap(),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
                *expected.last().unwrap()
            ))
        );
        assert_eq!(observer.calls, vec![expected.clone(), expected]);
        assert_eq!(observer.receipts[0], observer.receipts[1]);
        assert_eq!(execution.effects().len(), 1);
        assert_eq!(execution.live_claim_frontier().count(), 0);
    }
}

#[test]
fn returned_boundary_boolean_arguments_short_circuit_without_settling_on_crash() {
    let helpers = format!(
        "{BOOLEAN_HELPERS}\nmachine abort() -> bool crashes Abort {{ crash Abort; }}\nmachine trap() -> bool crashes Trap {{ crash Trap; }}"
    );
    for (first, second, cause) in [
        (false, true, None),
        (true, false, Some(terminal_psi::CrashCause::Abort)),
        (false, false, Some(terminal_psi::CrashCause::Trap)),
    ] {
        let source = source(
            "bool",
            &format!("identity({first}) && abort(), Scalar::identity({second}) || trap()"),
            &helpers,
            true,
        );
        let artifact = artifact(&checked(&source));
        let mut execution = start(&artifact);
        let claims = execution.live_claim_frontier().collect::<Vec<_>>();
        let mut observer = ObserveSettlement::default();
        if let Some(cause) = cause {
            pause_before_crashing_helper(&artifact, &mut execution, &mut observer, &claims, cause);
        }
        let status = execution
            .resume_with_effect_handler(&mut TerminalFuelMeter::unbounded(), &mut observer)
            .unwrap();
        if let Some(cause) = cause {
            assert_unsettled_helper_crash(
                &artifact,
                &mut execution,
                &mut observer,
                &claims,
                cause,
                status,
            );
        } else {
            assert_eq!(
                status,
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
                    TerminalScalarValue::Boolean(true)
                ))
            );
            assert_eq!(
                observer.calls,
                vec![vec![
                    TerminalScalarValue::Boolean(false),
                    TerminalScalarValue::Boolean(true)
                ]]
            );
            assert_eq!(execution.live_claim_frontier().count(), 0);
        }
    }
}

#[test]
fn returned_boundary_first_argument_crash_precedes_later_cast_and_retains_linear_claim() {
    for (first, second, cause) in [
        ("Abort", "Trap", terminal_psi::CrashCause::Abort),
        ("Trap", "Abort", terminal_psi::CrashCause::Trap),
    ] {
        let helpers = format!(
            "machine first() -> u8 crashes {first} {{ crash {first}; }}\nmachine second() -> u8 crashes {second} {{ crash {second}; }}"
        );
        let artifact = artifact(&checked(&source(
            "u16",
            "first() as u16, second() as u16",
            &helpers,
            true,
        )));
        let mut execution = start(&artifact);
        let claims = execution.live_claim_frontier().collect::<Vec<_>>();
        assert_eq!(claims.len(), 1);
        let mut observer = ObserveSettlement::default();
        pause_before_crashing_helper(&artifact, &mut execution, &mut observer, &claims, cause);
        let status = execution
            .resume_with_effect_handler(&mut TerminalFuelMeter::unbounded(), &mut observer)
            .unwrap();
        assert_unsettled_helper_crash(
            &artifact,
            &mut execution,
            &mut observer,
            &claims,
            cause,
            status,
        );
    }
}

fn pause_before_crashing_helper(
    artifact: &(Vec<u8>, Vec<u8>),
    execution: &mut TerminalExecution,
    observer: &mut ObserveSettlement,
    claims: &[semantic_vocabulary::ClaimId],
    cause: terminal_psi::CrashCause,
) {
    let module = decode_module(&artifact.0).unwrap();
    let root = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let call = root.blocks.iter().flat_map(|block| &block.operations).find(|operation| {
        let terminal_psi::OperationKind::Call { callee, .. } = operation.kind else { return false; };
        module.machines.iter().find(|machine| machine.id == callee).unwrap().blocks.iter()
            .any(|block| matches!(block.terminator, Terminator::Crash { cause: found, .. } if found == cause))
    }).unwrap();
    let mut meter = TerminalFuelMeter::with_allowance(0);
    for _ in 0..256 {
        let status = execution
            .resume_with_effect_handler(&mut meter, observer)
            .unwrap();
        let TerminalExecutionStatus::SponsorExhausted(exhaustion) = status else {
            panic!("expected to pause before helper call, got {status:?}");
        };
        if exhaustion.site == terminal_fuel::FuelChargeSite::Operation(call.id) {
            assert_eq!(
                execution.live_claim_frontier().collect::<Vec<_>>(),
                claims,
                "the caller still owns its receipt immediately before entering the crashing operand"
            );
            assert!(observer.calls.is_empty());
            return;
        }
        meter.replenish(1).unwrap();
    }
    panic!("did not reach the expected helper invocation within the bounded fixture");
}

fn assert_unsettled_helper_crash(
    artifact: &(Vec<u8>, Vec<u8>),
    execution: &mut TerminalExecution,
    observer: &mut ObserveSettlement,
    claims: &[semantic_vocabulary::ClaimId],
    cause: terminal_psi::CrashCause,
    status: TerminalExecutionStatus,
) {
    let TerminalExecutionStatus::Crashed(crash) = &status else {
        panic!("expected helper crash, got {status:?}");
    };
    assert_eq!(crash.cause, cause);
    let module = decode_module(&artifact.0).unwrap();
    let helper = module
        .machines
        .iter()
        .find(|machine| {
            machine.blocks.iter().any(|block| {
        matches!(block.terminator, Terminator::Crash { edge, .. } if edge == crash.edge)
    })
        })
        .unwrap();
    assert_ne!(helper.id, module.entry);
    assert!(helper.entry_claims.is_empty());
    // Both APIs describe the current helper frame, not the suspended caller.
    assert!(crash.frontier_lower_bound.is_empty());
    assert_eq!(
        execution.live_claim_frontier().collect::<Vec<_>>(),
        crash.frontier_lower_bound
    );
    let root = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let settlements = root
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| {
            if let terminal_psi::OperationKind::BoundaryCall {
                completion_receipts,
                ..
            } = &operation.kind
            {
                Some(completion_receipts)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(settlements.len(), 1);
    assert_eq!(
        settlements[0]
            .iter()
            .map(|receipt| receipt.claim)
            .collect::<Vec<_>>(),
        claims,
        "only the uninvoked outer boundary can settle the caller's receipt"
    );
    assert!(observer.calls.is_empty());
    assert!(execution.effects().is_empty());
    assert_eq!(
        execution
            .resume_with_effect_handler(&mut TerminalFuelMeter::unbounded(), observer)
            .unwrap(),
        status,
        "a crash has no cleanup or later boundary successor"
    );
    assert!(observer.calls.is_empty());
    assert!(execution.effects().is_empty());
}

#[test]
fn returned_boundary_computations_reject_outer_and_nested_source_custody_drift() {
    let checked = checked(&source(
        "bool",
        "identity(false), Scalar::identity(identity(true))",
        BOOLEAN_HELPERS,
        false,
    ));
    artifact(&checked);
    let root = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::enter")
        .unwrap();
    let state = &checked.typed.machine_states(root)[0];
    let StatementNode::LocalData(local) = &checked
        .typed
        .statement_table
        .statements(state.statement_nodes)[0]
    else {
        unreachable!();
    };
    let ExpressionNode::Call(call) = checked
        .typed
        .expression_table
        .expression(local.initial_value)
    else {
        unreachable!();
    };
    let arguments = checked
        .typed
        .expression_table
        .expression_handles(call.arguments);
    let receipt = &checked.typed.state_parameters(state)[0];
    let (outer_flow, _) = checked
        .facts
        .flow
        .control
        .calls
        .iter()
        .find(|(_, flow)| flow.authored_expression == local.initial_value && flow.call_ordinal == 0)
        .unwrap();
    for mutation in 0..11 {
        let mut changed = checked.clone();
        match mutation {
            0 => {
                let ExpressionNode::Call(call) = changed
                    .typed
                    .expression_table
                    .expression_mut(local.initial_value)
                else {
                    unreachable!();
                };
                call.target_symbol = symbols::SymbolHandle::invalid();
            }
            1 => changed
                .typed
                .expression_table
                .set_expression_handle_at_offset(call.arguments, 0, arguments[1]),
            2 => {
                let plan = changed
                    .facts
                    .flow
                    .terminal_boundary_scalar_returns
                    .machines
                    .iter_mut()
                    .find(|plan| plan.machine == root.symbol)
                    .unwrap();
                let CheckedUnitEffectOperationPlan::BoundaryCall { coordinate, .. } =
                    &mut plan.boundary_call
                else {
                    unreachable!();
                };
                coordinate.call_ordinal += 1;
            }
            3 => {
                changed
                    .facts
                    .flow
                    .terminal_boundary_scalar_returns
                    .machines
                    .iter_mut()
                    .find(|plan| plan.machine == root.symbol)
                    .unwrap()
                    .return_statement_ordinal += 1
            }
            4 => {
                let plan = changed
                    .facts
                    .flow
                    .terminal_boundary_scalar_returns
                    .machines
                    .iter_mut()
                    .find(|plan| plan.machine == root.symbol)
                    .unwrap();
                let CheckedUnitEffectOperationPlan::BoundaryCall {
                    scalar_arguments, ..
                } = &mut plan.boundary_call
                else {
                    unreachable!();
                };
                scalar_arguments.swap(0, 1);
            }
            5 | 10 => {
                let StatementNode::LocalData(changed_local) = &mut changed
                    .typed
                    .statement_table
                    .statements_mut(state.statement_nodes)[0]
                else {
                    unreachable!();
                };
                if mutation == 5 {
                    changed_local.type_reference = receipt.type_reference;
                } else {
                    changed_local.symbol = symbols::SymbolHandle::invalid();
                }
            }
            6 => {
                changed
                    .facts
                    .flow
                    .terminal_boundary_scalar_returns
                    .machines
                    .iter_mut()
                    .find(|plan| plan.machine == root.symbol)
                    .unwrap()
                    .result_type = typed_trees::types::PrimitiveType::U16;
            }
            7 => {
                let StatementNode::Expression(returned) = checked
                    .typed
                    .statement_table
                    .statements(state.statement_nodes)[1]
                else {
                    unreachable!();
                };
                let ExpressionNode::Name(path) =
                    changed.typed.expression_table.expression_mut(returned)
                else {
                    unreachable!();
                };
                path.symbol = receipt.symbol;
            }
            8 | 9 => {
                changed
                    .facts
                    .flow
                    .control
                    .calls
                    .get_mut(outer_flow)
                    .authored_expression = if mutation == 8 {
                    arena::Handle::invalid()
                } else {
                    arena::Handle::from_parts(
                        local.initial_value.arena_index(),
                        local.initial_value.generation() + 1,
                    )
                };
            }
            _ => unreachable!(),
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Root::enter").is_err(),
            "outer returned-boundary mutation={mutation}"
        );
    }
    let computations = &checked.facts.values.scalar_computations;
    let roots = computations
        .roots
        .iter()
        .filter(|(_, root_plan)| {
            root_plan.machine == root.symbol
                && matches!(
                    root_plan.role,
                    CheckedScalarExpressionRole::BoundaryCallArgument { .. }
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(roots.len(), 2);
    for (handle, plan) in roots {
        let mut changed = checked.clone();
        changed
            .facts
            .values
            .scalar_computations
            .roots
            .get_mut(handle)
            .statement_ordinal += 1;
        assert!(checked_trees_to_lowered_psi::lower_machine(&changed, "Root::enter").is_err());
        let mut changed = checked.clone();
        changed
            .facts
            .values
            .scalar_computations
            .nodes
            .get_mut(plan.root)
            .authored_root = local.initial_value;
        assert!(checked_trees_to_lowered_psi::lower_machine(&changed, "Root::enter").is_err());
    }
    for (_, node) in computations.nodes.iter() {
        let CheckedScalarComputationKind::Call { source_call, .. } = node.kind else {
            continue;
        };
        let authored = checked
            .facts
            .flow
            .control
            .calls
            .get(source_call)
            .authored_expression;
        let mut changed = checked.clone();
        changed
            .facts
            .flow
            .control
            .calls
            .get_mut(source_call)
            .authored_expression = arena::Handle::invalid();
        assert!(checked_trees_to_lowered_psi::lower_machine(&changed, "Root::enter").is_err());
        let ExpressionNode::Call(call) = checked.typed.expression_table.expression(authored) else {
            unreachable!();
        };
        if call.receiver.is_valid() {
            let mut changed = checked.clone();
            let ExpressionNode::Name(path) =
                changed.typed.expression_table.expression_mut(call.receiver)
            else {
                unreachable!();
            };
            path.symbol = symbols::SymbolHandle::invalid();
            assert!(checked_trees_to_lowered_psi::lower_machine(&changed, "Root::enter").is_err());
        }
    }
}
