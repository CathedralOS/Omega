//! Address demand establishes the original scalar ingress, independently of
//! whether its first loan is a computed argument or an explicit reference.
//! Entry initialization dominates the ordinary operation sequence; every later
//! read, assignment and loan uses the same published home. Qualified ingress
//! requires qualification-bearing storage custody, not a stripped scalar copy.

use super::{
    CheckedTrees, LoweringError, StructuralTypeDeclaration, terminal_scalar_type, unsupported,
};
use crate::emission::operation_emission::{
    LoweredScalarBinding, expressions::LoweredDirectExpression,
};
use crate::emission::store_destination::StoreDestination;
use crate::expression_preparation::{
    bindings::ScalarBindings, computation_graph::borrowed_arguments,
};
use crate::terminal_identities::{allocate_dense, place_id};
use checked_trees::CheckedUnitStructuralArgumentSourcePlan;

pub(super) struct Homes {
    pub(super) establishments: Vec<LoweredScalarBinding>,
    pub(super) storage: Vec<(
        symbols::SymbolHandle,
        semantic_vocabulary::PlaceId,
        semantic_vocabulary::ScalarType,
    )>,
}

pub(super) fn establish(
    checked: &CheckedTrees,
    state: &checked_trees::CheckedScalarStateGraph,
    types: &[StructuralTypeDeclaration],
    parameter_types: &[semantic_vocabulary::QualifiedScalarType],
    next_place: &mut u64,
    bindings: &mut ScalarBindings,
) -> Result<Homes, LoweringError> {
    let mut demands = Vec::new();
    let mut demand = |source: &checked_trees::CheckedUnitStructuralArgumentPlan| {
        if let CheckedUnitStructuralArgumentSourcePlan::ScalarParameter { parameter_index } =
            source.source
            && !demands.contains(&parameter_index)
        {
            demands.push(parameter_index);
        }
    };
    for argument in borrowed_arguments(checked, state.state, None)? {
        demand(argument);
    }
    for operation in &state.unit_operations {
        if let Some(call) = operation.call() {
            for argument in &call.structural_arguments {
                demand(argument);
            }
        }
        if let checked_trees::CheckedUnitEffectOperationPlan::EstablishReference {
            source, ..
        } = operation
        {
            demand(source);
        }
    }
    let (_, authored) = crate::declarations::state_declaration(checked, state.state)?;
    let parameters = checked
        .state_parameters(authored)
        .iter()
        .filter(|parameter| {
            !parameter.is_const
                && !parameter.relevance.is_erased()
                && checked
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
        })
        .collect::<Vec<_>>();
    let mut establishments = Vec::with_capacity(demands.len());
    let mut storage = Vec::with_capacity(demands.len());
    for parameter_index in demands {
        let parameter =
            parameters
                .get(parameter_index as usize)
                .ok_or(LoweringError::Unsupported(
                    "scalar ingress demand is outside its authored namespace",
                ))?;
        let scalar_type = terminal_scalar_type(
            checked
                .primitive_type_reference(parameter.type_reference)
                .ok_or(LoweringError::Unsupported(
                    "scalar ingress lost its primitive type",
                ))?,
        )?;
        let authored_position = checked
            .state_parameters(authored)
            .iter()
            .position(|actual| actual.symbol == parameter.symbol)
            .ok_or(LoweringError::Unsupported(
                "scalar ingress lost its authored position",
            ))?;
        let position = state
            .scalar_parameters
            .iter()
            .position(|actual| actual.source_position as usize == authored_position)
            .ok_or(LoweringError::Unsupported(
                "scalar ingress lost its published value",
            ))?;
        let value_type = parameter_types
            .get(position)
            .ok_or(LoweringError::Unsupported(
                "scalar ingress lost its exact incoming value type",
            ))?;
        if value_type.scalar_type != scalar_type || !value_type.qualifications.is_empty() {
            return unsupported(
                "scalar ingress address demand requires retained storage qualification custody",
            );
        }
        let identity = checked
            .typed
            .normalized_type_identity(parameter.type_reference)
            .into_string();
        let mut candidates = types.iter().filter(|actual| actual.identity == identity);
        let actual = candidates.next().ok_or(LoweringError::Unsupported(
            "scalar ingress has no retained original storage type",
        ))?;
        if candidates.next().is_some()
            || actual.shape != terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar_type)
        {
            return unsupported("scalar ingress substituted its original storage type");
        }
        let place = place_id(allocate_dense(next_place)?);
        bindings.register_scalar_parameter_home(
            parameter_index,
            place,
            scalar_type,
            if parameter.is_mutable {
                terminal_psi::StructuralAccess::MutableBorrow
            } else {
                terminal_psi::StructuralAccess::SharedBorrow
            },
        )?;
        storage.push((parameter.symbol, place, scalar_type));
        establishments.push(LoweredScalarBinding::StoredValue {
            value: LoweredDirectExpression::Local {
                position,
                scalar_type,
            },
            destination: StoreDestination::Initialize {
                place,
                structural_type: actual.id,
            },
        });
    }
    Ok(Homes {
        establishments,
        storage,
    })
}
