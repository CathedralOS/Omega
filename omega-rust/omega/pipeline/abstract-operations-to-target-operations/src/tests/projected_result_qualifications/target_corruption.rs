//! Target projected-roster and callee-coordinate corruption matrix.

use super::*;

#[derive(Clone, Copy)]
enum RosterLocation {
    TargetParameter,
    CallerOperationResult,
    CallerFunctionResult,
    CalleeParameter,
    CalleeSource,
    CalleeFunctionResult,
}

#[derive(Clone, Copy)]
enum RosterMutation {
    Missing,
    WrongPath,
    WrongDomain,
    Duplicate,
    Unsorted,
}

#[test]
fn every_projected_roster_carrier_rejects_each_canonical_corruption_class() {
    let source = projected_structural_call_return_plan();
    for location in [
        RosterLocation::TargetParameter,
        RosterLocation::CallerOperationResult,
        RosterLocation::CallerFunctionResult,
        RosterLocation::CalleeParameter,
        RosterLocation::CalleeSource,
        RosterLocation::CalleeFunctionResult,
    ] {
        for mutation in [
            RosterMutation::Missing,
            RosterMutation::WrongPath,
            RosterMutation::WrongDomain,
            RosterMutation::Duplicate,
            RosterMutation::Unsorted,
        ] {
            let mut target =
                lower_to_target_operations(&source, NativeTarget::linux_x64()).unwrap();
            mutate(roster_mut(&mut target, location), mutation);
            assert!(
                crate::validate_abstract_to_target_translation(
                    &source,
                    NativeTarget::linux_x64(),
                    &target,
                )
                .is_err(),
                "every carrier/mutation pair must fail closed"
            );
        }
    }
}

#[test]
fn callee_coordinate_machine_and_authored_access_substitution_fail_closed() {
    let source = projected_structural_call_return_plan();
    let baseline = lower_to_target_operations(&source, NativeTarget::linux_x64()).unwrap();

    let mut callee_coordinate = baseline.clone();
    let TargetOperation::ReturnStructuralCall { callee, .. } =
        &mut callee_coordinate.functions[0].operation
    else {
        unreachable!()
    };
    *callee = MachineId::new(999).unwrap();
    assert!(
        crate::validate_abstract_to_target_translation(
            &source,
            NativeTarget::linux_x64(),
            &callee_coordinate,
        )
        .is_err()
    );

    let mut callee_machine = baseline.clone();
    callee_machine.functions[1].machine = MachineId::new(999).unwrap();
    assert!(
        crate::validate_abstract_to_target_translation(
            &source,
            NativeTarget::linux_x64(),
            &callee_machine,
        )
        .is_err()
    );

    let mut access = baseline;
    let TargetOperation::ReturnStructuralCall {
        structural_parameters,
        ..
    } = &mut access.functions[0].operation
    else {
        unreachable!()
    };
    structural_parameters[0].access = StructuralAccess::SharedBorrow;
    assert!(crate::validate_abstract_to_target_translation(
        &source,
        NativeTarget::linux_x64(),
        &access,
    )
    .is_err());
}

fn roster_mut(
    target: &mut target_operations::TargetOperationPlan,
    location: RosterLocation,
) -> &mut Vec<terminal_psi::StructuralPathQualification> {
    match location {
        RosterLocation::TargetParameter => {
            let TargetOperation::ReturnStructuralCall {
                structural_parameters,
                ..
            } = &mut target.functions[0].operation
            else {
                unreachable!()
            };
            &mut structural_parameters[0].projected_qualifications
        }
        RosterLocation::CallerOperationResult => {
            let TargetOperation::ReturnStructuralCall {
                operation_result, ..
            } = &mut target.functions[0].operation
            else {
                unreachable!()
            };
            &mut operation_result.projected_qualifications
        }
        RosterLocation::CallerFunctionResult => {
            let TargetOperation::ReturnStructuralCall { result, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            &mut result.projected_qualifications
        }
        RosterLocation::CalleeParameter => {
            let TargetOperation::ReturnStructuralParameter { parameters, .. } =
                &mut target.functions[1].operation
            else {
                unreachable!()
            };
            &mut parameters[0].projected_qualifications
        }
        RosterLocation::CalleeSource => {
            let TargetOperation::ReturnStructuralParameter { source, .. } =
                &mut target.functions[1].operation
            else {
                unreachable!()
            };
            &mut source.projected_qualifications
        }
        RosterLocation::CalleeFunctionResult => {
            let TargetOperation::ReturnStructuralParameter { result, .. } =
                &mut target.functions[1].operation
            else {
                unreachable!()
            };
            &mut result.projected_qualifications
        }
    }
}

fn mutate(rows: &mut Vec<terminal_psi::StructuralPathQualification>, mutation: RosterMutation) {
    match mutation {
        RosterMutation::Missing => {
            rows.remove(0);
        }
        RosterMutation::WrongPath => {
            rows[0].path = vec![StructuralPathSegment::Field("aardvark".into())];
        }
        RosterMutation::WrongDomain => {
            rows[0].domain = semantic_vocabulary::StructuralDomainId::new(999).unwrap();
        }
        RosterMutation::Duplicate => {
            rows.insert(1, rows[0].clone());
        }
        RosterMutation::Unsorted => rows.swap(0, 1),
    }
}
