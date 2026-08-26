//! Exact positional runtime correspondence for faithful quotient definitions
//! and the bounded direct-lift inclusion rung.
//!
//! This judgment accepts only direct public parameters in declaration order,
//! preserves mutable/borrow mode, and matches quotient carriers through the
//! already-retained representative static application. The lift policy is
//! intentionally no wider: reorder, duplication, constants, and adaptation
//! remain for the general implication rung. Neither policy infers or selects a
//! relation, contract proof, or representative operation.

use super::{
    ExactQuotientRelation, InputRelation, RelationPlanError, RepresentativeStaticBinding,
    RepresentativeTelescope,
};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

use super::static_application::substituted_type_matches;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DefineRuntimePosition {
    pub(super) public_parameter: SymbolHandle,
    pub(super) representative_parameter: SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::quotients) struct DefineRuntimeCorrespondence {
    pub(super) positions: Vec<DefineRuntimePosition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::quotients) struct DirectLiftRuntimeCorrespondence {
    pub(super) positions: Vec<DefineRuntimePosition>,
}

#[derive(Clone, Copy)]
enum ExactPositionPolicy {
    Define,
    DirectLift,
}

pub(super) fn derive_define_runtime_correspondence(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    call: &TableCallExpression,
    input_relations: &[InputRelation],
    result_relation: ExactQuotientRelation,
    representative: &RepresentativeTelescope,
) -> Result<DefineRuntimeCorrespondence, RelationPlanError> {
    derive_exact_position_runtime_correspondence(
        program,
        machine,
        state,
        call,
        input_relations,
        result_relation,
        representative,
        ExactPositionPolicy::Define,
    )
    .map(|positions| DefineRuntimeCorrespondence { positions })
}

pub(super) fn derive_direct_lift_runtime_correspondence(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    call: &TableCallExpression,
    input_relations: &[InputRelation],
    result_relation: ExactQuotientRelation,
    representative: &RepresentativeTelescope,
) -> Result<DirectLiftRuntimeCorrespondence, RelationPlanError> {
    derive_exact_position_runtime_correspondence(
        program,
        machine,
        state,
        call,
        input_relations,
        result_relation,
        representative,
        ExactPositionPolicy::DirectLift,
    )
    .map(|positions| DirectLiftRuntimeCorrespondence { positions })
}

#[allow(clippy::too_many_arguments)]
fn derive_exact_position_runtime_correspondence(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    call: &TableCallExpression,
    input_relations: &[InputRelation],
    result_relation: ExactQuotientRelation,
    representative: &RepresentativeTelescope,
    policy: ExactPositionPolicy,
) -> Result<Vec<DefineRuntimePosition>, RelationPlanError> {
    if !program.machine_type_parameters(machine).is_empty() {
        return Err(match policy {
            ExactPositionPolicy::Define => RelationPlanError::DefineOwnerRequiresSubstitution,
            ExactPositionPolicy::DirectLift => {
                RelationPlanError::DirectLiftOwnerRequiresSubstitution
            }
        });
    }
    let public_parameters = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_const)
        .collect::<Vec<_>>();
    let arguments = program.expression_table.expression_handles(call.arguments);
    if public_parameters.len() != arguments.len()
        || arguments.len() != representative.parameters.len()
        || input_relations.len() != arguments.len()
    {
        return Err(match policy {
            ExactPositionPolicy::Define => RelationPlanError::DefineRuntimeArityMismatch,
            ExactPositionPolicy::DirectLift => RelationPlanError::DirectLiftRuntimeArityMismatch,
        });
    }
    if has_duplicate_parameter_symbols(public_parameters.iter().map(|parameter| parameter.symbol))
        || has_duplicate_parameter_symbols(
            representative
                .parameters
                .iter()
                .map(|parameter| parameter.symbol),
        )
    {
        return Err(match policy {
            ExactPositionPolicy::Define => RelationPlanError::DefineParameterIdentityNotUnique,
            ExactPositionPolicy::DirectLift => {
                RelationPlanError::DirectLiftParameterIdentityNotUnique
            }
        });
    }

    let mut positions = Vec::with_capacity(arguments.len());
    let mut seen_lift_arguments = Vec::new();
    for (position, (((public, argument), relation), representative_parameter)) in public_parameters
        .iter()
        .zip(arguments)
        .zip(input_relations)
        .zip(&representative.parameters)
        .enumerate()
    {
        let argument_symbol =
            direct_public_parameter_symbol(program, *argument).ok_or(match policy {
                ExactPositionPolicy::Define => {
                    RelationPlanError::DefineArgumentIsNotPublicParameter(position)
                }
                ExactPositionPolicy::DirectLift => {
                    RelationPlanError::DirectLiftArgumentIsNotPublicParameter(position)
                }
            })?;
        if matches!(policy, ExactPositionPolicy::DirectLift)
            && seen_lift_arguments.contains(&argument_symbol)
        {
            return Err(RelationPlanError::DirectLiftArgumentIdentityNotUnique);
        }
        seen_lift_arguments.push(argument_symbol);
        if argument_symbol != public.symbol {
            return Err(match policy {
                ExactPositionPolicy::Define => {
                    RelationPlanError::DefineArgumentOrderMismatch(position)
                }
                ExactPositionPolicy::DirectLift => {
                    RelationPlanError::DirectLiftArgumentOrderMismatch(position)
                }
            });
        }
        if public.is_mutable != representative_parameter.is_mutable {
            return Err(match policy {
                ExactPositionPolicy::Define => {
                    RelationPlanError::DefineParameterModeMismatch(position)
                }
                ExactPositionPolicy::DirectLift => {
                    RelationPlanError::DirectLiftParameterModeMismatch(position)
                }
            });
        }
        if !input_relation_matches_public_type(program, *relation, public.type_reference)
            || !input_relation_matches_representative_type(
                program,
                *relation,
                representative_parameter.type_reference,
                &representative.static_application.bindings,
            )
        {
            return Err(match policy {
                ExactPositionPolicy::Define => {
                    RelationPlanError::DefineParameterTypeMismatch(position)
                }
                ExactPositionPolicy::DirectLift => {
                    RelationPlanError::DirectLiftParameterTypeMismatch(position)
                }
            });
        }
        positions.push(DefineRuntimePosition {
            public_parameter: public.symbol,
            representative_parameter: representative_parameter.symbol,
        });
    }
    if !quotient_carrier_matches_type(
        program,
        result_relation,
        representative.return_type,
        &representative.static_application.bindings,
    ) {
        return Err(match policy {
            ExactPositionPolicy::Define => RelationPlanError::DefineResultTypeMismatch,
            ExactPositionPolicy::DirectLift => RelationPlanError::DirectLiftResultTypeMismatch,
        });
    }
    Ok(positions)
}

fn has_duplicate_parameter_symbols(symbols: impl IntoIterator<Item = SymbolHandle>) -> bool {
    let mut seen = Vec::new();
    for symbol in symbols {
        if seen.contains(&symbol) {
            return true;
        }
        seen.push(symbol);
    }
    false
}

fn direct_public_parameter_symbol(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    let expression = match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => inner.target,
        _ => expression,
    };
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    (program
        .expression_table
        .name_path_members(path.members)
        .len()
        == 1
        && path.symbol.is_valid())
    .then_some(path.symbol)
}

fn input_relation_matches_representative_type(
    program: &TypedTrees,
    relation: InputRelation,
    representative_type: TypeReferenceHandle,
    substitutions: &[RepresentativeStaticBinding],
) -> bool {
    match relation {
        InputRelation::ExactEquality(public_type) => {
            substituted_type_matches(program, representative_type, public_type, substitutions)
        }
        InputRelation::Quotient(relation) => {
            quotient_carrier_matches_type(program, relation, representative_type, substitutions)
        }
    }
}

fn input_relation_matches_public_type(
    program: &TypedTrees,
    relation: InputRelation,
    public_type: TypeReferenceHandle,
) -> bool {
    let relation_type = match relation {
        InputRelation::Quotient(relation) => relation.quotient_type,
        InputRelation::ExactEquality(type_reference) => type_reference,
    };
    program.normalized_type_identity(relation_type) == program.normalized_type_identity(public_type)
}

fn quotient_carrier_matches_type(
    program: &TypedTrees,
    relation: ExactQuotientRelation,
    representative_type: TypeReferenceHandle,
    substitutions: &[RepresentativeStaticBinding],
) -> bool {
    if !matches!(
        program
            .type_reference_table
            .type_reference(relation.quotient_type),
        TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. }
    ) {
        // Borrow/reference carrier substitution needs an exact shell-preserving
        // rewrite; do not erase that mode by unwrapping here.
        return false;
    }
    let Some(quotient) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == relation.quotient_symbol)
    else {
        return false;
    };
    let Some(metadata) = quotient.quotient.as_ref() else {
        return false;
    };
    let Some(carrier_symbol) = super::super::base_data_symbol(program, metadata.carrier) else {
        return false;
    };
    let Some(carrier) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == carrier_symbol)
    else {
        return false;
    };
    quotient.properties.multiplicity == carrier.properties.multiplicity
        && substituted_type_matches(
            program,
            representative_type,
            metadata.carrier,
            substitutions,
        )
}
