//! Bounded claim-free affine structural-result realizations for selected Unit calls.

use super::*;

pub(in crate::attached_unit) fn lower_selected_structural_result_realizations(
    checked: &CheckedTrees,
    roots: &[symbols::SymbolHandle],
    structural_types: &[StructuralTypeDeclaration],
    type_ids: &[(String, StructuralTypeId)],
    machine_ids: &[(symbols::SymbolHandle, MachineId)],
    machine_index_base: usize,
) -> Result<Vec<TerminalMachine>, LoweringError> {
    let plans = &checked.facts.flow.terminal_structural_returns;
    let mut machines = Vec::with_capacity(roots.len());
    for (index, source_machine) in roots.iter().enumerate() {
        let realizations = plans
            .claim_free_affine_machines
            .iter()
            .filter(|plan| plan.machine == *source_machine)
            .collect::<Vec<_>>();
        let [realization] = realizations.as_slice() else {
            return unsupported(
                "selected structural-result closure does not contain one exact checked realization",
            );
        };
        if realization.structural_parameter.position != 0
            || realization.structural_parameter.is_self
            || realization.structural_parameter.multiplicity != Multiplicity::Affine
            || realization.structural_parameter.access
                != checked_trees::CheckedStructuralAccess::Owned
            || !realization.structural_parameter.qualifications.is_empty()
            || realization
                .structural_parameter
                .fused_service_erasure
                .is_some()
            || realization.scalar_parameters.len() != 1
            || realization.scalar_parameters[0].source_position != 1
            || realization.result.multiplicity != Multiplicity::Affine
            || !realization.result.qualifications.is_empty()
            || realization.result.type_identity != realization.structural_parameter.type_identity
            || realization.return_statement_ordinal != 0
        {
            return unsupported(
                "selected structural-result realization exceeds the first mixed affine lane",
            );
        }
        let machine_index =
            machine_index_base
                .checked_add(index)
                .ok_or(LoweringError::Unsupported(
                    "selected structural-result machine count overflows usize",
                ))?;
        let identity_base = u64::try_from(machine_index)
            .map_err(|_| {
                LoweringError::Unsupported("selected structural-result machine count exceeds u64")
            })?
            .checked_mul(TERMINAL_MACHINE_IDENTITY_STRIDE)
            .ok_or(LoweringError::Unsupported(
                "selected structural-result identity range overflows",
            ))?;
        let terminal_machine = lookup_machine_id(machine_ids, *source_machine)?;
        let mut next_value = identity_base
            .checked_add(1)
            .ok_or(LoweringError::Unsupported(
                "selected structural-result value identity range overflows",
            ))?;
        let scalar_parameters = realization
            .scalar_parameters
            .iter()
            .map(|parameter| {
                Ok(ValueDeclaration {
                    id: value_id(allocate_dense(&mut next_value)?),
                    scalar_type: terminal_scalar_type(parameter.primitive_type)?,
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let mut next_place = identity_base
            .checked_add(1)
            .ok_or(LoweringError::Unsupported(
                "selected structural-result place identity range overflows",
            ))?;
        let structural_parameters = lower_unit_parameters(
            std::slice::from_ref(&realization.structural_parameter),
            type_ids,
            &[],
            &mut next_place,
        )?;
        let [structural_parameter] = structural_parameters.as_slice() else {
            unreachable!("one checked structural parameter lowers to one Terminal parameter")
        };
        let structural_parameter_place = structural_parameter.place;
        let structural_parameter_position = structural_parameter.position;
        let result_place = place_id(allocate_dense(&mut next_place)?);
        let result_type = lookup_type_id(type_ids, &realization.result.type_identity)?;
        if structural_parameter.structural_type != result_type
            || structural_types
                .iter()
                .filter(|declaration| declaration.id == result_type)
                .count()
                != 1
        {
            return unsupported(
                "selected structural-result realization lost its exact structural type",
            );
        }
        let block = block_id(
            identity_base
                .checked_add(1)
                .ok_or(LoweringError::Unsupported(
                    "selected structural-result block identity range overflows",
                ))?,
        );
        machines.push(TerminalMachine {
            id: terminal_machine,
            attachment: Some(lookup_type_id(
                type_ids,
                &realization.attachment_type_identity,
            )?),
            parameters: scalar_parameters,
            structural_parameters,
            ranked_scc: None,
            result: TerminalMachineResult::Structural(StructuralResultDeclaration {
                place: result_place,
                structural_type: result_type,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }),
            structural_places: vec![
                StructuralPlaceDeclaration {
                    id: structural_parameter_place,
                    kind: StructuralPlaceKind::Parameter {
                        position: structural_parameter_position,
                        is_self: false,
                    },
                },
                StructuralPlaceDeclaration {
                    id: result_place,
                    kind: StructuralPlaceKind::Result,
                },
            ],
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block,
            blocks: vec![Block {
                id: block,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnStructural {
                    edge: edge_id(identity_base.checked_add(1).ok_or(
                        LoweringError::Unsupported(
                            "selected structural-result edge identity range overflows",
                        ),
                    )?),
                    source: structural_parameter_place,
                    returned_claims: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: contract_id(terminal_machine.get()),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        });
    }
    Ok(machines)
}
