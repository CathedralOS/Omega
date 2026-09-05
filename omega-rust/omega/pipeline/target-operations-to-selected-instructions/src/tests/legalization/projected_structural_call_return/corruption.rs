use super::*;
use target_operations::TargetOperation;
use terminal_psi::{StructuralPathQualification, StructuralPathSegment};

#[test]
fn every_legalized_roster_location_rejects_every_canonical_corruption_on_all_targets() {
    for target_profile in targets() {
        let (source, target, unit) = projected_fixture(target_profile);
        let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
        for location in LOCATIONS {
            for mutation in MUTATIONS {
                let mut proposed = legalized.plan().clone();
                mutate(
                    rows(&mut proposed.projected_structural_call_returns[0], location),
                    mutation,
                );
                assert!(validate_legalized_operations(&target, &source, &unit, proposed).is_err());
            }
        }
    }
}

#[test]
fn machine_abi_and_optimizer_node_corruption_fail_closed() {
    let (source, target, unit) = projected_fixture(NativeTarget::linux_x64());
    let legalized = legalize_target_operations(&target, &source, &unit).unwrap();

    let mut machine = legalized.plan().clone();
    machine.projected_structural_call_returns[0].callee.machine =
        semantic_vocabulary::MachineId::new(99).unwrap();
    assert!(validate_legalized_operations(&target, &source, &unit, machine).is_err());

    let mut abi = legalized.plan().clone();
    let TargetOperation::ReturnStructuralCall {
        callee_call_plan, ..
    } = &mut abi.projected_structural_call_returns[0].caller.operation
    else {
        unreachable!()
    };
    callee_call_plan.shadow_bytes += 8;
    assert!(validate_legalized_operations(&target, &source, &unit, abi).is_err());

    let mut node = legalized.plan().clone();
    node.projected_structural_call_returns[0].caller_nodes[0]
        .effect
        .output += 1;
    assert!(validate_legalized_operations(&target, &source, &unit, node).is_err());
}

#[derive(Clone, Copy)]
enum Location {
    CallerParameter,
    CallerOperationResult,
    CallerResult,
    CalleeParameter,
    CalleeSource,
    CalleeResult,
}

const LOCATIONS: [Location; 6] = [
    Location::CallerParameter,
    Location::CallerOperationResult,
    Location::CallerResult,
    Location::CalleeParameter,
    Location::CalleeSource,
    Location::CalleeResult,
];

#[derive(Clone, Copy)]
enum Mutation {
    Missing,
    WrongPath,
    WrongDomain,
    Duplicate,
    Unsorted,
}

const MUTATIONS: [Mutation; 5] = [
    Mutation::Missing,
    Mutation::WrongPath,
    Mutation::WrongDomain,
    Mutation::Duplicate,
    Mutation::Unsorted,
];

fn rows(
    closure: &mut legalized_operations::LegalizedProjectedStructuralCallReturn,
    location: Location,
) -> &mut Vec<StructuralPathQualification> {
    match location {
        Location::CallerParameter => {
            let TargetOperation::ReturnStructuralCall {
                structural_parameters,
                ..
            } = &mut closure.caller.operation
            else {
                unreachable!()
            };
            &mut structural_parameters[0].projected_qualifications
        }
        Location::CallerOperationResult => {
            let TargetOperation::ReturnStructuralCall {
                operation_result, ..
            } = &mut closure.caller.operation
            else {
                unreachable!()
            };
            &mut operation_result.projected_qualifications
        }
        Location::CallerResult => {
            let TargetOperation::ReturnStructuralCall { result, .. } =
                &mut closure.caller.operation
            else {
                unreachable!()
            };
            &mut result.projected_qualifications
        }
        Location::CalleeParameter => {
            let TargetOperation::ReturnStructuralParameter { parameters, .. } =
                &mut closure.callee.operation
            else {
                unreachable!()
            };
            &mut parameters[0].projected_qualifications
        }
        Location::CalleeSource => {
            let TargetOperation::ReturnStructuralParameter { source, .. } =
                &mut closure.callee.operation
            else {
                unreachable!()
            };
            &mut source.projected_qualifications
        }
        Location::CalleeResult => {
            let TargetOperation::ReturnStructuralParameter { result, .. } =
                &mut closure.callee.operation
            else {
                unreachable!()
            };
            &mut result.projected_qualifications
        }
    }
}

fn mutate(rows: &mut Vec<StructuralPathQualification>, mutation: Mutation) {
    match mutation {
        Mutation::Missing => {
            rows.remove(0);
        }
        Mutation::WrongPath => rows[0].path = vec![StructuralPathSegment::Field("wrong".into())],
        Mutation::WrongDomain => {
            rows[0].domain = semantic_vocabulary::StructuralDomainId::new(99).unwrap()
        }
        Mutation::Duplicate => rows.insert(1, rows[0].clone()),
        Mutation::Unsorted => rows.swap(0, 1),
    }
}
