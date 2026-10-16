//! Rebase callee crash paths onto the exact caller structural argument source.

use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;
use crate::unit::attached_unit::primitive_locals::PrimitiveLocal;
use semantic_vocabulary::{CanonicalStructuralPathSegment, StructuralPlaceKind};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralArgument, StructuralFieldType,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, find_structural_type,
};

/// A reference operand's physical descriptor is not its observed backing.
/// The checked call resource selects the loan at operand completion; the
/// emitter's exact formation row supplies its already-evaluated address.
/// Content writes do not retarget that address, and a later descriptor write
/// cannot replace the selected occurrence with a newer loan.
pub(crate) fn captured_crash_argument(
    checked: &checked_trees::CheckedTrees,
    coordinate: crate::emission::operation_emission::buffer::SourceCallCoordinate,
    formal_position: u32,
    argument: &StructuralArgument,
    captures: &[lowered_psi::LoweredSourceLoanCapture],
    structural_types: &[StructuralTypeDeclaration],
    root_type: &mut impl FnMut(
        &StructuralArgument,
    ) -> Result<semantic_vocabulary::StructuralTypeId, LoweringError>,
) -> Result<StructuralArgument, LoweringError> {
    let Some((StructuralPathSegment::Referent, suffix)) = argument.path.split_first() else {
        return Ok(argument.clone());
    };
    let flow = &checked.facts.flow;
    let state = flow
        .control
        .states
        .iter()
        .find_map(|(_, state)| (state.state_symbol == coordinate.state).then_some(state))
        .ok_or(LoweringError::Unsupported(
            "crash observation has no checked caller state",
        ))?;
    let call_handle = state
        .calls
        .handles()
        .find(|handle| {
            let call = flow.control.calls.get(*handle);
            call.statement_index == coordinate.statement_index
                && call.call_ordinal == coordinate.call_ordinal
        })
        .ok_or(LoweringError::Unsupported(
            "crash observation has no exact checked invocation",
        ))?;
    let call = flow.control.calls.get(call_handle);
    let receiver_offset = usize::from(call.authored_receiver.is_valid());
    let owner_operand = call
        .operands
        .handles()
        .nth(formal_position as usize + receiver_offset)
        .ok_or(LoweringError::Unsupported(
            "crash observation has no captured actual",
        ))?;
    let borrow = &checked.facts.borrow;
    let mut resources = borrow
        .call_loan_resources
        .iter()
        .filter_map(|(_, resource)| {
            (resource.call == call_handle
                && resource.owner_operand == owner_operand
                && resource.capture_path.is_empty())
            .then_some(resource)
        });
    let resource = resources.next().ok_or(LoweringError::Unsupported(
        "reference crash observation has no captured loan resource",
    ))?;
    if resources.next().is_some() {
        return unsupported("reference crash observation has ambiguous loan resources");
    }
    let loan = match resource.source {
        checked_trees::CheckedCallBorrowLoanSource::StoredLoan { loan } => loan,
        checked_trees::CheckedCallBorrowLoanSource::CallAccess { parents, .. } => *borrow
            .call_loan_parents
            .span(parents)
            .and_then(|parents| parents.first())
            .ok_or(LoweringError::Unsupported(
                "reference crash observation has no retained formation lineage",
            ))?,
    };
    // Descriptor copies preserve a formation without emitting another one.
    // Follow their checked supplier DAG, not a convenient ancestor in the
    // call-access lineage. Every alternative must select the same capture.
    let mut pending = vec![loan];
    let mut visited = Vec::new();
    let mut captured = None;
    while let Some(loan) = pending.pop() {
        if visited.contains(&loan) {
            continue;
        }
        visited.push(loan);
        let mut selected = captures.iter().filter(|capture| capture.loan == loan);
        if let Some(capture) = selected.next() {
            if selected.any(|other| other != capture)
                || captured.is_some_and(|previous| previous != capture)
            {
                return unsupported(
                    "reference crash observation has conflicting formation captures",
                );
            }
            captured = Some(capture);
            continue;
        }
        let occurrence = borrow.loans.get(loan);
        let suppliers = borrow
            .loan_suppliers
            .span_or_empty(occurrence.supplying_loans);
        if suppliers.is_empty() {
            return unsupported("reference crash observation has no emitted formation capture");
        }
        for supplier in suppliers {
            let supplying = borrow.loans.get(*supplier);
            if occurrence.incoming_reference != supplying.incoming_reference
                || occurrence.root != supplying.root
                || borrow.access_segments.span_or_empty(occurrence.segments)
                    != borrow.access_segments.span_or_empty(supplying.segments)
            {
                return unsupported("reference crash transport changes its captured endpoint");
            }
            pending.push(*supplier);
        }
    }
    let mut observed = captured
        .ok_or(LoweringError::Unsupported(
            "reference crash observation has no resolved formation capture",
        ))?
        .source
        .clone();
    observed.path = terminal_semantics::reference_source_storage_path(
        structural_types.iter(),
        root_type(&observed)?,
        &observed.path,
    )
    .to_vec();
    observed.path.extend_from_slice(suffix);
    observed.access = argument.access;
    Ok(observed)
}

pub(crate) fn structural_crash_argument_type(
    argument: &StructuralArgument,
    parameters: &[StructuralParameterDeclaration],
    trivial_affine_locals: &[StructuralPlaceDeclaration],
    structural_results: &[(Option<StructuralPlaceDeclaration>, bool)],
    primitive_locals: &[PrimitiveLocal],
    operations: &[terminal_psi::Operation],
) -> Result<semantic_vocabulary::StructuralTypeId, LoweringError> {
    parameters
        .iter()
        .find(|parameter| parameter.place == argument.place)
        .map(|parameter| parameter.structural_type)
        .or_else(|| {
            trivial_affine_locals.iter().find_map(|local| {
                (local.id == argument.place)
                    .then_some(match local.kind {
                        StructuralPlaceKind::TrivialAffineLocal {
                            structural_type, ..
                        } => Some(structural_type),
                        _ => None,
                    })
                    .flatten()
            })
        })
        .or_else(|| {
            structural_results
                .iter()
                .filter_map(|(place, _)| place.as_ref())
                // These are the established caller referents, with their original producers.
                .chain(primitive_locals.iter().map(|local| &local.declaration))
                .find_map(|local| {
                    (local.id == argument.place)
                        .then_some(match local.kind {
                            StructuralPlaceKind::OperationResult {
                                structural_type, ..
                            } => Some(structural_type),
                            _ => None,
                        })
                        .flatten()
                })
        })
        .or_else(|| {
            operations.iter().find_map(|operation| {
                operation
                    .result
                    .structural()
                    .filter(|result| result.place == argument.place)
                    .map(|result| result.structural_type)
            })
        })
        .ok_or(LoweringError::Unsupported(
            "structural crash route argument has no caller structural source",
        ))
}

pub(crate) fn structural_crash_route_prefix_from_type(
    argument: &StructuralArgument,
    mut structural_type: semantic_vocabulary::StructuralTypeId,
    structural_types: &[StructuralTypeDeclaration],
) -> Result<Vec<CanonicalStructuralPathSegment>, LoweringError> {
    let mut prefix = Vec::with_capacity(argument.path.len());
    let mut selected_case: Option<&terminal_psi::StructuralCaseDeclaration> = None;
    for (position, segment) in argument.path.iter().enumerate() {
        let declaration = find_structural_type(structural_types, structural_type).ok_or(
            LoweringError::Unsupported("structural crash route argument path type is absent"),
        )?;
        if selected_case.is_some() && !matches!(segment, StructuralPathSegment::Field(_)) {
            return unsupported("structural crash route case must select its payload field");
        }
        match segment {
            StructuralPathSegment::AllElements => {
                return unsupported("universal qualification is not a crash operand path");
            }
            StructuralPathSegment::RepresentationView { .. } => {
                return unsupported(
                    "returned view origins cannot create canonical crash predicate places",
                );
            }
            StructuralPathSegment::Case(selected) => {
                let cases = match &declaration.shape {
                    StructuralTypeShape::Sum { cases }
                    | StructuralTypeShape::Mixed { cases, .. } => cases,
                    _ => return unsupported("structural crash route case has no source sum"),
                };
                let mut matches = cases.iter().filter(|case| case.id == *selected);
                let case = matches.next().ok_or(LoweringError::Unsupported(
                    "structural crash route case belongs to another declaration",
                ))?;
                if matches.next().is_some() {
                    return unsupported("structural crash route case is ambiguous");
                }
                selected_case = Some(case);
                prefix.push(CanonicalStructuralPathSegment::Case(case.id));
            }
            StructuralPathSegment::FixedByteRange { .. } => {
                return unsupported(
                    "byte window crash predicates require exact view extent custody",
                );
            }
            StructuralPathSegment::Referent => {
                return unsupported(
                    "reference crash paths require retained referent projection custody",
                );
            }
            StructuralPathSegment::RuntimeIndex { .. } => {
                return unsupported(
                    "runtime-indexed crash predicates have no canonical element prefix",
                );
            }
            StructuralPathSegment::Field(identity) => {
                let fields = if let Some(case) = selected_case.take() {
                    &case.fields
                } else {
                    match &declaration.shape {
                        terminal_psi::record_or_mixed_shape_pattern!(fields) => fields,
                        _ => {
                            return unsupported(
                                "structural crash route argument path receiver is not a record",
                            );
                        }
                    }
                };
                let field = terminal_psi::field_by_identity(fields, *identity)
                    .filter(|field| !field.relevance.is_erased())
                    .ok_or(LoweringError::Unsupported(
                        "structural crash route argument field is absent or erased",
                    ))?;
                prefix.push(CanonicalStructuralPathSegment::Field(field.id));
                match field.field_type {
                    StructuralFieldType::Structural(next) => structural_type = next,
                    StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                        ..
                    }) if position + 1 == argument.path.len()
                        && argument.access == StructuralAccess::MutableBorrow =>
                    {
                        return Ok(prefix);
                    }
                    _ => {
                        return unsupported(
                            "structural crash route argument field is not structural",
                        );
                    }
                }
            }
            StructuralPathSegment::FixedIndex(index) => {
                let StructuralTypeShape::FixedArray { element, length } = declaration.shape else {
                    return unsupported(
                        "structural crash route argument fixed index receiver is not an array",
                    );
                };
                if *index >= length {
                    return unsupported(
                        "structural crash route argument fixed index is out of bounds",
                    );
                }
                prefix.push(CanonicalStructuralPathSegment::FixedIndex(*index));
                structural_type = element;
            }
        }
    }
    if selected_case.is_some() {
        return unsupported("structural crash route case has no payload field");
    }
    Ok(prefix)
}
