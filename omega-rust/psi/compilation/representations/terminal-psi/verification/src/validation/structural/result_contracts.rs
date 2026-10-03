//! Exact qualification custody shared by structural calls and returns.

use crate::validation::{
    BTreeSet, OperationKind, PlaceId, StructuralDomainId, StructuralFieldType,
    StructuralMultiplicity, StructuralPlaceKind, StructuralTypeId, StructuralTypeShape,
    TerminalMachine, TerminalModule,
};
#[derive(Clone, Copy)]
pub(in crate::validation) struct StructuralResultSignature<'a> {
    pub(in crate::validation) structural_type: StructuralTypeId,
    pub(in crate::validation) multiplicity: StructuralMultiplicity,
    pub(in crate::validation) qualifications: &'a [StructuralDomainId],
    pub(in crate::validation) projected_qualifications:
        &'a [terminal_psi::StructuralPathQualification],
}

pub(in crate::validation) fn operation_signature(
    result: &terminal_psi::StructuralOperationResult,
) -> StructuralResultSignature<'_> {
    StructuralResultSignature {
        structural_type: result.structural_type,
        multiplicity: result.multiplicity,
        qualifications: &result.qualifications,
        projected_qualifications: &result.projected_qualifications,
    }
}

pub(in crate::validation) fn source_signature(
    machine: &TerminalMachine,
    source: PlaceId,
) -> Option<StructuralResultSignature<'_>> {
    machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == source)
        .or_else(|| crate::validation::block_views::parameter(machine, source))
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
        .or_else(|| {
            // An image literal publishes a descriptor without an owned
            // operation result. Its declaration and sole establishment still
            // give it an exact carrier and qualification contract.
            let structural_type = machine
                .structural_places
                .iter()
                .find_map(|place| match place.kind {
                    StructuralPlaceKind::ByteSequenceLiteral {
                        structural_type, ..
                    } if place.id == source => Some(structural_type),
                    _ => None,
                })?;
            let mut establishments = machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter_map(|operation| match &operation.kind {
                    OperationKind::EstablishByteSequenceLiteral {
                        destination,
                        qualifications,
                        ..
                    } if *destination == source => Some(qualifications.as_slice()),
                    _ => None,
                });
            let qualifications = establishments.next()?;
            establishments
                .next()
                .is_none()
                .then_some(StructuralResultSignature {
                    structural_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    qualifications,
                    projected_qualifications: &[],
                })
        })
}

pub(in crate::validation) fn matches_function_result(
    signature: StructuralResultSignature<'_>,
    result: &terminal_psi::StructuralResultDeclaration,
) -> bool {
    signature.structural_type == result.structural_type
        && signature.multiplicity == result.multiplicity
        && signature.qualifications == result.qualifications
        && signature.projected_qualifications == result.projected_qualifications
}

pub(in crate::validation) fn call_result_matches(
    result: &terminal_psi::StructuralOperationResult,
    callee: &terminal_psi::StructuralResultDeclaration,
) -> bool {
    matches_function_result(operation_signature(result), callee)
}

/// A bounded byte result owns its backing, unlike a borrowed descriptor. The
/// producing operation checks capacity and predicates (or imports an exact
/// checked callee result); control-flow validation checks source availability.
/// This judgment removes no claims and grants no qualification establishment.
pub(in crate::validation) fn bounded_owned_byte_return_source(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: PlaceId,
    declared: &terminal_psi::StructuralResultDeclaration,
) -> bool {
    if declared.multiplicity != StructuralMultiplicity::Unrestricted
        || !declared.projected_qualifications.is_empty()
        || !declared.reference_sources.is_empty()
        || !module.structural_types.iter().any(|declaration| {
            declaration.id == declared.structural_type
                && matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                    )
                )
        })
    {
        return false;
    }
    if let Some(result) =
        crate::validation::byte_sequence::concatenation::owned_result(machine, source)
    {
        return matches_function_result(operation_signature(result), declared);
    }
    terminal_semantics::byte_owned_call_result(module, machine, source)
        .is_some_and(|result| matches_function_result(operation_signature(result), declared))
}

/// Domains minted onto a return source by its producing operation's
/// qualification establishment rows. Those memberships are authorized
/// body-internal evidence — each row replays an authorized establishment
/// route — but they belong to the callee's contract, not the returning
/// machine's own signature, so they shed at the return edge: the
/// caller-visible result carries only the declared domains.
pub(in crate::validation) fn minted_source_qualification_domains(
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
pub(in crate::validation) fn matches_return_source(
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

pub(in crate::validation) fn has_empty_qualification_rosters(
    qualifications: &[StructuralDomainId],
    projected: &[terminal_psi::StructuralPathQualification],
) -> bool {
    qualifications.is_empty() && projected.is_empty()
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
pub(in crate::validation) fn plain_owned_call_result(
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
pub(in crate::validation) fn has_plain_owned_shape(
    module: &TerminalModule,
    root: StructuralTypeId,
) -> bool {
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
            // Erased declarations contribute no storage or loan authority.
            if field.relevance.is_erased() {
                return matches!(field.field_type, StructuralFieldType::Erased { .. });
            }
            match &field.field_type {
                StructuralFieldType::Scalar(_)
                | StructuralFieldType::Atomic(_)
                | StructuralFieldType::BoundedInteger(_)
                | StructuralFieldType::RefinedScalar(_)
                | StructuralFieldType::IeeeFloat(_) => true,
                StructuralFieldType::Structural(child) => visit(module, *child, active, complete),
                StructuralFieldType::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                )
                | StructuralFieldType::PredicateByteSequence {
                    carrier: terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                    ..
                } => true,
                StructuralFieldType::ByteSequence(_)
                | StructuralFieldType::PredicateByteSequence { .. }
                | StructuralFieldType::Erased { .. } => false,
            }
        };
        let supported = match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(_) => true,
            StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
            ) => true,
            StructuralTypeShape::ByteSequence(_) => false,
            StructuralTypeShape::FusedServiceBinding => false,
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
