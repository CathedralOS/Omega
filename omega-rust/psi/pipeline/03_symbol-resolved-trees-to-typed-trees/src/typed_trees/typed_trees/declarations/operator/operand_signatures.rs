//! Operator contract paths, requires clauses and operand signatures.

use crate::typed_trees::TypedTrees;
use crate::typed_trees::type_identity::TypeIdentityRequest;
use crate::typed_trees::typed_trees::declarations::operator::OperatorDefinition;
use crate::typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use symbols::SymbolHandle;

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
                .type_identity(TypeIdentityRequest {
                    binders: &binders,
                    ..TypeIdentityRequest::ordinary(parameter.type_reference)
                })
                .into_string()
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn normalized_operand_parameters(
    parameters: &[crate::typed_trees::signature::StateParameter],
) -> impl Iterator<Item = &crate::typed_trees::signature::StateParameter> {
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
