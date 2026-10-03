//! Qualification creates a new SSA binding on an ordinary no-code edge.
//! The old source declaration remains unchanged, including when it is still
//! carried in the prefix. The final destination alone receives added or erased
//! tags; explicit source-cast replay owns permission for that change.
use super::{
    BlockId, LoweringError, MachineId, StructuralArgument, Terminator, ValueDeclaration,
    unsupported,
};
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use semantic_vocabulary::EdgeId;

#[allow(clippy::too_many_arguments)]
pub(crate) fn emit(
    successor: BlockId,
    edge: EdgeId,
    machine: MachineId,
    arguments: &[LoweredDirectExpression],
    structural_arguments: &[StructuralArgument],
    sources: &[ValueDeclaration],
    destinations: &[ValueDeclaration],
    sets: &[terminal_psi::ScalarQualificationSet],
    coercions: &mut Vec<terminal_psi::ScalarQualificationCoercion>,
) -> Result<Terminator, LoweringError> {
    if !structural_arguments.is_empty()
        || arguments.is_empty()
        || arguments.len() != destinations.len()
    {
        return unsupported("scalar qualification requires its exact scalar successor arguments");
    }
    let mut values = Vec::with_capacity(arguments.len());
    for (ordinal, (argument, destination)) in arguments.iter().zip(destinations).enumerate() {
        let position = match argument {
            LoweredDirectExpression::Parameter { position, .. }
            | LoweredDirectExpression::Local { position, .. } => *position,
            LoweredDirectExpression::Boolean { expression } => match expression.as_ref() {
                LoweredBooleanReturnExpression::Parameter { position }
                | LoweredBooleanReturnExpression::Local { position } => *position,
                _ => return unsupported("scalar qualification operand must already be evaluated"),
            },
            _ => return unsupported("scalar qualification operand must already be evaluated"),
        };
        let source = sources.get(position).ok_or(LoweringError::Unsupported(
            "scalar qualification source is absent",
        ))?;
        if source.scalar_type != argument.scalar_type()
            || source.scalar_type != destination.scalar_type
        {
            return unsupported("scalar qualification changed its operand carrier");
        }
        if ordinal + 1 != arguments.len() {
            if position != ordinal || source.qualifications != destination.qualifications {
                return unsupported("scalar qualification changed a carried prefix membership");
            }
        } else {
            let source_domains = if source.qualifications.is_empty() {
                &[][..]
            } else {
                sets.iter()
                    .find(|set| set.id == source.qualifications)
                    .map(|set| set.domains.as_slice())
                    .ok_or(LoweringError::Unsupported(
                        "scalar qualification input set is absent",
                    ))?
            };
            let destination_domains = if destination.qualifications.is_empty() {
                &[][..]
            } else {
                sets.iter()
                    .find(|set| set.id == destination.qualifications)
                    .map(|set| set.domains.as_slice())
                    .ok_or(LoweringError::Unsupported(
                        "scalar qualification output set is absent",
                    ))?
            };
            let introduction = source_domains.len() < destination_domains.len()
                && source_domains
                    .iter()
                    .all(|domain| destination_domains.binary_search(domain).is_ok());
            let erasure = destination_domains.len() < source_domains.len()
                && destination_domains
                    .iter()
                    .all(|domain| source_domains.binary_search(domain).is_ok());
            if source.id == destination.id || !(introduction || erasure) {
                return unsupported(
                    "scalar qualification must add or erase membership on a fresh value",
                );
            }
            coercions.push(terminal_psi::ScalarQualificationCoercion {
                machine,
                edge,
                argument_ordinal: u32::try_from(ordinal).map_err(|_| {
                    LoweringError::Unsupported("scalar qualification ordinal exceeds u32")
                })?,
                source: source.id,
                destination: destination.id,
            });
        }
        values.push(source.id);
    }
    Ok(Terminator::Jump {
        edge,
        target: successor,
        arguments: values,
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        residual_affine_discards: Vec::new(),
        trivial_affine_discards: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::super::{ScalarType, block_id, edge_id, value_id};
    use super::{LoweredDirectExpression, MachineId, Terminator, ValueDeclaration, emit};
    use semantic_vocabulary::{ScalarDomainId, ScalarQualificationSetId};

    fn declaration(identity: u64, set: u64) -> ValueDeclaration {
        ValueDeclaration {
            id: value_id(identity),
            scalar_type: ScalarType::Boolean,
            qualifications: ScalarQualificationSetId::new(set),
        }
    }

    #[test]
    fn qualification_emission_names_the_new_binding_not_the_original_value() {
        let sources = vec![declaration(1, 0), declaration(2, 0)];
        let destinations = vec![vec![], vec![declaration(3, 0), declaration(4, 1)]];
        let arguments = (0..2)
            .map(|position| LoweredDirectExpression::Parameter {
                position,
                scalar_type: ScalarType::Boolean,
            })
            .collect::<Vec<_>>();
        let mut catalog = terminal_psi::ScalarQualificationCatalog {
            sets: vec![terminal_psi::ScalarQualificationSet {
                id: ScalarQualificationSetId::new(1),
                domains: vec![ScalarDomainId::new(1).unwrap()],
            }],
            ..Default::default()
        };
        let terminator = emit(
            block_id(2),
            edge_id(7),
            MachineId::new(1).unwrap(),
            &arguments,
            &[],
            &sources,
            &destinations[1],
            &catalog.sets,
            &mut catalog.coercions,
        )
        .unwrap();
        assert!(
            matches!(terminator, Terminator::Jump { edge, target, arguments, .. }
            if edge == edge_id(7) && target == block_id(2) && arguments == vec![value_id(1), value_id(2)])
        );
        assert_eq!(catalog.coercions.len(), 1);
        assert_eq!(catalog.coercions[0].source, value_id(2));
        assert_eq!(catalog.coercions[0].destination, value_id(4));
        assert_eq!(catalog.coercions[0].argument_ordinal, 1);
        assert!(sources[1].qualifications.is_empty());

        let mut changed = destinations.clone();
        changed[1][0].qualifications = ScalarQualificationSetId::new(1);
        assert!(
            emit(
                block_id(2),
                edge_id(8),
                MachineId::new(1).unwrap(),
                &arguments,
                &[],
                &sources,
                &changed[1],
                &catalog.sets,
                &mut catalog.coercions,
            )
            .is_err()
        );
        let mut changed = sources.clone();
        changed[1].qualifications = ScalarQualificationSetId::new(1);
        assert!(
            emit(
                block_id(2),
                edge_id(8),
                MachineId::new(1).unwrap(),
                &arguments,
                &[],
                &changed,
                &destinations[1],
                &catalog.sets,
                &mut catalog.coercions,
            )
            .is_err()
        );
    }
}
