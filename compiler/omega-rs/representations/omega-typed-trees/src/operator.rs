use omega_core::arena::HandleSpan;
use omega_core::operator_spelling::OperatorSpelling;
use omega_core::symbols::SymbolHandle;

use crate::TypedTrees;
use crate::data::TypeParameter;
use crate::domain::DomainDefinition;
use crate::types::{TypeReferenceHandle, TypeReferenceNode};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OperatorDefinition {
    pub is_boundary: bool,
    pub symbol: SymbolHandle,
    pub name: HandleSpan<crate::name::Identifier>,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub parameters: HandleSpan<crate::signature::StateParameter>,
    pub return_type: crate::types::TypeReferenceHandle,
    pub contracts: HandleSpan<crate::signature::SignatureContract>,
    /// Optional `spelling` clause carried from syntax (Wave 0 decision #3).
    pub spelling: Option<OperatorSpelling>,
    pub token_count: usize,
}

/// A spelled operator meaning visible at a use site: a root operator, or a
/// domain operator together with its owning domain.
#[derive(Debug, Clone, Copy)]
pub struct SpelledOperator<'program> {
    pub operator: &'program OperatorDefinition,
    pub domain: Option<&'program DomainDefinition>,
}

/// Resolve an operator `spelling` at a use site. Per Wave 0 decision #3 the
/// spelling is the first-level discriminator; the receiver type (the first
/// operand) then narrows the candidate set when the site knows it. Return
/// types never distinguish. The surviving candidates decide the dispatch:
/// exactly one selects the meaning, none is a missing operator, two or more
/// are ambiguous.
///
/// This is the single use-site resolution authority. The checked stage records
/// its outcome as durable evidence (`CheckedOperatorFacts`) for diagnostics and
/// proof lowering rather than re-resolving.
pub fn resolve_spelling<'program>(
    program: &'program TypedTrees,
    spelling: OperatorSpelling,
    receiver_type: Option<TypeReferenceHandle>,
) -> Vec<SpelledOperator<'program>> {
    let root_candidates = program
        .operators()
        .iter()
        .filter(|operator| operator.spelling == Some(spelling))
        .map(|operator| SpelledOperator {
            operator,
            domain: None,
        });
    let domain_candidates = program.domain_definitions().iter().flat_map(|domain| {
        program
            .domain_operators(domain)
            .iter()
            .filter(move |operator| operator.spelling == Some(spelling))
            .map(move |operator| SpelledOperator {
                operator,
                domain: Some(domain),
            })
    });

    root_candidates
        .chain(domain_candidates)
        .filter(|candidate| match receiver_type {
            Some(receiver_type) => {
                operator_matches_receiver(program, candidate.operator, receiver_type)
            }
            None => true,
        })
        .collect()
}

/// Whether the operator's first parameter (its receiver) accepts a value of
/// `receiver_type`, binding the operator's own type parameters structurally.
fn operator_matches_receiver(
    program: &TypedTrees,
    operator: &OperatorDefinition,
    receiver_type: TypeReferenceHandle,
) -> bool {
    let Some(receiver_parameter) = program.operator_parameters(operator).first() else {
        return false;
    };
    type_reference_matches(
        program,
        receiver_type,
        receiver_parameter.type_reference,
        program.operator_type_parameters(operator),
        &mut Vec::new(),
    )
}

fn type_reference_matches(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    expected: TypeReferenceHandle,
    type_parameters: &[TypeParameter],
    bindings: &mut Vec<(SymbolHandle, TypeReferenceHandle)>,
) -> bool {
    if !actual.is_valid() || !expected.is_valid() {
        return false;
    }
    if let Some(type_parameter) = expected_type_parameter(program, expected, type_parameters) {
        if let Some((_, bound_actual)) = bindings
            .iter()
            .find(|(symbol, _)| *symbol == type_parameter.symbol)
        {
            return type_reference_matches(program, actual, *bound_actual, &[], &mut Vec::new());
        }
        bindings.push((type_parameter.symbol, actual));
        return true;
    }

    match (
        program.type_reference_table.type_reference(actual),
        program.type_reference_table.type_reference(expected),
    ) {
        (
            TypeReferenceNode::Reference {
                referee: actual_referee,
                is_mutable: actual_mutable,
                is_relaxed: actual_relaxed,
                // Lifetimes do not affect operator/conformance type matching.
                lifetime: _,
            },
            TypeReferenceNode::Reference {
                referee: expected_referee,
                is_mutable: expected_mutable,
                is_relaxed: expected_relaxed,
                lifetime: _,
            },
        ) => {
            actual_mutable == expected_mutable
                && actual_relaxed == expected_relaxed
                && type_reference_matches(
                    program,
                    *actual_referee,
                    *expected_referee,
                    type_parameters,
                    bindings,
                )
        }
        (
            TypeReferenceNode::Constrained {
                base_type: actual_base,
                ..
            },
            _,
        ) => type_reference_matches(program, *actual_base, expected, type_parameters, bindings),
        (
            _,
            TypeReferenceNode::Constrained {
                base_type: expected_base,
                ..
            },
        ) => type_reference_matches(program, actual, *expected_base, type_parameters, bindings),
        (
            TypeReferenceNode::FixedArray {
                element_type: actual_element,
                length: actual_length,
            },
            TypeReferenceNode::FixedArray {
                element_type: expected_element,
                length: expected_length,
            },
        ) => {
            actual_length == expected_length
                && type_reference_matches(
                    program,
                    *actual_element,
                    *expected_element,
                    type_parameters,
                    bindings,
                )
        }
        (
            TypeReferenceNode::Slice {
                element_type: actual_element,
            },
            TypeReferenceNode::Slice {
                element_type: expected_element,
            },
        ) => type_reference_matches(
            program,
            *actual_element,
            *expected_element,
            type_parameters,
            bindings,
        ),
        (
            TypeReferenceNode::Named {
                symbol: actual_symbol,
                name: actual_name,
            },
            TypeReferenceNode::Named {
                symbol: expected_symbol,
                name: expected_name,
            },
        ) => {
            (actual_symbol.is_valid() && actual_symbol == expected_symbol)
                || actual_name == expected_name
        }
        (
            TypeReferenceNode::Generic {
                base_symbol: actual_symbol,
                base_name: actual_name,
                arguments: actual_arguments,
            },
            TypeReferenceNode::Generic {
                base_symbol: expected_symbol,
                base_name: expected_name,
                arguments: expected_arguments,
            },
        ) => {
            ((actual_symbol.is_valid() && actual_symbol == expected_symbol)
                || actual_name == expected_name)
                && type_reference_spans_match(
                    program,
                    *actual_arguments,
                    *expected_arguments,
                    type_parameters,
                    bindings,
                )
        }
        (TypeReferenceNode::Unit, TypeReferenceNode::Unit) => true,
        _ => false,
    }
}

fn type_reference_spans_match(
    program: &TypedTrees,
    actual: HandleSpan<TypeReferenceHandle>,
    expected: HandleSpan<TypeReferenceHandle>,
    type_parameters: &[TypeParameter],
    bindings: &mut Vec<(SymbolHandle, TypeReferenceHandle)>,
) -> bool {
    let actual = program.type_reference_table.type_reference_handles(actual);
    let expected = program
        .type_reference_table
        .type_reference_handles(expected);
    actual.len() == expected.len()
        && actual.iter().zip(expected).all(|(actual, expected)| {
            type_reference_matches(program, *actual, *expected, type_parameters, bindings)
        })
}

fn expected_type_parameter<'a>(
    program: &TypedTrees,
    expected: TypeReferenceHandle,
    type_parameters: &'a [TypeParameter],
) -> Option<&'a TypeParameter> {
    match program.type_reference_table.type_reference(expected) {
        TypeReferenceNode::Named { symbol, name }
        | TypeReferenceNode::Generic {
            base_symbol: symbol,
            base_name: name,
            ..
        } => type_parameters.iter().find(|parameter| {
            (symbol.is_valid() && parameter.symbol == *symbol) || parameter.name == *name
        }),
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Constrained { .. }
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Unit => None,
    }
}

/// All candidate indices carrying `spelling`, regardless of operand type. Useful
/// when a site only knows its receiver shape (e.g. "this is a slice") rather
/// than a fully normalized operand key.
pub fn candidates_for_spelling(
    operators: &[OperatorDefinition],
    spelling: OperatorSpelling,
) -> Vec<usize> {
    operators
        .iter()
        .enumerate()
        .filter(|(_, operator)| operator.spelling == Some(spelling))
        .map(|(index, _)| index)
        .collect()
}

/// The browsable path of the boundary operator governing a spelling (e.g.
/// `Slice::range`), taken from the `requires` contract owner of the first
/// spelled candidate that carries one. Failed subslice/index bounds
/// diagnostics name this path together with the spelling so the user can
/// look up the operator declaration and read the contract that sourced the
/// obligation.
///
/// Returns `None` when no spelled candidate carries a `requires` contract or
/// the carrying operator has no path members to name.
pub fn operator_contract_path(
    program: &TypedTrees,
    operators: &[OperatorDefinition],
    spelling: OperatorSpelling,
) -> Option<String> {
    operators
        .iter()
        .filter(|operator| operator.spelling == Some(spelling))
        .find(|operator| {
            program
                .operator_contracts(operator)
                .iter()
                .any(|contract| contract.kind == crate::signature::SignatureContractKind::Requires)
        })
        .and_then(|operator| {
            let path = program
                .operator_path_members(operator.name)
                .iter()
                .map(|member| member.as_str().to_owned())
                .collect::<Vec<_>>()
                .join("::");
            (!path.is_empty()).then_some(path)
        })
}

/// The `requires` clauses for a spelling, rendered as readable bound
/// obligations. The clause text is keyed on the spelling so a failed bound
/// reports the precise obligation (e.g.
/// `requires start <= end && end <= items.len` for `[..]`). Returns an empty
/// vector when no spelled candidate carries a `requires` contract, signalling
/// the caller that the obligation is not operator-sourced.
pub fn operator_requires_clauses(
    program: &TypedTrees,
    operators: &[OperatorDefinition],
    spelling: OperatorSpelling,
) -> Vec<String> {
    let has_requires = operators
        .iter()
        .filter(|operator| operator.spelling == Some(spelling))
        .any(|operator| {
            program
                .operator_contracts(operator)
                .iter()
                .any(|contract| contract.kind == crate::signature::SignatureContractKind::Requires)
        });
    if !has_requires {
        return Vec::new();
    }

    match spelling {
        OperatorSpelling::Index => vec!["index < items.len".to_owned()],
        OperatorSpelling::Range => vec!["start <= end".to_owned(), "end <= items.len".to_owned()],
        _ => Vec::new(),
    }
}

/// The canonical operand-type signature for an operator: its parameter types
/// normalized over the operator's own type parameters. The operator name and
/// return type are deliberately excluded — only operand types discriminate
/// within a spelling. Shared so dispatch and ambiguity validation agree.
pub fn operator_operand_signature(program: &TypedTrees, operator: &OperatorDefinition) -> String {
    let mut normalizer = TypeParameterNormalizer::new(
        program
            .operator_type_parameters(operator)
            .iter()
            .map(|parameter| parameter.symbol)
            .collect(),
    );
    program
        .operator_parameters(operator)
        .iter()
        .map(|parameter| {
            canonical_type_reference(program, parameter.type_reference, &mut normalizer)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn canonical_type_reference(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    normalizer: &mut TypeParameterNormalizer,
) -> String {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference {
            referee,
            is_mutable,
            is_relaxed,
            // Canonical form omits lifetimes (not part of type identity).
            lifetime: _,
        } => {
            let qualifier = match (*is_mutable, *is_relaxed) {
                (true, true) => "mut relaxed ",
                (true, false) => "mut ",
                (false, true) => "relaxed ",
                (false, false) => "",
            };
            format!(
                "&{qualifier}{}",
                canonical_type_reference(program, *referee, normalizer)
            )
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            format!(
                "{}[constraints]",
                canonical_type_reference(program, *base_type, normalizer)
            )
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            format!(
                "[{}; {length}]",
                canonical_type_reference(program, *element_type, normalizer)
            )
        }
        TypeReferenceNode::Slice { element_type } => {
            format!(
                "[{}]",
                canonical_type_reference(program, *element_type, normalizer)
            )
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            arguments,
        } => {
            let base = canonical_named_type(*base_symbol, base_name.as_str(), normalizer);
            let arguments = program
                .type_reference_table
                .type_reference_handles(*arguments)
                .iter()
                .map(|argument| canonical_type_reference(program, *argument, normalizer))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{base}<{arguments}>")
        }
        TypeReferenceNode::Named { symbol, name } => {
            canonical_named_type(*symbol, name.as_str(), normalizer)
        }
        TypeReferenceNode::DynamicTrait { symbol, name } => {
            format!(
                "dyn {}",
                canonical_named_type(*symbol, name.as_str(), normalizer)
            )
        }
        TypeReferenceNode::Unit => "()".to_owned(),
    }
}

fn canonical_named_type(
    symbol: SymbolHandle,
    name: &str,
    normalizer: &mut TypeParameterNormalizer,
) -> String {
    normalizer
        .canonical_index(symbol)
        .map(|index| format!("${index}"))
        .unwrap_or_else(|| name.to_owned())
}

struct TypeParameterNormalizer {
    declared: Vec<SymbolHandle>,
    canonical: Vec<(SymbolHandle, usize)>,
}

impl TypeParameterNormalizer {
    fn new(declared: Vec<SymbolHandle>) -> Self {
        Self {
            declared,
            canonical: Vec::new(),
        }
    }

    fn canonical_index(&mut self, symbol: SymbolHandle) -> Option<usize> {
        if !self.declared.contains(&symbol) {
            return None;
        }
        if let Some((_, index)) = self
            .canonical
            .iter()
            .find(|(candidate, _)| *candidate == symbol)
        {
            return Some(*index);
        }
        let index = self.canonical.len();
        self.canonical.push((symbol, index));
        Some(index)
    }
}
