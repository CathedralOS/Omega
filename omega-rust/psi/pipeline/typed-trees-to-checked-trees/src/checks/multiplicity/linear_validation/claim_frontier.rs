//! Initial linear places and the linear claim frontier.

use crate::checks::multiplicity::linear_obligations::{LinearClaimTemplate, LinearPlace};
use crate::checks::multiplicity::linear_validation::permission_production::established_provenance;
use crate::checks::multiplicity::type_multiplicity::{
    data_field_name, find_data_definition, type_multiplicity, type_multiplicity_with_substitutions,
};
use language_semantics::{Multiplicity, PermissionEventSource};
use symbols::SymbolHandle;
use typed_trees::statement::StatementNode;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn initial_linear_places(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Vec<LinearPlace> {
    let mut places = Vec::new();
    for parameter in program.state_parameters(state) {
        // A by-value `self` parameter is the language's terminal-consumer
        // form. The caller owns the consumption judgment.
        if parameter.is_self {
            continue;
        }
        let claims = linear_claim_frontier(program, parameter.type_reference);
        // Whole claim-free affine parameters are live on entry, but establish
        // no linear claim. Track their moves so a transferred parameter cannot
        // be moved again or disposed by the caller at exit.
        if claims.is_empty()
            && type_multiplicity(program, parameter.type_reference) == Multiplicity::Affine
            && matches!(
                program
                    .type_reference_table
                    .type_reference(parameter.type_reference),
                TypeReferenceNode::Named { .. }
                    | TypeReferenceNode::Generic { .. }
                    | TypeReferenceNode::FixedArray { .. }
            )
        {
            places.push(LinearPlace {
                symbol: parameter.symbol,
                name: parameter.name.as_str().to_owned(),
                path: Vec::new(),
                multiplicity: Multiplicity::Affine,
                claim_identity: None,
                provenance: None,
                live: true,
                ever_established: true,
                conditional: false,
            });
        }
        for claim in claims {
            places.push(LinearPlace {
                symbol: parameter.symbol,
                name: claim_place_name(program, parameter.name.as_str(), &claim.path),
                path: claim.path,
                multiplicity: claim.multiplicity,
                claim_identity: None,
                provenance: Some(established_provenance(
                    machine_symbol,
                    state_symbol,
                    PermissionEventSource::StateEntry,
                )),
                live: true,
                ever_established: true,
                conditional: claim.conditional,
            });
        }
    }
    for statement in program.statement_table.statements(state.statement_nodes) {
        let StatementNode::LocalData(local) = statement else {
            continue;
        };
        let claims = linear_claim_frontier(program, local.type_reference);
        for claim in &claims {
            places.push(LinearPlace {
                symbol: local.symbol,
                name: claim_place_name(program, local.name.as_str(), &claim.path),
                path: claim.path.clone(),
                multiplicity: claim.multiplicity,
                claim_identity: None,
                provenance: None,
                live: false,
                ever_established: false,
                conditional: claim.conditional,
            });
        }
        // An explicitly initialized affine destination owns one whole value,
        // independently of which validated expression produced it. The ordinary
        // LocalData write establishes this root; later replacement settles the
        // old value before establishing the new one, including mutable locals.
        // Absent initializers remain outside this roster: partial zero-fill
        // construction needs its own initialization judgment.
        if claims.is_empty()
            && type_multiplicity(program, local.type_reference) == Multiplicity::Affine
            && local.initial_value.is_valid()
            && matches!(
                program
                    .type_reference_table
                    .type_reference(local.type_reference),
                typed_trees::types::TypeReferenceNode::Named { .. }
                    | typed_trees::types::TypeReferenceNode::Generic { .. }
                    | typed_trees::types::TypeReferenceNode::FixedArray { .. }
            )
        {
            places.push(LinearPlace {
                symbol: local.symbol,
                name: local.name.as_str().to_owned(),
                path: Vec::new(),
                multiplicity: Multiplicity::Affine,
                claim_identity: None,
                provenance: None,
                live: false,
                ever_established: false,
                conditional: false,
            });
        }
    }
    places
}

pub(crate) fn linear_claim_frontier(
    program: &typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Vec<LinearClaimTemplate> {
    let mut claims = Vec::new();
    append_linear_claim_frontier(
        program,
        type_reference,
        &[],
        &[],
        &mut Vec::new(),
        &mut claims,
    );
    claims
}

fn append_linear_claim_frontier(
    program: &typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    path: &[facts::PlaceSegment],
    visiting: &mut Vec<SymbolHandle>,
    claims: &mut Vec<LinearClaimTemplate>,
) {
    if !type_reference.is_valid() {
        return;
    }
    let multiplicity = type_multiplicity_with_substitutions(program, type_reference, substitutions);
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            if multiplicity == Multiplicity::Linear {
                claims.push(LinearClaimTemplate {
                    path: path.to_vec(),
                    type_reference,
                    multiplicity,
                    conditional: path
                        .iter()
                        .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. })),
                });
                return;
            }
            append_linear_claim_frontier(
                program,
                *base_type,
                substitutions,
                path,
                visiting,
                claims,
            );
            return;
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: typed_trees::types::FixedArrayLength::Literal(length),
        } => {
            for index in 0..*length {
                let mut element_path = path.to_vec();
                element_path.push(facts::PlaceSegment::FixedIndex { index });
                append_linear_claim_frontier(
                    program,
                    *element_type,
                    substitutions,
                    &element_path,
                    visiting,
                    claims,
                );
            }
            return;
        }
        TypeReferenceNode::Named { symbol, .. } => {
            if let Some(replacement) =
                substitutions
                    .iter()
                    .rev()
                    .find_map(|(parameter, replacement)| {
                        (*parameter == *symbol).then_some(*replacement)
                    })
                && replacement != type_reference
            {
                append_linear_claim_frontier(
                    program,
                    replacement,
                    substitutions,
                    path,
                    visiting,
                    claims,
                );
                return;
            }
        }
        _ => {}
    }

    if multiplicity == Multiplicity::Linear {
        claims.push(LinearClaimTemplate {
            path: path.to_vec(),
            type_reference,
            multiplicity,
            conditional: path
                .iter()
                .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. })),
        });
        return;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { .. } => unreachable!("handled before multiplicity"),
        TypeReferenceNode::Named { symbol, name } => {
            let Some(definition) = find_data_definition(program, *symbol, name.as_str()) else {
                return;
            };
            append_data_linear_claim_frontier(
                program,
                definition,
                substitutions,
                path,
                visiting,
                claims,
            );
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            arguments,
            ..
        } => {
            let Some(definition) = find_data_definition(program, *base_symbol, base_name.as_str())
            else {
                return;
            };
            let mut instantiated = substitutions.to_vec();
            instantiated.extend(
                program
                    .data_type_parameters(definition)
                    .iter()
                    .zip(
                        program
                            .type_reference_table
                            .type_reference_handles(*arguments),
                    )
                    .filter_map(|(parameter, argument)| {
                        matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                            .then_some((parameter.symbol, *argument))
                    }),
            );
            append_data_linear_claim_frontier(
                program,
                definition,
                &instantiated,
                path,
                visiting,
                claims,
            );
        }
        TypeReferenceNode::FixedArray { .. } => {
            // Const-parameter lengths must become literal before this stage
            // can enumerate the complete fixed-index ownership frontier.
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Unit => {}
    }
}

fn append_data_linear_claim_frontier(
    program: &typed_trees::TypedTrees,
    definition: &typed_trees::data::DataDefinition,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    path: &[facts::PlaceSegment],
    visiting: &mut Vec<SymbolHandle>,
    claims: &mut Vec<LinearClaimTemplate>,
) {
    if visiting.contains(&definition.symbol) {
        return;
    }
    visiting.push(definition.symbol);
    for member in program.data_members(definition) {
        match member {
            typed_trees::data::DataMember::Field(field) => {
                let mut field_path = path.to_vec();
                field_path.push(facts::PlaceSegment::Field {
                    symbol: field.symbol,
                });
                append_linear_claim_frontier(
                    program,
                    field.type_reference,
                    substitutions,
                    &field_path,
                    visiting,
                    claims,
                );
            }
            typed_trees::data::DataMember::Variant(variant) => {
                let mut case_path = path.to_vec();
                case_path.push(facts::PlaceSegment::Case {
                    variant: variant.symbol,
                });
                for field in program.data_payload_fields(variant) {
                    let mut field_path = case_path.clone();
                    field_path.push(facts::PlaceSegment::Field {
                        symbol: field.symbol,
                    });
                    append_linear_claim_frontier(
                        program,
                        field.type_reference,
                        substitutions,
                        &field_path,
                        visiting,
                        claims,
                    );
                }
            }
        }
    }
    visiting.pop();
}

fn claim_place_name(
    program: &typed_trees::TypedTrees,
    root: &str,
    path: &[facts::PlaceSegment],
) -> String {
    let mut name = root.to_owned();
    for segment in path {
        match segment {
            facts::PlaceSegment::Case { variant } => {
                let case = program.data_definitions().iter().find_map(|definition| {
                    program.data_members(definition).iter().find_map(|member| {
                        let typed_trees::data::DataMember::Variant(candidate) = member else {
                            return None;
                        };
                        (candidate.symbol == *variant).then_some(candidate.name.as_str())
                    })
                });
                name.push_str("::");
                name.push_str(case.unwrap_or("<case>"));
            }
            facts::PlaceSegment::Field { symbol } => {
                let field = data_field_name(program, *symbol);
                name.push('.');
                name.push_str(field.unwrap_or("<field>"));
            }
            facts::PlaceSegment::FixedIndex { index } => {
                name.push('[');
                name.push_str(&index.to_string());
                name.push(']');
            }
            facts::PlaceSegment::FixedRange { start, end } => {
                name.push('[');
                name.push_str(&start.to_string());
                name.push_str("..");
                name.push_str(&end.to_string());
                name.push(']');
            }
            facts::PlaceSegment::Index { .. } => name.push_str("[<index>]"),
        }
    }
    name
}
