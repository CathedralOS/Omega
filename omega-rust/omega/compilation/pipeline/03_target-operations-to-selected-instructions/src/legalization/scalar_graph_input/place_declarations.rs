//! Rejoin each structural declaration to its parameter, result or local producer.
//! Storage families compose in one graph; whole-unit custody separately checks
//! uniqueness, dominance, loans and the current ownership frontier.
use super::{AbstractOperation, PsiOptimizationFunction};
use semantic_vocabulary::StructuralPlaceKind;

pub(super) fn matches_producers(function: &PsiOptimizationFunction) -> bool {
    function.declared_places
        == function
            .structural_places
            .iter()
            .filter(|place| !matches!(place.kind, StructuralPlaceKind::ProviderAttachment { .. }))
            .map(|place| place.id)
            .collect::<std::collections::BTreeSet<_>>()
        && function.structural_places.iter().all(|place| {
            // Provider attachments are semantic specialization witnesses, not
            // receiver storage. Whole-unit custody checks the exact erased
            // field, boundary and service; they must compose with runtime
            // parameters and results without entering the declared-place set.
            if let StructuralPlaceKind::ProviderAttachment { attachment, .. } = place.kind {
                return function.attachment == Some(attachment);
            }
            if matches!(place.kind, StructuralPlaceKind::ByteSequenceLiteral { .. }) {
                return super::literals::declaration_producer(function, place.id).is_some();
            }
            // An empty-record local is justified by its one exact
            // establishment of the type the place declares.
            if let StructuralPlaceKind::TrivialAffineLocal {
                structural_type, ..
            } = place.kind
            {
                return function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.nodes)
                    .filter(|node| {
                        matches!(&node.operation,
                            AbstractOperation::EstablishTrivialAffineLocal {
                                place: established,
                                structural_type: declaration,
                                ..
                            } if *established == *place && declaration.id == structural_type)
                    })
                    .count()
                    == 1;
            }
            // One graph may own primitive storage alongside aggregate results.
            // Keep each place joined to its exact producer; the primitive input
            // validator separately checks the declared type and initializer.
            if let Some((producer, result, _)) =
                super::primitive_locals::producer(function, place.id)
            {
                return place.kind
                    == StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type: result.structural_type,
                    }
                    && super::primitive_locals::valid_result(function, producer, result);
            }
            if place.kind == StructuralPlaceKind::Result {
                return function
                    .result
                    .structural()
                    .is_some_and(|result| result.place == place.id);
            }
            if let Ok((producer, result)) =
                super::structural_case::source_result(function, place.id)
            {
                return place.kind
                    == StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type: result.structural_type,
                    };
            }
            function.structural_parameters.iter().any(|parameter| {
                parameter.place == place.id
                    && place.kind
                        == StructuralPlaceKind::Parameter {
                            position: parameter.position,
                        }
            }) || function.blocks.iter().any(|block| {
                block.structural_parameters.iter().any(|parameter| {
                    parameter.place == place.id
                        && place.kind
                            == StructuralPlaceKind::BlockParameter {
                                block: block.id,
                                position: parameter.position,
                            }
                })
            })
        })
}
