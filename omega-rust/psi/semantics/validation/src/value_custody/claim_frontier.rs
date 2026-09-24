//! The exact linear claim frontier of a type.
//!
//! A linear value contributes one claim at its own place; transparent records,
//! active cases, and fixed arrays contribute one entry per contained linear
//! claim instead of inventing an aggregate root claim. Both the multiplicity
//! checker (which tracks the frontier as owned places) and the lowering replay
//! (which re-derives a selection transfer's consumed claim set) must agree on
//! this enumeration, so it lives here rather than in either consumer.
//!
//! Paths are deterministic: fields and case payload fields walk declaration
//! order, and fixed arrays walk ascending element index. A `Case` segment
//! marks the claim conditional -- it is live only while that case is active.

use language_semantics::Multiplicity;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// One claim in a type's linear frontier: the exact path below the root, the
/// claim's own type, and whether a case segment makes it conditional on the
/// active variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimFrontierClaim {
    pub path: Vec<facts::PlaceSegment>,
    pub type_reference: TypeReferenceHandle,
    pub conditional: bool,
}

pub fn linear_claim_frontier(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Vec<ClaimFrontierClaim> {
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
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    path: &[facts::PlaceSegment],
    visiting: &mut Vec<SymbolHandle>,
    claims: &mut Vec<ClaimFrontierClaim>,
) {
    if !type_reference.is_valid() {
        return;
    }
    let multiplicity = type_multiplicity_with_substitutions(program, type_reference, substitutions);
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            if multiplicity == Multiplicity::Linear {
                push_claim(claims, path, type_reference);
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
        push_claim(claims, path, type_reference);
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
    program: &TypedTrees,
    definition: &typed_trees::data::DataDefinition,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    path: &[facts::PlaceSegment],
    visiting: &mut Vec<SymbolHandle>,
    claims: &mut Vec<ClaimFrontierClaim>,
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

fn push_claim(
    claims: &mut Vec<ClaimFrontierClaim>,
    path: &[facts::PlaceSegment],
    type_reference: TypeReferenceHandle,
) {
    claims.push(ClaimFrontierClaim {
        path: path.to_vec(),
        type_reference,
        conditional: path
            .iter()
            .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. })),
    });
}

/// Multiplicity with closed generic substitutions applied: a type parameter's
/// occurrence takes the argument's multiplicity; everything else is the
/// declaration's own.
fn type_multiplicity_with_substitutions(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Multiplicity {
    if !type_reference.is_valid() {
        return Multiplicity::Affine;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_multiplicity_with_substitutions(program, *base_type, substitutions)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            type_multiplicity_with_substitutions(program, *element_type, substitutions)
        }
        TypeReferenceNode::Named { symbol, .. } => substitutions
            .iter()
            .rev()
            .find_map(|(parameter, replacement)| {
                (*parameter == *symbol && *replacement != type_reference).then_some(*replacement)
            })
            .map(|replacement| {
                type_multiplicity_with_substitutions(program, replacement, substitutions)
            })
            .unwrap_or_else(|| program.type_multiplicity(type_reference)),
        _ => program.type_multiplicity(type_reference),
    }
}

/// The declaration a named type resolves to. A resolved symbol is the
/// declaration's identity; the spelled name only stands in for a reference
/// that never resolved, so a same-named declaration elsewhere cannot answer
/// for a resolved one.
fn find_data_definition<'program>(
    program: &'program TypedTrees,
    symbol: SymbolHandle,
    name: &str,
) -> Option<&'program typed_trees::data::DataDefinition> {
    if symbol.is_valid() {
        return program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == symbol);
    }
    program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == name)
}
