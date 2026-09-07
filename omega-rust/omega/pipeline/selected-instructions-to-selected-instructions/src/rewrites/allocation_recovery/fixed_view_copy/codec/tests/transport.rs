//! Wire and strong-identity custody of explicit edge transport; no rewrite admission is inferred.
use super::*;
use selected_instructions::{
    SelectedTerminator, SelectedValueBinding, SelectedValueTransport, VirtualRegisterId,
};
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, ValueId};
fn binding_mut(plan: &mut FixedViewCopyPlan) -> &mut SelectedValueBinding {
    let SelectedTerminator::ConditionalBranch { when_nonzero, .. } =
        &mut std::sync::Arc::make_mut(&mut plan.transformed).functions[0].blocks[0].terminator
    else {
        panic!("conditional fixture");
    };
    &mut when_nonzero.bindings[0]
}
#[test]
fn current_artifact_and_identity_bind_transport_role_and_both_registers() {
    let mut original = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    let SelectedTerminator::ConditionalBranch { when_nonzero, .. } =
        &mut std::sync::Arc::make_mut(&mut original.transformed).functions[0].blocks[0].terminator
    else {
        panic!("conditional fixture");
    };
    when_nonzero.bindings.push(SelectedValueBinding {
        semantic: abstract_operations::ValueBinding {
            parameter: ValueId::new(40).unwrap(),
            argument: ValueId::new(41).unwrap(),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        },
        transport: SelectedValueTransport::Registers {
            argument: VirtualRegisterId(17),
            parameter: VirtualRegisterId(29),
        },
    });
    let identity = target_operations_to_selected_instructions::selected_instruction_plan_identity(
        &original.transformed,
    );
    assert_eq!(
        FixedViewCopyPlan::decode(&original.encode()).unwrap(),
        original
    );
    for transport in [
        SelectedValueTransport::Unused,
        SelectedValueTransport::Registers {
            argument: VirtualRegisterId(18),
            parameter: VirtualRegisterId(29),
        },
        SelectedValueTransport::Registers {
            argument: VirtualRegisterId(17),
            parameter: VirtualRegisterId(30),
        },
    ] {
        let mut changed = original.clone();
        binding_mut(&mut changed).transport = transport;
        assert_ne!(
            target_operations_to_selected_instructions::selected_instruction_plan_identity(
                &changed.transformed
            ),
            identity
        );
        assert_ne!(changed.encode(), original.encode());
        assert_eq!(
            FixedViewCopyPlan::decode(&changed.encode()).unwrap(),
            changed
        );
    }
}
