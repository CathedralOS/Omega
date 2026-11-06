//! Bind each requested guarantee to the call that must preserve it.
//!
//! This checks the request's coordinates and declared type relationship.
//! Capturing the original lender and proving preservation remain independent
//! semantic judgments; this table contains no proof result.

use crate::codec_error::{CodecError, malformed};
use terminal_psi::{
    BoundaryCompletion, BoundaryDeclarationRole, BoundaryLenderPreservationDemand, Operation,
    OperationKind, StructuralAccess, StructuralPathSegment, TerminalModule,
};
use terminal_semantics::qualification_path_carrier;

/// Call and declaration foundations must already have passed. Stream only
/// callers with requests, retaining a found bit per row rather than building
/// another index over every operation in the program.
pub(super) fn validate(module: &TerminalModule) -> Result<(), CodecError> {
    let demands = &module.boundary_lender_preservation;
    if demands.is_empty() {
        return Ok(());
    }
    let mut found = vec![false; demands.len()];
    for machine in &module.machines {
        let start = demands.partition_point(|demand| demand.machine < machine.id);
        let end = demands.partition_point(|demand| demand.machine <= machine.id);
        let caller_demands = &demands[start..end];
        if caller_demands.is_empty() {
            continue;
        }
        for operation in machine.operations() {
            let first = caller_demands.partition_point(|demand| demand.operation < operation.id);
            let last = caller_demands.partition_point(|demand| demand.operation <= operation.id);
            if first == last {
                continue;
            }
            for present in &mut found[start + first..start + last] {
                if *present {
                    return malformed("lender preservation names a repeated caller or operation");
                }
                *present = true;
            }
            validate_call(module, operation, &caller_demands[first..last])?;
        }
    }
    if found.iter().any(|present| !present) {
        return malformed("lender preservation names an absent caller or operation");
    }
    Ok(())
}

fn validate_call(
    module: &TerminalModule,
    operation: &Operation,
    demands: &[BoundaryLenderPreservationDemand],
) -> Result<(), CodecError> {
    let OperationKind::BoundaryCall {
        boundary,
        structural_arguments,
        ..
    } = &operation.kind
    else {
        return malformed("lender preservation does not name a boundary call");
    };
    let declaration =
        module
            .boundary_machine(*boundary)
            .ok_or(CodecError::MalformedStructuralFoundation(
                "lender preservation has no boundary declaration",
            ))?;
    if declaration.role != BoundaryDeclarationRole::Callable
        || declaration.completion != BoundaryCompletion::NormalReturn
    {
        return malformed("lender preservation requires a normally returning boundary call");
    }
    for demand in demands {
        let requirement = &demand.requirement;
        if !declaration.normal_qualifications.contains(requirement) {
            return malformed("lender preservation is not a declared normal qualification");
        }
        let parameter = declaration
            .structural_parameters
            .get(requirement.argument_index as usize)
            .ok_or(CodecError::MalformedStructuralFoundation(
                "lender preservation has no structural parameter",
            ))?;
        let actual = structural_arguments
            .get(requirement.argument_index as usize)
            .ok_or(CodecError::MalformedStructuralFoundation(
                "lender preservation has no structural argument",
            ))?;
        let domain = module.structural_domain(requirement.domain).ok_or(
            CodecError::MalformedStructuralFoundation("lender preservation has no domain"),
        )?;
        if parameter.access != StructuralAccess::MutableBorrow
            || actual.access != StructuralAccess::MutableBorrow
            || !requirement.path.contains(&StructuralPathSegment::Referent)
            || qualification_path_carrier(
                module.structural_types.iter(),
                parameter.structural_type,
                &requirement.path,
            ) != Some(domain.carrier)
        {
            return malformed("lender preservation has the wrong mutable stored-reference path");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sections::semantic_module::normal_qualification_tests;
    use semantic_vocabulary::{MachineId, OperationId};
    use terminal_psi::{OperationResult, StructuralArgument};

    fn module() -> TerminalModule {
        let mut module = normal_qualification_tests::module();
        let declaration = &module.boundary_machines[0];
        let caller = &mut module.machines[0];
        caller.structural_parameters = declaration.structural_parameters.clone();
        caller.parameter_order = declaration.parameter_order.clone();
        let operation = OperationId::new(1).unwrap();
        caller.blocks[0].operations.push(Operation {
            id: operation,
            result: OperationResult::Unit,
            static_reach_binding: None,
            suspension_crossing: None,
            kind: OperationKind::BoundaryCall {
                boundary: declaration.id,
                arguments: Vec::new(),
                structural_arguments: declaration
                    .structural_parameters
                    .iter()
                    .map(|parameter| StructuralArgument::whole(parameter.place, parameter.access))
                    .collect(),
                completion_receipts: Vec::new(),
                requirement_obligations: Vec::new(),
            },
        });
        module.boundary_lender_preservation = vec![BoundaryLenderPreservationDemand {
            machine: caller.id,
            operation,
            requirement: declaration.normal_qualifications[2].clone(),
        }];
        module
    }

    #[test]
    fn lender_request_shape_binds_exact_caller_call_and_public_row() {
        let original = module();
        // Shape acceptance is deliberately separate from proving membership
        // and preservation of the captured lender.
        assert_eq!(validate(&original), Ok(()));
        let mut changed = original.clone();
        changed.boundary_lender_preservation[0].machine = MachineId::new(2).unwrap();
        assert_eq!(
            validate(&changed),
            malformed("lender preservation names an absent caller or operation"),
        );
        let mut changed = original.clone();
        changed.boundary_lender_preservation[0].operation = OperationId::new(2).unwrap();
        assert_eq!(
            validate(&changed),
            malformed("lender preservation names an absent caller or operation"),
        );
        let mut changed = original.clone();
        let repeated = changed.machines[0].blocks[0].operations[0].clone();
        changed.machines[0].blocks[0].operations.push(repeated);
        assert_eq!(
            validate(&changed),
            malformed("lender preservation names a repeated caller or operation"),
        );
        let mut changed = original.clone();
        changed.machines[0].blocks[0].operations[0].kind = OperationKind::BoundaryCall {
            boundary: semantic_vocabulary::BoundaryMachineId::new(2).unwrap(),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
            requirement_obligations: Vec::new(),
        };
        assert_eq!(
            validate(&changed),
            malformed("lender preservation has no boundary declaration"),
        );
        let mut changed = original.clone();
        changed.boundary_machines[0].normal_qualifications.clear();
        assert_eq!(
            validate(&changed),
            malformed("lender preservation is not a declared normal qualification"),
        );
        let mut changed = original;
        changed.machines[0].blocks[0].operations[0].kind =
            OperationKind::BooleanConstant { value: true };
        assert_eq!(
            validate(&changed),
            malformed("lender preservation does not name a boundary call"),
        );
    }

    #[test]
    fn lender_request_shape_refuses_unsupported_access_and_path() {
        let original = module();
        let wrong_path = "lender preservation has the wrong mutable stored-reference path";
        let mut changed = original.clone();
        changed.boundary_machines[0].structural_parameters[2].access = StructuralAccess::Owned;
        assert_eq!(validate(&changed), malformed(wrong_path));
        let mut changed = original.clone();
        let OperationKind::BoundaryCall {
            structural_arguments,
            ..
        } = &mut changed.machines[0].blocks[0].operations[0].kind
        else {
            panic!("fixture boundary call");
        };
        structural_arguments[2].access = StructuralAccess::SharedBorrow;
        assert_eq!(validate(&changed), malformed(wrong_path));
        let mut changed = original.clone();
        changed.boundary_lender_preservation[0]
            .requirement
            .path
            .pop();
        assert_eq!(
            validate(&changed),
            malformed("lender preservation is not a declared normal qualification"),
        );
        let mut changed = original.clone();
        changed.boundary_lender_preservation[0].requirement =
            changed.boundary_machines[0].normal_qualifications[0].clone();
        assert_eq!(validate(&changed), malformed(wrong_path));
        let mut changed = original;
        changed.structural_domains[0].carrier =
            changed.boundary_machines[0].structural_parameters[2].structural_type;
        assert_eq!(validate(&changed), malformed(wrong_path));
    }
}
