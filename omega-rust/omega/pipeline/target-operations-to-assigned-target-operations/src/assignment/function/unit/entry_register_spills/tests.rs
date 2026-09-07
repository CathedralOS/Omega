use super::*;
use semantic_vocabulary::{BlockId, EdgeId, IntegerSign, IntegerType, ScalarType};

fn fixture(target: NativeTarget) -> target_operations::TargetUnitBody {
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 8); 10],
            result: None,
        },
    )
    .unwrap();
    let scalar_parameters = call_plan
        .parameters
        .iter()
        .enumerate()
        .map(|(index, placement)| target_operations::ScalarAbiValue {
            value: ValueId::new(u64::try_from(index).unwrap() + 1).unwrap(),
            scalar_type,
            placement: placement.clone(),
        })
        .collect();
    target_operations::TargetUnitBody {
        structural_types: Vec::new(),
        call_plan,
        scalar_parameters,
        parameters: Vec::new(),
        operations: vec![TargetUnitOperation::Continue {
            psi_edge: EdgeId::new(1).unwrap(),
            source_block: BlockId::new(1).unwrap(),
            target_block: BlockId::new(2).unwrap(),
            bindings: Vec::new(),
            cleanup_actions: Vec::new(),
        }],
    }
}

#[test]
fn continuation_entry_sources_use_ordered_spills_and_original_incoming_stack() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let body = fixture(target);
        let mut cursor = 17;
        let spills = assign(&body, target, &mut cursor).unwrap();
        assert_eq!(spills[0].byte_offset, 24);
        assert_eq!(cursor, 24 + u32::try_from(spills.len()).unwrap() * 8);
        for (index, parameter) in body.scalar_parameters.iter().enumerate() {
            let source = TargetUnitScalarArgumentSource::Parameter {
                parameter_index: u32::try_from(index).unwrap(),
                source_value: parameter.value,
                scalar_type: parameter.scalar_type,
            };
            let AssignedUnitScalarArgumentSource::Parameter { location, .. } =
                parameter_source(&body, target, source).unwrap()
            else {
                unreachable!();
            };
            match parameter.placement.locations.as_slice() {
                [ValueLocation::Register { .. }] => assert_eq!(
                    location,
                    AssignedScalarLocation::FrameSpill {
                        byte_offset: u32::try_from(index).unwrap() * 8
                    }
                ),
                [
                    ValueLocation::Stack {
                        stack_byte_offset, ..
                    },
                ] => assert_eq!(
                    location,
                    AssignedScalarLocation::IncomingStack {
                        byte_offset: *stack_byte_offset
                    }
                ),
                _ => unreachable!(),
            }
            let mut wrong_source = source;
            let TargetUnitScalarArgumentSource::Parameter { source_value, .. } = &mut wrong_source
            else {
                unreachable!();
            };
            *source_value = ValueId::new(99).unwrap();
            assert_eq!(parameter_source(&body, target, wrong_source), None);
        }
        let mut ordinary = body;
        ordinary.operations.clear();
        let mut cursor = 17;
        assert!(assign(&ordinary, target, &mut cursor).unwrap().is_empty());
        assert_eq!(cursor, 17);
    }
}
