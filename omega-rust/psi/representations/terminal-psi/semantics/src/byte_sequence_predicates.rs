//! Primitive byte predicates replay from exact values and declared invariants.
//! These queries grant no availability or loan authority: receiving validation
//! separately checks dominance, source access and current length observations.

use semantic_vocabulary::{PlaceId, StructuralPlaceKind};
use terminal_psi::{
    ByteSequencePredicate, ByteSequencePredicateSet, OperationKind, StructuralAccess,
    StructuralParameterDeclaration, StructuralTypeShape, TerminalMachine, TerminalModule,
};

/// The exact plain byte predicates carried by a formal. Routed/content domains
/// need their own evidence and cannot be treated as byte classifications.
pub fn byte_parameter_predicates(
    module: &TerminalModule,
    parameter: &StructuralParameterDeclaration,
) -> Option<ByteSequencePredicateSet> {
    byte_parameter_predicates_from_domains(&module.structural_domains, parameter)
}

pub fn byte_parameter_predicates_from_domains(
    domains: &[terminal_psi::StructuralDomainDeclaration],
    parameter: &StructuralParameterDeclaration,
) -> Option<ByteSequencePredicateSet> {
    byte_parameter_predicates_in(domains.iter(), parameter)
}

pub fn byte_parameter_predicates_in<'domain>(
    domains: impl Iterator<Item = &'domain terminal_psi::StructuralDomainDeclaration> + Clone,
    parameter: &StructuralParameterDeclaration,
) -> Option<ByteSequencePredicateSet> {
    if !parameter.projected_qualifications.is_empty() {
        return None;
    }
    let mut predicates = ByteSequencePredicateSet::EMPTY;
    for identity in &parameter.qualifications {
        let domain = domains.clone().find(|domain| domain.id == *identity)?;
        if domain.carrier != parameter.structural_type
            || domain.content_projection.is_some()
            || !domain.establishment_routes.is_empty()
        {
            return None;
        }
        predicates.insert(domain.byte_predicate?);
    }
    Some(predicates)
}

/// Reconstruct a whole source's value predicates. Owned concatenations are
/// immutable materializations; their inputs' declaration invariants must hold
/// at construction, not at the later destination mutation.
pub fn byte_sequence_source_predicates(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: PlaceId,
) -> Option<ByteSequencePredicateSet> {
    fn source_predicates(
        module: &TerminalModule,
        machine: &TerminalMachine,
        source: PlaceId,
        fuel: usize,
        memo: &mut Vec<(PlaceId, Option<ByteSequencePredicateSet>)>,
    ) -> Option<ByteSequencePredicateSet> {
        if let Some((_, result)) = memo.iter().find(|(place, _)| *place == source) {
            return *result;
        }
        let fuel = fuel.checked_sub(1)?;
        let place = machine
            .structural_places
            .iter()
            .find(|place| place.id == source)?;
        let result = (|| match place.kind {
            StructuralPlaceKind::Parameter { position, is_self } => {
                let parameter = machine.structural_parameters.iter().find(|parameter| {
                    parameter.place == source
                        && parameter.position == position
                        && parameter.is_self == is_self
                })?;
                if !matches!(
                    parameter.access,
                    StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                ) {
                    return None;
                }
                byte_parameter_predicates(module, parameter)
            }
            StructuralPlaceKind::ByteSequenceLiteral { .. } => {
                let mut producers = machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .filter_map(|operation| match &operation.kind {
                        OperationKind::EstablishByteSequenceLiteral {
                            destination, bytes, ..
                        } if *destination == source => Some(bytes),
                        _ => None,
                    });
                let bytes = producers.next()?;
                if producers.next().is_some() {
                    return None;
                }
                let mut predicates = ByteSequencePredicateSet::EMPTY;
                for predicate in ByteSequencePredicate::ALL {
                    if predicate.holds_for(bytes) {
                        predicates.insert(predicate);
                    }
                }
                Some(predicates)
            }
            StructuralPlaceKind::OperationResult { producer, .. } => {
                let operation = machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .find(|operation| operation.id == producer)?;
                if operation.result.structural()?.place != source {
                    return None;
                }
                let OperationKind::EstablishByteSequenceConcatenation { left, right, .. } =
                    &operation.kind
                else {
                    return None;
                };
                let mut operand_predicates =
                    |operand: &terminal_psi::control_flow::ByteSequenceOperand| match operand {
                        terminal_psi::control_flow::ByteSequenceOperand::Whole {
                            source, ..
                        } => source_predicates(module, machine, *source, fuel, memo),
                        terminal_psi::control_flow::ByteSequenceOperand::Field {
                            source,
                            field,
                            ..
                        } => {
                            let parameter = machine
                                .structural_parameters
                                .iter()
                                .find(|parameter| parameter.place == source.place)?;
                            if source.access != StructuralAccess::SharedBorrow
                                || !matches!(
                                    parameter.access,
                                    StructuralAccess::SharedBorrow
                                        | StructuralAccess::MutableBorrow
                                )
                            {
                                return None;
                            }
                            let declaration = crate::runtime_structural_path_tip(
                                module.structural_types.iter(),
                                parameter.structural_type,
                                &source.path,
                            )?;
                            let StructuralTypeShape::Record { fields } = &declaration.shape else {
                                return None;
                            };
                            let field = fields.iter().find(|candidate| {
                                candidate.id == *field && !candidate.relevance.is_erased()
                            })?;
                            field.field_type.byte_sequence_carrier()?;
                            Some(field.field_type.byte_predicates())
                        }
                    };
                Some(operand_predicates(left)?.concatenation(operand_predicates(right)?))
            }
            _ => None,
        })();
        memo.push((source, result));
        result
    }
    source_predicates(
        module,
        machine,
        source,
        machine.structural_places.len().saturating_add(1),
        &mut Vec::new(),
    )
}

/// Literal qualification rosters are claims about these exact octets, never
/// trusted annotations. Unsupported nominal qualifications cannot be minted.
pub fn byte_literal_qualifications_hold(
    domains: &[terminal_psi::StructuralDomainDeclaration],
    carrier: semantic_vocabulary::StructuralTypeId,
    qualifications: &[semantic_vocabulary::StructuralDomainId],
    bytes: &[u8],
) -> bool {
    byte_literal_qualifications_hold_in(domains.iter(), carrier, qualifications, bytes)
}

pub fn byte_literal_qualifications_hold_in<'domain>(
    domains: impl Iterator<Item = &'domain terminal_psi::StructuralDomainDeclaration> + Clone,
    carrier: semantic_vocabulary::StructuralTypeId,
    qualifications: &[semantic_vocabulary::StructuralDomainId],
    bytes: &[u8],
) -> bool {
    !qualifications.windows(2).any(|pair| pair[0] >= pair[1])
        && qualifications.iter().all(|identity| {
            domains
                .clone()
                .find(|domain| domain.id == *identity)
                .is_some_and(|domain| {
                    domain.carrier == carrier
                        && domain.content_projection.is_none()
                        && domain.establishment_routes.is_empty()
                        && domain
                            .byte_predicate
                            .is_some_and(|predicate| predicate.holds_for(bytes))
                })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{DomainSemanticId, StructuralDomainId, StructuralTypeId};
    use terminal_psi::{StructuralDomainDeclaration, StructuralEstablishmentRoute};

    #[test]
    fn literal_membership_replays_octets_carrier_and_nonrouted_definition() {
        let carrier = StructuralTypeId::new(1).unwrap();
        let identity = StructuralDomainId::new(1).unwrap();
        let mut domains = [StructuralDomainDeclaration {
            id: identity,
            semantic_domain: DomainSemanticId::new(1).unwrap(),
            identity: "bytes".into(),
            carrier,
            content_projection: None,
            byte_predicate: Some(ByteSequencePredicate::ValidUtf8),
            establishment_routes: Vec::new(),
        }];
        assert!(byte_literal_qualifications_hold(
            &domains,
            carrier,
            &[identity],
            "é".as_bytes()
        ));
        assert!(!byte_literal_qualifications_hold(
            &domains,
            carrier,
            &[identity],
            &[0xff]
        ));
        assert!(!byte_literal_qualifications_hold(
            &domains,
            StructuralTypeId::new(2).unwrap(),
            &[identity],
            b"ok"
        ));
        assert!(!byte_literal_qualifications_hold(
            &domains,
            carrier,
            &[identity, identity],
            b"ok"
        ));
        domains[0]
            .establishment_routes
            .push(StructuralEstablishmentRoute::ExactMachine {
                machine: "issuer".into(),
            });
        assert!(!byte_literal_qualifications_hold(
            &domains,
            carrier,
            &[identity],
            b"ok"
        ));
        domains[0].establishment_routes.clear();
        domains[0].byte_predicate = None;
        assert!(!byte_literal_qualifications_hold(
            &domains,
            carrier,
            &[identity],
            b"ok"
        ));
    }
}
