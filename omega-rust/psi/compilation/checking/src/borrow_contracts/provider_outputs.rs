//! An opaque completion supplies backing and access, while its declaration
//! supplies lifetime dependencies. Unlike an ordinary returned-view relation,
//! these dependencies neither locate the output inside an input nor require
//! an input to supply the output's access. Keep each input's typed frontier
//! until capture: a stored descriptor is not its container's outer borrow.
//! This relation describes writes through parameters. Returned values use the
//! ordinary reference-result relation, including invocation-owned origins.

use super::carried_lifetimes::{self, RegionIdentity};
use super::{
    BorrowOwnerSegment, StateParameter, TypeReferenceHandle, ViewReturnAmbiguity,
    ViewReturnInputSource,
};
use language_semantics::ReferenceAccess;

#[derive(Debug, Clone)]
pub struct ProviderLifetimeDependency {
    pub parameter_index: usize,
    pub source_type: TypeReferenceHandle,
    pub source_path: Vec<BorrowOwnerSegment>,
    pub source: ViewReturnInputSource,
    pub access: ReferenceAccess,
}

#[derive(Debug, Clone)]
pub struct ProviderOutput {
    pub destination: ProviderOutputDestination,
    pub owner_path: Vec<BorrowOwnerSegment>,
    pub access: ReferenceAccess,
    pub dependencies: Vec<ProviderLifetimeDependency>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderOutputDestination {
    Parameter(usize),
}

/// Resolve every writable output leaf using the complete applied type frontier.
/// This relation retains lifetimes only; provider admission separately supplies
/// the reference's backing, access and stability promise.
pub fn resolve_provider_outputs(
    program: &dyn crate::CheckingInput,
    parameters: &[StateParameter],
) -> Result<Vec<ProviderOutput>, ViewReturnAmbiguity> {
    let mut inputs = Vec::new();
    for (parameter_index, parameter) in parameters.iter().enumerate() {
        let leaves = carried_lifetimes::symbolic_input_lifetimes(
            program,
            parameter.type_reference,
            &[],
            &[],
            &[],
        )
        .ok_or_else(|| ViewReturnAmbiguity::IncompleteStructure {
            subject: format!("input `{}`", parameter.name),
        })?;
        for leaf in leaves {
            inputs.push((parameter_index, parameter, leaf));
        }
    }
    let mut destinations = Vec::new();
    for (parameter_index, parameter) in parameters.iter().enumerate() {
        let Some(shell) = crate::reference_shell(program, parameter.type_reference) else {
            continue;
        };
        if !shell.access.is_exclusive() {
            continue;
        }
        destinations.push((
            ProviderOutputDestination::Parameter(parameter_index),
            shell.referee,
            format!("output `{}`", parameter.name),
        ));
    }
    let mut outputs = Vec::new();
    for (destination, reference, subject) in destinations {
        let leaves = carried_lifetimes::borrowed_storage_lifetimes(program, reference)
            .ok_or_else(|| ViewReturnAmbiguity::IncompleteStructure { subject })?;
        for leaf in leaves {
            let matching: Vec<_> = inputs
                .iter()
                .filter(|(_, _, input)| {
                    leaf.lifetime
                        .as_ref()
                        .is_none_or(|region| input.lifetime.as_ref() == Some(region))
                })
                .collect();
            if matching.is_empty() {
                return Err(ViewReturnAmbiguity::LifetimeMatchesNoInput {
                    lifetime: match leaf.lifetime {
                        Some(RegionIdentity::Declared(name)) => name,
                        Some(region) => format!("{region:?}"),
                        None => "elided".to_owned(),
                    },
                });
            }
            if leaf.lifetime.is_none() && matching.len() != 1 {
                return Err(ViewReturnAmbiguity::ElidedMultipleInputs {
                    candidates: matching
                        .iter()
                        .map(|(_, parameter, input)| {
                            format!("{} carried source {:?}", parameter.name, input.owner_path)
                        })
                        .collect(),
                });
            }
            outputs.push(ProviderOutput {
                destination,
                owner_path: leaf.owner_path,
                access: leaf.access,
                dependencies: matching
                    .into_iter()
                    .map(
                        |(parameter_index, parameter, input)| ProviderLifetimeDependency {
                            parameter_index: *parameter_index,
                            source_type: parameter.type_reference,
                            source_path: input.owner_path.clone(),
                            source: input.source,
                            access: input.access,
                        },
                    )
                    .collect(),
            });
        }
    }
    if !crate::machine_calls::calls::provider_completion_covers_writable_slots(
        program, parameters, &outputs,
    ) {
        return Err(ViewReturnAmbiguity::IncompleteStructure {
            subject: "provider writable reference frontier".to_owned(),
        });
    }
    Ok(outputs)
}
