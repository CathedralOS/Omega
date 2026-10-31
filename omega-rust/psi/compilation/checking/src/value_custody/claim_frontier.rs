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

/// An owned `self` receiver's custody obligation. The parameter's authored
/// `Named` reference spells the machine itself rather than a data
/// declaration, so `linear_claim_frontier` on it finds no definition and
/// reports nothing; `self` is ordinary parameter syntax for the attached
/// data type, so its claims enumerate against the attachment — the stored
/// generic application when one exists, else the bare declaration.
///
/// The returned multiplicity is the receiver value's whole multiplicity,
/// matching what `type_multiplicity` answers for an ordinary parameter's
/// reference. Claims carry the field-level type references of the data
/// declaration; a whole-value linear claim carries an invalid reference
/// because no `Named` reference to the attached data exists in the table.
pub fn owned_receiver_claims(
    program: &dyn crate::CheckingInput,
    machine_symbol: SymbolHandle,
) -> (Multiplicity, Vec<ClaimFrontierClaim>) {
    let Some(machine) = program.machine(machine_symbol) else {
        return (Multiplicity::Affine, Vec::new());
    };
    if machine.attached_data_application.is_valid() {
        return (
            program.type_multiplicity(machine.attached_data_application),
            linear_claim_frontier(program, machine.attached_data_application),
        );
    }
    let Some(definition) = program.data_definition_by_symbol(machine.attached_data_symbol) else {
        return (Multiplicity::Affine, Vec::new());
    };
    let multiplicity = definition.properties.multiplicity;
    if multiplicity == Multiplicity::Linear {
        return (
            multiplicity,
            vec![ClaimFrontierClaim {
                path: Vec::new(),
                type_reference: TypeReferenceHandle::invalid(),
                conditional: false,
            }],
        );
    }
    let mut claims = Vec::new();
    append_data_linear_claim_frontier(
        program,
        definition,
        &[],
        &mut Vec::new(),
        &mut Vec::new(),
        &mut claims,
        false,
    );
    (multiplicity, claims)
}

pub fn linear_claim_frontier(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Vec<ClaimFrontierClaim> {
    crate::value_custody::type_properties::memoized_claim_frontier(program, type_reference)
}

/// Whether the exact type frontier has any linear obligation, including
/// erased fields. This uses the same declaration/substitution walk as the
/// complete frontier, stopping at the first claim and visiting arrays once.
pub fn type_carries_linear_obligation(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> bool {
    let mut claims = Vec::new();
    append_linear_claim_frontier(
        program,
        type_reference,
        &[],
        &mut Vec::new(),
        &mut Vec::new(),
        &mut claims,
        true,
    );
    !claims.is_empty()
}

/// A fresh custody walk on a memo miss or a mutable preparation input.
pub(crate) fn linear_claim_frontier_uncached(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Vec<ClaimFrontierClaim> {
    let mut claims = Vec::new();
    // Declaration relationships stay typed-owned; only this walk's result
    // belongs to the immutable checking invocation.
    append_linear_claim_frontier(
        program,
        type_reference,
        &[],
        &mut Vec::new(),
        &mut Vec::new(),
        &mut claims,
        false,
    );
    claims
}

fn append_linear_claim_frontier(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    path: &mut Vec<facts::PlaceSegment>,
    visiting: &mut Vec<SymbolHandle>,
    claims: &mut Vec<ClaimFrontierClaim>,
    first_only: bool,
) {
    if !type_reference.is_valid() {
        return;
    }
    let multiplicity = type_multiplicity(program, type_reference, substitutions);
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
                first_only,
            );
            return;
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: typed_trees::types::FixedArrayLength::Literal(length),
        } => {
            if *length == 0 {
                return;
            }
            if first_only {
                path.push(facts::PlaceSegment::FixedIndex { index: 0 });
                append_linear_claim_frontier(
                    program,
                    *element_type,
                    substitutions,
                    path,
                    visiting,
                    claims,
                    true,
                );
                path.pop();
                return;
            }
            // Every element has the same frontier below its index. Walk one
            // element and repeat its claims per index: a `[u8; 16384]` buffer
            // would otherwise walk sixteen thousand identical elements.
            let mut element_claims = Vec::new();
            append_linear_claim_frontier(
                program,
                *element_type,
                substitutions,
                &mut Vec::new(),
                visiting,
                &mut element_claims,
                first_only,
            );
            let prefix_is_conditional = path
                .iter()
                .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }));
            for index in 0..*length {
                for element_claim in &element_claims {
                    let mut claim_path =
                        Vec::with_capacity(path.len() + 1 + element_claim.path.len());
                    claim_path.extend_from_slice(path);
                    claim_path.push(facts::PlaceSegment::FixedIndex { index });
                    claim_path.extend_from_slice(&element_claim.path);
                    claims.push(ClaimFrontierClaim {
                        path: claim_path,
                        type_reference: element_claim.type_reference,
                        conditional: prefix_is_conditional || element_claim.conditional,
                    });
                }
            }
            return;
        }
        TypeReferenceNode::Named { symbol, .. } => {
            if let Some((replacement, enclosing)) =
                enclosing_substitution(substitutions, *symbol, type_reference)
            {
                append_linear_claim_frontier(
                    program,
                    replacement,
                    enclosing,
                    path,
                    visiting,
                    claims,
                    first_only,
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
                first_only,
            );
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            arguments,
            ..
        } => {
            // Preparation retains a closed application's concrete members.
            // Their symbols, not the template's similarly named fields, are
            // the places receiving custody and typed path resolution use.
            // Before preparation, an open application still walks its exact
            // declaration under substitutions; that does not grant a closed
            // instance or rename its members.
            if let Some(definition) = program.closed_data_definition(type_reference) {
                append_data_linear_claim_frontier(
                    program,
                    definition,
                    substitutions,
                    path,
                    visiting,
                    claims,
                    first_only,
                );
                return;
            }
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
                first_only,
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
    program: &dyn crate::CheckingInput,
    definition: &typed_trees::data::DataDefinition,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    path: &mut Vec<facts::PlaceSegment>,
    visiting: &mut Vec<SymbolHandle>,
    claims: &mut Vec<ClaimFrontierClaim>,
    first_only: bool,
) {
    if visiting.contains(&definition.symbol) {
        return;
    }
    visiting.push(definition.symbol);
    for member in program.data_members(definition) {
        if first_only && !claims.is_empty() {
            break;
        }
        match member {
            typed_trees::data::DataMember::Field(field) => {
                append_field_claim_frontier(
                    program,
                    field,
                    substitutions,
                    path,
                    visiting,
                    claims,
                    first_only,
                );
            }
            typed_trees::data::DataMember::Variant(variant) => {
                path.push(facts::PlaceSegment::Case {
                    variant: variant.symbol,
                });
                for field in program.data_payload_fields(variant) {
                    if first_only && !claims.is_empty() {
                        break;
                    }
                    append_field_claim_frontier(
                        program,
                        field,
                        substitutions,
                        path,
                        visiting,
                        claims,
                        first_only,
                    );
                }
                path.pop();
            }
        }
    }
    visiting.pop();
}

/// Descend into `field`'s declared type under the current path prefix.
#[allow(clippy::too_many_arguments)]
fn append_field_claim_frontier(
    program: &dyn crate::CheckingInput,
    field: &typed_trees::data::DataField,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    path: &mut Vec<facts::PlaceSegment>,
    visiting: &mut Vec<SymbolHandle>,
    claims: &mut Vec<ClaimFrontierClaim>,
    first_only: bool,
) {
    path.push(facts::PlaceSegment::Field {
        symbol: field.symbol,
    });
    append_linear_claim_frontier(
        program,
        field.type_reference,
        substitutions,
        path,
        visiting,
        claims,
        first_only,
    );
    path.pop();
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
fn type_multiplicity(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Multiplicity {
    if !type_reference.is_valid() {
        return Multiplicity::Affine;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_multiplicity(program, *base_type, substitutions)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            type_multiplicity(program, *element_type, substitutions)
        }
        TypeReferenceNode::Named { symbol, .. } => {
            match enclosing_substitution(substitutions, *symbol, type_reference) {
                Some((replacement, enclosing)) => {
                    type_multiplicity(program, replacement, enclosing)
                }
                None => frontier_type_multiplicity(program, type_reference),
            }
        }
        _ => frontier_type_multiplicity(program, type_reference),
    }
}

/// The argument bound to parameter `symbol`, with the substitutions that
/// were in force where that argument was spelled. A generic application
/// appends its own parameter bindings after the enclosing ones, and an
/// argument can name an enclosing parameter (`Pair<B, A>` inside
/// `swap<A, B>`): resolving it against the whole list would find the
/// application's own `A -> B`, `B -> A` again and never reach the enclosing
/// `A -> u64`. A self-mapping entry is no binding at all.
fn enclosing_substitution(
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    symbol: SymbolHandle,
    occurrence: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, &[(SymbolHandle, TypeReferenceHandle)])> {
    substitutions
        .iter()
        .enumerate()
        .rev()
        .find_map(|(position, (parameter, replacement))| {
            (*parameter == symbol && *replacement != occurrence)
                .then_some((*replacement, &substitutions[..position]))
        })
}

/// `TypedTrees::type_multiplicity` against the frontier's declaration and
/// parameter indexes: named and generic arms reach the typed-owned declaration
/// relationships instead of rescanning the definition table. The
/// order keeps the representation's exact precedence — a resolved symbol hits
/// the declaration index, a parameter symbol its exact declaration, a spelled
/// primitive name the builtin domain, and only then a name lookup — so every
/// verdict is the same value the scanning version produced.
fn frontier_type_multiplicity(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Multiplicity {
    if !type_reference.is_valid() {
        return Multiplicity::Affine;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Unit => Multiplicity::Unrestricted,
        TypeReferenceNode::Constrained { base_type, .. } => {
            frontier_type_multiplicity(program, *base_type)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            frontier_type_multiplicity(program, *element_type)
        }
        TypeReferenceNode::DynamicTrait { .. } | TypeReferenceNode::Slice { .. } => {
            Multiplicity::Affine
        }
        TypeReferenceNode::Named { symbol, name } => {
            if symbol.is_valid()
                && let Some(definition) = last_data_definition_by_symbol(program, *symbol)
            {
                return definition.properties.multiplicity;
            }
            if symbol.is_valid()
                && let Some(parameter) = program.data_type_parameter_by_symbol(*symbol)
            {
                return parameter.bounds.multiplicity;
            }
            if typed_trees::types::PrimitiveType::from_name(name.as_str()).is_some() {
                return Multiplicity::Unrestricted;
            }
            program
                .data_definition_by_name_str(name.as_str())
                .map(|definition| definition.properties.multiplicity)
                .unwrap_or(Multiplicity::Affine)
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            ..
        } => {
            if base_symbol.is_valid()
                && let Some(definition) = last_data_definition_by_symbol(program, *base_symbol)
            {
                return definition.properties.multiplicity;
            }
            program
                .data_definition_by_name_str(base_name.as_str())
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
    program: &'program dyn crate::CheckingInput,
    symbol: SymbolHandle,
    name: &str,
) -> Option<&'program typed_trees::data::DataDefinition> {
    if symbol.is_valid() {
        return last_data_definition_by_symbol(program, symbol);
    }
    program.data_definition_by_name_str(name)
}

/// Custody formerly collected declaration positions into a map, whose last
/// duplicate won. Preserve that answer through the representation's complete
/// position index rather than substituting its first-match convenience query.
fn last_data_definition_by_symbol(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<&typed_trees::data::DataDefinition> {
    let position = program
        .with_data_definition_index(|index| index.positions_for_symbol(symbol).last().copied())?;
    program.data_definitions().get(position as usize)
}
