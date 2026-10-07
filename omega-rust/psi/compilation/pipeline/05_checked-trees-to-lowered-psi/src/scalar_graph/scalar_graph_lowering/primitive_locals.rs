//! Scalar-local referents share the enclosing call closure's place/type namespace.

use super::super::{StructuralTypeId, allocate_dense, lookup_type_id, place_id};
use super::{
    CheckedScalarMachineGraph, CheckedTrees, LoweringError, PlaceId, ScalarType,
    terminal_scalar_type, unsupported,
};
use crate::emission::store_destination::StoreDestination;
pub(crate) struct PrimitiveLocal {
    pub symbol: symbols::SymbolHandle,
    pub statement_ordinal: u32,
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
    pub scalar_type: ScalarType,
    pub is_mutable: bool,
}

pub(crate) fn allocate(
    checked: &CheckedTrees,
    graph: &CheckedScalarMachineGraph,
    type_ids: &[(String, StructuralTypeId)],
    next_place: &mut u64,
) -> Result<Vec<PrimitiveLocal>, LoweringError> {
    let mut locals = Vec::new();
    for state in &graph.states {
        let (_, source) = crate::declarations::state_declaration(checked, state.state)?;
        let statements = checked.statement_table.statements(source.statement_nodes);
        let mut originals = state.primitive_locals.clone();
        for argument in crate::expression_preparation::computation_graph::borrowed_arguments(
            checked,
            state.state,
            None,
        )? {
            let Some(facts::PlaceRoot::Symbol(symbol)) = argument.structural_storage_root() else {
                continue;
            };
            if originals.iter().any(|original| original.symbol == symbol) {
                continue;
            }
            let Some((ordinal, local)) = checked.typed.state_local_declaration(source, symbol)
            else {
                continue;
            };
            let Some(primitive_type) = checked.primitive_type_reference(local.type_reference)
            else {
                continue;
            };
            originals.push(checked_trees::CheckedScalarPrimitiveLocalPlan {
                statement_ordinal: u32::try_from(ordinal).map_err(|_| {
                    LoweringError::Unsupported("primitive original statement exceeds u32")
                })?,
                symbol,
                primitive_type,
                type_identity: checked
                    .typed
                    .normalized_type_identity(local.type_reference)
                    .into_string(),
            });
        }
        originals.sort_by_key(|original| original.statement_ordinal);
        if state
            .primitive_locals
            .windows(2)
            .any(|pair| pair[0].statement_ordinal >= pair[1].statement_ordinal)
        {
            return unsupported("scalar primitive locals disagree with declaration order");
        }
        for local in &originals {
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
                            == if authored.is_mutable {
                                checked_trees::CheckedScalarBindingDestination::StorageInitialize {
                                    symbol: local.symbol,
                                }
                            } else {
                                checked_trees::CheckedScalarBindingDestination::Immutable
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
                is_mutable: authored.is_mutable,
            });
        }
    }
    Ok(locals)
}

pub(super) fn destination(
    locals: &[PrimitiveLocal],
    storage: &[(symbols::SymbolHandle, PlaceId, ScalarType)],
    authored: Option<&checked_trees::statement::StatementNode>,
    binding: &checked_trees::CheckedScalarBinding,
) -> Result<Option<StoreDestination>, LoweringError> {
    use checked_trees::CheckedScalarBindingDestination;
    let symbol = match binding.destination {
        CheckedScalarBindingDestination::Immutable => {
            let Some(checked_trees::statement::StatementNode::LocalData(local)) = authored else {
                return Ok(None);
            };
            let Some(home) = locals.iter().find(|home| home.symbol == local.symbol) else {
                return Ok(None);
            };
            if home.is_mutable
                || home.statement_ordinal != binding.statement_ordinal
                || home.scalar_type != terminal_scalar_type(binding.primitive_type)?
            {
                return unsupported("immutable primitive address demand changes its declaration");
            }
            return Ok(Some(StoreDestination::Initialize {
                place: home.place,
                structural_type: home.structural_type,
            }));
        }
        CheckedScalarBindingDestination::StorageInitialize { symbol }
        | CheckedScalarBindingDestination::StorageAssign { symbol } => symbol,
    };
    if matches!(
        binding.destination,
        CheckedScalarBindingDestination::StorageAssign { .. }
    ) {
        let mut homes = storage.iter().filter(|row| row.0 == symbol);
        if let Some((_, place, scalar_type)) = homes.next() {
            if homes.next().is_some()
                || *scalar_type != terminal_scalar_type(binding.primitive_type)?
            {
                return unsupported("primitive assignment changes its original home or carrier");
            }
            return Ok(Some(StoreDestination::Assign { place: *place }));
        }
    }
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
