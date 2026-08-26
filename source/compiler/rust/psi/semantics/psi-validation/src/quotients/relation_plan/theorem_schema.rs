//! Compiler-derived structural contract expected from a selected quotient
//! theorem.
//!
//! This stage retains the schema only. It neither compares the selected
//! theorem against it nor grants executable quotient authority.

use super::{ExactQuotientRelation, InputRelation, RelationPlanError, RepresentativeTelescope};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::signature::SignatureContractKind;
use psi_typed_trees::types::TypeReferenceHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TheoremParameterRole {
    QuotientLeft,
    QuotientRight,
    Shared,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TheoremSchemaParameter {
    pub(super) representative_position: usize,
    pub(super) role: TheoremParameterRole,
    pub(super) type_reference: TypeReferenceHandle,
    /// The theorem parameter is always ordinary (never an attached receiver),
    /// while retaining the representative position's exact access mode for
    /// later signature verification.
    pub(super) is_mutable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TheoremRelationPremise {
    pub(super) representative_position: usize,
    pub(super) relation: ExactQuotientRelation,
    pub(super) left_parameter: usize,
    pub(super) right_parameter: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TheoremContractOwner {
    Machine,
    State,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TheoremContractFactLocation {
    pub(super) owner: TheoremContractOwner,
    pub(super) contract_position: usize,
    pub(super) fact_position: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TheoremApplicationSide {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TheoremLegalityPremise {
    /// Together with the selected application's structural argument vector,
    /// the exact contract coordinate and side are the complete left/right
    /// `requires` substitution carrier. Stage 2 must consume these fields and
    /// must not reconstruct an application from a rendered formula.
    pub(super) application: TheoremApplicationSide,
    pub(super) fact: TheoremContractFactLocation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TheoremRepresentativeApplication {
    pub(super) machine_symbol: SymbolHandle,
    pub(super) state_symbol: SymbolHandle,
    /// Parameter-arena positions in representative runtime-telescope order.
    pub(super) arguments: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExpectedTheoremSchema {
    pub(super) parameters: Vec<TheoremSchemaParameter>,
    pub(super) relation_premises: Vec<TheoremRelationPremise>,
    pub(super) legality_premises: Vec<TheoremLegalityPremise>,
    pub(super) left_application: TheoremRepresentativeApplication,
    pub(super) right_application: TheoremRepresentativeApplication,
    /// The structural `ensures` conclusion is this exact nominal relation
    /// applied to the results of `left_application` and `right_application`.
    /// No normalized display label participates in its identity.
    pub(super) result_relation: ExactQuotientRelation,
}

pub(super) fn derive_expected_theorem_schema(
    program: &TypedTrees,
    input_relations: &[InputRelation],
    result_relation: ExactQuotientRelation,
    representative: &RepresentativeTelescope,
) -> Result<ExpectedTheoremSchema, RelationPlanError> {
    if input_relations.len() != representative.parameters.len() {
        return Err(RelationPlanError::TheoremSchemaRuntimeArityMismatch);
    }

    let mut parameters = Vec::with_capacity(
        input_relations
            .iter()
            .map(|relation| usize::from(matches!(relation, InputRelation::Quotient(_))) + 1)
            .sum(),
    );
    let mut relation_premises = Vec::new();
    let mut left_arguments = Vec::with_capacity(input_relations.len());
    let mut right_arguments = Vec::with_capacity(input_relations.len());
    for (representative_position, (relation, parameter)) in input_relations
        .iter()
        .zip(&representative.parameters)
        .enumerate()
    {
        match relation {
            InputRelation::Quotient(relation) => {
                let left_parameter = parameters.len();
                parameters.push(TheoremSchemaParameter {
                    representative_position,
                    role: TheoremParameterRole::QuotientLeft,
                    type_reference: parameter.type_reference,
                    is_mutable: parameter.is_mutable,
                });
                let right_parameter = parameters.len();
                parameters.push(TheoremSchemaParameter {
                    representative_position,
                    role: TheoremParameterRole::QuotientRight,
                    type_reference: parameter.type_reference,
                    is_mutable: parameter.is_mutable,
                });
                left_arguments.push(left_parameter);
                right_arguments.push(right_parameter);
                relation_premises.push(TheoremRelationPremise {
                    representative_position,
                    relation: *relation,
                    left_parameter,
                    right_parameter,
                });
            }
            InputRelation::ExactEquality(_) => {
                let shared = parameters.len();
                parameters.push(TheoremSchemaParameter {
                    representative_position,
                    role: TheoremParameterRole::Shared,
                    type_reference: parameter.type_reference,
                    is_mutable: parameter.is_mutable,
                });
                left_arguments.push(shared);
                right_arguments.push(shared);
            }
        }
    }

    let left_application = TheoremRepresentativeApplication {
        machine_symbol: representative.machine_symbol,
        state_symbol: representative.state_symbol,
        arguments: left_arguments,
    };
    let right_application = TheoremRepresentativeApplication {
        machine_symbol: representative.machine_symbol,
        state_symbol: representative.state_symbol,
        arguments: right_arguments,
    };
    let mut legality_premises = Vec::new();
    for (owner, contracts) in [
        (
            TheoremContractOwner::Machine,
            program
                .signature_contracts
                .span_or_empty(representative.machine_contracts),
        ),
        (
            TheoremContractOwner::State,
            program
                .signature_contracts
                .span_or_empty(representative.state_contracts),
        ),
    ] {
        for (contract_position, contract) in contracts.iter().enumerate() {
            if contract.kind != SignatureContractKind::Requires {
                continue;
            }
            for fact_position in 0..contract.facts.len() {
                let fact = TheoremContractFactLocation {
                    owner,
                    contract_position,
                    fact_position,
                };
                legality_premises.push(TheoremLegalityPremise {
                    application: TheoremApplicationSide::Left,
                    fact,
                });
                legality_premises.push(TheoremLegalityPremise {
                    application: TheoremApplicationSide::Right,
                    fact,
                });
            }
        }
    }

    Ok(ExpectedTheoremSchema {
        parameters,
        relation_premises,
        legality_premises,
        left_application,
        right_application,
        result_relation,
    })
}
