//! Exact qualification custody shared by structural calls and returns.

use super::*;

#[derive(Clone, Copy)]
pub(super) struct StructuralResultSignature<'a> {
    pub(super) structural_type: StructuralTypeId,
    pub(super) multiplicity: StructuralMultiplicity,
    pub(super) qualifications: &'a [StructuralDomainId],
    pub(super) projected_qualifications: &'a [terminal_psi::StructuralPathQualification],
}

pub(super) fn operation_signature(
    result: &terminal_psi::StructuralOperationResult,
) -> StructuralResultSignature<'_> {
    StructuralResultSignature {
        structural_type: result.structural_type,
        multiplicity: result.multiplicity,
        qualifications: &result.qualifications,
        projected_qualifications: &result.projected_qualifications,
    }
}

pub(super) fn source_signature(
    machine: &TerminalMachine,
    source: PlaceId,
) -> Option<StructuralResultSignature<'_>> {
    machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == source)
        .map(|parameter| StructuralResultSignature {
            structural_type: parameter.structural_type,
            multiplicity: parameter.multiplicity,
            qualifications: &parameter.qualifications,
            projected_qualifications: &parameter.projected_qualifications,
        })
        .or_else(|| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find_map(|operation| {
                    operation.result.structural().and_then(|result| {
                        (result.place == source).then(|| operation_signature(result))
                    })
                })
        })
}

pub(super) fn matches_function_result(
    signature: StructuralResultSignature<'_>,
    result: &terminal_psi::StructuralResultDeclaration,
) -> bool {
    signature.structural_type == result.structural_type
        && signature.multiplicity == result.multiplicity
        && signature.qualifications == result.qualifications
        && signature.projected_qualifications == result.projected_qualifications
}

pub(super) fn call_result_matches(
    result: &terminal_psi::StructuralOperationResult,
    callee: &terminal_psi::StructuralResultDeclaration,
) -> bool {
    matches_function_result(operation_signature(result), callee)
}

pub(super) fn has_empty_qualification_rosters(
    qualifications: &[StructuralDomainId],
    projected: &[terminal_psi::StructuralPathQualification],
) -> bool {
    qualifications.is_empty() && projected.is_empty()
}

/// Whole-value transfer is independent of native fragment width. Borrowed byte
/// views and erased carriers still need their own retained custody contracts.
pub(super) fn has_plain_owned_shape(module: &TerminalModule, root: StructuralTypeId) -> bool {
    fn visit(
        module: &TerminalModule,
        root: StructuralTypeId,
        active: &mut Vec<StructuralTypeId>,
        complete: &mut BTreeSet<StructuralTypeId>,
    ) -> bool {
        if complete.contains(&root) {
            return true;
        }
        if active.contains(&root) {
            return false;
        }
        let mut declarations = module
            .structural_types
            .iter()
            .filter(|declaration| declaration.id == root);
        let Some(declaration) = declarations.next() else {
            return false;
        };
        if declarations.next().is_some() {
            return false;
        }
        active.push(root);
        let mut field_is_owned = |field: &terminal_psi::StructuralFieldDeclaration| {
            !field.relevance.is_erased()
                && match &field.field_type {
                    StructuralFieldType::Scalar(_) | StructuralFieldType::IeeeFloat(_) => true,
                    StructuralFieldType::Structural(child) => {
                        visit(module, *child, active, complete)
                    }
                    StructuralFieldType::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                    ) => true,
                    StructuralFieldType::ByteSequence(_) | StructuralFieldType::Erased { .. } => {
                        false
                    }
                }
        };
        let supported = match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(_) => true,
            StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
            ) => true,
            StructuralTypeShape::ByteSequence(_) => false,
            StructuralTypeShape::Record { fields } => fields.iter().all(&mut field_is_owned),
            StructuralTypeShape::Sum { cases } => cases
                .iter()
                .flat_map(|case| &case.fields)
                .all(&mut field_is_owned),
            StructuralTypeShape::Mixed { fields, cases } => fields
                .iter()
                .chain(cases.iter().flat_map(|case| &case.fields))
                .all(&mut field_is_owned),
            StructuralTypeShape::FixedArray { element, .. } => {
                visit(module, *element, active, complete)
            }
        };
        active.pop();
        if supported {
            complete.insert(root);
        }
        supported
    }
    visit(module, root, &mut Vec::new(), &mut BTreeSet::new())
}
