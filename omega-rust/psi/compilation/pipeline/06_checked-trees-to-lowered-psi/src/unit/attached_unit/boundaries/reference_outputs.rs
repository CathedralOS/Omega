//! Translate admitted completion contracts once into the same packed structural
//! namespace as the boundary signature. An input anchor never becomes backing.

use crate::LoweringError;
use crate::expression_preparation::bindings::structural_paths::lower_static_argument_path;
use checked_trees::{
    CheckedBoundaryLifetimeDependency, CheckedBoundaryMachinePlan,
    CheckedBoundaryReferenceDestination,
};
use terminal_psi::{
    BoundaryLifetimeDependency, BoundaryReferenceDestination, BoundaryReferenceOutput,
    StructuralAccess, StructuralParameterDeclaration, StructuralTypeDeclaration,
    TerminalIdentityTableBuilder,
};

pub(super) fn lower(
    identities: &mut TerminalIdentityTableBuilder,
    plan: &CheckedBoundaryMachinePlan,
    parameters: &[StructuralParameterDeclaration],
    types: &[StructuralTypeDeclaration],
) -> Result<Vec<BoundaryReferenceOutput>, LoweringError> {
    let parameter = |position| {
        plan.structural_parameters
            .iter()
            .position(|parameter| parameter.position == position)
            .and_then(|position| parameters.get(position))
            .ok_or(LoweringError::Unsupported(
                "boundary reference contract names an absent structural parameter",
            ))
    };
    let mut outputs = Vec::with_capacity(plan.reference_outputs.len());
    for output in &plan.reference_outputs {
        let (destination, root) = match output.destination {
            CheckedBoundaryReferenceDestination::Parameter(position) => {
                let parameter = parameter(position)?;
                (
                    BoundaryReferenceDestination::Parameter(parameter.position),
                    parameter.structural_type,
                )
            }
        };
        let path = lower_static_argument_path(identities, &output.path, root, types)?;
        let mut dependencies = Vec::with_capacity(output.dependencies.len());
        for dependency in &output.dependencies {
            dependencies.push(match dependency {
                CheckedBoundaryLifetimeDependency::Referent {
                    parameter_index,
                    access,
                } => BoundaryLifetimeDependency::Referent {
                    argument_index: parameter(*parameter_index)?.position,
                    access: lower_access(*access),
                },
                CheckedBoundaryLifetimeDependency::Stored {
                    parameter_index,
                    path,
                    access,
                } => {
                    let parameter = parameter(*parameter_index)?;
                    BoundaryLifetimeDependency::Stored {
                        argument_index: parameter.position,
                        access: lower_access(*access),
                        path: lower_static_argument_path(
                            identities,
                            path,
                            parameter.structural_type,
                            types,
                        )?,
                    }
                }
            });
        }
        outputs.push(BoundaryReferenceOutput {
            destination,
            path,
            access: lower_access(output.access),
            dependencies,
        });
    }
    Ok(outputs)
}

fn lower_access(access: language_semantics::ReferenceAccess) -> StructuralAccess {
    match access {
        language_semantics::ReferenceAccess::Shared => StructuralAccess::SharedBorrow,
        language_semantics::ReferenceAccess::Mutable => StructuralAccess::MutableBorrow,
        language_semantics::ReferenceAccess::WriteOnly => StructuralAccess::WriteOnlyBorrow,
    }
}
