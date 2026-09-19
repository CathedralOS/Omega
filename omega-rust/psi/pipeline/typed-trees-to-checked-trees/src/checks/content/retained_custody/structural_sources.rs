//! Exact qualification sources beneath structural value shells.
//!
//! Borrowed-source rejection and boundary partition coverage share this walk.
//! It preserves lexical generic bindings and structural paths, visits an array's
//! element type once, and reports incomplete expansion explicitly. These are
//! declaration subjects, not live claims or authority evidence.

use super::{DomainApplication, expression_names_parameter};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{DataMember, TypeParameterKind};
use typed_trees::domain::ProofFact;
use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
use typed_trees::types::{
    FixedArrayLength, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

#[derive(Clone, PartialEq, Eq)]
pub(super) struct ContentSource {
    pub(super) domain: DomainApplication,
    pub(super) borrowed: bool,
    pub(super) nested: bool,
    pub(super) path: Vec<SourceSegment>,
}

/// Array coverage stays symbolic: source discovery visits an element type once,
/// even when its declared extent is much larger than the authored proof.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SourceSegment {
    Field(SymbolHandle),
    Case(SymbolHandle),
    Elements(Option<usize>),
}

pub(super) fn parameter_sources(
    program: &TypedTrees,
    parameter: &StateParameter,
    contracts: &[&SignatureContract],
) -> (Vec<ContentSource>, bool) {
    let (mut sources, complete) = domain_sources(program, parameter.type_reference);
    for contract in contracts
        .iter()
        .filter(|contract| contract.kind == SignatureContractKind::Requires)
    {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            if let ProofFact::Membership(membership) = fact
                && membership.domain_symbol.is_valid()
                && membership.semantic_domain.is_valid()
                && expression_names_parameter(program, membership.value, parameter)
            {
                let source = ContentSource {
                    domain: DomainApplication {
                        symbol: membership.domain_symbol,
                        semantic_domain: membership.semantic_domain,
                    },
                    borrowed: super::direct_reference(program, parameter.type_reference).is_some(),
                    nested: false,
                    path: Vec::new(),
                };
                if !sources.contains(&source) {
                    sources.push(source);
                }
            }
        }
    }
    (sources, complete)
}

// Each actual retains the lexical environment in which it was supplied. A
// reused generic binder must not capture its own outer argument.
#[derive(Clone, Copy)]
struct TypeBinding {
    parameter: SymbolHandle,
    argument: TypeReferenceHandle,
    outer_scope: usize,
}

pub(super) fn domain_sources(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> (Vec<ContentSource>, bool) {
    let mut sources = Vec::new();
    let complete = append_sources(
        program,
        reference,
        &[],
        false,
        false,
        &[],
        &mut Vec::new(),
        &mut sources,
    );
    (sources, complete)
}

#[allow(clippy::too_many_arguments)]
fn append_sources(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    substitutions: &[TypeBinding],
    borrowed: bool,
    nested: bool,
    path: &[SourceSegment],
    visiting: &mut Vec<(SymbolHandle, usize)>,
    sources: &mut Vec<ContentSource>,
) -> bool {
    let (reference, substitutions) = substituted_head(program, reference, substitutions);
    if !reference.is_valid() {
        return true;
    }
    let borrowed = borrowed || reference_shell(program, reference, substitutions);
    let definition_symbol = match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            for constraint in program.type_reference_table.constraints(*constraints) {
                if let TypeConstraintNode::Domain(domain) = constraint {
                    let source = ContentSource {
                        domain: DomainApplication {
                            symbol: domain.symbol,
                            semantic_domain: domain.semantic_id,
                        },
                        borrowed,
                        nested,
                        path: path.to_vec(),
                    };
                    if domain.semantic_id.is_valid() && !sources.contains(&source) {
                        sources.push(source);
                    }
                }
            }
            return append_sources(
                program,
                *base_type,
                substitutions,
                borrowed,
                nested,
                path,
                visiting,
                sources,
            );
        }
        TypeReferenceNode::Reference { referee, .. } => {
            return append_sources(
                program,
                *referee,
                substitutions,
                true,
                nested,
                path,
                visiting,
                sources,
            );
        }
        TypeReferenceNode::FixedArray {
            length: FixedArrayLength::Literal(0),
            ..
        } => return true,
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(_),
        }
        | TypeReferenceNode::Slice { element_type } => {
            let length = match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::FixedArray {
                    length: FixedArrayLength::Literal(length),
                    ..
                } => Some(*length),
                _ => None,
            };
            let mut element_path = path.to_vec();
            element_path.push(SourceSegment::Elements(length));
            return append_sources(
                program,
                *element_type,
                substitutions,
                borrowed,
                true,
                &element_path,
                visiting,
                sources,
            );
        }
        TypeReferenceNode::FixedArray { .. } => return false,
        TypeReferenceNode::Named { symbol, .. } => *symbol,
        TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
        _ => return true,
    };
    let Some(definition) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == definition_symbol)
    else {
        return true;
    };
    // Finite Wrapper<Wrapper<T>> consumes type-argument structure. Recursive
    // Node<Wrapper<T>> does not. This is a termination measure over the type
    // tree, not a resource capacity or an arbitrary recursion/fuel limit.
    let size = type_size(program, reference, substitutions);
    if visiting
        .iter()
        .any(|(owner, ancestor_size)| *owner == definition_symbol && size >= *ancestor_size)
    {
        return false;
    }
    let mut instantiated = substitutions.to_vec();
    if let TypeReferenceNode::Generic { arguments, .. } =
        program.type_reference_table.type_reference(reference)
    {
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
                    matches!(parameter.kind, TypeParameterKind::Type).then_some(TypeBinding {
                        parameter: parameter.symbol,
                        argument: *argument,
                        outer_scope: substitutions.len(),
                    })
                }),
        );
    }
    visiting.push((definition_symbol, size));
    let mut complete = true;
    for member in program.data_members(definition) {
        match member {
            DataMember::Field(field) => {
                let mut field_path = path.to_vec();
                field_path.push(SourceSegment::Field(field.symbol));
                complete &= append_sources(
                    program,
                    field.type_reference,
                    &instantiated,
                    borrowed,
                    true,
                    &field_path,
                    visiting,
                    sources,
                );
            }
            DataMember::Variant(variant) => {
                for field in program.data_payload_fields(variant) {
                    let mut field_path = path.to_vec();
                    field_path.push(SourceSegment::Case(variant.symbol));
                    field_path.push(SourceSegment::Field(field.symbol));
                    complete &= append_sources(
                        program,
                        field.type_reference,
                        &instantiated,
                        borrowed,
                        true,
                        &field_path,
                        visiting,
                        sources,
                    );
                }
            }
        }
    }
    visiting.pop();
    complete
}

fn substituted_head<'scope>(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
    mut substitutions: &'scope [TypeBinding],
) -> (TypeReferenceHandle, &'scope [TypeBinding]) {
    while let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(reference)
    {
        let Some(binding) = substitutions
            .iter()
            .rev()
            .find(|binding| binding.parameter == *symbol)
        else {
            break;
        };
        reference = binding.argument;
        substitutions = &substitutions[..binding.outer_scope];
    }
    (reference, substitutions)
}

fn reference_shell(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
    mut substitutions: &[TypeBinding],
) -> bool {
    loop {
        (reference, substitutions) = substituted_head(program, reference, substitutions);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Reference { .. } => return true,
            _ => return false,
        }
    }
}

fn type_size(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    substitutions: &[TypeBinding],
) -> usize {
    let (reference, substitutions) = substituted_head(program, reference, substitutions);
    let children = match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_size(program, *base_type, substitutions)
        }
        TypeReferenceNode::Reference { referee, .. } => type_size(program, *referee, substitutions),
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            type_size(program, *element_type, substitutions)
        }
        TypeReferenceNode::Generic { arguments, .. } => program
            .type_reference_table
            .type_reference_handles(*arguments)
            .iter()
            .fold(0usize, |size, argument| {
                size.saturating_add(type_size(program, *argument, substitutions))
            }),
        _ => 0,
    };
    children.saturating_add(1)
}
