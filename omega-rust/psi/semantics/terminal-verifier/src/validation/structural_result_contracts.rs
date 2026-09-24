//! Exact qualification custody shared by structural calls and returns.

use super::{
    BTreeSet, OperationKind, PlaceId, StructuralDomainId, StructuralFieldType,
    StructuralMultiplicity, StructuralTypeId, StructuralTypeShape, TerminalMachine, TerminalModule,
};
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
        .or_else(|| super::block_views::parameter(machine, source))
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

/// Domains minted onto a return source by its producing operation's
/// qualification establishment rows. Those memberships are authorized
/// body-internal evidence — each row replays an authorized establishment
/// route — but they belong to the callee's contract, not the returning
/// machine's own signature, so they shed at the return edge: the
/// caller-visible result carries only the declared domains.
pub(super) fn minted_source_qualification_domains(
    machine: &TerminalMachine,
    source: PlaceId,
) -> BTreeSet<StructuralDomainId> {
    machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| {
            operation.result.structural().and_then(|result| {
                (result.place == source).then(|| {
                    result
                        .qualification_establishments
                        .iter()
                        .map(|binding| binding.domain)
                        .collect::<BTreeSet<_>>()
                })
            })
        })
        .unwrap_or_default()
}

/// A `ReturnStructural` source matches the machine's declared result when the
/// result introduces at least the source's whole-root qualifications: the
/// signature's `established by` authority mints declared domains onto the
/// returned value at the edge, so the source need not already carry them.
/// The source may never carry a qualification the result does not declare,
/// except domains minted onto it by its producer's authorized qualification
/// establishment rows — those shed at the contract edge instead.
pub(super) fn matches_return_source(
    source: StructuralResultSignature<'_>,
    result: &terminal_psi::StructuralResultDeclaration,
) -> bool {
    source.structural_type == result.structural_type
        && source.multiplicity == result.multiplicity
        && source
            .qualifications
            .iter()
            .all(|domain| result.qualifications.contains(domain))
        && source.projected_qualifications == result.projected_qualifications
}

pub(super) fn has_empty_qualification_rosters(
    qualifications: &[StructuralDomainId],
    projected: &[terminal_psi::StructuralPathQualification],
) -> bool {
    qualifications.is_empty() && projected.is_empty()
}

/// A result type is a whole borrowed view — `&'a V`, `&'a [u8]`, or
/// `&'a [T]` — when its top-level shape is a reference or a borrowed
/// slice/element view: the loan's storage lives in its carrier, so a
/// shared-borrowed parameter may forward it without owning fresh custody.
pub(super) fn borrowed_view_shape(module: &TerminalModule, root: StructuralTypeId) -> bool {
    matches!(
        module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == root)
            .map(|declaration| &declaration.shape),
        Some(
            StructuralTypeShape::Reference { .. }
                | StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView,
                )
                | StructuralTypeShape::ElementView { .. }
        )
    )
}

/// Calls establish their exact declared result independently of its aggregate
/// syntax. Return admission must not demand artificial claims for a plain array
/// merely because its producer was a call rather than a parameter.
/// Operation validation checks the callee/result contract; the frontier still
/// checks that this exact owned result is live and has not been partially moved.
/// A call result whose only qualification rosters are domains minted by its
/// own authorized establishment rows is plain here as well: minted
/// memberships are discharged evidence that sheds at the contract edge, not
/// custody the terminator owes claims for.
pub(super) fn plain_owned_call_result(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: PlaceId,
) -> bool {
    machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .any(|operation| {
            matches!(
                operation.kind,
                OperationKind::CallStructural { .. }
                    | OperationKind::CallStructuralWithScalarArguments { .. }
                    | OperationKind::BoundaryCall { .. }
            ) && operation.result.structural().is_some_and(|result| {
                let minted = result
                    .qualification_establishments
                    .iter()
                    .map(|binding| binding.domain)
                    .collect::<BTreeSet<_>>();
                result.place == source
                    && matches!(
                        result.multiplicity,
                        StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
                    )
                    && result
                        .qualifications
                        .iter()
                        .all(|domain| minted.contains(domain))
                    && result
                        .projected_qualifications
                        .iter()
                        .all(|projection| minted.contains(&projection.domain))
                    && result.claims.is_empty()
                    && has_plain_owned_shape(module, result.structural_type)
            })
        })
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
                    StructuralFieldType::Scalar(_)
                    | StructuralFieldType::BoundedInteger(_)
                    | StructuralFieldType::IeeeFloat(_) => true,
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
            StructuralTypeShape::ElementView { .. } => false,
            StructuralTypeShape::Reference { .. } => false,
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
