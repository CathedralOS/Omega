//! Exact qualification custody shared by structural calls and returns.

use crate::validation::{
    BTreeSet, OperationKind, PlaceId, StructuralDomainId, StructuralMultiplicity,
    StructuralPlaceKind, StructuralTypeId, StructuralTypeShape, TerminalMachine, TerminalModule,
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

/// A constructor's owned operand uses the same finite affine projection
/// contract as an ordinary call. Its root stays live with partial custody;
/// this signature names only the selected child and grants no new claims.
pub(in crate::validation) fn constructor_argument_signature<'a>(
    module: &TerminalModule,
    machine: &'a TerminalMachine,
    argument: &terminal_psi::StructuralArgument,
) -> Option<StructuralResultSignature<'a>> {
    let source = source_signature(machine, argument.place)?;
    if argument.path.is_empty() {
        return Some(source);
    }
    if argument.access != terminal_psi::StructuralAccess::Owned
        || !argument.path.iter().all(|segment| {
            matches!(
                segment,
                terminal_psi::StructuralPathSegment::Field(_)
                    | terminal_psi::StructuralPathSegment::FixedIndex(_)
            )
        })
    {
        return None;
    }
    let root = crate::validation::partial_affine_root_type(machine, argument.place)?;
    let structural_type = crate::validation::resolve_structural_path(module, root, &argument.path)?;
    Some(StructuralResultSignature {
        structural_type,
        ..source
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
    terminal_semantics::descriptor_results::plain_owned_shape(module.structural_types.iter(), root)
}

/// Plain storage and explicitly shared byte descriptors form an owned carrier
/// only after exact return-slot replay proves that no backing is activation-local.
/// ElementView carries access outside its shape and ordinary references have
/// their own return contract; neither acquires shared permission here.
pub(in crate::validation) fn owned_descriptor_return_source(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: PlaceId,
) -> bool {
    let Some(signature) = source_signature(machine, source) else {
        return false;
    };
    let owned = machine
        .structural_parameters
        .iter()
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .find(|parameter| parameter.place == source)
        .map(|parameter| parameter.access == terminal_psi::StructuralAccess::Owned)
        .unwrap_or_else(|| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter_map(|operation| operation.result.structural())
                .any(|result| result.place == source && result.claims.is_empty())
        });
    owned
        && matches!(
            signature.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        )
        && has_empty_qualification_rosters(
            signature.qualifications,
            signature.projected_qualifications,
        )
        && terminal_semantics::descriptor_results::owned_shared_byte_descriptor_shape(
            module.structural_types.iter(),
            signature.structural_type,
        )
        && terminal_semantics::descriptor_results::return_slots_have_external_backing(
            module, machine, source,
        )
}
