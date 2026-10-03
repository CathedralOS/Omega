//! Primitive view calls retain source storage through serialized verification.
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use semantic_vocabulary::{IeeeFloatValue, IntegerSign, IntegerType, IntegerValue};
use terminal_interpreter::{TerminalExecutionResult, TerminalScalarValue};
use terminal_psi::OperationKind;

const SOURCE: &str = "machine read(value: &u32) -> u32 { value }
    machine choose(value: f32) -> u32 {
        let view: &u32 = &value as &u32;
        read(view)
    }";

#[test]
fn scalar_reference_calls_preserve_ieee_bits_through_serialized_artifacts() {
    for (width, bits) in [
        (32, 0x8000_0000),
        (32, 0x7fc0_0042),
        (64, 0x8000_0000_0000_0000),
        (64, 0x7ff8_0000_0000_0042),
    ] {
        let source = SOURCE.replace("32", &width.to_string());
        let argument = TerminalScalarValue::IeeeFloat(if width == 32 {
            IeeeFloatValue::Binary32(bits as u32)
        } else {
            IeeeFloatValue::Binary64(bits)
        });
        let (module, execution) = super::execute(&source, &[argument]);
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, width).unwrap(),
                value: IntegerValue::Unsigned(bits as u128),
            })
        );
        let machine = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let homes = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::EstablishPrimitiveLocal { .. }
                )
            })
            .map(|operation| operation.result.structural().unwrap().place)
            .collect::<Vec<_>>();
        assert_eq!(homes.len(), 1, "the scalar ingress has one original home");
        let references = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match &operation.kind {
                OperationKind::EstablishReference { source } => Some(source),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(references.len(), 1);
        assert_eq!(references[0].place, homes[0]);
        assert!(references[0].path.is_empty());
    }
}

#[test]
fn scalar_reference_calls_reject_removed_source_projection() {
    let checked = crate::front_end::checked_program(SOURCE);
    checked_trees_to_lowered_psi::lower_machine(&checked, TerminalMachineSelection::Name("choose"))
        .expect("unaltered retained source lowers");
    let spans = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .filter_map(|(_, node)| match node.kind {
            checked_trees::CheckedScalarComputationKind::Call {
                structural_arguments,
                ..
            } if !structural_arguments.is_empty() => Some(structural_arguments),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut changed = checked.clone();
    let mut removed = 0;
    for span in spans {
        let argument = changed
            .facts
            .values
            .scalar_computations
            .structural_arguments
            .get_mut(span.start())
            .as_place_mut()
            .unwrap();
        if argument.path == [checked_trees::CheckedUnitStructuralPathSegment::Referent] {
            argument.path.clear();
            removed += 1;
        }
    }
    assert!(
        removed > 0,
        "the source call retains the carrier projection"
    );
    assert!(
        checked_trees_to_lowered_psi::lower_machine(
            &changed,
            TerminalMachineSelection::Name("choose")
        )
        .is_err()
    );
}

#[test]
fn scalar_reference_calls_share_one_home_across_different_viewed_carriers() {
    let source = "machine read_unsigned(value: &u32) -> u32 { value }
        machine read_signed(value: &i32) -> i32 { value }
        machine choose(value: f32) -> bool {
            let first: &u32 = &value as &u32;
            let second: &i32 = &value as &i32;
            let unsigned: u32 = read_unsigned(first);
            let signed: i32 = read_signed(second);
            unsigned == 2147483648 && signed == -2147483648
        }";
    let (module, execution) = super::execute(
        source,
        &[TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(
            0x8000_0000,
        ))],
    );
    assert_eq!(
        execution.value(),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true))
    );
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let homes = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                OperationKind::EstablishPrimitiveLocal { .. }
            )
        })
        .map(|operation| operation.result.structural().unwrap().place)
        .collect::<Vec<_>>();
    assert_eq!(homes.len(), 1);
    let sources = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            OperationKind::EstablishReference { source } => Some(source.place),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        sources,
        [homes[0], homes[0]],
        "both views keep the original ingress backing"
    );
}

#[test]
fn scalar_reference_calls_keep_conditionally_valid_observations_on_the_proved_route() {
    let source = "machine choose(value: u8) -> bool requires value <= 1u8 {
        let view: &bool = &value as &bool;
        view
    }";
    for value in [0, 1] {
        let (module, execution) = super::execute(
            source,
            &[TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                value: IntegerValue::Unsigned(value),
            }],
        );
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(value == 1))
        );
        assert!(
            module
                .machines
                .iter()
                .flat_map(|machine| &machine.blocks)
                .flat_map(|block| &block.operations)
                .all(|operation| !matches!(
                    operation.kind,
                    OperationKind::EstablishReference { .. }
                )),
            "a contextually proved bool view is not a total storage-carrier relation"
        );
    }
}
