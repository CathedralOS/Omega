use super::*;

pub(crate) fn validate_byte_sequence_literal_witnesses(
    function: &PsiOptimizationFunction,
    expected_literals: &[(
        terminal_psi::StructuralPlaceDeclaration,
        terminal_psi::StructuralTypeDeclaration,
    )],
) -> Result<(), OptimizationUnitValidationError> {
    let mut expected = expected_literals
        .iter()
        .map(|(place, structural_type)| (place.id, (*place, structural_type)))
        .collect::<BTreeMap<_, _>>();
    let mut actual = 0_usize;
    for node in function.blocks.iter().flat_map(|block| &block.nodes) {
        let O::EstablishByteSequenceLiteral {
            place,
            structural_type,
            ..
        } = &node.operation
        else {
            continue;
        };
        actual += 1;
        if expected
            .remove(&place.id)
            .is_none_or(|(expected_place, expected_type)| {
                *place != expected_place || structural_type != expected_type
            })
        {
            return Err(
                OptimizationUnitValidationError::ByteSequenceLiteralEstablishmentMismatch(
                    function.machine,
                ),
            );
        }
    }
    if actual != expected_literals.len() || !expected.is_empty() {
        return Err(
            OptimizationUnitValidationError::ByteSequenceLiteralEstablishmentMismatch(
                function.machine,
            ),
        );
    }
    Ok(())
}

pub(crate) fn validate_trivial_affine_local_witnesses(
    function: &PsiOptimizationFunction,
    expected_locals: &[(
        terminal_psi::StructuralPlaceDeclaration,
        terminal_psi::StructuralTypeDeclaration,
    )],
) -> Result<(), OptimizationUnitValidationError> {
    let explicit = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter_map(|node| match &node.operation {
            O::EstablishTrivialAffineLocal {
                psi_operation,
                place,
                structural_type,
            } => Some((*psi_operation, *place, structural_type)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let structural_returns = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter_map(|node| match &node.operation {
            O::ReturnStructural {
                trivial_affine_locals,
                trivial_affine_discards,
                ..
            } => Some((trivial_affine_locals, trivial_affine_discards)),
            _ => None,
        })
        .collect::<Vec<_>>();

    if !explicit.is_empty() {
        let exact = structural_returns.is_empty()
            && explicit.len() == expected_locals.len()
            && explicit.iter().zip(expected_locals).all(
                |((_, actual_place, actual_type), (expected_place, expected_type))| {
                    actual_place == expected_place && *actual_type == expected_type
                },
            );
        if !exact {
            return Err(
                OptimizationUnitValidationError::TrivialAffineLocalEstablishmentMismatch(
                    function.machine,
                ),
            );
        }
        return Ok(());
    }

    if !expected_locals.is_empty() && structural_returns.len() != 1 {
        return Err(
            OptimizationUnitValidationError::TrivialAffineLocalEstablishmentMismatch(
                function.machine,
            ),
        );
    }

    let executable_operations = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter(|node| !matches!(node.operation, O::ReturnStructural { .. }))
        .flat_map(|node| expected_provenance(&node.operation))
        .filter_map(|site| match site {
            PsiProvenance::Operation(operation) => Some(operation),
            PsiProvenance::Edge(_) => None,
        })
        .collect::<BTreeSet<_>>();

    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            let O::ReturnStructural {
                source,
                trivial_affine_locals,
                trivial_affine_discards,
                ..
            } = &node.operation
            else {
                continue;
            };
            if trivial_affine_locals.is_empty()
                && trivial_affine_discards.is_empty()
                && expected_locals.is_empty()
            {
                continue;
            }
            let node_index = u32::try_from(node_index).expect("unit node index fits u32");
            let mut hidden_operations = BTreeSet::new();
            if trivial_affine_locals.len() != expected_locals.len()
                || trivial_affine_locals.iter().zip(expected_locals).any(
                    |((operation, actual_place, actual_type), (expected_place, expected_type))| {
                        actual_place != expected_place
                            || actual_type != expected_type
                            || !hidden_operations.insert(*operation)
                            || executable_operations.contains(operation)
                    },
                )
            {
                return Err(
                    OptimizationUnitValidationError::StructuralReturnTrivialAffineLocalsMismatch {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    },
                );
            }

            let Some(returned_parameter) = function.structural_parameters.first() else {
                return Err(
                    OptimizationUnitValidationError::StructuralReturnTrivialAffineShapeMismatch {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    },
                );
            };
            let Some(result) = function.result.structural() else {
                return Err(
                    OptimizationUnitValidationError::StructuralReturnTrivialAffineShapeMismatch {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    },
                );
            };
            if !function.parameters.is_empty()
                || returned_parameter.place != *source
                || returned_parameter.is_self
                || returned_parameter.multiplicity != terminal_psi::StructuralMultiplicity::Linear
                || result.multiplicity != terminal_psi::StructuralMultiplicity::Linear
                || returned_parameter.structural_type != result.structural_type
                || returned_parameter.qualifications != result.qualifications
                || returned_parameter.projected_qualifications != result.projected_qualifications
                || returned_parameter.place == result.place
                || function
                    .structural_parameters
                    .iter()
                    .skip(1)
                    .any(|parameter| {
                        parameter.is_self
                            || parameter.multiplicity
                                != terminal_psi::StructuralMultiplicity::Affine
                            || !parameter.qualifications.is_empty()
                            || !parameter.projected_qualifications.is_empty()
                    })
            {
                return Err(
                    OptimizationUnitValidationError::StructuralReturnTrivialAffineShapeMismatch {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    },
                );
            }
            let expected_discards = trivial_affine_locals
                .iter()
                .rev()
                .map(|(_, local, _)| local.id)
                .chain(
                    function
                        .structural_parameters
                        .iter()
                        .skip(1)
                        .rev()
                        .map(|parameter| parameter.place),
                )
                .collect::<Vec<_>>();
            if *trivial_affine_discards != expected_discards {
                return Err(
                    OptimizationUnitValidationError::StructuralReturnAffineDiscardsMismatch {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    },
                );
            }
        }
    }
    Ok(())
}
