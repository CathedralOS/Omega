//! Operator contract paths, requires clauses and operand signatures.

use crate::TypedTrees;
use crate::typed_trees::declarations::operator::OperatorDefinition;
use crate::types::{TypeReferenceHandle, TypeReferenceNode};
use language_core::operator_spelling::OperatorSpelling;
use symbols::SymbolHandle;

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
    let parameters = program.operator_parameters(operator);
    for parameter in normalized_operand_parameters(parameters) {
        collect_type_parameter_occurrences(program, parameter.type_reference, &mut normalizer);
    }
    let binders = normalizer.bindings();
    normalized_operand_parameters(parameters)
        .map(|parameter| {
            program
                .normalized_type_identity_with_binders(parameter.type_reference, &binders)
                .into_string()
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn normalized_operand_parameters(
    parameters: &[crate::signature::StateParameter],
) -> impl Iterator<Item = &crate::signature::StateParameter> {
    parameters
        .iter()
        .filter(|parameter| parameter.is_self)
        .chain(parameters.iter().filter(|parameter| !parameter.is_self))
}

pub(crate) fn collect_type_parameter_occurrences(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    normalizer: &mut TypeParameterNormalizer,
) {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            collect_type_parameter_occurrences(program, *referee, normalizer);
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            collect_type_parameter_occurrences(program, *base_type, normalizer);
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            collect_type_parameter_occurrences(program, *element_type, normalizer);
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            normalizer.canonical_index(*base_symbol);
            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                collect_type_parameter_occurrences(program, *argument, normalizer);
            }
        }
        TypeReferenceNode::Named { symbol, .. }
        | TypeReferenceNode::DynamicTrait { symbol, .. } => {
            normalizer.canonical_index(*symbol);
        }
        TypeReferenceNode::ConstExpression(_) => {}
        TypeReferenceNode::Unit => {}
    }
}

pub(crate) struct TypeParameterNormalizer {
    declared: Vec<SymbolHandle>,
    canonical: Vec<(SymbolHandle, usize)>,
}

impl TypeParameterNormalizer {
    pub(crate) fn new(declared: Vec<SymbolHandle>) -> Self {
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

    pub(crate) fn bindings(&self) -> Vec<(SymbolHandle, String)> {
        self.canonical
            .iter()
            .map(|(symbol, index)| (*symbol, format!("${index}")))
            .collect()
    }
}
