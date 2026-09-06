//! Borrowed arguments do not transfer owned custody, regardless of multiplicity.

use super::*;

#[test]
fn affine_borrows_remain_reusable_across_internal_and_boundary_calls() {
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let mut module = hard_root_module();
        for machine in &mut module.machines {
            machine.entry_claims.clear();
            let parameter = &mut machine.structural_parameters[0];
            parameter.multiplicity = StructuralMultiplicity::Affine;
            parameter.access = access;
            parameter.qualifications.clear();
            for operation in &mut machine.blocks[0].operations {
                match &mut operation.kind {
                    OperationKind::CallUnit {
                        structural_arguments,
                        claim_transfers,
                        ..
                    } => {
                        structural_arguments[0].access = access;
                        claim_transfers.clear();
                    }
                    OperationKind::BoundaryCall {
                        structural_arguments,
                        completion_receipts,
                        ..
                    } => {
                        structural_arguments[0].access = access;
                        completion_receipts.clear();
                    }
                    _ => {}
                }
            }
        }
        let boundary = &mut module.boundary_machines[0];
        boundary.requires.clear();
        boundary.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
        boundary.structural_parameters[0].access = access;

        let mut second_call = module.machines[0].blocks[0].operations[0].clone();
        second_call.id = operation_id(4);
        module.machines[0].blocks[0].operations.push(second_call);
        let mut second_boundary = module.machines[1].blocks[0].operations[1].clone();
        second_boundary.id = operation_id(5);
        module.machines[1].blocks[0]
            .operations
            .push(second_boundary);
        validate_module(&module)
            .expect("a loan neither consumes nor demands owned frontier custody");
    }
}
