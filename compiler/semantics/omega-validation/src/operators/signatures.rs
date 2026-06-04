use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(super) fn operator_signature_key(
    program: &TypedTrees,
    operator: &omega_typed_trees::operator::OperatorDefinition,
) -> String {
    let mut type_parameters = TypeParameterNormalizer::new(
        program
            .operator_type_parameters(operator)
            .iter()
            .map(|parameter| parameter.symbol)
            .collect(),
    );
    let parameter_types = program
        .operator_parameters(operator)
        .iter()
        .map(|parameter| {
            canonical_type_reference(program, parameter.type_reference, &mut type_parameters)
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("{}({parameter_types})", operator_name(program, operator))
}

/// The canonical operand-type key for an operator (its parameter types,
/// normalized over type parameters) without the operator name. Used by the
/// spelling-overlap ambiguity rule, where the spelling already serves as the
/// first-level discriminator.
///
/// Delegates to the shared `operator_operand_signature` in `omega-typed-trees`
/// so declaration-level ambiguity validation and expression-level spelling
/// dispatch normalize operand types identically.
pub(super) fn operator_operand_key(
    program: &TypedTrees,
    operator: &omega_typed_trees::operator::OperatorDefinition,
) -> String {
    omega_typed_trees::operator::operator_operand_signature(program, operator)
}

pub(super) fn operator_name(
    program: &TypedTrees,
    operator: &omega_typed_trees::operator::OperatorDefinition,
) -> String {
    program
        .operator_path_members(operator.name)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::")
}

fn canonical_type_reference(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    type_parameters: &mut TypeParameterNormalizer,
) -> String {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference {
            referee,
            is_mutable,
            is_relaxed,
        } => {
            let qualifier = match (*is_mutable, *is_relaxed) {
                (true, true) => "mut relaxed ",
                (true, false) => "mut ",
                (false, true) => "relaxed ",
                (false, false) => "",
            };
            format!(
                "&{qualifier}{}",
                canonical_type_reference(program, *referee, type_parameters)
            )
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            format!(
                "{}[constraints]",
                canonical_type_reference(program, *base_type, type_parameters)
            )
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            format!(
                "[{}; {length}]",
                canonical_type_reference(program, *element_type, type_parameters)
            )
        }
        TypeReferenceNode::Slice { element_type } => {
            format!(
                "[{}]",
                canonical_type_reference(program, *element_type, type_parameters)
            )
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            arguments,
        } => {
            let base = canonical_named_type(*base_symbol, base_name.as_str(), type_parameters);
            let arguments = program
                .type_reference_table
                .type_reference_handles(*arguments)
                .iter()
                .map(|argument| canonical_type_reference(program, *argument, type_parameters))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{base}<{arguments}>")
        }
        TypeReferenceNode::Named { symbol, name } => {
            canonical_named_type(*symbol, name.as_str(), type_parameters)
        }
        TypeReferenceNode::DynamicTrait { symbol, name } => {
            format!(
                "dyn {}",
                canonical_named_type(*symbol, name.as_str(), type_parameters)
            )
        }
        TypeReferenceNode::Unit => "()".to_owned(),
    }
}

fn canonical_named_type(
    symbol: SymbolHandle,
    name: &str,
    type_parameters: &mut TypeParameterNormalizer,
) -> String {
    type_parameters
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
