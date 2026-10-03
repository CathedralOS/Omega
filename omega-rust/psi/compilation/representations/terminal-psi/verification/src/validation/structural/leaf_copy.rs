//! A leaf copy observes an `Unrestricted` subtree without consuming or
//! refining its owner. Declaration lookup supplies nominal identity, not
//! liveness: control-flow availability and the ownership frontier
//! independently check the occurrence before it executes. The result's
//! declared `Unrestricted` multiplicity is the copyability evidence — an
//! affine leaf would ride the move/restoration contract instead.
//!
//! One carrier exception stands beside that contract: an `Affine` result
//! whose type is a `SharedBorrow` reference to the selected leaf is a
//! shared loan, not a copy. `references` registers it as a live reference
//! carrier rooted at the source, so the source root stays loaned until the
//! carrier is released, and the carrier keeps its affine end-exactly-once
//! obligation. A mutable-borrow result never qualifies: an exclusive loan
//! is minted only by `EstablishReference`, whose source authority is
//! checked.

use crate::validation::{
    BTreeSet, CanonicalStructuralPathSegment, IntegerSign, IntegerType, ModuleError, OperationKind,
    PlaceId, ScalarType, StructuralAccess, StructuralFieldType, StructuralMultiplicity,
    StructuralPathSegment, StructuralTypeId, StructuralTypeShape, TerminalMachine, TerminalModule,
    resolve_leaf_copy_path,
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
    let whole_bytes = path.is_empty()
        && module.structural_types.iter().any(|declaration| {
            declaration.id == result.structural_type
                && matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                    )
                )
        });
    let exact_byte_copy = whole_bytes
        && crate::validation::structural::result_contracts::source_signature(machine, source)
            .is_some_and(|signature| {
                signature.structural_type == result.structural_type
                    && signature.multiplicity == StructuralMultiplicity::Unrestricted
                    && signature.qualifications == result.qualifications
                    && signature.projected_qualifications.is_empty()
                    && terminal_semantics::byte_qualification_predicates_in(
                        module.structural_domains.iter(),
                        result.structural_type,
                        &result.qualifications,
                    )
                    .is_some()
            })
        && result.projected_qualifications.is_empty()
        && result.qualification_establishments.is_empty();
    if (result.multiplicity != StructuralMultiplicity::Unrestricted && !shared_borrow_relocation)
        || (whole_bytes && !exact_byte_copy)
        || (!whole_bytes && !result.qualifications.is_empty())
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
    // A static ordinal proves bounds only for a fixed array. Copying an
    // unrestricted element out of a view must carry a RuntimeIndex obligation
    // against that view's live length, even when the authored index is literal.
    // Otherwise replacing RuntimeIndex by FixedIndex could erase the proof.
    // Shared-reference results keep their separate loan-formation contract.
    if result.multiplicity == StructuralMultiplicity::Unrestricted {
        for (position, segment) in path.iter().enumerate() {
            if matches!(segment, StructuralPathSegment::FixedIndex(_))
                && crate::validation::foundation::resolve_runtime_projection(
                    module,
                    signature.structural_type,
                    &path[..position],
                )
                .is_some_and(|container| {
                    module.structural_types.iter().any(|declaration| {
                        declaration.id == container
                            && matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
                    })
                })
            {
                return Err(invalid());
            }
        }
    }
    let selected_type =
        resolve_leaf_copy_path(module, signature.structural_type, path).ok_or_else(invalid)?;
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
    if selected_type != expected_type
        && !byte_carrier_leaf_copy(module, signature.structural_type, path, expected_type)
    {
        return Err(invalid());
    }
    Ok(())
}

/// Whether a leaf copy mints a record out of a byte carrier: `path` ends at a
/// literal index inside a `u8` fixed array, and `expected` is a plain record
/// whose fixed scalar layout fits the elements remaining at that index. The
/// source recast judgment proved the representation; this re-checks the
/// static shape independently so a malformed module cannot mint a record the
/// region cannot host. A runtime-selected index keeps `index < length` only
/// — it cannot prove the wider `index + extent` bound, so it stays outside
/// this arm.
fn byte_carrier_leaf_copy(
    module: &TerminalModule,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    expected: StructuralTypeId,
) -> bool {
    let Some((StructuralPathSegment::FixedIndex(index), prefix)) = path.split_last() else {
        return false;
    };
    let Some(parent) =
        crate::validation::foundation::resolve_runtime_projection(module, root, prefix)
    else {
        return false;
    };
    let Some(declaration) = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == parent)
    else {
        return false;
    };
    let StructuralTypeShape::FixedArray { element, length } = &declaration.shape else {
        return false;
    };
    let byte_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 is valid"));
    if !module.structural_types.iter().any(|declaration| {
        declaration.id == *element
            && declaration.shape == StructuralTypeShape::PrimitiveScalar(byte_type)
    }) {
        return false;
    }
    let Some(layout) =
        terminal_semantics::byte_carrier_scalar_layout(module.structural_types.iter(), expected)
    else {
        return false;
    };
    index
        .checked_add(layout.size)
        .is_some_and(|end| end <= *length)
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
    let result_declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == result.structural_type);
    // Beside the plain `Unrestricted` copy, two `Affine` result shapes keep
    // the copied custody honest: a `SharedBorrow` reference result is a loan
    // relocation registered as a live reference carrier, and a plain
    // structural leaf with `Affine` multiplicity is an owned copy the
    // successor takes custody of — copying out of a borrowed case payload
    // never moves the stored value itself.
    let shared_borrow_relocation = result.multiplicity == StructuralMultiplicity::Affine
        && result_declaration.is_some_and(|declaration| {
            matches!(
                declaration.shape,
                StructuralTypeShape::Reference {
                    access: StructuralAccess::SharedBorrow,
                    ..
                }
            )
        });
    let owned_copy = result.multiplicity == StructuralMultiplicity::Affine
        && result_declaration.is_some_and(|declaration| {
            !matches!(declaration.shape, StructuralTypeShape::Reference { .. })
        });
    if (result.multiplicity != StructuralMultiplicity::Unrestricted
        && !shared_borrow_relocation
        && !owned_copy)
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
    let expected_type = result_declaration
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

/// The structural result a leaf-copy operation installed at `source`,
/// independent of the declared custody. Each caller composes the custody
/// class it needs against this producer identity: a loan join names an
/// unrestricted copy, while a whole owned transfer accepts any declared
/// multiplicity the operation certified.
pub(in crate::validation) fn copied_result_source(
    machine: &TerminalMachine,
    source: PlaceId,
) -> Option<terminal_psi::StructuralOperationResult> {
    machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| {
            matches!(
                operation.kind,
                OperationKind::StructuralLeafCopy { .. }
                    | OperationKind::StructuralCaseLeafCopy { .. }
            )
            .then(|| operation.result.structural())
            .flatten()
            .filter(|result| result.place == source)
            .cloned()
        })
}

/// A copied leaf's result is owned the way an `EstablishScalarCase` result
/// is: the copy is fresh storage, so an `Unrestricted` machine result may
/// publish it without the plain-shape proviso that payloadless returns need.
/// Control-flow availability and frontier accounting still apply.
pub(in crate::validation) fn copied_return_source(
    machine: &TerminalMachine,
    source: PlaceId,
) -> bool {
    copied_result_source(machine, source).is_some_and(|result| {
        result.multiplicity == StructuralMultiplicity::Unrestricted
            && result.qualifications.is_empty()
            && result.projected_qualifications.is_empty()
            && result.claims.is_empty()
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
