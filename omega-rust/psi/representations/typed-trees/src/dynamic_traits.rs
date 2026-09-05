use crate::TypedTrees;
use crate::signature::StateSignature;
use crate::trait_definition::{DynamicSignatureIneligibility, TraitDefinition};
use crate::types::{TypeReferenceHandle, TypeReferenceNode};

impl TypedTrees {
    /// Derive the signature-only portion of one requirement's local dynamic
    /// eligibility. Contract privacy, lifetime projection, and operational
    /// envelope fitting are separate later judgments; this query never claims
    /// those checks have already happened.
    pub fn dynamic_signature_eligibility(
        &self,
        trait_definition: &TraitDefinition,
        requirement: &StateSignature,
    ) -> Result<(), DynamicSignatureIneligibility> {
        if trait_definition.is_boundary {
            return Err(DynamicSignatureIneligibility::BoundaryRequirement);
        }
        if !self.state_signature_type_parameters(requirement).is_empty() {
            return Err(DynamicSignatureIneligibility::RequirementLocalGenerics);
        }

        let parameters = self.state_signature_parameters(requirement);
        let receivers = parameters
            .iter()
            .filter(|parameter| parameter.is_self)
            .collect::<Vec<_>>();
        let receiver = match receivers.as_slice() {
            [receiver] => *receiver,
            [] => return Err(DynamicSignatureIneligibility::MissingBorrowedReceiver),
            _ => return Err(DynamicSignatureIneligibility::MultipleReceivers),
        };
        if !is_reference_to_self(self, receiver.type_reference) {
            return Err(DynamicSignatureIneligibility::ByValueReceiver);
        }
        if parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .any(|parameter| type_reference_contains_self(self, parameter.type_reference))
        {
            return Err(DynamicSignatureIneligibility::SelfOutsideReceiver);
        }
        if requirement.return_type.is_valid()
            && type_reference_contains_self(self, requirement.return_type)
        {
            return Err(DynamicSignatureIneligibility::SelfResult);
        }
        Ok(())
    }

    /// Requirements that survive the currently implemented signature-only
    /// dynamic-surface projection, in trait declaration order.
    pub fn dynamic_signature_surface<'program>(
        &'program self,
        trait_definition: &'program TraitDefinition,
    ) -> impl Iterator<Item = &'program StateSignature> + 'program {
        self.trait_machine_signatures(trait_definition)
            .iter()
            .filter(|requirement| {
                self.dynamic_signature_eligibility(trait_definition, requirement)
                    .is_ok()
            })
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
