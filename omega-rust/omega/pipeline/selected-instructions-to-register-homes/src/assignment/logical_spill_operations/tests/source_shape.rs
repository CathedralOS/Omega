use optimization_unit::ValueDefinitionSite;
use register_model::{RegisterOperandAccess, RegisterViewId};
use selected_instructions::{VirtualRegisterId, VirtualRegisterOrigin};
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, ValueId};

use super::fixtures::raw_fixture;
use crate::{LogicalSpillOperationError, VirtualFixedConstraint, VirtualFixedConstraintSite};

fn compute(
    fixture: &super::fixtures::RawFixture,
) -> Result<Option<crate::LogicalSpillAction>, LogicalSpillOperationError> {
    super::super::compute::action::compute_action(
        0,
        &fixture.selected,
        &fixture.ranges,
        &fixture.legality,
        &fixture.choices,
    )
}

#[test]
fn raw_v1_shape_reconstructs_exact_store_reload_and_rewrite_boundaries() {
    let fixture = raw_fixture();
    let action = compute(&fixture).unwrap().unwrap();
    assert_eq!(action.incoming, VirtualRegisterId(2));
    assert_eq!(action.victim, VirtualRegisterId(0));
    assert_eq!(action.store.before_instruction.0, 2);
    assert_eq!(action.reload.before_instruction.0, 3);
    assert_eq!(
        action
            .rewrites
            .iter()
            .map(|rewrite| (rewrite.point.0, rewrite.instruction.0))
            .collect::<Vec<_>>(),
        vec![(6, 3), (8, 4)]
    );
}

#[test]
fn v1_refuses_unsupported_victim_type_origin_and_role() {
    let mut fixture = raw_fixture();
    fixture.selected.virtual_registers[0].scalar_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap());
    assert!(matches!(
        compute(&fixture),
        Err(LogicalSpillOperationError::UnsupportedScalarType { .. })
    ));

    let mut fixture = raw_fixture();
    fixture.selected.virtual_registers[0].origin = VirtualRegisterOrigin::EntryParameter {
        source_value: ValueId::new(30).unwrap(),
        parameter_index: 0,
    };
    fixture.selected.virtual_registers[0].definition_site =
        ValueDefinitionSite::FunctionParameter(0);
    assert!(matches!(
        compute(&fixture),
        Err(LogicalSpillOperationError::UnsupportedOrigin { .. })
    ));

    let mut fixture = raw_fixture();
    fixture.choices.choice.as_mut().unwrap().selected_victim = VirtualRegisterId(2);
    assert!(matches!(
        compute(&fixture),
        Err(LogicalSpillOperationError::UnsupportedVictimRole { .. })
    ));
}

#[test]
fn v1_refuses_nonlocal_fixed_missing_and_non_use_future_suffixes() {
    let mut fixture = raw_fixture();
    let fragment = fixture.ranges.virtual_registers[0].fragments[0];
    fixture.ranges.virtual_registers[0].fragments.push(fragment);
    assert!(matches!(
        compute(&fixture),
        Err(LogicalSpillOperationError::UnsupportedRangeShape { .. })
    ));

    let mut fixture = raw_fixture();
    fixture.ranges.virtual_registers[0]
        .fixed_constraints
        .push(VirtualFixedConstraint {
            site: VirtualFixedConstraintSite::Operand {
                position: crate::LivenessPosition(3),
                point: crate::LiveRangePoint(6),
                instruction: selected_instructions::SelectedInstructionId(3),
                operand: 0,
                access: RegisterOperandAccess::Use,
            },
            view: RegisterViewId(0),
        });
    assert!(matches!(
        compute(&fixture),
        Err(LogicalSpillOperationError::FutureFixedUse { .. })
    ));

    let mut fixture = raw_fixture();
    fixture.ranges.virtual_registers[0].occurrences.truncate(1);
    assert!(matches!(
        compute(&fixture),
        Err(LogicalSpillOperationError::NoFutureUse { .. })
    ));

    let mut fixture = raw_fixture();
    fixture.selected.blocks[0].instructions[3].operands[0].access = RegisterOperandAccess::UseDef;
    fixture.ranges.virtual_registers[0].occurrences[1].access = RegisterOperandAccess::UseDef;
    assert!(matches!(
        compute(&fixture),
        Err(LogicalSpillOperationError::FutureUseMismatch { .. })
    ));
}

#[test]
fn v1_refuses_pressure_definition_drift() {
    let mut fixture = raw_fixture();
    fixture.ranges.virtual_registers[2].occurrences[0].point = crate::LiveRangePoint(4);
    assert!(matches!(
        compute(&fixture),
        Err(LogicalSpillOperationError::IncomingDefinitionMismatch { .. })
    ));
}
