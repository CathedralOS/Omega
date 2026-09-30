//! Nested boundary applications (`a * b + c`) execute as one Terminal
//! `BoundaryCall` per authored occurrence, in authored grouping and operand
//! order, each joined to its exact source call. A reordered or duplicated
//! inner application fails closed before any Terminal artifact exists.
use checked_trees::{CheckedScalarComputationKind, CheckedScalarExpressionRole};
use checked_trees_to_lowered_psi::{TerminalMachineSelection, lower_machine};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::IeeeFloatValue;
use terminal_codec::{encode_module, encode_proof_section};
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult,
    TerminalScalarValue, TerminalStructuralInputs, interpret_terminal_artifact_measured,
};

const SOURCE: &str = "data Root {}
     pub boundary machine + Float::add(left: f64, right: f64) -> f64;
     pub boundary machine - Float::subtract(left: f64, right: f64) -> f64;
     pub boundary machine * Float::multiply(left: f64, right: f64) -> f64;
     machine Root::enter(a: f64, b: f64, c: f64) {
         let product_sum: f64 = a * b + c;
         let grouped: f64 = a * (b + c);
         let difference: f64 = c - a * b;
         let chained: f64 = (a * b + c) * a;
         transition { _ -> done(product_sum, grouped, difference, chained) }
         state done(first: f64, second: f64, third: f64, fourth: f64) {}
     }";

fn float(value: f64) -> TerminalScalarValue {
    TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary64(value.to_bits()))
}

fn host(value: &TerminalScalarValue) -> f64 {
    let TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary64(bits)) = value else {
        panic!("binary64 operand");
    };
    f64::from_bits(*bits)
}

/// Performs each application with host arithmetic and records the order.
struct Applications {
    names: Vec<(semantic_vocabulary::BoundaryMachineId, &'static str)>,
    observed: Vec<(&'static str, Vec<f64>)>,
}

impl TerminalEffectHandler for Applications {
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new(
            "every application returns a scalar",
        ))
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall {
            boundary,
            arguments,
            ..
        } = effect
        else {
            return Err(TerminalEffectRejection::new("only boundary applications"));
        };
        let name = self
            .names
            .iter()
            .find(|(id, _)| id == boundary)
            .map(|(_, name)| *name)
            .ok_or(TerminalEffectRejection::new("unknown boundary"))?;
        let [left, right] = arguments.as_slice() else {
            return Err(TerminalEffectRejection::new("binary application"));
        };
        let (left, right) = (host(left), host(right));
        self.observed.push((name, vec![left, right]));
        let result = match name {
            "add" => left + right,
            "subtract" => left - right,
            "divide" => left / right,
            _ => left * right,
        };
        Ok(TerminalEffectResult::Scalar(float(result)))
    }
}

#[test]
fn result_policy_rejects_missing_duplicated_or_substituted_checked_selection() {
    let checked = crate::front_end::checked_program(
        "data Root {} pub boundary machine + Float::add(left: f64, right: f64) -> f64;
         machine Root::enter(a: f64 in Saturating, b: f64 in Saturating) { let result: f64 in Saturating = a + b; }",
    );
    lowered(&checked).expect("unmodified selection lowers");
    let (handle, selection) = checked
        .facts
        .operators
        .named_requirement_uses
        .iter()
        .next()
        .unwrap();
    for mutation in 0..5 {
        let mut substituted = checked.clone();
        match mutation {
            0 => substituted.facts.operators.named_requirement_uses = Default::default(),
            1 => {
                substituted
                    .facts
                    .operators
                    .named_requirement_uses
                    .insert(*selection);
            }
            2 => {
                substituted
                    .facts
                    .operators
                    .named_requirement_uses
                    .get_mut(handle)
                    .requirement_symbol = symbols::SymbolHandle::invalid()
            }
            3 => {
                substituted
                    .facts
                    .operators
                    .named_requirement_uses
                    .get_mut(handle)
                    .policy_adapter =
                    checked_trees::CheckedArithmeticPolicyAdapter::FloatSaturatingOverflowOnly {
                        format: numerics::float_semantics::FloatFormat::BINARY32,
                    }
            }
            _ => {
                substituted
                    .facts
                    .operators
                    .named_requirement_uses
                    .get_mut(handle)
                    .origin = checked_trees::CheckedValueOrigin::default()
            }
        }
        assert!(
            lowered(&substituted).is_err(),
            "selection mutation {mutation} rejects"
        );
    }
}

#[test]
fn adapted_results_reach_consumers_and_traps_stop_later_calls() {
    for policy in ["Saturating", "Trapping"] {
        let checked = crate::front_end::checked_program(&format!(
            "data Root {{}}
             pub boundary machine + Float::add(left: f64, right: f64) -> f64;
             pub boundary machine * Float::multiply(left: f64, right: f64) -> f64;
             machine Root::enter(a: f64 in {policy}, b: f64 in {policy}) {{
                 let result: f64 in {policy} = a * b + a;
                 let observed: f64 = Float::add(result as f64, 0.0);
             }}",
        ));
        let lowered = lowered(&checked).expect("composed policy calls lower");
        let module = encode_module(&lowered.semantic_module).unwrap();
        let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap();
        let mut applications = Applications {
            names: lowered
                .semantic_module
                .boundary_machines
                .iter()
                .map(|declaration| {
                    (
                        declaration.id,
                        if declaration.identity.contains("multiply") {
                            "multiply"
                        } else {
                            "add"
                        },
                    )
                })
                .collect(),
            observed: Vec::new(),
        };
        let outcome = interpret_terminal_artifact_measured(
            &module,
            &proof,
            &AdmissionProfile::default(),
            &[float(f64::MAX), float(2.0)],
            TerminalStructuralInputs::default(),
            &mut applications,
        );
        if policy == "Saturating" {
            outcome.expect("saturated results complete");
            assert_eq!(
                applications.observed,
                [
                    ("multiply", vec![f64::MAX, 2.0]),
                    ("add", vec![f64::MAX, f64::MAX]),
                    ("add", vec![f64::MAX, 0.0]),
                ],
                "each consumer sees the adapted value without repeating arithmetic"
            );
        } else {
            let error = outcome.expect_err("overflow traps before the outer call");
            assert!(
                matches!(
                    error,
                    terminal_interpreter::TerminalArtifactInterpretError::Execution(
                        terminal_interpreter::TerminalInterpretError::Crash(_)
                    )
                ),
                "expected an operation crash: {error:?}"
            );
            assert_eq!(applications.observed, [("multiply", vec![f64::MAX, 2.0])]);
        }
    }
}

#[test]
fn unused_trapping_result_survives_scalar_elimination() {
    let checked = crate::front_end::checked_program(
        "data Root {} pub boundary machine + Float::add(left: f64, right: f64) -> f64;
         machine Root::enter(a: f64 in Trapping, b: f64 in Trapping) {
             let discarded: f64 in Trapping = a + b;
             let after: f64 = Float::add(1.0, 2.0);
         }",
    );
    let selections = optimization::PsiOptimizationSelections::new([
        optimization::PsiOptimization::DeadPureScalarElimination,
    ])
    .unwrap();
    let optimized =
        lowered_psi_to_lowered_psi::run_psi_optimization(lowered(&checked).unwrap(), selections)
            .unwrap();
    let lowered = optimized.lowered();
    assert_policy_custody(&checked, lowered).expect("optimization keeps policy custody");
    let mut applications = Applications {
        names: lowered
            .semantic_module
            .boundary_machines
            .iter()
            .map(|boundary| (boundary.id, "add"))
            .collect(),
        observed: Vec::new(),
    };
    let outcome = interpret_terminal_artifact_measured(
        &encode_module(&lowered.semantic_module).unwrap(),
        &encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
        &[float(f64::MAX), float(f64::MAX)],
        TerminalStructuralInputs::default(),
        &mut applications,
    );
    assert!(
        matches!(
            outcome,
            Err(
                terminal_interpreter::TerminalArtifactInterpretError::Execution(
                    terminal_interpreter::TerminalInterpretError::Crash(_)
                )
            )
        ),
        "a discarded result still owns Trap: {outcome:?}"
    );
    assert_eq!(applications.observed, [("add", vec![f64::MAX, f64::MAX])]);
}

fn lowered(checked: &checked_trees::CheckedTrees) -> Result<lowered_psi::LoweredPsi, String> {
    lower_machine(checked, TerminalMachineSelection::Name("Root::enter"))
        .map_err(|error| format!("{error:?}"))
}

#[test]
fn float_result_policy_follows_calls_not_surrounding_expression_shape() {
    for policy in ["Saturating", "Trapping"] {
        for (body, expected) in [
            (
                "machine Root::enter(a: f64 in POLICY, b: f64 in POLICY) { let result: f64 in POLICY = a * b + a; }",
                2,
            ),
            (
                "machine Root::enter(a: f64 in POLICY, b: f64 in POLICY) { let result: f64 in POLICY = a + b; }",
                1,
            ),
            (
                "machine Root::enter(a: f64 in POLICY, b: f64 in POLICY) -> f64 { a + b }",
                1,
            ),
        ] {
            let source = format!(
                "data Root {{}} pub boundary machine + Float::add(left: f64, right: f64) -> f64; pub boundary machine * Float::multiply(left: f64, right: f64) -> f64; {}",
                body.replace("POLICY", policy),
            );
            let checked = crate::front_end::checked_program(&source);
            let lowered =
                lowered(&checked).unwrap_or_else(|error| panic!("{policy}: {body}: {error}"));
            let adapters = lowered
                .semantic_module
                .machines
                .iter()
                .flat_map(|machine| &machine.blocks)
                .flat_map(|block| &block.operations)
                .filter(|operation| {
                    matches!(
                        operation.kind,
                        terminal_psi::OperationKind::FloatResultPolicy { .. }
                    )
                })
                .count();
            assert_eq!(adapters, expected, "{policy}: {body}");
            terminal_verifier::verify_module(
                &lowered.semantic_module,
                &lowered.proof_bundle,
                &AdmissionProfile::default(),
            )
            .expect("emitted policies independently verify");
            assert_policy_custody(&checked, &lowered).expect("source policy independently rejoins");
        }
    }
}

fn assert_policy_custody(
    checked: &checked_trees::CheckedTrees,
    lowered: &lowered_psi::LoweredPsi,
) -> Result<(), String> {
    let optimized =
        lowered_psi_to_lowered_psi::run_psi_optimization(lowered.clone(), Default::default())
            .map_err(|error| format!("{error:?}"))?;
    let artifact = lowered_psi_to_terminal_psi::finalize_terminal_artifact(&optimized, None)
        .map_err(|error| format!("{error:?}"))?;
    lowered_psi_to_terminal_psi::checked_boundary_operator_scope(checked, &artifact, lowered)
        .map(|_| ())
        .map_err(str::to_owned)
}

#[test]
fn source_policy_replay_rejects_valid_but_wrong_adapters() {
    let checked = crate::front_end::checked_program(
        "data Root {} pub boundary machine / Float::divide(left: f64, right: f64) -> f64;
         machine Root::enter(a: f64 in Saturating, b: f64 in Saturating) -> f64 { a / b }",
    );
    let lowered = lowered(&checked).unwrap();
    assert_policy_custody(&checked, &lowered).unwrap();
    for mutation in 0..8 {
        let mut substituted = lowered.clone();
        substituted.debug_map = None;
        let block = substituted
            .semantic_module
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .find(|block| {
                block.operations.iter().any(|operation| {
                    matches!(
                        operation.kind,
                        terminal_psi::OperationKind::FloatResultPolicy { .. }
                    )
                })
            })
            .unwrap();
        let position = block
            .operations
            .iter()
            .position(|operation| {
                matches!(
                    operation.kind,
                    terminal_psi::OperationKind::FloatResultPolicy { .. }
                )
            })
            .unwrap();
        let adapted = block.operations[position].result.scalar().unwrap().id;
        let terminal_psi::OperationKind::FloatResultPolicy {
            policy:
                terminal_psi::FloatResultPolicy::Saturating {
                    raw_result,
                    left,
                    right,
                    rule,
                },
        } = &mut block.operations[position].kind
        else {
            panic!("saturating result");
        };
        let raw = *raw_result;
        let fallback = *left;
        match mutation {
            0 => *rule = terminal_psi::FloatSaturationRule::FiniteOperandOverflow,
            1 => std::mem::swap(left, right),
            2 => *raw_result = *left,
            3 => {
                block
                    .terminator
                    .map_scalar_uses(&mut |value| if value == adapted { raw } else { value });
            }
            4 => {
                block.operations.remove(position);
                block
                    .terminator
                    .map_scalar_uses(&mut |value| if value == adapted { raw } else { value });
            }
            5 => {
                let mut duplicate = block.operations[position].clone();
                duplicate.id = semantic_vocabulary::OperationId::new(1_000_000_000).unwrap();
                duplicate.result.scalar_mut().unwrap().id =
                    semantic_vocabulary::ValueId::new(1_000_000_000).unwrap();
                block.operations.push(duplicate);
            }
            6 => substituted.source_call_occurrences.clear(),
            _ => {
                block.operations.drain(position - 1..=position);
                block
                    .terminator
                    .map_scalar_uses(&mut |value| if value == adapted { fallback } else { value });
            }
        }
        terminal_verifier::verify_module(
            &substituted.semantic_module,
            &substituted.proof_bundle,
            &AdmissionProfile::default(),
        )
        .expect("mutation is independently valid Terminal, but not the authored policy");
        let rejection =
            assert_policy_custody(&checked, &substituted).expect_err("source policy rejects");
        assert!(
            rejection.starts_with("float"),
            "mutation {mutation} must reach policy replay, not fail artifact formation: {rejection}"
        );
    }
}

/// The BoundaryCallArgument root of one statement's outer application.
fn argument_root(
    checked: &checked_trees::CheckedTrees,
    statement: u32,
    argument_ordinal: u32,
) -> checked_trees::CheckedScalarComputationHandle {
    checked
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .map(|(_, root)| root)
        .find(|root| {
            root.statement_ordinal == statement
                && root.role
                    == CheckedScalarExpressionRole::BoundaryCallArgument {
                        call_ordinal: 0,
                        argument_ordinal,
                    }
        })
        .unwrap_or_else(|| panic!("statement {statement} argument {argument_ordinal} root"))
        .root
}

#[test]
fn policy_only_casts_preserve_payload_custody() {
    let checked = crate::front_end::checked_program(
        "data Root { source: f64; selected: f64 in Trapping; }
         machine Root::enter(&mut self, other: f32) {
             self.selected = self.source as f64 in Trapping;
             let erased: f64 = self.selected as f64;
             self.source = erased;
         }",
    );
    let lowered = lowered(&checked).expect("policy selection preserves field payload");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("policy selection independently verifies");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::enter")
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    let other_type = checked
        .state_parameters(state)
        .last()
        .unwrap()
        .type_reference;
    let checked_trees::statement::StatementNode::Assignment(assignment) =
        &checked.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("field store");
    };
    let source = assignment.value;
    let mut changed_carrier = checked.clone();
    let checked_trees::expression::ExpressionNode::Cast(cast) = changed_carrier
        .typed
        .expression_table
        .expression_mut(source)
    else {
        panic!("policy cast");
    };
    cast.target_type = other_type;
    assert!(
        self::lowered(&changed_carrier).is_err(),
        "a changed carrier cannot forward its payload"
    );
    let mut changed_domain = checked;
    let mut semantic_domain = arena::HandleSpan::empty();
    changed_domain.typed.expression_table.push_name_path_member(
        &mut semantic_domain,
        checked_trees::name::Identifier::generated("UnprovedDomain"),
    );
    let checked_trees::expression::ExpressionNode::Cast(cast) =
        changed_domain.typed.expression_table.expression_mut(source)
    else {
        panic!("policy cast");
    };
    cast.semantic_domain = semantic_domain;
    assert!(
        self::lowered(&changed_domain).is_err(),
        "a semantic qualification cannot use payload-only custody"
    );
}

#[test]
fn nested_applications_run_once_in_authored_order_with_exact_occurrences() {
    let checked = crate::front_end::checked_program(SOURCE);
    for (statement, argument) in [(0, 0), (1, 1), (2, 1)] {
        let node = checked
            .facts
            .values
            .scalar_computations
            .nodes
            .get(argument_root(&checked, statement, argument));
        assert!(
            matches!(
                node.kind,
                CheckedScalarComputationKind::BoundaryCall {
                    call_ordinal: 1,
                    ..
                }
            ),
            "statement {statement}: the nested application is its own occurrence"
        );
    }
    let lowered = lowered(&checked).expect("nested applications lower");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("nested applications independently verify");
    // Every application is one BoundaryCall with one exact source-call row.
    let mut coordinates = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::BoundaryCall { .. }
            )
        })
        .map(|operation| {
            let rows = lowered
                .source_call_occurrences
                .iter()
                .filter(|row| row.terminal_operation == operation.id)
                .collect::<Vec<_>>();
            let [row] = rows.as_slice() else {
                panic!("one source-call row per application");
            };
            (row.statement_index, row.call_ordinal)
        })
        .collect::<Vec<_>>();
    coordinates.sort_unstable();
    assert_eq!(
        coordinates,
        [
            (0, 0),
            (0, 1),
            (1, 0),
            (1, 1),
            (2, 0),
            (2, 1),
            (3, 0),
            (3, 1),
            (3, 2)
        ],
        "each authored occurrence runs exactly once"
    );
    let names = lowered
        .semantic_module
        .boundary_machines
        .iter()
        .map(|declaration| {
            let name = ["subtract", "multiply", "add"]
                .into_iter()
                .find(|name| declaration.identity.contains(name))
                .expect("known operator declaration");
            (declaration.id, name)
        })
        .collect();
    let module = encode_module(&lowered.semantic_module).unwrap();
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap();
    let mut applications = Applications {
        names,
        observed: Vec::new(),
    };
    interpret_terminal_artifact_measured(
        &module,
        &proof,
        &AdmissionProfile::default(),
        &[float(3.0), float(4.0), float(2.0)],
        TerminalStructuralInputs::default(),
        &mut applications,
    )
    .expect("nested applications interpret");
    assert_eq!(
        applications.observed,
        [
            ("multiply", vec![3.0, 4.0]),
            ("add", vec![12.0, 2.0]),
            ("add", vec![4.0, 2.0]),
            ("multiply", vec![3.0, 6.0]),
            ("multiply", vec![3.0, 4.0]),
            ("subtract", vec![2.0, 12.0]),
            ("multiply", vec![3.0, 4.0]),
            ("add", vec![12.0, 2.0]),
            ("multiply", vec![14.0, 3.0]),
        ],
        "inner applications complete before their consumer, in authored order"
    );
}

#[test]
fn reordered_or_duplicated_inner_applications_fail_closed() {
    let checked = crate::front_end::checked_program(SOURCE);
    let grouped = argument_root(&checked, 1, 1);
    let difference = argument_root(&checked, 2, 1);
    let product = argument_root(&checked, 0, 0);

    // Reordered operands: `b + c` evaluated as `c + b`.
    let mut reordered = checked.clone();
    let plans = &mut reordered.facts.values.scalar_computations;
    let CheckedScalarComputationKind::BoundaryCall { arguments, .. } =
        plans.nodes.get(grouped).kind.clone()
    else {
        panic!("grouped sum is an application");
    };
    let mut swapped = plans.operands.span(arguments).unwrap().to_vec();
    swapped.reverse();
    let swapped = plans.operands.insert_many(swapped);
    let CheckedScalarComputationKind::BoundaryCall { arguments, .. } =
        &mut plans.nodes.get_mut(grouped).kind
    else {
        unreachable!();
    };
    *arguments = swapped;
    assert!(lowered(&reordered).is_err(), "reordered operands reject");

    // Duplicated occurrence: the third statement's product claims the
    // first statement's application instead of its own.
    let mut duplicated = checked.clone();
    let plans = &mut duplicated.facts.values.scalar_computations;
    let CheckedScalarComputationKind::BoundaryCall { source_call, .. } =
        plans.nodes.get(product).kind.clone()
    else {
        panic!("first product is an application");
    };
    let CheckedScalarComputationKind::BoundaryCall {
        source_call: claimed,
        ..
    } = &mut plans.nodes.get_mut(difference).kind
    else {
        panic!("third product is an application");
    };
    *claimed = source_call;
    assert!(
        lowered(&duplicated).is_err(),
        "duplicated occurrence rejects"
    );

    // Duplicated evaluation: `(a * b + c) * a` with the inner product
    // consumed as both operands of the sum would invoke it twice.
    let chained = argument_root(&checked, 3, 0);
    let mut twice = checked.clone();
    let plans = &mut twice.facts.values.scalar_computations;
    let CheckedScalarComputationKind::BoundaryCall { arguments, .. } =
        plans.nodes.get(chained).kind.clone()
    else {
        panic!("chained sum is an application");
    };
    let inner = plans.operands.span(arguments).unwrap()[0];
    assert!(matches!(
        plans.nodes.get(inner).kind,
        CheckedScalarComputationKind::BoundaryCall {
            call_ordinal: 2,
            ..
        }
    ));
    let repeated = plans.operands.insert_many([inner, inner]);
    let CheckedScalarComputationKind::BoundaryCall { arguments, .. } =
        &mut plans.nodes.get_mut(chained).kind
    else {
        unreachable!();
    };
    *arguments = repeated;
    assert!(
        lowered(&twice).is_err(),
        "a repeated inner application rejects"
    );
}
