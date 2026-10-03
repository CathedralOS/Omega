//! Exact authored destination and value custody for whole sum-field replacement.
//! Construction and copying share the destination join, but keep their source
//! obligations separate. Emission still owes live ownership and window repair.

use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::data::DataMember;
use checked_trees::statement::StatementNode;
use checked_trees::types::TypeReferenceNode;
use checked_trees::{
    CheckedScalarExpressionRole, CheckedStoreRoot, CheckedStructuralAccess, CheckedTrees,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment,
};

pub(crate) fn validate(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    operation: &CheckedUnitEffectOperationPlan,
    operations: &[CheckedUnitEffectOperationPlan],
    parameters: &[CheckedUnitStructuralParameterPlan],
) -> Result<(), LoweringError> {
    let (statement_index, destination, carrier_path, field_identity, type_identity) =
        match operation {
            CheckedUnitEffectOperationPlan::StructuralCaseFieldStore(store) => (
                store.statement_index,
                store.destination,
                &store.carrier_path,
                &store.field_identity,
                &store.value.type_identity,
            ),
            CheckedUnitEffectOperationPlan::StructuralCaseLiteralFieldStore(store) => (
                store.statement_index,
                store.destination,
                &store.carrier_path,
                &store.field_identity,
                &store.type_identity,
            ),
            _ => return unsupported("case store source replay received another operation"),
        };
    let (owner, state) =
        crate::expression_preparation::source_custody::authored_state(checked, state_symbol)?;
    if owner.symbol != machine {
        return unsupported("case store has a different authored machine");
    }
    let Some(StatementNode::Assignment(assignment)) = checked
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index as usize)
    else {
        return unsupported("case store has no authored assignment");
    };
    // The case operation owns the complete replacement. An alternate store
    // family, an explicit repair or an atomic event cannot also write here.
    if operations.iter().filter(|candidate| {
        candidate.store_statement_index() == Some(statement_index)
            || matches!(candidate,
                CheckedUnitEffectOperationPlan::StoreStructuralField { statement_index: ordinal, .. }
                    if *ordinal == statement_index)
            || matches!(candidate, CheckedUnitEffectOperationPlan::AtomicAccess(access)
                if access.statement_index == statement_index)
    }).count() != 1 {
        return unsupported("case assignment has conflicting store custody");
    }
    let authored = crate::emission::call_source_custody::projected_receivers::store_destination(
        checked,
        machine,
        state_symbol,
        Some(statement_index as usize),
        assignment.target,
    )?;
    let (root, mut path) = match destination {
        CheckedStoreRoot::Parameter { position } => (
            checked
                .state_parameters(state)
                .get(position as usize)
                .ok_or(LoweringError::Unsupported(
                    "case store destination parameter is absent",
                ))?
                .symbol,
            Vec::new(),
        ),
        CheckedStoreRoot::Local { symbol } => super::local_store_destination(
            checked,
            machine,
            state,
            state_symbol,
            statement_index,
            symbol,
        )?,
    };
    path.extend(carrier_path.iter().cloned());
    path.push(CheckedUnitStructuralPathSegment::Field(
        field_identity.clone(),
    ));
    if authored.root != root || authored.path != path {
        return unsupported("case store destination drifted from its authored place");
    }
    let reference =
        validation::declared_place_type_raw(&checked.typed, owner, Some(state), assignment.target)
            .ok_or(LoweringError::Unsupported(
                "case store destination type is absent",
            ))?;
    let TypeReferenceNode::Named { symbol, .. } =
        checked.type_reference_table.type_reference(reference)
    else {
        return unsupported("case store destination is not an unqualified nominal sum");
    };
    let data = checked
        .data_definitions()
        .iter()
        .find(|data| data.symbol == *symbol)
        .ok_or(LoweringError::Unsupported(
            "case store destination declaration is absent",
        ))?;
    if checked.normalized_type_identity(reference).as_str() != type_identity
        || data.supply_mode != language_semantics::DataSupplyMode::CheckedShape
        || !data.lifetime_parameters.is_empty()
        || !checked.data_type_parameters(data).is_empty()
        || data.quotient.is_some()
        || !data.where_facts.is_empty()
        || data.zero_gated
        || data.properties.multiplicity != language_semantics::Multiplicity::Unrestricted
        || !checked
            .data_members(data)
            .iter()
            .any(|member| matches!(member, DataMember::Variant(_)))
    {
        return unsupported("case store destination lost its closed unrestricted sum contract");
    }
    match operation {
        CheckedUnitEffectOperationPlan::StructuralCaseFieldStore(store) => {
            if !checked.data_members(data).iter().all(|member| {
                matches!(member, DataMember::Variant(variant)
                    if checked.data_payload_fields(variant).is_empty())
            }) {
                return unsupported("copied case field requires a payload-free sum");
            }
            let CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
                store.value.source
            else {
                return unsupported("copied case field has no parameter source");
            };
            let parameter =
                parameters
                    .get(parameter_index as usize)
                    .ok_or(LoweringError::Unsupported(
                        "copied case field parameter is absent",
                    ))?;
            let declared = checked
                .state_parameters(state)
                .get(parameter.position as usize)
                .ok_or(LoweringError::Unsupported(
                    "copied case field source declaration is absent",
                ))?;
            let source =
                crate::emission::call_source_custody::projected_receivers::store_destination(
                    checked,
                    machine,
                    state_symbol,
                    Some(statement_index as usize),
                    assignment.value,
                )?;
            let source_type = validation::declared_place_type_raw(
                &checked.typed,
                owner,
                Some(state),
                assignment.value,
            )
            .and_then(|reference| validation::unwrapped_type_reference(&checked.typed, reference))
            .ok_or(LoweringError::Unsupported(
                "copied case field source type is absent",
            ))?;
            if parameter.is_self
                || declared.is_self
                || source.root != declared.symbol
                || source.path != store.value.path
                || !store
                    .value
                    .path
                    .iter()
                    .all(|segment| matches!(segment, CheckedUnitStructuralPathSegment::Field(_)))
                || checked.normalized_type_identity(source_type).as_str() != type_identity
                || parameter.access != store.value.access
                || !matches!(
                    parameter.access,
                    CheckedStructuralAccess::Owned | CheckedStructuralAccess::SharedBorrow
                )
                || parameter.multiplicity != language_semantics::Multiplicity::Unrestricted
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
                || parameter.fused_service_erasure.is_some()
            {
                return unsupported("copied case field value drifted from its authored parameter");
            }
        }
        CheckedUnitEffectOperationPlan::StructuralCaseLiteralFieldStore(store) => {
            let constructor = validation::structural_case_constructor(
                &checked.typed,
                assignment.value,
                reference,
            )
            .ok_or(LoweringError::Unsupported(
                "case field store has no authored constructor",
            ))?;
            let variant = checked
                .data_members(data)
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(variant) if variant.symbol == constructor.case => {
                        Some(variant)
                    }
                    _ => None,
                })
                .ok_or(LoweringError::Unsupported(
                    "case field store constructor has a foreign case",
                ))?;
            if store.literal != assignment.value
                || checked
                    .normalized_type_identity(constructor.type_reference)
                    .as_str()
                    != type_identity
                || variant.path_identity() != store.case_identity
                || constructor.fields.len() != store.fields.len()
            {
                return unsupported("case field store substituted its authored constructor");
            }
            for (ordinal, ((symbol, expression, reference), field)) in
                constructor.fields.iter().zip(&store.fields).enumerate()
            {
                let declaration = checked
                    .data_payload_fields(variant)
                    .iter()
                    .find(|field| field.symbol == *symbol)
                    .or_else(|| {
                        checked
                            .data_members(data)
                            .iter()
                            .find_map(|member| match member {
                                DataMember::Field(field) if field.symbol == *symbol => Some(field),
                                _ => None,
                            })
                    })
                    .ok_or(LoweringError::Unsupported(
                        "case field store operand declaration is absent",
                    ))?;
                let ordinal = u32::try_from(ordinal).map_err(|_| {
                    LoweringError::Unsupported("case field store operand ordinal overflow")
                })?;
                let (binding, value) = checked
                    .facts
                    .values
                    .scalar_expressions
                    .bound_expression_at(
                        state_symbol,
                        statement_index,
                        CheckedScalarExpressionRole::StructuralValueField {
                            expression: assignment.value,
                            field_ordinal: ordinal,
                        },
                    )
                    .ok_or(LoweringError::Unsupported(
                        "case field store operand binding is absent",
                    ))?;
                if field.field_ordinal != ordinal
                    || field.field_identity != declaration.path_identity()
                    || checked.primitive_type_reference(*reference) != Some(field.primitive_type)
                    || binding.expression != *expression
                    || binding.destination.is_valid()
                    || value != &field.expression
                {
                    return unsupported("case field store substituted or reordered an operand");
                }
                crate::expression_preparation::source_custody::validate_pure(
                    checked,
                    binding,
                    super::terminal_scalar_type(field.primitive_type)?,
                )?;
            }
        }
        _ => return unsupported("case store source replay received another operation"),
    }
    Ok(())
}
