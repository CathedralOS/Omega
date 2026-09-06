//! Free Unit signatures retain scalar parameters without a fabricated attachment.

use super::*;
use checked_trees::{CheckedCallScalarArgument, CheckedTerminalSignatureEligibility};

const SOURCE: &str = r#"
    boundary trait Host {
        machine send(first: u16, second: u16) reaches Host;
    }
    data Sink {}
    machine Sink::finish(first: u16, second: u16) reaches Host {
        Host::send(first, second);
    }
    machine consume(first: u16, second: u16) reaches Host {
        Sink::finish(second, first);
    }
    machine empty() {}
"#;

#[test]
fn free_unit_parameters_preserve_positions_and_authored_call_order() {
    let checked = checked(SOURCE);
    let symbol = machine_named(&checked, "consume");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(symbol)
        .expect("free scalar Unit caller has its complete ordinary body");
    assert!(plan.attachment_type_identity.is_none());
    assert!(plan.structural_parameters.is_empty());
    assert!(matches!(plan.scalar_parameters.as_slice(), [first, second]
        if first.source_position == 0 && first.primitive_type == PrimitiveType::U16
            && second.source_position == 1 && second.primitive_type == PrimitiveType::U16));
    let [
        CheckedUnitEffectOperationPlan::CallUnit {
            scalar_arguments,
            coordinate,
            ..
        },
        CheckedUnitEffectOperationPlan::ReturnUnit { .. },
    ] = plan.operations.as_slice()
    else {
        panic!("the authored call and Unit return remain complete");
    };
    assert_eq!(
        (coordinate.statement_index, coordinate.call_ordinal),
        (0, 0)
    );
    assert!(matches!(
        scalar_arguments.as_slice(),
        [
            CheckedCallScalarArgument::Pure(CheckedScalarExpression::Parameter {
                position: 1,
                primitive_type: PrimitiveType::U16
            }),
            CheckedCallScalarArgument::Pure(CheckedScalarExpression::Parameter {
                position: 0,
                primitive_type: PrimitiveType::U16
            }),
        ]
    ));
    let selection = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find(|selection| selection.machine == symbol)
        .unwrap();
    assert_eq!(
        selection.signature,
        CheckedTerminalSignatureEligibility::FreeUnitEffect
    );
    let empty = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "empty"))
        .expect("the empty scalar signature needs no attachment either");
    assert!(empty.attachment_type_identity.is_none());
    assert!(empty.scalar_parameters.is_empty());
}

#[test]
fn free_unit_scalar_admission_rejects_parameter_shape_drift() {
    let original = checked(SOURCE);
    let symbol = machine_named(&original, "consume");
    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol)
        .unwrap();
    let state = &original.machine_states(machine)[0];
    let parameter_symbol = original.state_parameters(state)[0].symbol;
    for mutation in 0..4 {
        let mut changed = original.clone();
        let handle = changed
            .typed
            .state_parameters
            .iter()
            .find_map(|(handle, parameter)| {
                (parameter.symbol == parameter_symbol).then_some(handle)
            })
            .unwrap();
        let parameter = changed.typed.state_parameters.get_mut(handle);
        match mutation {
            0 => parameter.is_self = true,
            1 => parameter.is_const = true,
            2 => parameter.is_mutable = true,
            _ => parameter.type_reference = arena::Handle::invalid(),
        }
        let rebuilt =
            crate::flow::build_checked_unit_effect_plans(&changed.typed, &changed.facts, &[], &[]);
        assert!(
            rebuilt.for_machine(symbol).is_none(),
            "parameter mutation {mutation} rejects"
        );
    }
}

#[test]
fn free_scalar_admission_does_not_admit_unrelated_structural_signatures() {
    let checked = checked(
        r#"
        data Record { value: u16; }
        machine record_input(value: u16, record: Record) {}
        machine reference_input(value: &u16) {}
    "#,
    );
    for name in ["record_input", "reference_input"] {
        assert!(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(machine_named(&checked, name))
                .is_none()
        );
    }
}
