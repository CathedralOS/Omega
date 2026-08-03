use psi_typed_trees::TypedTrees;
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Explain why one requirement is absent from a local `dyn Trait` surface.
/// Eligibility is intentionally per requirement: an ineligible sibling does
/// not invalidate calls to the rest of the trait.
pub(crate) fn dynamic_requirement_call_error(
    program: &TypedTrees,
    receiver_type: TypeReferenceHandle,
    target: &str,
) -> Option<String> {
    let trait_symbol = dynamic_trait_symbol(program, receiver_type)?;
    let trait_definition = program
        .traits()
        .iter()
        .find(|definition| definition.symbol == trait_symbol)?;
    let requirement = program
        .trait_machine_signatures(trait_definition)
        .iter()
        .find(|signature| signature.name.as_str() == target)?;

    let reason = if trait_definition.is_boundary {
        Some("boundary-machine requirements are not local dynamic calls")
    } else if !program
        .state_signature_type_parameters(requirement)
        .is_empty()
    {
        Some("the requirement has requirement-local generic parameters")
    } else {
        let parameters = program.state_signature_parameters(requirement);
        let receivers = parameters
            .iter()
            .filter(|parameter| parameter.is_self)
            .collect::<Vec<_>>();
        match receivers.as_slice() {
            [receiver] if is_reference_to_self(program, receiver.type_reference) => {
                if parameters
                    .iter()
                    .filter(|parameter| !parameter.is_self)
                    .any(|parameter| {
                        type_reference_contains_self(program, parameter.type_reference)
                    })
                {
                    Some("`Self` appears outside the borrowed receiver")
                } else if requirement.return_type.is_valid()
                    && type_reference_contains_self(program, requirement.return_type)
                {
                    Some("`Self` appears in the result type")
                } else {
                    None
                }
            }
            [_] => Some("the receiver is by value rather than `&self` or `&mut self`"),
            [] => Some("the requirement has no `&self` or `&mut self` receiver"),
            _ => Some("the requirement has more than one receiver"),
        }
    }?;

    Some(format!(
        "requirement `{}::{}` is absent from `dyn {}`: {reason}",
        trait_definition.name, requirement.name, trait_definition.name
    ))
}

fn dynamic_trait_symbol(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<psi_symbols::SymbolHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => dynamic_trait_symbol(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            dynamic_trait_symbol(program, *base_type)
        }
        TypeReferenceNode::DynamicTrait { symbol, .. } => Some(*symbol),
        _ => None,
    }
}

fn is_reference_to_self(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    let TypeReferenceNode::Reference { referee, .. } =
        program.type_reference_table.type_reference(type_reference)
    else {
        return false;
    };
    matches!(
        program.type_reference_table.type_reference(*referee),
        TypeReferenceNode::Named { name, .. } if name.as_str() == "Self"
    )
}

fn type_reference_contains_self(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { name, .. } => name.as_str() == "Self",
        TypeReferenceNode::Reference { referee, .. } => {
            type_reference_contains_self(program, *referee)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_contains_self(program, *base_type)
        }
        TypeReferenceNode::Generic { arguments, .. } => program
            .type_reference_table
            .type_reference_handles(*arguments)
            .iter()
            .any(|argument| type_reference_contains_self(program, *argument)),
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            type_reference_contains_self(program, *element_type)
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => false,
    }
}
