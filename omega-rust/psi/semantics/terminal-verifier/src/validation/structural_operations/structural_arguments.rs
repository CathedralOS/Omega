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

pub(crate) fn structural_paths_may_overlap(
    left: &[StructuralPathSegment],
    right: &[StructuralPathSegment],
) -> bool {
    left.iter().zip(right).all(|(left, right)| left == right)
}

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
            && is_static_borrow_path(&argument.path))
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
            StructuralPathSegment::Referent => false,
        })
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
    let Some(actual) = caller
        .structural_parameters
        .iter()
        .find(|actual| actual.place == argument.place)
    else {
        return false;
    };
    let indexed_path_is_material = !argument
        .path
        .iter()
        .any(|segment| matches!(segment, StructuralPathSegment::FixedIndex(_)))
        || (is_material_write_only_type(module, actual.structural_type)
            && resolve_structural_path(module, actual.structural_type, &argument.path)
                .is_some_and(|leaf| {
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
            actual.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        )
        && actual.multiplicity == StructuralMultiplicity::Unrestricted
        && indexed_path_is_material
}

pub(crate) fn is_unrestricted_shared_subloan(
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
    (terminal_psi::is_bounded_structural_scalar_store_path(&argument.path)
        || (matches!(
            actual.access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        ) && is_static_borrow_path(&argument.path)))
        && argument.access == StructuralAccess::SharedBorrow
        && expected.access == StructuralAccess::SharedBorrow
        && expected.multiplicity == StructuralMultiplicity::Unrestricted
        && actual.multiplicity == StructuralMultiplicity::Unrestricted
}

pub(crate) fn is_unrestricted_mutable_subloan(
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
    is_static_borrow_path(&argument.path)
        && argument.access == StructuralAccess::MutableBorrow
        && expected.access == StructuralAccess::MutableBorrow
        && expected.multiplicity == StructuralMultiplicity::Unrestricted
        && actual.access == StructuralAccess::MutableBorrow
        && actual.multiplicity == StructuralMultiplicity::Unrestricted
}
