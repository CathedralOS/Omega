//! Structural argument sources, qualifications, paths and subloans.
//!
//! `validate_structural_arguments` derives the `CallShape` of the call site,
//! checks the arity, then each argument (`argument_checks`) and every
//! pair of arguments (`argument_pairs`).

mod argument_checks;
mod argument_pairs;

use crate::validation::{
    BTreeSet, ModuleError, OperationId, OperationKind, PlaceId, StructuralAccess,
    StructuralArgument, StructuralDomainId, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralPlaceKind, StructuralTypeId,
    StructuralTypeShape, TerminalMachine, TerminalModule, is_nonempty_field_path,
    is_partial_affine_path, partial_affine_root_type, resolve_structural_path,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StructuralArgumentSourcePolicy {
    OnlyParameters,
    /// Whole linear call results retain their exact completed qualification and
    /// claim occurrence; the frontier separately requires that occurrence live.
    ParametersOrLinearCallResults,
    /// Boundary calls accept borrowed byte literals and whole affine call results,
    /// but not construction-local establishments.
    ParametersOrBoundaryActuals,
    /// Unit and scalar-result calls retain construction locals and whole
    /// ordinary and boundary affine results, whole owned primitive arrays,
    /// and whole immutable byte views.
    ParametersOrAffineLocalsAndCallResults,
    /// Whole record establishments and claim-free affine call results.
    /// Frontier validation separately requires their producer to have run.
    ParametersOrAffineOperationResults,
}

pub(crate) fn is_structural_call_result(caller: &TerminalMachine, place: PlaceId) -> bool {
    caller.structural_places.iter().any(|declaration| {
        declaration.id == place
            && matches!(declaration.kind,
            StructuralPlaceKind::OperationResult { producer, .. }
                if caller.blocks.iter().flat_map(|block| &block.operations).any(|operation| {
                    operation.id == producer && matches!(operation.kind,
                        OperationKind::CallStructuralWithScalarArguments { .. }
                            | OperationKind::BoundaryCall { .. })
                }))
    })
}

/// This is a static occurrence lookup, not availability evidence. The same
/// claim can visit several results, so select by exact place and producer, never
/// by a machine-wide search for the first occurrence of that claim identity.
pub(crate) fn linear_call_result(
    caller: &TerminalMachine,
    place: PlaceId,
) -> Option<(
    &terminal_psi::Operation,
    &terminal_psi::StructuralOperationResult,
)> {
    let mut declarations = caller
        .structural_places
        .iter()
        .filter(|declaration| declaration.id == place);
    let declaration = declarations.next()?;
    if declarations.next().is_some() {
        return None;
    }
    let StructuralPlaceKind::OperationResult {
        producer,
        structural_type,
    } = declaration.kind
    else {
        return None;
    };
    let mut operations = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| operation.id == producer);
    let operation = operations.next()?;
    if operations.next().is_some()
        || !matches!(
            operation.kind,
            OperationKind::CallStructural { .. }
                | OperationKind::CallStructuralWithScalarArguments { .. }
                | OperationKind::BoundaryCall { .. }
        )
    {
        return None;
    }
    let result = operation.result.structural()?;
    (result.place == place
        && result.structural_type == structural_type
        && result.multiplicity == StructuralMultiplicity::Linear
        && !result.claims.is_empty())
    .then_some((operation, result))
}

/// The qualification evidence a byte-sequence literal occurrence carries:
/// its establishment operation replays the domains checking admitted on the
/// literal's bytes. This is a static occurrence lookup — one literal place
/// has exactly one establishment — so select by exact destination, never by
/// a machine-wide search for the first establishment.
pub(crate) fn literal_qualifications(
    caller: &TerminalMachine,
    place: PlaceId,
) -> Option<&[StructuralDomainId]> {
    let mut operations = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            OperationKind::EstablishByteSequenceLiteral {
                destination,
                qualifications,
                ..
            } if *destination == place => Some(qualifications.as_slice()),
            _ => None,
        });
    let qualifications = operations.next()?;
    if operations.next().is_some() {
        return None;
    }
    Some(qualifications)
}

/// What the call's kind implies for every argument: the operation kind at
/// the call site, whether it is a Unit call, a borrowed call, an ordinary
/// call under the source policy, whether results may be projected, and
/// whether the call's own result is a live reference roster.
#[derive(Clone, Copy)]
pub(super) struct CallShape<'a> {
    pub(super) call_kind: Option<&'a OperationKind>,
    pub(super) borrowed_call: bool,
    pub(super) ordinary_call: bool,
    pub(super) result_projection: bool,
    /// The call returns reference custody, so its owned operands may name a
    /// declared-field subtree of a result carrier: the callee's published
    /// result source map rejoins each leaf loan under that exact projection.
    pub(super) reference_result_call: bool,
}

pub(crate) fn validate_structural_arguments(
    module: &TerminalModule,
    caller: &TerminalMachine,
    arguments: &[StructuralArgument],
    expected: &[StructuralParameterDeclaration],
    operation: OperationId,
    allow_projected: bool,
    source_policy: StructuralArgumentSourcePolicy,
) -> Result<(), ModuleError> {
    let call_operation = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|candidate| candidate.id == operation);
    let call_kind = call_operation.map(|candidate| &candidate.kind);
    let unit_call = matches!(call_kind, Some(OperationKind::CallUnit { .. }));
    let borrowed_call = matches!(
        call_kind,
        Some(
            OperationKind::CallUnit { .. }
                | OperationKind::CallStructural { .. }
                | OperationKind::CallStructuralWithScalarArguments { .. }
        )
    );
    let ordinary_call = matches!(
        call_kind,
        Some(OperationKind::CallUnit { .. } | OperationKind::CallStructuralScalar { .. })
    ) || (source_policy
        == StructuralArgumentSourcePolicy::ParametersOrAffineLocalsAndCallResults
        && matches!(
            call_kind,
            Some(
                OperationKind::CallStructural { .. }
                    | OperationKind::CallStructuralWithScalarArguments { .. }
            )
        ));
    let reference_result_call = call_operation
        .and_then(|candidate| candidate.result.structural())
        .is_some_and(|result| {
            crate::validation::references::contains_reference(module, result.structural_type)
        });
    let result_projection = allow_projected && (unit_call || reference_result_call);
    let shape = CallShape {
        call_kind,
        borrowed_call,
        ordinary_call,
        result_projection,
        reference_result_call,
    };
    if arguments.len() != expected.len() {
        return Err(ModuleError::StructuralArgumentArityMismatch {
            operation,
            expected: expected.len(),
            actual: arguments.len(),
        });
    }
    for (index, (argument, expected)) in arguments.iter().zip(expected).enumerate() {
        argument_checks::validate_structural_argument(
            module,
            caller,
            operation,
            allow_projected,
            source_policy,
            &shape,
            index,
            argument,
            expected,
        )?;
    }
    argument_pairs::validate_argument_pairs(module, caller, arguments, operation, &shape)?;
    Ok(())
}

pub(crate) fn structural_occurrence_carries_qualification(
    root: &[StructuralDomainId],
    projected: &[terminal_psi::StructuralPathQualification],
    path: &[StructuralPathSegment],
    domain: StructuralDomainId,
) -> bool {
    if path.is_empty() {
        root.contains(&domain)
    } else {
        projected
            .iter()
            .any(|qualification| qualification.path == path && qualification.domain == domain)
    }
}

pub(crate) fn structural_access_can_supply(
    source: StructuralAccess,
    presented: StructuralAccess,
) -> bool {
    match source {
        StructuralAccess::Owned => true,
        StructuralAccess::SharedBorrow => presented == StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow => matches!(
            presented,
            StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow
        ),
        StructuralAccess::WriteOnlyBorrow => presented == StructuralAccess::WriteOnlyBorrow,
    }
}

fn structural_access_is_exclusive(access: StructuralAccess) -> bool {
    matches!(
        access,
        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
    )
}

pub(crate) use terminal_semantics::structural_paths_may_overlap;

fn literal_index_path(path: &[StructuralPathSegment]) -> Option<&[StructuralPathSegment]> {
    let field_count = path
        .iter()
        .position(|segment| matches!(segment, StructuralPathSegment::FixedIndex(_)))?;
    let (fields, indexes) = path.split_at(field_count);
    (!indexes.is_empty()
        && fields
            .iter()
            .all(|segment| matches!(segment, StructuralPathSegment::Field(_)))
        && indexes
            .iter()
            .all(|segment| matches!(segment, StructuralPathSegment::FixedIndex(_))))
    .then_some(fields)
}

#[cfg(test)]
#[test]
fn fixed_byte_window_overlap_replays_intervals_indexes_and_parent_custody() {
    let range = |start, end| StructuralPathSegment::FixedByteRange { start, end };
    let field = StructuralPathSegment::Field("bytes".into());
    let left = vec![field.clone(), range(1, 3)];
    for (right, overlaps) in [
        (vec![], true),
        (vec![field.clone()], true),
        (vec![field.clone(), range(2, 4)], true),
        (vec![field.clone(), range(3, 4)], false),
        (
            vec![field.clone(), StructuralPathSegment::FixedIndex(1)],
            true,
        ),
        (
            vec![field.clone(), StructuralPathSegment::FixedIndex(3)],
            false,
        ),
        (vec![field.clone(), range(4, 4)], true),
        (
            vec![StructuralPathSegment::Field("other".into()), range(1, 3)],
            false,
        ),
    ] {
        assert_eq!(structural_paths_may_overlap(&left, &right), overlaps);
        assert_eq!(structural_paths_may_overlap(&right, &left), overlaps);
    }
}

fn is_direct_literal_index_path(path: &[StructuralPathSegment]) -> bool {
    literal_index_path(path).is_some_and(|fields| fields.is_empty())
}

pub(crate) fn is_literal_indexed_field_path(path: &[StructuralPathSegment]) -> bool {
    literal_index_path(path).is_some_and(|fields| !fields.is_empty())
}

pub(crate) fn is_admitted_unit_call_argument_path(
    module: &TerminalModule,
    caller: &TerminalMachine,
    argument: &StructuralArgument,
) -> bool {
    argument.path.is_empty()
        || crate::validation::references::is_reference_projection(module, caller, argument)
        || is_nonempty_field_path(&argument.path)
        || is_literal_indexed_field_path(&argument.path)
        || is_direct_literal_index_path(&argument.path)
        || (matches!(argument.access, StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow)
            && matches!(argument.path.split_last(),
                Some((StructuralPathSegment::FixedByteRange { .. }, backing))
                    if backing.iter().all(|segment| matches!(segment, StructuralPathSegment::Field(identity) if !identity.is_empty()))))
        || (matches!(argument.access, StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow)
            && is_static_borrow_path(&argument.path))
        // A runtime-indexed shared borrow keeps the owning root and replays
        // the selector's published bounds; the dynamic element is borrowed,
        // not moved, and its access stays `SharedBorrow`.
        || (argument.access == StructuralAccess::SharedBorrow
            && is_runtime_indexed_borrow_path(&argument.path))
        // Structural paths already retain only fields and literal indexes;
        // write-only subloans may interleave them. Resolution checks each hop.
        || argument.access == StructuralAccess::WriteOnlyBorrow
        || (argument.access == StructuralAccess::Owned
            && partial_affine_root_type(caller, argument.place).is_some_and(|structural_type| {
                is_partial_affine_path(module, structural_type, &argument.path)
            }))
}

// Borrowing a statically located element does not transfer its owner. Shape
// recognition must not manufacture linear custody for a deeper array path;
// the receiving check separately resolves every field and fixed-index bound.
// First-class reference paths retain their own authority rules.

fn is_static_borrow_path(path: &[StructuralPathSegment]) -> bool {
    !path.is_empty()
        && path.iter().all(|segment| match segment {
            StructuralPathSegment::Field(identity) => !identity.is_empty(),
            StructuralPathSegment::FixedIndex(_) => true,
            StructuralPathSegment::Referent
            | StructuralPathSegment::FixedByteRange { .. }
            | StructuralPathSegment::RuntimeIndex { .. } => false,
        })
}

/// A borrowed projection that crosses at least one runtime index. Fields,
/// literal indexes, and `RuntimeIndex` hops may interleave; each runtime hop
/// still resolves to its fixed array's element type while
/// `validate_structural_argument` independently replays the selector's
/// published entry range. `Referent` and byte ranges stay out.
pub(crate) fn is_runtime_indexed_borrow_path(path: &[StructuralPathSegment]) -> bool {
    let mut saw_runtime_index = false;
    for segment in path {
        match segment {
            StructuralPathSegment::Field(identity) => {
                if identity.is_empty() {
                    return false;
                }
            }
            StructuralPathSegment::FixedIndex(_) => {}
            StructuralPathSegment::RuntimeIndex { .. } => saw_runtime_index = true,
            StructuralPathSegment::Referent | StructuralPathSegment::FixedByteRange { .. } => {
                return false;
            }
        }
    }
    saw_runtime_index
}

fn is_material_write_only_type(module: &TerminalModule, structural_type: StructuralTypeId) -> bool {
    // Foundation establishes an acyclic, closed graph without bounding depth.
    // Traverse iteratively and inspect each shared declaration only once.
    let mut pending = vec![structural_type];
    let mut visited = BTreeSet::new();
    while let Some(current) = pending.pop() {
        if !visited.insert(current) {
            continue;
        }
        let Some(declaration) = module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == current)
        else {
            return false;
        };
        match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(_) => {}
            StructuralTypeShape::Record { fields } => {
                for field in fields {
                    if field.relevance.is_erased() {
                        return false;
                    }
                    match field.field_type {
                        StructuralFieldType::Scalar(_)
                        | StructuralFieldType::BoundedInteger(_)
                        | StructuralFieldType::IeeeFloat(_) => {}
                        StructuralFieldType::Structural(next) => pending.push(next),
                        StructuralFieldType::ByteSequence(_)
                        | StructuralFieldType::Erased { .. } => return false,
                    }
                }
            }
            StructuralTypeShape::FixedArray { element, .. } => pending.push(*element),
            StructuralTypeShape::ByteSequence(_)
            | StructuralTypeShape::ElementView { .. }
            | StructuralTypeShape::Reference { .. }
            | StructuralTypeShape::Sum { .. }
            | StructuralTypeShape::Mixed { .. } => return false,
        }
    }
    true
}

pub(crate) fn is_unrestricted_write_only_subloan(
    module: &TerminalModule,
    caller: &TerminalMachine,
    expected: &StructuralParameterDeclaration,
    argument: &StructuralArgument,
) -> bool {
    // An established, claim-free result keeps whole-value custody like an
    // owned parameter does: it can lend one exact subtree as a write-only
    // projection while its producer's cleanup stays intact.
    let Some((actual_type, actual_access, actual_multiplicity)) = caller
        .structural_parameters
        .iter()
        .find(|actual| actual.place == argument.place)
        .map(|actual| (actual.structural_type, actual.access, actual.multiplicity))
        .or_else(|| {
            let result =
                crate::validation::record::completed_source(module, caller, argument.place)?;
            (result.multiplicity == StructuralMultiplicity::Unrestricted
                && !caller
                    .entry_claims
                    .iter()
                    .any(|claim| claim.input == argument.place)
                && !caller
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == argument.place))
            .then_some((
                result.structural_type,
                StructuralAccess::Owned,
                result.multiplicity,
            ))
        })
    else {
        return false;
    };
    let indexed_path_is_material = !argument
        .path
        .iter()
        .any(|segment| matches!(segment, StructuralPathSegment::FixedIndex(_)))
        || (is_material_write_only_type(module, actual_type)
            && resolve_structural_path(module, actual_type, &argument.path).is_some_and(|leaf| {
                module.structural_types.iter().any(|declaration| {
                    declaration.id == leaf
                        && matches!(
                            &declaration.shape,
                            StructuralTypeShape::PrimitiveScalar(_)
                                | StructuralTypeShape::Record { .. }
                        )
                })
            }));

    !argument.path.is_empty()
        && argument.access == StructuralAccess::WriteOnlyBorrow
        && expected.access == StructuralAccess::WriteOnlyBorrow
        && expected.multiplicity == StructuralMultiplicity::Unrestricted
        && matches!(
            actual_access,
            StructuralAccess::Owned
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow
        )
        && actual_multiplicity == StructuralMultiplicity::Unrestricted
        && indexed_path_is_material
        && (actual_access != StructuralAccess::Owned
            || resolve_structural_path(module, actual_type, &argument.path)
                == Some(expected.structural_type))
}

pub(crate) fn is_unrestricted_shared_subloan(
    module: &TerminalModule,
    caller: &TerminalMachine,
    expected: &StructuralParameterDeclaration,
    argument: &StructuralArgument,
) -> bool {
    let Some(actual) = caller
        .structural_parameters
        .iter()
        .find(|actual| actual.place == argument.place)
    else {
        return false;
    };
    if argument.access == StructuralAccess::SharedBorrow
        && terminal_semantics::fixed_byte_array_window(
            module.structural_types.iter(),
            actual,
            argument,
            expected,
        )
        .is_some()
    {
        return true;
    }
    (terminal_psi::is_bounded_structural_scalar_store_path(&argument.path)
        || (matches!(
            actual.access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        ) && (is_static_borrow_path(&argument.path)
            // A shared observation through a runtime index lends one
            // element of the same root without moving it; the selector's
            // bounds are replayed by the argument check itself.
            || is_runtime_indexed_borrow_path(&argument.path))))
        && argument.access == StructuralAccess::SharedBorrow
        && expected.access == StructuralAccess::SharedBorrow
        && expected.multiplicity == StructuralMultiplicity::Unrestricted
        && actual.multiplicity == StructuralMultiplicity::Unrestricted
}

pub(crate) fn is_unrestricted_mutable_subloan(
    module: &TerminalModule,
    caller: &TerminalMachine,
    expected: &StructuralParameterDeclaration,
    argument: &StructuralArgument,
) -> bool {
    let Some(actual) = caller
        .structural_parameters
        .iter()
        .find(|actual| actual.place == argument.place)
    else {
        return false;
    };
    if argument.access == StructuralAccess::MutableBorrow
        && terminal_semantics::fixed_byte_array_window(
            module.structural_types.iter(),
            actual,
            argument,
            expected,
        )
        .is_some()
    {
        return true;
    }
    is_static_borrow_path(&argument.path)
        && argument.access == StructuralAccess::MutableBorrow
        && expected.access == StructuralAccess::MutableBorrow
        && expected.multiplicity == StructuralMultiplicity::Unrestricted
        // Owned storage can lend its exact subtree. Do not let this also
        // authorize inline byte-field presentation: that field has no
        // standalone type, and this predicate also gates the view adapter.
        && (actual.access == StructuralAccess::MutableBorrow
            || (actual.access == StructuralAccess::Owned
                && resolve_structural_path(module, actual.structural_type, &argument.path)
                    == Some(expected.structural_type)))
        && actual.multiplicity == StructuralMultiplicity::Unrestricted
}
