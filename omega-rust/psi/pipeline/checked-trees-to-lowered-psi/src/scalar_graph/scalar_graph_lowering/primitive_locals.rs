//! Scalar-local referents share the enclosing call closure's place/type namespace.

use super::*;

pub(crate) struct PrimitiveLocal {
    pub symbol: symbols::SymbolHandle,
    pub statement_ordinal: u32,
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
    pub scalar_type: ScalarType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StoreDestination {
    Initialize {
        place: PlaceId,
        structural_type: StructuralTypeId,
    },
    Assign {
        place: PlaceId,
    },
}

pub(crate) fn allocate(
    checked: &CheckedTrees,
    graph: &CheckedScalarMachineGraph,
    type_ids: &[(String, StructuralTypeId)],
    next_place: &mut u64,
) -> Result<Vec<PrimitiveLocal>, LoweringError> {
    let mut locals = Vec::new();
    for state in &graph.states {
        let (_, source) = source_custody::authored_state(checked, state.state)?;
        let statements = checked.statement_table.statements(source.statement_nodes);
        if state
            .primitive_locals
            .windows(2)
            .any(|pair| pair[0].statement_ordinal >= pair[1].statement_ordinal)
        {
            return unsupported("scalar primitive locals disagree with declaration order");
        }
        for local in &state.primitive_locals {
            let Some(checked_trees::statement::StatementNode::LocalData(authored)) =
                statements.get(local.statement_ordinal as usize)
            else {
                return unsupported("scalar primitive local lost its authored declaration");
            };
            if !local.symbol.is_valid()
                || locals
                    .iter()
                    .any(|prior: &PrimitiveLocal| prior.symbol == local.symbol)
                || authored.symbol != local.symbol
                || !authored.is_mutable
                || !checked
                    .expression_table
                    .expression_is_valid(authored.initial_value)
                || !matches!(
                    checked
                        .type_reference_table
                        .type_reference(authored.type_reference),
                    checked_trees::types::TypeReferenceNode::Named { .. }
                )
                || checked.primitive_type_reference(authored.type_reference)
                    != Some(local.primitive_type)
                || checked
                    .typed
                    .normalized_type_identity(authored.type_reference)
                    .into_string()
                    != local.type_identity
                || !state.bindings.iter().any(|binding| {
                    binding.statement_ordinal == local.statement_ordinal
                        && binding.primitive_type == local.primitive_type
                        && binding.destination
                            == checked_trees::CheckedScalarBindingDestination::StorageInitialize {
                                symbol: local.symbol,
                            }
                })
            {
                return unsupported("scalar primitive local substituted its declaration or type");
            }
            let mut shapes = checked
                .facts
                .flow
                .terminal_scalar_graphs
                .structural_types
                .iter()
                .filter(|shape| shape.identity == local.type_identity);
            let shape = shapes.next().ok_or(LoweringError::Unsupported(
                "scalar primitive local has no retained referent shape",
            ))?;
            if shapes.next().is_some()
                || shape.shape
                    != checked_trees::CheckedUnitStructuralTypeShape::PrimitiveScalar(
                        local.primitive_type,
                    )
            {
                return unsupported("scalar primitive local substituted its referent shape");
            }
            locals.push(PrimitiveLocal {
                symbol: local.symbol,
                statement_ordinal: local.statement_ordinal,
                place: place_id(allocate_dense(next_place)?),
                structural_type: lookup_type_id(type_ids, &local.type_identity)?,
                scalar_type: terminal_scalar_type(local.primitive_type)?,
            });
        }
    }
    Ok(locals)
}

pub(super) fn destination(
    locals: &[PrimitiveLocal],
    binding: &checked_trees::CheckedScalarBinding,
) -> Result<Option<StoreDestination>, LoweringError> {
    use checked_trees::CheckedScalarBindingDestination;
    let symbol = match binding.destination {
        CheckedScalarBindingDestination::Immutable => return Ok(None),
        CheckedScalarBindingDestination::StorageInitialize { symbol }
        | CheckedScalarBindingDestination::StorageAssign { symbol } => symbol,
    };
    let Some(local) = locals.iter().find(|local| local.symbol == symbol) else {
        return Ok(None);
    };
    if local.scalar_type != terminal_scalar_type(binding.primitive_type)? {
        return unsupported("scalar primitive store changes its referent type");
    }
    Ok(Some(match binding.destination {
        CheckedScalarBindingDestination::StorageInitialize { .. } => {
            if binding.statement_ordinal != local.statement_ordinal {
                return unsupported("scalar primitive establishment moved from its declaration");
            }
            StoreDestination::Initialize {
                place: local.place,
                structural_type: local.structural_type,
            }
        }
        CheckedScalarBindingDestination::StorageAssign { .. } => {
            if binding.statement_ordinal <= local.statement_ordinal {
                return unsupported("scalar primitive assignment precedes establishment");
            }
            StoreDestination::Assign { place: local.place }
        }
        CheckedScalarBindingDestination::Immutable => {
            return unsupported("primitive store has no mutable destination");
        }
    }))
}

pub(super) fn storage_before(
    locals: &[PrimitiveLocal],
    statement: u32,
) -> Vec<(symbols::SymbolHandle, PlaceId, ScalarType)> {
    locals
        .iter()
        .filter(|local| local.statement_ordinal < statement)
        .map(|local| (local.symbol, local.place, local.scalar_type))
        .collect()
}
