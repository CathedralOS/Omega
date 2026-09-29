//! A float-to-integer policy cast executes as one Terminal `BoundaryCall` of
//! the conversion overload its result qualification selects. The erasure
//! `(value as i32 in Saturating) as i32` adds no operation; the application
//! rejoins its exact source call, and a substituted overload fails closed
//! before any Terminal artifact exists.
use checked_trees::{CheckedScalarComputationKind, CheckedScalarExpressionRole};
use checked_trees_to_lowered_psi::{TerminalMachineSelection, lower_machine};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IeeeFloatValue, IntegerSign, IntegerType, IntegerValue};
use terminal_codec::{encode_module, encode_proof_section};
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult,
    TerminalScalarValue, TerminalStructuralInputs, interpret_terminal_artifact_measured,
};

const SOURCE: &str = "data Root {}
     pub data I32 {}
     pub boundary operator I32::from_f64(value: f64) -> i32
     requires value == value && value > -2147483649.0 && value < 2147483648.0;
     pub boundary operator I32::from_f64(value: f64) -> i32 in Trapping;
     pub boundary operator I32::from_f64(value: f64) -> i32 in Saturating;
     machine Root::enter(value: f64) {
         let clamped: i32 = (value as i32 in Saturating) as i32;
         transition { _ -> done(clamped) }
         state done(result: i32) {}
     }";

/// Answers every conversion with the Saturating law and records each
/// application's declaration identity and operand.
struct Conversions {
    observed: Vec<(String, f64)>,
    identities: Vec<(semantic_vocabulary::BoundaryMachineId, String)>,
}

impl TerminalEffectHandler for Conversions {
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new(
            "the conversion returns a scalar",
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
            return Err(TerminalEffectRejection::new("only conversions"));
        };
        let identity = self
            .identities
            .iter()
            .find(|(id, _)| id == boundary)
            .map(|(_, identity)| identity.clone())
            .ok_or(TerminalEffectRejection::new("unknown boundary"))?;
        let [TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary64(bits))] = arguments.as_slice()
        else {
            return Err(TerminalEffectRejection::new("one binary64 operand"));
        };
        let operand = f64::from_bits(*bits);
        self.observed.push((identity, operand));
        let saturated = if operand.is_nan() {
            0
        } else {
            operand.clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
        };
        Ok(TerminalEffectResult::Scalar(TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Signed, 32).expect("i32"),
            value: IntegerValue::Signed(i128::from(saturated)),
        }))
    }
}

fn lowered(checked: &checked_trees::CheckedTrees) -> Result<lowered_psi::LoweredPsi, String> {
    lower_machine(checked, TerminalMachineSelection::Name("Root::enter"))
        .map_err(|error| format!("{error:?}"))
}

/// The local initializer's computation root.
fn initializer_root(
    checked: &checked_trees::CheckedTrees,
) -> checked_trees::CheckedScalarComputationHandle {
    checked
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .map(|(_, root)| root)
        .find(|root| {
            root.statement_ordinal == 0
                && matches!(
                    root.role,
                    CheckedScalarExpressionRole::LocalInitializer { .. }
                )
        })
        .expect("the erased conversion is the local's computation")
        .root
}

/// The `I32::from_f64` overload whose result spelling contains `policy`, or
/// the unqualified one for an empty `policy`.
fn overload(checked: &checked_trees::CheckedTrees, policy: &str) -> symbols::SymbolHandle {
    checked
        .operators()
        .iter()
        .find(|operator| {
            let result = checked.display_type_reference_with_constraints(operator.return_type);
            checked
                .operator_path_members(operator.name)
                .last()
                .is_some_and(|member| member.as_str() == "from_f64")
                && if policy.is_empty() {
                    !result.contains(" in ")
                } else {
                    result.contains(policy)
                }
        })
        .map(|operator| operator.symbol)
        .unwrap_or_else(|| panic!("`{policy}` overload"))
}

#[test]
fn policy_cast_runs_its_selected_saturating_conversion_once() {
    let checked = crate::front_end::checked_program(SOURCE);
    let saturating = overload(&checked, "Saturating");
    let node = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .get(initializer_root(&checked));
    let CheckedScalarComputationKind::BoundaryCall {
        target_machine,
        target_state,
        ..
    } = node.kind
    else {
        panic!("the erasure adds no operation: {:?}", node.kind);
    };
    assert_eq!((target_machine, target_state), (saturating, saturating));

    let lowered = lowered(&checked).expect("the policy cast lowers");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the conversion independently verifies");
    let applications = lowered
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
            lowered
                .source_call_occurrences
                .iter()
                .filter(|row| row.terminal_operation == operation.id)
                .map(|row| (row.statement_index, row.call_ordinal))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(applications, [vec![(0, 0)]], "one exact source-call row");
    let identities = lowered
        .semantic_module
        .boundary_machines
        .iter()
        .map(|declaration| (declaration.id, declaration.identity.clone()))
        .collect::<Vec<_>>();
    let [(_, identity)] = identities.as_slice() else {
        panic!("only the selected overload is a boundary: {identities:?}");
    };
    assert!(identity.contains("Saturating"), "{identity}");
    let module = encode_module(&lowered.semantic_module).unwrap();
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap();
    let mut conversions = Conversions {
        observed: Vec::new(),
        identities,
    };
    interpret_terminal_artifact_measured(
        &module,
        &proof,
        &AdmissionProfile::default(),
        &[TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary64(
            3.0e9_f64.to_bits(),
        ))],
        TerminalStructuralInputs::default(),
        &mut conversions,
    )
    .expect("the policy cast interprets");
    assert_eq!(conversions.observed.len(), 1, "one application");
    assert_eq!(conversions.observed[0].1, 3.0e9);
}

#[test]
fn substituted_conversion_overload_fails_closed() {
    let checked = crate::front_end::checked_program(SOURCE);
    let root = initializer_root(&checked);
    for substitute in [overload(&checked, ""), overload(&checked, "Trapping")] {
        let mut substituted = checked.clone();
        let CheckedScalarComputationKind::BoundaryCall {
            target_machine,
            target_state,
            ..
        } = &mut substituted
            .facts
            .values
            .scalar_computations
            .nodes
            .get_mut(root)
            .kind
        else {
            panic!("conversion application");
        };
        *target_machine = substitute;
        *target_state = substitute;
        assert!(
            lowered(&substituted).is_err(),
            "a substituted conversion overload rejects"
        );
    }
}
