use omega_core::arena::HandleSpan;
use omega_core::operator_spelling::OperatorSpelling;
use omega_core::symbols::SymbolHandle;

use crate::TypedTrees;
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

/// Outcome of resolving an operator spelling at an expression site against a
/// candidate set, keyed on operator path + parameter types. Per Wave 0 decision
/// #3 the spelling is the first-level discriminator; the operand types must then
/// uniquely select a single candidate. Return types never distinguish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpellingDispatch {
    /// No declared operator carries this spelling for these operand types.
    NoCandidate,
    /// Exactly one candidate matched: its index within the supplied slice.
    Unique(usize),
    /// Two or more candidates share the spelling and operand-type key, so the
    /// meaning is ambiguous and cannot be selected. Carries their indices.
    Ambiguous(Vec<usize>),
}

impl SpellingDispatch {
    pub fn unique(&self) -> Option<usize> {
        match self {
            Self::Unique(index) => Some(*index),
            _ => None,
        }
    }

    pub fn is_ambiguous(&self) -> bool {
        matches!(self, Self::Ambiguous(_))
    }
}

/// Resolve `spelling` against `operators`, selecting the unique candidate whose
/// normalized operand-type key matches `operand_key`. This is the expression
/// level dispatch rule: it keys on operator path + parameter types only.
///
/// `operand_key` is produced by [`operator_operand_signature`] over the same
/// `program`, ensuring the operand normalization is identical on both sides.
pub fn resolve_spelling(
    program: &TypedTrees,
    operators: &[OperatorDefinition],
    spelling: OperatorSpelling,
    operand_key: &str,
) -> SpellingDispatch {
    let matches: Vec<usize> = operators
        .iter()
        .enumerate()
        .filter(|(_, operator)| operator.spelling == Some(spelling))
        .filter(|(_, operator)| operator_operand_signature(program, operator) == operand_key)
        .map(|(index, _)| index)
        .collect();

    match matches.len() {
        0 => SpellingDispatch::NoCandidate,
        1 => SpellingDispatch::Unique(matches[0]),
        _ => SpellingDispatch::Ambiguous(matches),
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
        } => {
            let qualifier = if *is_mutable { "mut " } else { "" };
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
