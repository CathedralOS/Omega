use psi_typed_trees::TypedTrees;
use psi_typed_trees::trait_definition::DynamicSignatureIneligibility;
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

    let reason = match program
        .dynamic_signature_eligibility(trait_definition, requirement)
        .err()?
    {
        DynamicSignatureIneligibility::BoundaryRequirement => {
            "boundary-machine requirements are not local dynamic calls"
        }
        DynamicSignatureIneligibility::RequirementLocalGenerics => {
            "the requirement has requirement-local generic parameters"
        }
        DynamicSignatureIneligibility::MissingBorrowedReceiver => {
            "the requirement has no `&self` or `&mut self` receiver"
        }
        DynamicSignatureIneligibility::ByValueReceiver => {
            "the receiver is by value rather than `&self` or `&mut self`"
        }
        DynamicSignatureIneligibility::MultipleReceivers => {
            "the requirement has more than one receiver"
        }
        DynamicSignatureIneligibility::SelfOutsideReceiver => {
            "`Self` appears outside the borrowed receiver"
        }
        DynamicSignatureIneligibility::SelfResult => "`Self` appears in the result type",
    };

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
