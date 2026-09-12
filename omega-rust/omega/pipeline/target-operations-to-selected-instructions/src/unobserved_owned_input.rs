//! Bounded no-observation eligibility; this predicate supplies no ownership authority.
use legalized_operations::{
    LegalizedScalarArgument, LegalizedScalarFunction, LegalizedScalarInstructionKind as Instruction,
};
use terminal_psi::{
    StructuralAccess, StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape,
};

pub(crate) fn parameter(parameter: &StructuralParameterDeclaration) -> bool {
    parameter.access == StructuralAccess::Owned
        && matches!(
            parameter.multiplicity,
            StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
        )
        && !parameter.is_self
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
}

pub(crate) fn plain_type(
    root: semantic_vocabulary::StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> bool {
    let mut pending = vec![root];
    let mut seen = std::collections::BTreeSet::new();
    while let Some(identity) = pending.pop() {
        if !seen.insert(identity) {
            continue;
        }
        let Some(declaration) = declarations
            .iter()
            .find(|declaration| declaration.id == identity)
        else {
            return false;
        };
        match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(_) => {}
            StructuralTypeShape::Record { fields } => {
                for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                    match field.field_type {
                        // Retain the complete bound in the structural contract.
                        // No-observation eligibility grants no mutation authority.
                        StructuralFieldType::Scalar(_)
                        | StructuralFieldType::BoundedInteger(_)
                        | StructuralFieldType::IeeeFloat(_) => {}
                        StructuralFieldType::Structural(nested) => pending.push(nested),
                        _ => return false,
                    }
                }
            }
            StructuralTypeShape::FixedArray { element, length } if *length > 0 => {
                pending.push(*element)
            }
            _ => return false,
        }
    }
    // Geometry replay also rejects recursive storage and unsupported scalar widths.
    crate::structural_reference_input::shape(root, declarations).is_some()
}

pub(crate) fn accepts(function: &LegalizedScalarFunction) -> bool {
    let Some(contract) = &function.structural else {
        return false;
    };
    !contract.parameters.is_empty()
        && contract.entry_claims.is_empty()
        && contract.published_service_ceiling.is_empty()
        && function.attachment.is_none()
        && !function.blocks.iter().any(|block| matches!(&block.terminator,
            legalized_operations::LegalizedScalarTerminator::Return(returned)
                if matches!(returned.value, legalized_operations::LegalizedScalarReturnValue::StructuralParameter { .. })))
        && contract
            .parameters
            .iter()
            .map(|parameter| &parameter.semantic)
            .chain(
                function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.structural_parameters),
            )
            .all(|declaration| {
                parameter(declaration)
                    && plain_type(declaration.structural_type, &contract.structural_types)
            })
        && function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .all(|instruction| match &instruction.kind {
                Instruction::Constant(_)
                | Instruction::BooleanNot { .. }
                | Instruction::IntegerWiden { .. }
                | Instruction::ExactBinary { .. }
                | Instruction::Compare { .. } => true,
                Instruction::EstablishPrimitiveLocal { result, .. } => {
                    crate::selection::primitive_local_input::local(function, result.place).is_some()
                }
                Instruction::PrimitiveLocalStore { destination, .. }
                | Instruction::PrimitiveScalarRead {
                    source: destination,
                } => {
                    crate::selection::primitive_local_input::local(function, *destination).is_some()
                }
                Instruction::Call(call) => call.arguments.iter().all(|argument| match argument {
                    LegalizedScalarArgument::Scalar { .. } => true,
                    LegalizedScalarArgument::Structural { semantic, .. } => {
                        semantic.access != StructuralAccess::Owned
                            && semantic.path.is_empty()
                            && crate::selection::primitive_local_input::local(
                                function,
                                semantic.place,
                            )
                            .is_some()
                            && !contract
                                .parameters
                                .iter()
                                .map(|parameter| &parameter.semantic)
                                .chain(
                                    function
                                        .blocks
                                        .iter()
                                        .flat_map(|block| &block.structural_parameters),
                                )
                                .any(|parameter| parameter.place == semantic.place)
                    }
                }),
                _ => false,
            })
}
