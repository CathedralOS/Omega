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
            _ => left * right,
        };
        Ok(TerminalEffectResult::Scalar(float(result)))
    }
}

fn lowered(checked: &checked_trees::CheckedTrees) -> Result<lowered_psi::LoweredPsi, String> {
    lower_machine(checked, TerminalMachineSelection::Name("Root::enter"))
        .map_err(|error| format!("{error:?}"))
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
