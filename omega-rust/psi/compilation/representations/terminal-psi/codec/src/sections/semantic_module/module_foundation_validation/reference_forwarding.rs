//! Check which captured references a call requests to forward unchanged.
//!
//! These coordinates identify inputs and an output; they do not prove that
//! the selected implementation forwards any value, preserves its lifetime,
//! or satisfies a qualification on its referent.

use crate::codec_error::{CodecError, malformed};
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    BoundaryCompletion, BoundaryDeclarationRole, BoundaryReferenceDestination,
    BoundaryReferenceForwardingDemand, BoundaryReferenceSource, Operation, OperationKind,
    StructuralAccess, StructuralParameterDeclaration, StructuralPathSegment, StructuralTypeShape,
    TerminalModule,
};
use terminal_semantics::runtime_structural_path_tip;

/// Machine and call foundations have passed. Match the sorted request roster
/// without retaining another index over all operations.
pub(super) fn validate(module: &TerminalModule) -> Result<(), CodecError> {
    let demands = &module.boundary_reference_forwarding;
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
                    return malformed("reference forwarding names a repeated caller or operation");
                }
                *present = true;
            }
            validate_call(module, operation, &caller_demands[first..last])?;
        }
    }
    if found.iter().any(|present| !present) {
        return malformed("reference forwarding names an absent caller or operation");
    }
    Ok(())
}

fn validate_call(
    module: &TerminalModule,
    operation: &Operation,
    demands: &[BoundaryReferenceForwardingDemand],
) -> Result<(), CodecError> {
    let OperationKind::BoundaryCall {
        boundary,
        structural_arguments,
        ..
    } = &operation.kind
    else {
        return malformed("reference forwarding does not name a boundary call");
    };
    let declaration =
        module
            .boundary_machine(*boundary)
            .ok_or(CodecError::MalformedStructuralFoundation(
                "reference forwarding has no boundary declaration",
            ))?;
    if declaration.role != BoundaryDeclarationRole::Callable
        || declaration.completion != BoundaryCompletion::NormalReturn
    {
        return malformed("reference forwarding requires a normally returning boundary call");
    }
    let parameter = |index: u32| {
        declaration
            .structural_parameters
            .get(index as usize)
            .filter(|_| structural_arguments.get(index as usize).is_some())
            .ok_or(CodecError::MalformedStructuralFoundation(
                "reference forwarding names an absent structural argument",
            ))
    };
    for demand in demands {
        let output = declaration
            .reference_outputs
            .get(demand.output as usize)
            .ok_or(CodecError::MalformedStructuralFoundation(
                "reference forwarding names an absent output",
            ))?;
        let BoundaryReferenceDestination::Parameter(index) = output.destination;
        let destination = parameter(index)?;
        let output_reference = runtime_structural_path_tip(
            module.structural_types.iter(),
            destination.structural_type,
            &output.path,
        );
        let Some(StructuralTypeShape::Reference { referent, access }) =
            output_reference.map(|declaration| &declaration.shape)
        else {
            return malformed("reference forwarding output is not a stored reference");
        };
        if !matches!(
            destination.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        ) || *access != output.access
        {
            return malformed("reference forwarding output has incompatible access");
        }
        if demand.sources.is_empty() {
            return malformed("reference forwarding has no captured source");
        }
        for source in &demand.sources {
            let source = match source {
                BoundaryReferenceSource::Reference { argument_index } => {
                    let parameter = parameter(*argument_index)?;
                    (parameter.access != StructuralAccess::Owned)
                        .then_some((parameter.structural_type, parameter.access))
                }
                BoundaryReferenceSource::Stored {
                    argument_index,
                    path,
                } => stored_reference(module, parameter(*argument_index)?, path),
            };
            if !source.is_some_and(|(source_referent, source_access)| {
                source_referent == *referent && access.admits_referent(source_access)
            }) {
                return malformed("reference forwarding source has incompatible carrier or access");
            }
        }
    }
    Ok(())
}

fn stored_reference(
    module: &TerminalModule,
    parameter: &StructuralParameterDeclaration,
    path: &[StructuralPathSegment],
) -> Option<(StructuralTypeId, StructuralAccess)> {
    if parameter.access == StructuralAccess::WriteOnlyBorrow {
        return None;
    }
    let mut shared_storage = parameter.access == StructuralAccess::SharedBorrow;
    for (position, segment) in path.iter().enumerate() {
        match segment {
            StructuralPathSegment::Field(_)
            | StructuralPathSegment::Case(_)
            | StructuralPathSegment::FixedIndex(_) => {}
            StructuralPathSegment::Referent => {
                let parent = runtime_structural_path_tip(
                    module.structural_types.iter(),
                    parameter.structural_type,
                    &path[..position],
                )?;
                match parent.shape {
                    StructuralTypeShape::Reference {
                        access: StructuralAccess::SharedBorrow,
                        ..
                    } => {
                        shared_storage = true;
                    }
                    StructuralTypeShape::Reference {
                        access: StructuralAccess::MutableBorrow,
                        ..
                    } => {}
                    _ => return None,
                }
            }
            _ => return None,
        }
    }
    let selected = runtime_structural_path_tip(
        module.structural_types.iter(),
        parameter.structural_type,
        path,
    )?;
    let StructuralTypeShape::Reference { referent, access } = selected.shape else {
        return None;
    };
    let access = if shared_storage {
        if !StructuralAccess::SharedBorrow.admits_referent(access) {
            return None;
        }
        StructuralAccess::SharedBorrow
    } else {
        access
    };
    Some((referent, access))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sections::semantic_module::normal_qualification_tests;
    use semantic_vocabulary::{MachineId, OperationId, PlaceId};
    use terminal_psi::{BoundaryParameterKind, OperationResult, StructuralArgument};

    fn module() -> TerminalModule {
        let mut module = normal_qualification_tests::module();
        let declaration = &mut module.boundary_machines[0];
        let mut parameter = declaration.structural_parameters[2].clone();
        parameter.place = PlaceId::new(4).unwrap();
        parameter.position = 3;
        parameter.structural_type = StructuralTypeId::new(2).unwrap();
        declaration.structural_parameters.push(parameter);
        declaration
            .parameter_order
            .push(BoundaryParameterKind::Structural);
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
        module.boundary_reference_forwarding = vec![BoundaryReferenceForwardingDemand {
            machine: caller.id,
            operation,
            output: 0,
            sources: vec![
                BoundaryReferenceSource::Reference { argument_index: 3 },
                BoundaryReferenceSource::Stored {
                    argument_index: 2,
                    path: Vec::new(),
                },
            ],
        }];
        module
    }

    #[test]
    fn forwarding_shape_distinguishes_outer_reference_from_stored_reference() {
        let original = module();
        // This only validates the request's coordinates and types. It does
        // not admit the implementation or prove the requested forwarding.
        assert_eq!(validate(&original), Ok(()));
        let error = "reference forwarding source has incompatible carrier or access";
        let mut changed = original.clone();
        changed.boundary_reference_forwarding[0].sources =
            vec![BoundaryReferenceSource::Reference { argument_index: 2 }];
        assert_eq!(validate(&changed), malformed(error));
        let mut changed = original.clone();
        changed.boundary_reference_forwarding[0].sources = vec![BoundaryReferenceSource::Stored {
            argument_index: 3,
            path: Vec::new(),
        }];
        assert_eq!(validate(&changed), malformed(error));
        let mut changed = original;
        changed.boundary_machines[0].structural_parameters[3].access = StructuralAccess::Owned;
        assert_eq!(validate(&changed), malformed(error));
    }

    #[test]
    fn forwarding_shape_binds_real_call_output_and_nonempty_sources() {
        let original = module();
        let mut changed = original.clone();
        changed.boundary_reference_forwarding[0].machine = MachineId::new(2).unwrap();
        assert_eq!(
            validate(&changed),
            malformed("reference forwarding names an absent caller or operation")
        );
        let mut changed = original.clone();
        changed.boundary_reference_forwarding[0].operation = OperationId::new(2).unwrap();
        assert_eq!(
            validate(&changed),
            malformed("reference forwarding names an absent caller or operation")
        );
        let mut changed = original.clone();
        changed.boundary_reference_forwarding[0].output = 1;
        assert_eq!(
            validate(&changed),
            malformed("reference forwarding names an absent output")
        );
        let mut changed = original.clone();
        changed.boundary_reference_forwarding[0].sources.clear();
        assert_eq!(
            validate(&changed),
            malformed("reference forwarding has no captured source")
        );
        let mut changed = original.clone();
        changed.machines[0].blocks[0].operations[0].kind =
            OperationKind::BooleanConstant { value: true };
        assert_eq!(
            validate(&changed),
            malformed("reference forwarding does not name a boundary call")
        );
        let mut changed = original;
        let repeated = changed.machines[0].blocks[0].operations[0].clone();
        changed.machines[0].blocks[0].operations.push(repeated);
        assert_eq!(
            validate(&changed),
            malformed("reference forwarding names a repeated caller or operation")
        );
    }

    #[test]
    fn forwarding_shape_uses_packed_structural_indices_in_mixed_signatures() {
        let mut module = module();
        let declaration = &mut module.boundary_machines[0];
        // A scalar before these formals changes their authored positions,
        // while the forwarding and call rosters keep packed structural indices.
        declaration
            .parameter_order
            .insert(0, BoundaryParameterKind::Scalar);
        for parameter in &mut declaration.structural_parameters {
            parameter.position += 1;
        }
        assert_eq!(validate(&module), Ok(()));
    }

    #[test]
    fn forwarding_shape_cannot_read_write_only_storage_or_upgrade_shared_access() {
        let original = module();
        let error = "reference forwarding source has incompatible carrier or access";
        let mut changed = original.clone();
        changed.boundary_reference_forwarding[0].sources.remove(0);
        changed.boundary_machines[0].structural_parameters[2].access =
            StructuralAccess::WriteOnlyBorrow;
        assert_eq!(validate(&changed), malformed(error));
        let mut changed = original.clone();
        changed.boundary_reference_forwarding[0].sources.remove(0);
        changed.boundary_machines[0].structural_parameters[2].access =
            StructuralAccess::SharedBorrow;
        // The same parameter is also the destination; first provide separate
        // writable output storage so the test reaches source attenuation.
        changed.boundary_machines[0].reference_outputs[0].destination =
            BoundaryReferenceDestination::Parameter(1);
        changed.boundary_machines[0].structural_parameters[1].structural_type =
            StructuralTypeId::new(3).unwrap();
        assert_eq!(validate(&changed), malformed(error));
        let mut changed = original;
        changed.boundary_machines[0].structural_parameters[3].access =
            StructuralAccess::SharedBorrow;
        assert_eq!(validate(&changed), malformed(error));
        changed.structural_types[2].shape = StructuralTypeShape::Reference {
            referent: StructuralTypeId::new(2).unwrap(),
            access: StructuralAccess::SharedBorrow,
        };
        changed.boundary_machines[0].reference_outputs[0].access = StructuralAccess::SharedBorrow;
        assert_eq!(validate(&changed), Ok(()));
    }

    #[test]
    fn forwarding_shape_checks_stored_paths_and_each_reference_crossing() {
        let mut original = module();
        let mut nested = original.structural_types[2].clone();
        nested.id = StructuralTypeId::new(5).unwrap();
        nested.shape = StructuralTypeShape::Reference {
            referent: StructuralTypeId::new(3).unwrap(),
            access: StructuralAccess::MutableBorrow,
        };
        original.structural_types.push(nested);
        original.boundary_machines[0].structural_parameters[3].structural_type =
            StructuralTypeId::new(5).unwrap();
        original.boundary_reference_forwarding[0].sources = vec![BoundaryReferenceSource::Stored {
            argument_index: 3,
            path: vec![StructuralPathSegment::Referent],
        }];
        assert_eq!(validate(&original), Ok(()));
        let error = "reference forwarding source has incompatible carrier or access";
        for access in [
            StructuralAccess::SharedBorrow,
            StructuralAccess::WriteOnlyBorrow,
        ] {
            let mut changed = original.clone();
            changed.structural_types[4].shape = StructuralTypeShape::Reference {
                referent: StructuralTypeId::new(3).unwrap(),
                access,
            };
            assert_eq!(validate(&changed), malformed(error));
        }
        let mut changed = original;
        changed.boundary_reference_forwarding[0].sources = vec![BoundaryReferenceSource::Stored {
            argument_index: 3,
            path: vec![StructuralPathSegment::AllElements],
        }];
        assert_eq!(validate(&changed), malformed(error));
    }

    #[test]
    fn forwarding_shape_selects_the_reference_leaf_inside_an_argument() {
        let mut module = module();
        let StructuralTypeShape::Record { fields, .. } = &mut module.structural_types[3].shape
        else {
            panic!("fixture record");
        };
        fields[0].field_type =
            terminal_psi::StructuralFieldType::Structural(StructuralTypeId::new(3).unwrap());
        module.boundary_reference_forwarding[0].sources = vec![BoundaryReferenceSource::Stored {
            argument_index: 1,
            path: vec![StructuralPathSegment::Field(
                terminal_psi::TerminalIdentityIndex(8),
            )],
        }];
        assert_eq!(validate(&module), Ok(()));
        module.boundary_reference_forwarding[0].sources = vec![BoundaryReferenceSource::Stored {
            argument_index: 1,
            path: vec![StructuralPathSegment::Field(
                terminal_psi::TerminalIdentityIndex(9),
            )],
        }];
        assert_eq!(
            validate(&module),
            malformed("reference forwarding source has incompatible carrier or access")
        );
    }
}
