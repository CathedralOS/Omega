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
    crate::machine_calls::effect_inference::plan_scope::memoized_claim_frontier(
        program,
        type_reference,
    )
}

/// The uncached whole-table walk; the memoized entry routes here on a miss or
/// when no program plan scope is open.
pub(crate) fn linear_claim_frontier_uncached(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Vec<ClaimFrontierClaim> {
    let mut claims = Vec::new();
    // Data definitions are only ever identified by their own symbol during
    // the walk; index them once so each named-type expansion is O(1) instead
    // of a whole-table scan per recursion level.
    let data_definitions_by_symbol: std::collections::HashMap<SymbolHandle, usize> = program
        .data_definitions()
        .iter()
        .enumerate()
        .map(|(index, definition)| (definition.symbol, index))
        .collect();
    // Multiplicity answers below look up the same declaration and parameter
    // identities; `type_multiplicity` rescans both tables per named type, so
    // index the parameter table once the same way.
    let parameters_by_symbol: std::collections::HashMap<
        SymbolHandle,
        (Multiplicity, typed_trees::data::TypeParameterKind),
    > = program
        .data_type_parameters
        .iter()
        .filter(|(_, parameter)| parameter.symbol.is_valid())
        .map(|(_, parameter)| {
            (
                parameter.symbol,
                (parameter.bounds.multiplicity, parameter.kind.clone()),
            )
        })
        .collect();
    append_linear_claim_frontier(
        program,
        type_reference,
        &[],
        &mut Vec::new(),
        &mut Vec::new(),
        &data_definitions_by_symbol,
        &parameters_by_symbol,
        &mut claims,
    );
    claims
}

fn append_linear_claim_frontier(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    path: &mut Vec<facts::PlaceSegment>,
    visiting: &mut Vec<SymbolHandle>,
    data_definitions_by_symbol: &std::collections::HashMap<SymbolHandle, usize>,
    parameters_by_symbol: &std::collections::HashMap<
        SymbolHandle,
        (Multiplicity, typed_trees::data::TypeParameterKind),
    >,
    claims: &mut Vec<ClaimFrontierClaim>,
) {
    if !type_reference.is_valid() {
        return;
    }
    let multiplicity = type_multiplicity_with_substitutions(
        program,
        type_reference,
        substitutions,
        data_definitions_by_symbol,
        parameters_by_symbol,
    );
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
                data_definitions_by_symbol,
                parameters_by_symbol,
                claims,
            );
            return;
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: typed_trees::types::FixedArrayLength::Literal(length),
        } => {
            for index in 0..*length {
                path.push(facts::PlaceSegment::FixedIndex { index });
                append_linear_claim_frontier(
                    program,
                    *element_type,
                    substitutions,
                    path,
                    visiting,
                    data_definitions_by_symbol,
                    parameters_by_symbol,
                    claims,
                );
                path.pop();
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
                    data_definitions_by_symbol,
                    parameters_by_symbol,
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
            let Some(definition) =
                find_data_definition(program, data_definitions_by_symbol, *symbol, name.as_str())
            else {
                return;
            };
            append_data_linear_claim_frontier(
                program,
                definition,
                substitutions,
                path,
                visiting,
                data_definitions_by_symbol,
                parameters_by_symbol,
                claims,
            );
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            arguments,
            ..
        } => {
            let Some(definition) = find_data_definition(
                program,
                data_definitions_by_symbol,
                *base_symbol,
                base_name.as_str(),
            ) else {
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
                data_definitions_by_symbol,
                parameters_by_symbol,
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
    path: &mut Vec<facts::PlaceSegment>,
    visiting: &mut Vec<SymbolHandle>,
    data_definitions_by_symbol: &std::collections::HashMap<SymbolHandle, usize>,
    parameters_by_symbol: &std::collections::HashMap<
        SymbolHandle,
        (Multiplicity, typed_trees::data::TypeParameterKind),
    >,
    claims: &mut Vec<ClaimFrontierClaim>,
) {
    if visiting.contains(&definition.symbol) {
        return;
    }
    visiting.push(definition.symbol);
    for member in program.data_members(definition) {
        match member {
            typed_trees::data::DataMember::Field(field) => {
                path.push(facts::PlaceSegment::Field {
                    symbol: field.symbol,
                });
                append_linear_claim_frontier(
                    program,
                    field.type_reference,
                    substitutions,
                    path,
                    visiting,
                    data_definitions_by_symbol,
                    parameters_by_symbol,
                    claims,
                );
                path.pop();
            }
            typed_trees::data::DataMember::Variant(variant) => {
                path.push(facts::PlaceSegment::Case {
                    variant: variant.symbol,
                });
                for field in program.data_payload_fields(variant) {
                    path.push(facts::PlaceSegment::Field {
                        symbol: field.symbol,
                    });
                    append_linear_claim_frontier(
                        program,
                        field.type_reference,
                        substitutions,
                        path,
                        visiting,
                        data_definitions_by_symbol,
                        parameters_by_symbol,
                        claims,
                    );
                    path.pop();
                }
                path.pop();
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
    data_definitions_by_symbol: &std::collections::HashMap<SymbolHandle, usize>,
    parameters_by_symbol: &std::collections::HashMap<
        SymbolHandle,
        (Multiplicity, typed_trees::data::TypeParameterKind),
    >,
) -> Multiplicity {
    if !type_reference.is_valid() {
        return Multiplicity::Affine;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => type_multiplicity_with_substitutions(
            program,
            *base_type,
            substitutions,
            data_definitions_by_symbol,
            parameters_by_symbol,
        ),
        TypeReferenceNode::FixedArray { element_type, .. } => type_multiplicity_with_substitutions(
            program,
            *element_type,
            substitutions,
            data_definitions_by_symbol,
            parameters_by_symbol,
        ),
        TypeReferenceNode::Named { symbol, .. } => substitutions
            .iter()
            .rev()
            .find_map(|(parameter, replacement)| {
                (*parameter == *symbol && *replacement != type_reference).then_some(*replacement)
            })
            .map(|replacement| {
                type_multiplicity_with_substitutions(
                    program,
                    replacement,
                    substitutions,
                    data_definitions_by_symbol,
                    parameters_by_symbol,
                )
            })
            .unwrap_or_else(|| {
                frontier_type_multiplicity(
                    program,
                    data_definitions_by_symbol,
                    parameters_by_symbol,
                    type_reference,
                )
            }),
        _ => frontier_type_multiplicity(
            program,
            data_definitions_by_symbol,
            parameters_by_symbol,
            type_reference,
        ),
    }
}

/// `TypedTrees::type_multiplicity` against the frontier's declaration and
/// parameter indexes: the named and generic arms below reach the declaration
/// through the symbol index instead of rescanning the definition table. The
/// order keeps the representation's exact precedence — a resolved symbol hits
/// the declaration index, a parameter symbol the parameter map, a spelled
/// primitive name the builtin domain, and only then a name lookup — so every
/// verdict is the same value the scanning version produced.
fn frontier_type_multiplicity(
    program: &TypedTrees,
    data_definitions_by_symbol: &std::collections::HashMap<SymbolHandle, usize>,
    parameters_by_symbol: &std::collections::HashMap<
        SymbolHandle,
        (Multiplicity, typed_trees::data::TypeParameterKind),
    >,
    type_reference: TypeReferenceHandle,
) -> Multiplicity {
    if !type_reference.is_valid() {
        return Multiplicity::Affine;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Unit => Multiplicity::Unrestricted,
        TypeReferenceNode::Constrained { base_type, .. } => frontier_type_multiplicity(
            program,
            data_definitions_by_symbol,
            parameters_by_symbol,
            *base_type,
        ),
        TypeReferenceNode::FixedArray { element_type, .. } => frontier_type_multiplicity(
            program,
            data_definitions_by_symbol,
            parameters_by_symbol,
            *element_type,
        ),
        TypeReferenceNode::DynamicTrait { .. } | TypeReferenceNode::Slice { .. } => {
            Multiplicity::Affine
        }
        TypeReferenceNode::Named { symbol, name } => {
            if symbol.is_valid()
                && let Some(index) = data_definitions_by_symbol.get(symbol)
            {
                return program.data_definitions()[*index].properties.multiplicity;
            }
            if symbol.is_valid()
                && let Some((multiplicity, kind)) = parameters_by_symbol.get(symbol)
            {
                if *multiplicity == Multiplicity::Affine
                    && matches!(kind, typed_trees::data::TypeParameterKind::Type)
                    && let Some(receiver) =
                        program.attached_receiver_parameter_multiplicity(*symbol)
                {
                    return receiver;
                }
                return *multiplicity;
            }
            if typed_trees::types::PrimitiveType::from_name(name.as_str()).is_some() {
                return Multiplicity::Unrestricted;
            }
            program
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == name.as_str())
                .map(|definition| definition.properties.multiplicity)
                .unwrap_or(Multiplicity::Affine)
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            ..
        } => {
            if base_symbol.is_valid()
                && let Some(index) = data_definitions_by_symbol.get(base_symbol)
            {
                return program.data_definitions()[*index].properties.multiplicity;
            }
            program
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == base_name.as_str())
                .map(|definition| definition.properties.multiplicity)
                .unwrap_or(Multiplicity::Affine)
        }
    }
}

/// The declaration a named type resolves to. A resolved symbol is the
/// declaration's identity; the spelled name only stands in for a reference
/// that never resolved, so a same-named declaration elsewhere cannot answer
/// for a resolved one.
fn find_data_definition<'program>(
    program: &'program TypedTrees,
    data_definitions_by_symbol: &std::collections::HashMap<SymbolHandle, usize>,
    symbol: SymbolHandle,
    name: &str,
) -> Option<&'program typed_trees::data::DataDefinition> {
    if symbol.is_valid() {
        return data_definitions_by_symbol
            .get(&symbol)
            .map(|index| &program.data_definitions()[*index]);
    }
    program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == name)
}
