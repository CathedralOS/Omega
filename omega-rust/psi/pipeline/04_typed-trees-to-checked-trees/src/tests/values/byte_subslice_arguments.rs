//! Endpoint custody is retained before executable range admission.
use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::typed_program;
use checked_trees::CheckedScalarExpressionRole;

#[test]
fn byte_subslice_full_view_retains_an_ordinary_checked_call_plan() {
    let checked = lower_typed_trees(
        typed_program(
            "boundary trait Host { machine write(bytes: &[u8]); }
         data Relay {}
         machine Relay::write(bytes: &[u8]) reaches Host { Host::write(bytes); }
         data Helper {}
         machine Helper::write(bytes: &[u8]) reaches Host {
            Relay::write(bytes[..]);
            Host::write(bytes);
         }",
        ),
        &CheckingRequest::settled(),
    )
    .expect("full exclusive byte view checks without extra bounds");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| {
            machine
                .attached_data
                .as_ref()
                .is_some_and(|name| name.as_str() == "Helper")
        })
        .expect("helper");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine.symbol)
        .expect("full view retains the callable helper");
    let checked_trees::CheckedUnitEffectOperationPlan::CallUnit {
        structural_arguments,
        ..
    } = &plan.operations[0]
    else {
        panic!("ordinary Relay call")
    };
    assert!(matches!(
        structural_arguments[0].source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
            root: checked_trees::CheckedStorageRoot::Parameter { index: 0 },
            start: None,
            end: None,
            ..
        }
    ));
    assert!(
        checked
            .facts
            .values
            .scalar_expressions
            .source_bindings
            .iter()
            .all(|(_, binding)| binding.state != plan.state
                || !matches!(
                    binding.role,
                    CheckedScalarExpressionRole::SubsliceStart { .. }
                        | CheckedScalarExpressionRole::SubsliceEnd { .. }
                ))
    );
}
