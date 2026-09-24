//! A leaf copy observes an `Unrestricted` subtree without consuming or
//! refining its owner. Declaration lookup supplies nominal identity, not
//! liveness: control-flow availability and the ownership frontier
//! independently check the occurrence before it executes. The result's
//! declared `Unrestricted` multiplicity is the copyability evidence — an
//! affine leaf would ride the move/restoration contract instead.
//!
//! One carrier exception stands beside that contract: a `SharedBorrow`
//! reference leaf relocates affinely. The copy transfers the loan's
//! descriptor into fresh storage — it does not mint custody — so the
//! result keeps the carrier's affine end-exactly-once obligation while
//! the source loan stays minted on its root. A mutable-borrow leaf
//! never qualifies: duplicating an exclusive loan's descriptor would
//! mint a second custody edge over the same referent.

use crate::validation::{
    BTreeSet, CanonicalStructuralPathSegment, ModuleError, OperationKind, PlaceId,
    StructuralAccess, StructuralFieldType, StructuralMultiplicity, StructuralPathSegment,
    StructuralTypeId, StructuralTypeShape, TerminalMachine, TerminalModule,
    resolve_structural_path,
};

pub(in crate::validation) fn validate(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    source: PlaceId,
    path: &[StructuralPathSegment],
) -> Result<(), ModuleError> {
    let invalid = || ModuleError::InvalidStructuralLeafCopy {
        operation: operation.id,
        source,
    };
    let Some(result) = operation.result.structural() else {
        return Err(invalid());
    };
    let shared_borrow_relocation = result.multiplicity == StructuralMultiplicity::Affine
        && module.structural_types.iter().any(|declaration| {
            declaration.id == result.structural_type
                && matches!(
                    declaration.shape,
                    StructuralTypeShape::Reference {
                        access: StructuralAccess::SharedBorrow,
                        ..
                    }
                )
        });
    if (result.multiplicity != StructuralMultiplicity::Unrestricted && !shared_borrow_relocation)
        || !result.qualifications.is_empty()
        || !result.claims.is_empty()
    {
        return Err(invalid());
    }
    let parameter = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == source)
        .or_else(|| crate::validation::block_views::parameter(machine, source));
    if parameter.is_some_and(|parameter| parameter.access == StructuralAccess::WriteOnlyBorrow) {
        return Err(ModuleError::StructuralObservationRequiresReadableAccess {
            operation: operation.id,
            source,
        });
    }
    let signature =
        crate::validation::structural::result_contracts::source_signature(machine, source)
            .ok_or_else(invalid)?;
    let selected_type =
        resolve_structural_path(module, signature.structural_type, path).ok_or_else(invalid)?;
    // A `&`-leaf copy mints the reference descriptor over the selected
    // storage, so its result is the `SharedBorrow` declaration whose
    // referent is the leaf — not the leaf storage itself.
    let expected_type = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == result.structural_type)
        .and_then(|declaration| match declaration.shape {
            StructuralTypeShape::Reference {
                referent,
                access: StructuralAccess::SharedBorrow,
            } => Some(referent),
            _ => None,
        })
        .unwrap_or(result.structural_type);
    if selected_type != expected_type {
        return Err(invalid());
    }
    Ok(())
}

/// The case-qualified sibling of `validate`: the canonical path descends
/// through at least one `Case` step into a payload the dominating edge proved
/// active, so the projection reaches storage `resolve_structural_path` cannot
/// name. Result shape, readable-access and availability obligations are the
/// same as the plain leaf copy; the leaf type resolves through the canonical
/// walk instead.
pub(in crate::validation) fn validate_case_leaf_copy(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    source: PlaceId,
    path: &[CanonicalStructuralPathSegment],
) -> Result<(), ModuleError> {
    let invalid = || ModuleError::InvalidStructuralLeafCopy {
        operation: operation.id,
        source,
    };
    let Some(result) = operation.result.structural() else {
        return Err(invalid());
    };
    if result.multiplicity != StructuralMultiplicity::Unrestricted
        || !result.qualifications.is_empty()
        || !result.claims.is_empty()
    {
        return Err(invalid());
    }
    let parameter = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == source)
        .or_else(|| crate::validation::block_views::parameter(machine, source));
    if parameter.is_some_and(|parameter| parameter.access == StructuralAccess::WriteOnlyBorrow) {
        return Err(ModuleError::StructuralObservationRequiresReadableAccess {
            operation: operation.id,
            source,
        });
    }
    let signature =
        crate::validation::structural::result_contracts::source_signature(machine, source)
            .ok_or_else(invalid)?;
    let selected_type =
        resolve_canonical_leaf_type(module, signature.structural_type, path).ok_or_else(invalid)?;
    if selected_type != result.structural_type {
        return Err(invalid());
    }
    Ok(())
}

/// Walk a canonical path from `root_type` to the structural leaf it selects.
/// `Case` steps enter the selected case's payload namespace on a `Sum` or
/// `Mixed` shape and the next segment must be a `Field` inside it; `Field`
/// and `FixedIndex` steps behave like the ordinary projection. At least one
/// `Case` step is required — a record-only canonical path is plain
/// `StructuralLeafCopy` territory. The leaf must be structural: scalar leaves
/// belong to the scalar field operations.
fn resolve_canonical_leaf_type(
    module: &TerminalModule,
    root_type: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
) -> Option<StructuralTypeId> {
    let mut structural_type = root_type;
    let mut case_fields: Option<&[terminal_psi::StructuralFieldDeclaration]> = None;
    let mut saw_case = false;
    for (index, segment) in path.iter().enumerate() {
        let is_last = index + 1 == path.len();
        if let CanonicalStructuralPathSegment::Case(case_id) = segment {
            if case_fields.is_some() || is_last {
                return None;
            }
            let declaration = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == structural_type)?;
            let cases = match &declaration.shape {
                StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
                    cases
                }
                _ => return None,
            };
            case_fields = Some(&cases.iter().find(|case| case.id == *case_id)?.fields);
            saw_case = true;
            continue;
        }
        match segment {
            CanonicalStructuralPathSegment::Field(field_id) => {
                let field = if let Some(fields) = case_fields.take() {
                    fields
                        .iter()
                        .find(|field| field.id == *field_id)
                        .filter(|field| !field.relevance.is_erased())?
                } else {
                    let declaration = module
                        .structural_types
                        .iter()
                        .find(|declaration| declaration.id == structural_type)?;
                    let fields = match &declaration.shape {
                        StructuralTypeShape::Record { fields }
                        | StructuralTypeShape::Mixed { fields, .. } => fields,
                        _ => return None,
                    };
                    fields
                        .iter()
                        .find(|field| field.id == *field_id)
                        .filter(|field| !field.relevance.is_erased())?
                };
                let &StructuralFieldType::Structural(next) = &field.field_type else {
                    return None;
                };
                if is_last {
                    return saw_case.then_some(next);
                }
                structural_type = next;
            }
            CanonicalStructuralPathSegment::FixedIndex(index) => {
                if case_fields.is_some() {
                    return None;
                }
                let declaration = module
                    .structural_types
                    .iter()
                    .find(|declaration| declaration.id == structural_type)?;
                let StructuralTypeShape::FixedArray { element, length } = &declaration.shape else {
                    return None;
                };
                if *index >= *length {
                    return None;
                }
                if is_last {
                    return saw_case.then_some(*element);
                }
                structural_type = *element;
            }
            CanonicalStructuralPathSegment::Case(_) => unreachable!(),
        }
    }
    None
}

/// A copied leaf's result is owned the way an `EstablishScalarCase` result
/// is: the copy is fresh storage, so an `Unrestricted` machine result may
/// publish it without the plain-shape proviso that payloadless returns need.
/// Control-flow availability and frontier accounting still apply.
pub(in crate::validation) fn copied_return_source(
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
                OperationKind::StructuralLeafCopy { .. }
                    | OperationKind::StructuralCaseLeafCopy { .. }
            ) && operation.result.structural().is_some_and(|result| {
                result.place == source
                    && result.multiplicity == StructuralMultiplicity::Unrestricted
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty()
            })
        })
}

pub(in crate::validation) fn validate_available(
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    available: &BTreeSet<PlaceId>,
) -> Result<(), ModuleError> {
    let source = match operation.kind {
        OperationKind::StructuralLeafCopy { source, .. }
        | OperationKind::StructuralCaseLeafCopy { source, .. } => source,
        _ => return Ok(()),
    };
    if !machine
        .structural_parameters
        .iter()
        .any(|parameter| parameter.place == source)
        && !available.contains(&source)
    {
        return Err(ModuleError::InvalidStructuralLeafCopy {
            operation: operation.id,
            source,
        });
    }
    Ok(())
}
