//! Public postconditions describe the call's current observation coordinates.
//!
//! The checked call origins own alias and representation custody. An exact
//! formal root selects its positional actual, then its complete field path is
//! composed in that coordinate owner. Recasts translate scalar leaves through
//! existing normalized geometry; the same field spelling supplies no relation.
//! The string bound consumers receive only exact, value-preserving coordinates.
//! Unknown or runtime-selected storage supplies no shortcut fact. Callers apply
//! the complete write frame first, and owned-copy guarantees never requalify
//! the storage from which the copy was taken.

use crate::{CallWriteOrigins, LocalWriteSource};
use facts::PlaceRoot;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::signature::{SignatureContractKind, StateSignature};
use typed_trees::state::State;

/// Every `ProofFact::Expression` under `signature`'s `ensures` contracts, in
/// declaration order.
pub fn ensures_contract_expressions<'program>(
    program: &'program dyn crate::CheckingInput,
    signature: &'program StateSignature,
) -> impl Iterator<Item = ExpressionHandle> + 'program {
    program
        .signature_contracts
        .span_or_empty(signature.contracts)
        .iter()
        .filter(|contract| matches!(contract.kind, SignatureContractKind::Ensures))
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts).iter())
        .filter_map(|fact| match fact {
            ProofFact::Expression(expression) => Some(*expression),
            _ => None,
        })
}

/// Project integer bounds from the exact public signature onto captured live
/// reference arguments. Several possible origins supply a fact only when
/// their complete projected coordinates agree, never one fact per may route.
pub fn collect_parameter_bound_comparisons(
    program: &dyn crate::CheckingInput,
    state: &State,
    signature: &StateSignature,
    origins: &CallWriteOrigins,
    conjunct: ExpressionHandle,
    comparisons: &mut Vec<(String, BinaryOperator, i64)>,
) {
    let Some(comparison) = program.expression_table.binary_node(conjunct) else {
        return;
    };
    if comparison.operator == BinaryOperator::And {
        collect_parameter_bound_comparisons(
            program,
            state,
            signature,
            origins,
            comparison.left,
            comparisons,
        );
        collect_parameter_bound_comparisons(
            program,
            state,
            signature,
            origins,
            comparison.right,
            comparisons,
        );
        return;
    }
    let Some(place) = crate::canonical_place_from_expression(program, comparison.left) else {
        return;
    };
    let parameters = program.state_signature_parameters(signature);
    let Some(position) = parameters
        .iter()
        .position(|parameter| place.root == PlaceRoot::Symbol(parameter.symbol))
    else {
        return;
    };
    let parameter = &parameters[position];
    let Some(reference) = crate::reference_shell(program, parameter.type_reference) else {
        return;
    };
    let Some(leaf) =
        crate::project_type_reference_from_segments(program, reference.referee, &place.segments)
    else {
        return;
    };
    let Some(primitive) = program.primitive_type_reference(
        crate::value_custody::places::assignment_value_type(program, leaf),
    ) else {
        return;
    };
    if primitive.integer_sign_bits().is_none() {
        return;
    }
    let spelling = match comparison.operator {
        BinaryOperator::Less => language_core::OperatorSpelling::Less,
        BinaryOperator::LessOrEqual => language_core::OperatorSpelling::LessEqual,
        BinaryOperator::Greater => language_core::OperatorSpelling::Greater,
        BinaryOperator::GreaterOrEqual => language_core::OperatorSpelling::GreaterEqual,
        _ => return,
    };
    let operand_types = [comparison.left, comparison.right].map(|operand| {
        crate::parameter_expression_result_type_reference(
            program,
            signature.symbol,
            parameters,
            operand,
        )
    });
    if !typed_trees::operator::has_builtin_spelled_expression_meaning(
        program,
        signature.symbol,
        conjunct,
        spelling,
        &operand_types,
    ) {
        return;
    }
    let Some(literal) = program.expression_table.integer_node(comparison.right) else {
        return;
    };
    let Some(value) = literal.value_i64() else {
        return;
    };
    let Some(sources) = origins
        .arguments
        .get(position)
        .and_then(|origins| origins.as_ref())
        .filter(|origins| !origins.is_empty())
    else {
        return;
    };
    let relative = LocalWriteSource {
        is_image_storage: false,
        root: parameter.symbol,
        segments: place.segments,
        representation: None,
        recasts: Vec::new(),
        path: String::new(),
        collection_coarse: false,
    };
    let mut coordinate: Option<crate::ScalarStorageCoordinate> = None;
    for source in sources {
        let Some(projected) = source
            .substitute_storage(&relative)
            .and_then(|source| source.scalar_storage_coordinate(program, state))
            .filter(|coordinate| {
                coordinate.preserves_scalar_value
                    && coordinate.source_primitive == primitive
                    && coordinate.view_primitive == primitive
            })
        else {
            return;
        };
        if coordinate
            .as_ref()
            .is_some_and(|existing| existing.place != projected.place)
        {
            return;
        }
        coordinate = Some(projected);
    }
    let Some(coordinate) = coordinate else {
        return;
    };
    let Some(path) = coordinate.label(program) else {
        return;
    };
    comparisons.push((path, comparison.operator, value));
}
