//! Replay declared byte invariants against the current optimized operands.
//! A retained capacity fact cannot authorize replacing a qualified destination
//! with a different source of equal length but incompatible byte content.
use abstract_operations::AbstractOperation as Operation;
use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
use semantic_vocabulary::{PlaceId, StructuralPlaceKind};
use terminal_psi::{
    ByteSequencePredicate, ByteSequencePredicateSet, StructuralAccess, StructuralTypeShape,
};

pub(crate) fn replacement_preserves_predicates(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    operation: &Operation,
) -> bool {
    let (source, required) = match operation {
        Operation::ByteSequenceStore {
            destination,
            source,
            ..
        } => {
            let Some(parameter) = function
                .structural_parameters
                .iter()
                .find(|parameter| parameter.place == *destination)
            else {
                return false;
            };
            (
                *source,
                terminal_semantics::byte_parameter_predicates_from_domains(
                    &unit.structural_domains,
                    parameter,
                ),
            )
        }
        Operation::StructuralByteSequenceFieldStore {
            destination,
            path,
            field,
            source,
            ..
        } => (
            *source,
            field_predicates(unit, function, *destination, path, *field),
        ),
        _ => return false,
    };
    let Some(required) = required else {
        return false;
    };
    required.is_empty()
        || source_predicates(
            unit,
            function,
            source,
            function.structural_places.len().saturating_add(1),
            &mut Vec::new(),
        )
        .is_some_and(|available| available.implies(required))
}

fn field_predicates(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    root: PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    field: semantic_vocabulary::StructuralFieldId,
) -> Option<ByteSequencePredicateSet> {
    let parameter = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == root)?;
    let declaration = terminal_semantics::runtime_structural_path_tip(
        unit.structural_types.iter(),
        parameter.structural_type,
        path,
    )?;
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return None;
    };
    let field = fields
        .iter()
        .find(|candidate| candidate.id == field && !candidate.relevance.is_erased())?;
    field.field_type.byte_sequence_carrier()?;
    Some(field.field_type.byte_predicates())
}

fn source_predicates(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    source: PlaceId,
    remaining: usize,
    memo: &mut Vec<(PlaceId, Option<ByteSequencePredicateSet>)>,
) -> Option<ByteSequencePredicateSet> {
    if let Some((_, predicates)) = memo.iter().find(|(place, _)| *place == source) {
        return *predicates;
    }
    let remaining = remaining.checked_sub(1)?;
    // A completed source may be reused by both operands of many later concats.
    // Reserve its unknown result before walking children: each source is replayed
    // once, and an in-progress revisit rejects a cycle without recursive expansion.
    let index = memo.len();
    memo.push((source, None));
    let result = derive_source_predicates(unit, function, source, remaining, memo);
    memo[index].1 = result;
    result
}

fn derive_source_predicates(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    source: PlaceId,
    remaining: usize,
    memo: &mut Vec<(PlaceId, Option<ByteSequencePredicateSet>)>,
) -> Option<ByteSequencePredicateSet> {
    let declaration = function
        .structural_places
        .iter()
        .find(|place| place.id == source)?;
    match declaration.kind {
        StructuralPlaceKind::Parameter { position, is_self } => {
            let parameter = function.structural_parameters.iter().find(|parameter| {
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
            terminal_semantics::byte_parameter_predicates_from_domains(
                &unit.structural_domains,
                parameter,
            )
        }
        StructuralPlaceKind::ByteSequenceLiteral { .. } => {
            let mut producers = function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .filter_map(|node| match &node.operation {
                    Operation::EstablishByteSequenceLiteral { place, bytes, .. }
                        if place.id == source =>
                    {
                        Some(bytes)
                    }
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
        StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } => {
            let node = function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .find(|node| node.operation.psi_operation() == Some(producer))?;
            let Operation::EstablishByteSequenceConcatenation {
                result,
                left,
                right,
                ..
            } = &node.operation
            else {
                return None;
            };
            if result.place != source || result.structural_type != structural_type {
                return None;
            }
            let mut operand =
                |operand: &terminal_psi::control_flow::ByteSequenceOperand| match operand {
                    terminal_psi::control_flow::ByteSequenceOperand::Whole { source, .. } => {
                        source_predicates(unit, function, *source, remaining, memo)
                    }
                    terminal_psi::control_flow::ByteSequenceOperand::Field {
                        source,
                        field,
                        ..
                    } => {
                        if source.access != StructuralAccess::SharedBorrow {
                            return None;
                        }
                        field_predicates(unit, function, source.place, &source.path, *field)
                    }
                };
            Some(operand(left)?.concatenation(operand(right)?))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{ObligationId, OperationId, StructuralTypeId, ValueId};
    use terminal_psi::control_flow::ByteSequenceOperand;

    // This raw graph exercises predicate traversal only. Forty doubling nodes
    // describe exponentially many paths while retaining forty-one source places.
    fn shared_concatenations() -> PsiOptimizationUnit {
        let mut unit = crate::tests::fixtures::scalar_units::unit();
        let function = &mut unit.functions[0];
        let template = function.blocks[0].nodes[0].clone();
        function.blocks[0].nodes.clear();
        let structural_type = StructuralTypeId::new(1).unwrap();
        let declaration = terminal_psi::StructuralTypeDeclaration {
            id: structural_type,
            identity: "bytes".into(),
            shape: StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity: 1 << 40 },
            ),
        };
        let literal = terminal_psi::StructuralPlaceDeclaration {
            id: PlaceId::new(1).unwrap(),
            kind: StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal: 0,
                structural_type,
            },
        };
        function.structural_places.push(literal);
        let mut node = template.clone();
        node.operation = Operation::EstablishByteSequenceLiteral {
            psi_operation: OperationId::new(1).unwrap(),
            place: literal,
            structural_type: declaration,
            bytes: b"A".to_vec(),
            qualifications: Vec::new(),
        };
        function.blocks[0].nodes.push(node);
        for identity in 2..=41 {
            let place = PlaceId::new(identity).unwrap();
            let producer = OperationId::new(identity).unwrap();
            function
                .structural_places
                .push(terminal_psi::StructuralPlaceDeclaration {
                    id: place,
                    kind: StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type,
                    },
                });
            let operand = ByteSequenceOperand::Whole {
                source: PlaceId::new(identity - 1).unwrap(),
                length: ValueId::new(1).unwrap(),
            };
            let mut node = template.clone();
            node.operation = Operation::EstablishByteSequenceConcatenation {
                psi_operation: producer,
                left: operand.clone(),
                right: operand,
                capacity_obligation: ObligationId::new(1).unwrap(),
                result: terminal_psi::StructuralOperationResult {
                    place,
                    structural_type,
                    multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    qualification_establishments: Vec::new(),
                    claims: Vec::new(),
                },
            };
            function.blocks[0].nodes.push(node);
        }
        unit
    }

    #[test]
    fn shared_concatenation_predicates_visit_each_source_once() {
        let unit = shared_concatenations();
        let function = &unit.functions[0];
        let mut memo = Vec::new();
        let predicates =
            source_predicates(&unit, function, PlaceId::new(41).unwrap(), 42, &mut memo).unwrap();
        assert!(predicates.holds_for(b"AAAA"));
        assert!(
            predicates
                .predicates()
                .any(|predicate| predicate == ByteSequencePredicate::ValidUtf8)
        );
        assert_eq!(memo.len(), 41);
        let cached = memo.clone();
        assert_eq!(
            source_predicates(&unit, function, PlaceId::new(41).unwrap(), 42, &mut memo),
            Some(predicates)
        );
        assert_eq!(memo, cached);
    }

    #[test]
    fn shared_concatenation_predicates_reject_cycles_and_unknown_sources() {
        for corrupted_source in [2, 1000] {
            let mut unit = shared_concatenations();
            let Operation::EstablishByteSequenceConcatenation { left, .. } =
                &mut unit.functions[0].blocks[0].nodes[1].operation
            else {
                panic!("concat");
            };
            *left = ByteSequenceOperand::Whole {
                source: PlaceId::new(corrupted_source).unwrap(),
                length: ValueId::new(1).unwrap(),
            };
            let mut memo = Vec::new();
            assert_eq!(
                source_predicates(
                    &unit,
                    &unit.functions[0],
                    PlaceId::new(41).unwrap(),
                    42,
                    &mut memo
                ),
                None
            );
            assert!(memo.len() <= 41);
        }
    }
}
