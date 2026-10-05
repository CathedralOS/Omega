//! A routed binding follows ordinary call sequencing and structural loans.
//! This join preserves the exact selected requirement at each argument, rather
//! than recognizing a one-hop helper body. The argument builder already owns
//! authored operand order and loan access; no shape here establishes a binding.

use crate::execution::terminal_unit::{
    CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment, TypedTrees,
};

pub(super) fn fused_service_arguments_match(
    program: &TypedTrees,
    caller_state: &typed_trees::state::State,
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    target_state: &typed_trees::state::State,
    statement_index: usize,
    arguments: &[CheckedUnitStructuralArgumentPlan],
) -> bool {
    let targets = program
        .state_parameters(target_state)
        .iter()
        .filter(|parameter| {
            !parameter.relevance.is_erased()
                && program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
        })
        .collect::<Vec<_>>();
    if targets.len() != arguments.len() {
        return false;
    }
    for (target, argument) in targets.into_iter().zip(arguments) {
        let target_carrier = match typed_trees::service::classify_exact_service_parameter_carrier(
            program,
            target.type_reference,
        ) {
            Ok(Some(carrier)) => carrier,
            Ok(None) => continue,
            Err(_) => return false,
        };
        let Some(source_index) = argument.source_parameter_index() else {
            // Binding locals/results need their own establishment/transfer
            // receipt before that source family can supply authority.
            return false;
        };
        let Some(source_plan) = caller_parameters.get(source_index as usize) else {
            return false;
        };
        let Some(source) = program
            .state_parameters(caller_state)
            .get(source_plan.position as usize)
        else {
            return false;
        };
        let mut place = crate::flow::CanonicalPlace {
            root: facts::PlaceRoot::Symbol((source).symbol),
            segments: Vec::new(),
        };
        for segment in &argument.path {
            let CheckedUnitStructuralPathSegment::Field(identity) = segment else {
                return false;
            };
            // Field identities are local to their declaring carrier. Another
            // record's same-spelled field cannot disqualify or supply this loan.
            let owner = {
                let Some(reference) = crate::flow::canonical_place_type_reference(
                    program,
                    caller_state.symbol,
                    statement_index,
                    &place,
                )
                .and_then(|reference| validation::unwrapped_type_reference(program, reference)) else {
                    return false;
                };
                match program.type_reference_table.type_reference(reference) {
                    typed_trees::types::TypeReferenceNode::Named { symbol, .. } => *symbol,
                    typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. } => {
                        *base_symbol
                    }
                    _ => return false,
                }
            };
            let Some(definition) = program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == owner)
            else {
                return false;
            };
            let mut fields = program
                .data_members(definition)
                .iter()
                .filter_map(|member| match member {
                    typed_trees::data::DataMember::Field(field)
                        if field.path_identity() == *identity =>
                    {
                        Some(field)
                    }
                    _ => None,
                });
            let Some(field) = fields.next() else {
                return false;
            };
            if fields.next().is_some() {
                return false;
            }
            place.segments.push(facts::PlaceSegment::Field {
                symbol: field.symbol,
            });
        }
        let Some(source_type) = crate::flow::canonical_place_type_reference(
            program,
            caller_state.symbol,
            statement_index,
            &place,
        ) else {
            return false;
        };
        let Ok(Some(source_carrier)) =
            typed_trees::service::classify_exact_service_parameter_carrier(program, source_type)
        else {
            return false;
        };
        let target_access = target_carrier
            .access
            .map_or(CheckedStructuralAccess::Owned, Into::into);
        if source_carrier.carrier != target_carrier.carrier
            || argument.access != target_access
            || !access_preserved(source_plan.access, argument.access)
            || argument.type_identity
                != program
                    .normalized_type_identity(target_carrier.carrier_type)
                    .into_string()
            || program
                .fused_service_erasure(target_carrier.carrier.requirement)
                .is_none()
        {
            return false;
        }
        if argument.path.is_empty() {
            let Some(receipt) = &source_plan.fused_service_erasure else {
                return false;
            };
            if receipt.source_parameter != source.symbol
                || receipt.requirement != source_carrier.carrier.requirement
                || receipt.carrier_type_identity
                    != program
                        .normalized_type_identity(source.type_reference)
                        .into_string()
            {
                return false;
            }
        }
    }
    true
}

fn access_preserved(source: CheckedStructuralAccess, argument: CheckedStructuralAccess) -> bool {
    use CheckedStructuralAccess::{MutableBorrow, Owned, SharedBorrow};
    matches!(
        (source, argument),
        (Owned, Owned | SharedBorrow | MutableBorrow)
            | (MutableBorrow, MutableBorrow | SharedBorrow)
            | (SharedBorrow, SharedBorrow)
    )
}
