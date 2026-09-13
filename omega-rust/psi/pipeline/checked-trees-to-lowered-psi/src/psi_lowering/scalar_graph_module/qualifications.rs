//! Qualification creates a new SSA binding on an ordinary no-code edge.
//! The old source declaration remains unchanged, including when it is still
//! carried in the prefix. The final destination alone receives added or erased
//! tags; explicit source-cast replay owns permission for that change.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn emit(
    target: usize,
    arguments: &[LoweredDirectExpression],
    structural_arguments: &[StructuralArgument],
    sources: &[ValueDeclaration],
    state_parameters: &[Vec<ValueDeclaration>],
    machine: MachineId,
    identity_base: u64,
    next_edge_identity: &mut u64,
    catalog: &mut terminal_psi::ScalarQualificationCatalog,
) -> Result<Terminator, LoweringError> {
    let destinations = state_parameters
        .get(target)
        .ok_or(LoweringError::Unsupported(
            "scalar qualification has no successor declarations",
        ))?;
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
                catalog
                    .sets
                    .iter()
                    .find(|set| set.id == source.qualifications)
                    .map(|set| set.domains.as_slice())
                    .ok_or(LoweringError::Unsupported(
                        "scalar qualification input set is absent",
                    ))?
            };
            let destination_domains = if destination.qualifications.is_empty() {
                &[][..]
            } else {
                catalog
                    .sets
                    .iter()
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
            catalog
                .coercions
                .push(terminal_psi::ScalarQualificationCoercion {
                    machine,
                    edge: edge_id(*next_edge_identity),
                    argument_ordinal: u32::try_from(ordinal).map_err(|_| {
                        LoweringError::Unsupported("scalar qualification ordinal exceeds u32")
                    })?,
                    source: source.id,
                    destination: destination.id,
                });
        }
        values.push(source.id);
    }
    let edge = edge_id(*next_edge_identity);
    *next_edge_identity = next_edge_identity
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "scalar qualification edge identity overflow",
        ))?;
    Ok(Terminator::Jump {
        edge,
        target: scalar_source_block(identity_base, target),
        arguments: values,
        structural_arguments: Vec::new(),
        residual_affine_discards: Vec::new(),
        trivial_affine_discards: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let mut next_edge = 7;
        let terminator = emit(
            1,
            &arguments,
            &[],
            &sources,
            &destinations,
            MachineId::new(1).unwrap(),
            0,
            &mut next_edge,
            &mut catalog,
        )
        .unwrap();
        assert!(
            matches!(terminator, Terminator::Jump { edge, target, arguments, .. }
            if edge == edge_id(7) && target == block_id(2) && arguments == vec![value_id(1), value_id(2)])
        );
        assert_eq!(next_edge, 8);
        assert_eq!(catalog.coercions.len(), 1);
        assert_eq!(catalog.coercions[0].source, value_id(2));
        assert_eq!(catalog.coercions[0].destination, value_id(4));
        assert_eq!(catalog.coercions[0].argument_ordinal, 1);
        assert!(sources[1].qualifications.is_empty());

        let mut changed = destinations.clone();
        changed[1][0].qualifications = ScalarQualificationSetId::new(1);
        assert!(
            emit(
                1,
                &arguments,
                &[],
                &sources,
                &changed,
                MachineId::new(1).unwrap(),
                0,
                &mut next_edge,
                &mut catalog
            )
            .is_err()
        );
        let mut changed = sources.clone();
        changed[1].qualifications = ScalarQualificationSetId::new(1);
        assert!(
            emit(
                1,
                &arguments,
                &[],
                &changed,
                &destinations,
                MachineId::new(1).unwrap(),
                0,
                &mut next_edge,
                &mut catalog
            )
            .is_err()
        );
    }
}
