use crate::descriptor_lifetimes::{
    DescriptorFlowStep, DescriptorLifetime, DescriptorPosition, descriptor_path,
};
use semantic_vocabulary::{BlockId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    StructuralAccess, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalAffineCleanupAction,
};

pub struct ReferenceCompletionOccurrence<'a, Identity> {
    pub identity: &'a Identity,
    pub parents: Vec<&'a Identity>,
    pub returned: bool,
    pub in_caller_storage: bool,
    pub disposed: bool,
}

/// Determine whether every path from this loan's formation to this return
/// explicitly closes all its remaining carrier names. An edge captures its
/// arguments before cleanup, so a forwarded name keeps the path open.
pub fn disposed_reference_on_return(
    machine: &terminal_psi::TerminalMachine,
    types: &[StructuralTypeDeclaration],
    lifetime: &DescriptorLifetime,
    formed_in: BlockId,
    at: DescriptorPosition,
    cleanup: &[TerminalAffineCleanupAction],
    flow: Option<&BTreeMap<DescriptorPosition, DescriptorFlowStep>>,
) -> bool {
    // An empty intersection at a join proves no release: a different arrival
    // can still owe the loan. A closing cut must name actual authority there
    // and dispose every spelling that can survive on any arrival.
    let closes_names = |position, actions: &[TerminalAffineCleanupAction]| {
        lifetime
            .authorized
            .get(&position)
            .is_some_and(|names| !names.is_empty())
            && lifetime.surviving.get(&position).is_some_and(|names| {
                !names.is_empty()
                    && names.iter().all(|name| {
                        machine
                            .structural_place_type(name.root)
                            .and_then(|root| descriptor_path(types, root, &name.path))
                            .is_some_and(|path| {
                                cleanup_disposes_reference(actions, name.root, &path)
                            })
                    })
            })
    };
    if lifetime
        .surviving
        .get(&at)
        .is_some_and(|names| !names.is_empty())
    {
        return closes_names(at, cleanup);
    }
    let Some(flow) = flow else { return false };
    let mut pending = vec![formed_in];
    let mut visited = BTreeSet::new();
    let mut found_cleanup = false;
    while let Some(id) = pending.pop() {
        if !visited.insert(id) {
            continue;
        }
        if id == at.0 {
            return false;
        }
        let Some(block) = machine.block(&id) else {
            return false;
        };
        let Ok(end) = u32::try_from(block.operations.len()) else {
            return false;
        };
        let before = (id, end);
        let Some(step) = flow.get(&before) else {
            return false;
        };
        for (identity, target) in block.terminator.successor_edges() {
            let Some(edge) = step
                .successors
                .iter()
                .find(|edge| edge.edge == Some(identity) && edge.target.0 == target)
            else {
                return false;
            };
            let closed = block
                .terminator
                .cleanup_rosters()
                .any(|(id, actions)| id == identity && closes_names(before, actions))
                && lifetime
                    .surviving
                    .get(&before)
                    .is_some_and(|names| edge.transport_locations(names, false, true).is_empty());
            if closed {
                found_cleanup = true;
            } else {
                pending.push(target);
            }
        }
    }
    found_cleanup
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceCompletionError {
    DuplicateOccurrence,
    MissingParent,
    CyclicAncestry,
    UndisposedParent,
    SurvivingChild,
}

impl ReferenceCompletionError {
    pub const fn description(self) -> &'static str {
        match self {
            Self::DuplicateOccurrence => "reference completion has duplicate occurrence identities",
            Self::MissingParent => "reference completion lost an immediate parent",
            Self::CyclicAncestry => "reference completion has cyclic ancestry",
            Self::UndisposedParent => {
                "reference completion does not dispose an intermediate parent"
            }
            Self::SurvivingChild => "reference completion leaves a child of a closing parent live",
        }
    }
}

pub fn validate_reference_result_completion<Identity: PartialEq>(
    occurrences: &[ReferenceCompletionOccurrence<'_, Identity>],
) -> Result<(), ReferenceCompletionError> {
    for (position, occurrence) in occurrences.iter().enumerate() {
        if occurrences[..position]
            .iter()
            .any(|previous| previous.identity == occurrence.identity)
        {
            return Err(ReferenceCompletionError::DuplicateOccurrence);
        }
    }
    let mut visited = vec![0u8; occurrences.len()];
    let mut closing = vec![false; occurrences.len()];
    for (position, occurrence) in occurrences
        .iter()
        .enumerate()
        .filter(|(_, occurrence)| occurrence.returned || occurrence.in_caller_storage)
    {
        if occurrence.disposed {
            return Err(ReferenceCompletionError::UndisposedParent);
        }
        let mut pending = vec![(position, false)];
        while let Some((position, completed)) = pending.pop() {
            if completed {
                visited[position] = 2;
                continue;
            }
            match visited[position] {
                1 => return Err(ReferenceCompletionError::CyclicAncestry),
                2 => continue,
                _ => {}
            }
            visited[position] = 1;
            pending.push((position, true));
            for parent in occurrences[position].parents.iter().rev() {
                let parent_position = occurrences
                    .iter()
                    .position(|candidate| candidate.identity == *parent)
                    .ok_or(ReferenceCompletionError::MissingParent)?;
                let parent = &occurrences[parent_position];
                if !parent.in_caller_storage && (!parent.disposed || parent.returned) {
                    return Err(ReferenceCompletionError::UndisposedParent);
                }
                closing[parent_position] = !parent.in_caller_storage;
                pending.push((parent_position, false));
            }
        }
    }
    for (position, occurrence) in occurrences.iter().enumerate() {
        if !closing[position] {
            continue;
        }
        if occurrences.iter().any(|child| {
            child.parents.contains(&occurrence.identity)
                && !child.returned
                && !child.in_caller_storage
                && !child.disposed
        }) {
            return Err(ReferenceCompletionError::SurvivingChild);
        }
    }
    Ok(())
}

/// A borrow keeps the resource it arrived with. Literal storage can own
/// shared permission; an input or an earlier loan keeps its own identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowedLoanSource {
    Owner,
    Incoming,
    Loan(semantic_vocabulary::ObligationId),
    Provider,
}

// Inline byte-view fields carry their type on the field itself. They need
// not have a separate type declaration, but still name an exact leaf.
fn borrowed_descriptor_shape(
    module: &terminal_psi::TerminalModule,
    root_type: StructuralTypeId,
    path: &[crate::descriptor_lifetimes::DescriptorPathSegment],
) -> Option<StructuralTypeShape> {
    let path = crate::descriptor_lifetimes::descriptor_path(
        &module.types.structural_types,
        root_type,
        path,
    )?;
    let shape = if let Some(tip) =
        crate::runtime_structural_path_tip(module.types.structural_types.iter(), root_type, &path)
    {
        match &tip.shape {
            StructuralTypeShape::ByteSequence(carrier) => {
                StructuralTypeShape::ByteSequence(*carrier)
            }
            StructuralTypeShape::ElementView { element, access } => {
                StructuralTypeShape::ElementView {
                    element: *element,
                    access: *access,
                }
            }
            _ => return None,
        }
    } else {
        StructuralTypeShape::ByteSequence(
            crate::runtime_structural_field(
                module.types.structural_types.iter(),
                root_type,
                &path,
            )?
            .field_type
            .byte_sequence_carrier()?,
        )
    };
    (shape.is_borrowed_byte_carrier() || matches!(shape, StructuralTypeShape::ElementView { .. }))
        .then_some(shape)
}

/// Different slices may borrow the same input value. Every possible source
/// must name that same input or field. Address and length checks keep
/// all slice and capture information.
pub fn incoming_view_occurrence(
    module: &terminal_psi::TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    origins: &[crate::descriptor_results::DescriptorSourceOrigin],
) -> Result<crate::descriptor_lifetimes::DescriptorLocation, &'static str> {
    use crate::descriptor_lifetimes::{descriptor_location, descriptor_path};
    use crate::descriptor_results::{DescriptorCallOrigin, DescriptorSourceOrigin};
    let mut original = None;
    for origin in origins {
        let location = match origin {
            DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument {
                source,
                projection,
            }) if !projection.indirect => {
                let parameter = crate::exactly_one::exactly_one(
                    machine.structural_parameters.iter(),
                    |parameter| parameter.place == *source,
                )
                .ok_or("the view's original incoming borrow cannot be identified")?;
                let path = projection
                    .original_descriptor_path()
                    .ok_or("the view's original borrowed field cannot be identified")?;
                descriptor_location(
                    &module.types.structural_types,
                    *source,
                    parameter.structural_type,
                    path,
                    None,
                )
                .ok_or("the view's original borrowed field cannot be identified")?
            }
            DescriptorSourceOrigin::IncomingSlot(location) => location.clone(),
            DescriptorSourceOrigin::IncomingContents { query, projection }
                if !projection.indirect
                    && projection
                        .original_descriptor_path()
                        .is_some_and(|path| path.is_empty()) =>
            {
                query
                    .exact_location()
                    .ok_or("the view's original borrowed field cannot be identified")?
            }
            _ => return Err("the view's original borrow cannot be identified"),
        };
        let parameter =
            crate::exactly_one::exactly_one(machine.structural_parameters.iter(), |parameter| {
                parameter.place == location.root
            })
            .ok_or("the view's original incoming borrow cannot be identified")?;
        let path = descriptor_path(
            &module.types.structural_types,
            parameter.structural_type,
            &location.path,
        )
        .ok_or("the view's original borrowed field cannot be identified")?;
        if !crate::projection_stays_in_inline_storage(
            module.types.structural_types.iter(),
            parameter.structural_type,
            &path,
        ) || borrowed_descriptor_shape(module, parameter.structural_type, &location.path)
            .is_none()
        {
            return Err("the view's original borrowed field cannot be identified");
        }
        if original
            .as_ref()
            .is_some_and(|previous| *previous != location)
        {
            return Err("the view can name different incoming borrows or fields");
        }
        original = Some(location);
    }
    original.ok_or("the view's original borrow cannot be identified")
}

/// A shared reference whose arrivals each bring a different live loan
/// (`match choice { true -> incoming, _ -> incoming }` forms one loan per
/// arm) has no single parent loan at the join. It still borrows the
/// caller's storage when the arrival replay traces every arrival to the
/// same exact field of one borrowed input: that input is the lender of
/// each arm's loan. Arrivals from different inputs or local storage stay
/// refused, and so do exclusive references, whose reborrow must descend
/// from the one loan its arrival brought.
fn joined_incoming_loans(
    module: &terminal_psi::TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    analysis: &mut crate::descriptor_results::DescriptorAnalysis<'_>,
    at: crate::descriptor_lifetimes::DescriptorPosition,
    descriptor: &crate::descriptor_lifetimes::DescriptorLocation,
) -> Result<BorrowedLoanSource, &'static str> {
    use crate::descriptor_results::{DescriptorCallOrigin, DescriptorSourceOrigin};
    const UNRESOLVED: &str =
        "reference descriptor has an unresolved surviving immediate loan occurrence";
    let shared = machine
        .structural_place_type(descriptor.root)
        .and_then(|ty| {
            let path = crate::descriptor_lifetimes::descriptor_path(
                &module.types.structural_types,
                ty,
                &descriptor.path,
            )?;
            crate::runtime_structural_path_tip(module.types.structural_types.iter(), ty, &path)
        })
        .is_some_and(|row| {
            matches!(
                row.shape,
                StructuralTypeShape::Reference {
                    access: StructuralAccess::SharedBorrow,
                    ..
                }
            )
        });
    if !shared {
        return Err(UNRESOLVED);
    }
    let origins = analysis
        .symbolic_source_origins(machine.id, at, descriptor)
        .ok_or(UNRESOLVED)?;
    let [DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument { source, projection })] =
        origins.as_slice()
    else {
        return Err("joined reference arms borrow from different lenders");
    };
    let parameter =
        crate::exactly_one::exactly_one(machine.structural_parameters.iter(), |parameter| {
            parameter.place == *source
        })
        .filter(|parameter| parameter.access != StructuralAccess::Owned)
        .ok_or("joined reference arms borrow from different lenders")?;
    let path = projection.original_descriptor_path().ok_or(UNRESOLVED)?;
    if projection.window.is_some()
        || projection
            .enclosing_storage()
            .is_none_or(|storage| storage.path != path)
        || !crate::projection_stays_in_inline_storage(
            module.types.structural_types.iter(),
            parameter.structural_type,
            path,
        )
    {
        return Err(UNRESOLVED);
    }
    Ok(BorrowedLoanSource::Incoming)
}

/// Complete borrowed-resource correspondence at this use. A family keeps
/// every incoming storage identity; it does not select a new loan's parent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BorrowedLoanSources {
    Single(BorrowedLoanSource),
    Incoming(Vec<crate::descriptor_results::DescriptorCallOrigin>),
    Selected(crate::descriptor_lifetimes::DescriptorLoanChoice),
}

impl BorrowedLoanSources {
    pub fn single(self) -> Result<BorrowedLoanSource, &'static str> {
        match self {
            Self::Single(source) => Ok(source),
            Self::Incoming(family) if family.len() == 1 => Ok(BorrowedLoanSource::Incoming),
            Self::Incoming(_) => {
                Err("reference descriptor has no single immediate incoming occurrence")
            }
            Self::Selected(_) => {
                Err("reference descriptor has an unresolved surviving immediate loan occurrence")
            }
        }
    }
}

/// Select one immediate resource when a new borrow or result needs it.
pub fn borrowed_loan_source<'a>(
    module: &terminal_psi::TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    analysis: &mut crate::descriptor_results::DescriptorAnalysis<'_>,
    at: crate::descriptor_lifetimes::DescriptorPosition,
    descriptor: &crate::descriptor_lifetimes::DescriptorLocation,
    candidates: impl Iterator<
        Item = (
            semantic_vocabulary::ObligationId,
            &'a crate::descriptor_lifetimes::DescriptorLifetime,
        ),
    >,
) -> Result<BorrowedLoanSource, &'static str> {
    borrowed_loan_sources(module, machine, analysis, at, descriptor, candidates)?.single()
}

/// Authenticate the resources carried by this borrowed transport.
pub fn borrowed_loan_sources<'a>(
    module: &terminal_psi::TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    analysis: &mut crate::descriptor_results::DescriptorAnalysis<'_>,
    at: crate::descriptor_lifetimes::DescriptorPosition,
    descriptor: &crate::descriptor_lifetimes::DescriptorLocation,
    candidates: impl Iterator<
        Item = (
            semantic_vocabulary::ObligationId,
            &'a crate::descriptor_lifetimes::DescriptorLifetime,
        ),
    >,
) -> Result<BorrowedLoanSources, &'static str> {
    let mut parent = None;
    let mut alternatives = Vec::new();
    // Loans that may still own the descriptor here, but not on every
    // arrival: each arm of a join formed its own loan.
    let mut joined = Vec::new();
    for (identity, lifetime) in candidates {
        // An input a provider-returned view keeps borrowed is retained for
        // the result's use, but the provider lends the bytes: that
        // lifetime authorizes no carrier anywhere and is never a view's
        // immediate loan occurrence.
        if lifetime
            .authorized
            .values()
            .all(std::collections::BTreeSet::is_empty)
        {
            continue;
        }
        let authorized = lifetime
            .authorized
            .get(&at)
            .is_some_and(|names| names.contains(descriptor));
        if lifetime
            .surviving
            .get(&at)
            .is_some_and(|names| names.contains(descriptor))
            && !authorized
        {
            if !lifetime.live.contains(&at) {
                return Err(
                    "reference descriptor has an unresolved surviving immediate loan occurrence",
                );
            }
            joined.push((identity, lifetime));
            continue;
        }
        if !authorized {
            continue;
        }
        if let Some(first) = parent {
            if alternatives.is_empty() {
                alternatives.push(first);
            }
            alternatives.push((identity, lifetime));
        } else {
            parent = Some((identity, lifetime));
        }
    }
    if !alternatives.is_empty() {
        if joined.is_empty()
            && let Some(choice) = crate::descriptor_lifetimes::DescriptorLoanChoice::returned(
                (at, descriptor.clone()),
                &alternatives,
            )
        {
            return Ok(BorrowedLoanSources::Selected(choice));
        }
        return Err("reference descriptor has ambiguous immediate loan occurrences");
    }
    let parent = parent.map(|(id, _)| id);
    if !joined.is_empty() {
        const UNRESOLVED: &str =
            "reference descriptor has an unresolved surviving immediate loan occurrence";
        if parent.is_some() {
            return Err(UNRESOLVED);
        }
        let unresolved = match joined_incoming_loans(module, machine, analysis, at, descriptor) {
            Ok(source) => return Ok(BorrowedLoanSources::Single(source)),
            Err(reason) => reason,
        };
        let flow = analysis.flow_arc(machine.id).map_err(|_| unresolved)?;
        let selected = |incoming| {
            crate::descriptor_lifetimes::joined_loan_authority(
                &flow,
                (at, descriptor.clone()),
                &joined,
                incoming,
            )
        };
        if let Some(choice) = selected(None) {
            return Ok(BorrowedLoanSources::Selected(choice));
        }
        // The first arrival at a loop header may carry an input directly;
        // later arrivals can carry the specific view formed by the loop.
        // Trace both permissions to their actual formation on each edge.
        let [(previous_id, previous)] = joined.as_slice() else {
            return Err(unresolved);
        };
        if !previous.authorizes_after_formation(at, descriptor) {
            return Err(unresolved);
        }
        let origins = analysis
            .symbolic_source_origins(machine.id, at, descriptor)
            .ok_or(unresolved)?;
        let original =
            incoming_view_occurrence(module, machine, &origins).map_err(|_| unresolved)?;
        let incoming = crate::descriptor_lifetimes::descriptor_entry_lifetime(
            &flow,
            (machine.entry, 0),
            &original,
        );
        return selected(Some((&original, &incoming)))
            .filter(|choice| {
                choice.includes_incoming() && choice.loans().any(|loan| loan == *previous_id)
            })
            .map(BorrowedLoanSources::Selected)
            .ok_or(unresolved);
    }
    if let Some(parent) = parent {
        return Ok(BorrowedLoanSources::Single(BorrowedLoanSource::Loan(
            parent,
        )));
    }
    // A borrowed block header can continue a borrowed machine input
    // without creating a new loan occurrence. Read its original storage
    // from the same arrival replay used for ordinary reference paths.
    let view_shape = machine
        .structural_place_type(descriptor.root)
        .and_then(|ty| borrowed_descriptor_shape(module, ty, &descriptor.path));
    let view = view_shape.is_some();
    // Every immutable literal or empty arrival supplies shared permission.
    // Their identities may differ, but no arrival may introduce mutable,
    // local, provider or unresolved backing into this owner-backed case.
    if view_shape.as_ref().is_some_and(|shape| {
        shape.is_shared_readable_byte_view()
            || matches!(
                shape,
                StructuralTypeShape::ElementView {
                    access: None | Some(StructuralAccess::SharedBorrow),
                    ..
                }
            )
    }) && analysis
        .source_origins(machine.id, at, descriptor)
        .is_some_and(|origins| {
            !origins.is_empty()
                && origins.iter().all(|origin| match origin {
                    crate::descriptor_results::DescriptorCallOrigin::Empty => true,
                    crate::descriptor_results::DescriptorCallOrigin::Literal {
                        machine: owner,
                        place,
                    } => module
                        .machine(*owner)
                        .and_then(|machine| machine.structural_place(*place))
                        .is_some_and(|place| {
                            matches!(place.kind,
                                semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                                    structural_type, ..
                                } if module.structural_type(structural_type)
                                    .is_some_and(|ty| ty.shape.is_shared_readable_byte_view()))
                        }),
                    _ => false,
                })
        })
    {
        return Ok(BorrowedLoanSources::Single(BorrowedLoanSource::Owner));
    }
    let source_origins = analysis.symbolic_source_origins(machine.id, at, descriptor);
    let provider = source_origins.as_ref().is_some_and(|origins| {
        !origins.is_empty()
            && origins.iter().all(|origin| {
                matches!(
                    origin,
                    crate::descriptor_results::DescriptorSourceOrigin::Provider(_)
                )
            })
    });
    // An incoming storage argument can keep different original inputs
    // on different arrivals. Authenticate every address without choosing
    // one of those inputs as the parent of a new loan.
    'incoming_storage: {
        // A provider lends its payload independently of the variable that
        // holds its view. Its invocation is checked below, not as an input address.
        if provider {
            break 'incoming_storage;
        }
        let Some(arrival) = crate::arrival_parameter(machine, descriptor.root)
            .filter(|arrival| arrival.access != StructuralAccess::Owned)
        else {
            break 'incoming_storage;
        };
        let family = analysis.captured_storage_family(machine.id, at, descriptor)?;
        for origin in &family {
            let crate::descriptor_results::DescriptorCallOrigin::Argument { source, projection } =
                origin
            else {
                break 'incoming_storage;
            };
            let Some(parameter) = crate::exactly_one::exactly_one(
                machine.structural_parameters.iter(),
                |parameter| parameter.place == *source,
            )
            .filter(|parameter| parameter.access != StructuralAccess::Owned) else {
                break 'incoming_storage;
            };
            let path = projection
                .captured_storage_path()
                .ok_or("borrowed successor has no exact captured original input path")?;
            if !crate::projection_stays_in_inline_storage(
                module.types.structural_types.iter(),
                parameter.structural_type,
                &path,
            ) {
                break 'incoming_storage;
            }
            let current_path = crate::descriptor_lifetimes::descriptor_path(
                &module.types.structural_types,
                arrival.structural_type,
                &descriptor.path,
            )
            .ok_or("borrowed successor has no exact current referent path")?;
            let original = crate::runtime_structural_path_tip(
                module.types.structural_types.iter(),
                parameter.structural_type,
                &path,
            )
            .ok_or("borrowed successor has no exact original referent type")?;
            let current = crate::runtime_structural_path_tip(
                module.types.structural_types.iter(),
                arrival.structural_type,
                &current_path,
            )
            .ok_or("borrowed successor has no exact current referent type")?;
            if original.id != current.id {
                return Err("borrowed successor changes its original incoming referent type");
            }
            if !crate::reference_source_projection_accessible(
                module.types.structural_types.iter(),
                parameter.structural_type,
                parameter.multiplicity,
                parameter.access,
                &path,
                arrival.access,
            ) {
                return Err("borrowed successor exceeds its original incoming access");
            }
        }
        return Ok(BorrowedLoanSources::Incoming(family));
    }
    let origins = source_origins
        .ok_or("reference descriptor has no complete original loan correspondence")?;
    if !origins.is_empty()
        && origins.iter().all(|origin| {
            matches!(
                origin,
                crate::descriptor_results::DescriptorSourceOrigin::Provider(_)
            )
        })
    {
        if origins.iter().any(|origin| {
            matches!(origin,
            crate::descriptor_results::DescriptorSourceOrigin::Provider(provider)
                if provider.invocations.is_empty())
        }) {
            return Err("provider view has no establishing invocation");
        }
        if origins.iter().any(|origin| {
            let crate::descriptor_results::DescriptorSourceOrigin::Provider(provider) = origin
            else {
                return false;
            };
            provider.projection.indirect
                || !provider
                    .projection
                    .original_descriptor_path()
                    .is_some_and(|path| path.is_empty())
        }) {
            return Err("a view inside provider storage needs its own original borrow");
        }
        let root = machine
            .structural_place_type(descriptor.root)
            .ok_or("provider borrow has no exact descriptor type")?;
        let mut path = crate::descriptor_lifetimes::descriptor_path(
            &module.types.structural_types,
            root,
            &descriptor.path,
        )
        .ok_or("provider borrow has no exact descriptor path")?;
        let reference_access = crate::runtime_structural_path_tip(
            module.types.structural_types.iter(),
            root,
            &path,
        )
        .and_then(|row| match row.shape {
            StructuralTypeShape::Reference {
                access: access @ (StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow),
                ..
            } => Some(access),
            _ => None,
        });
        if reference_access.is_some() {
            path.push(StructuralPathSegment::Referent);
        }
        let access = reference_access.unwrap_or(StructuralAccess::SharedBorrow);
        if !(view || reference_access.is_some())
            || crate::structural_projection_authority(
                machine,
                &module.types.structural_types,
                &terminal_psi::StructuralArgument {
                    place: descriptor.root,
                    path,
                    access,
                },
            ) != Some(access)
        {
            return Err("a writable provider borrow needs its exact payload region");
        }
        // A mutable reference can lend its provider's storage when every possible
        // result grants exclusive access. A writable input debt could be the
        // original storage; that case still needs a captured write footprint.
        if access == StructuralAccess::MutableBorrow
            && !origins.iter().all(|origin| {
                use crate::descriptor_results::{DescriptorProviderOutput, DescriptorSourceOrigin};
                let DescriptorSourceOrigin::Provider(provider) = origin else {
                    return false;
                };
                let DescriptorProviderOutput::Result(path) = &provider.output else {
                    return false;
                };
                let Some(boundary) = module.boundary_machine(provider.boundary) else {
                    return false;
                };
                let terminal_psi::BoundaryMachineResult::Structural(result) = &boundary.result
                else {
                    return false;
                };
                provider
                    .lifetime_dependencies
                    .iter()
                    .all(|debt| debt.access == StructuralAccess::SharedBorrow)
                    && crate::runtime_structural_path_tip(
                        module.types.structural_types.iter(),
                        result.structural_type,
                        path,
                    )
                    .is_some_and(|row| {
                        matches!(
                            row.shape,
                            StructuralTypeShape::Reference {
                                access: StructuralAccess::MutableBorrow,
                                ..
                            }
                        )
                    })
            })
        {
            return Err("a writable provider borrow needs its exact payload region");
        }
        return Ok(BorrowedLoanSources::Single(BorrowedLoanSource::Provider));
    }
    // A fixed-array reference to an incoming view's whole window borrows
    // that view's own payload, as the view does.
    if view || incoming_view_occurrence(module, machine, &origins).is_ok() {
        incoming_view_occurrence(module, machine, &origins)?;
        return Ok(BorrowedLoanSources::Single(BorrowedLoanSource::Incoming));
    }
    let [origin] = origins.as_slice() else {
        return Err("reference descriptor has no single immediate incoming occurrence");
    };
    let original = origin
        .captured_entry_slot()
        .ok_or("reference descriptor has no authenticated formation or incoming loan")?;
    let parameter = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == original.root)
        .ok_or("incoming reference occurrence names no declared input")?;
    let path = crate::descriptor_lifetimes::descriptor_path(
        &module.types.structural_types,
        parameter.structural_type,
        &original.path,
    )
    .ok_or("incoming reference occurrence has no exact declared path")?;
    if !crate::projection_stays_in_inline_storage(
        module.types.structural_types.iter(),
        parameter.structural_type,
        &path,
    ) || !crate::runtime_structural_path_tip(
        module.types.structural_types.iter(),
        parameter.structural_type,
        &path,
    )
    .is_some_and(|row| {
        matches!(row.shape, StructuralTypeShape::Reference { .. })
            || view
                && crate::descriptor_results::is_descriptor(
                    module.types.structural_types.iter(),
                    row.id,
                )
    }) {
        return Err("incoming reference parent is not an original contained reference leaf");
    }
    // None denotes this verified caller-owned resource, not a failed
    // lookup. No local Loan identity is invented for an incoming leaf.
    Ok(BorrowedLoanSources::Single(BorrowedLoanSource::Incoming))
}

pub fn reference_loan_parent<'a>(
    module: &terminal_psi::TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    analysis: &mut crate::descriptor_results::DescriptorAnalysis<'_>,
    at: crate::descriptor_lifetimes::DescriptorPosition,
    descriptor: &crate::descriptor_lifetimes::DescriptorLocation,
    candidates: impl Iterator<
        Item = (
            semantic_vocabulary::ObligationId,
            &'a crate::descriptor_lifetimes::DescriptorLifetime,
        ),
    >,
) -> Result<Option<semantic_vocabulary::ObligationId>, &'static str> {
    match borrowed_loan_source(module, machine, analysis, at, descriptor, candidates)? {
        BorrowedLoanSource::Loan(parent) => Ok(Some(parent)),
        // Both resources were authenticated above and neither was formed by a
        // local loan obligation. Provider keepalive dependencies have their own ends.
        BorrowedLoanSource::Incoming | BorrowedLoanSource::Provider => Ok(None),
        BorrowedLoanSource::Owner => {
            Err("owned view backing is not an incoming reference occurrence")
        }
    }
}

/// The storage one call input keeps borrowed after the call returns, and
/// for how long. `path` is the caller's storage path under the input's
/// place: the argument's own path, or the longer exact source a returned
/// reference's published origin binds (`grid.cells` for `pick(@mutable grid,
/// i)` returning `@mutable grid.cells[i].x`). The argument's own loan still
/// covers its whole place for the call; this narrower region is what
/// lifetimes.md, "Returned views", requires to "remain live for the
/// result's use".
pub struct RetainedCallLoan {
    pub position: usize,
    pub path: Vec<StructuralPathSegment>,
    pub lifetime: crate::descriptor_lifetimes::DescriptorLifetime,
}

/// The exact retained region for a bound source beneath `argument`.
/// A loan row captures fields, cases and fixed indices exactly; a
/// runtime selector chosen inside the callee, a referent crossing or a
/// representation step stops the region at the storage enclosing it.
/// Stopping early only widens the retained region, never narrows it.
fn retained_call_path(
    argument: &terminal_psi::StructuralArgument,
    source: &[StructuralPathSegment],
) -> Vec<StructuralPathSegment> {
    let exact = source[argument.path.len()..]
        .iter()
        .take_while(|segment| {
            matches!(
                segment,
                StructuralPathSegment::Field(_)
                    | StructuralPathSegment::Case(_)
                    | StructuralPathSegment::FixedIndex(_)
            )
        })
        .count();
    source[..argument.path.len() + exact].to_vec()
}

fn retain_call_lifetime(
    retained: &mut Vec<RetainedCallLoan>,
    position: usize,
    path: Vec<StructuralPathSegment>,
    lifetime: crate::descriptor_lifetimes::DescriptorLifetime,
) {
    if let Some(previous) = retained
        .iter_mut()
        .find(|previous| previous.position == position && previous.path == path)
    {
        let previous = &mut previous.lifetime;
        if previous.returned_origin != lifetime.returned_origin {
            previous.returned_origin = None;
        }
        previous.live.extend(lifetime.live);
        for (at, names) in lifetime.demanded {
            previous.demanded.entry(at).or_default().extend(names);
        }
        for (at, names) in lifetime.surviving {
            previous.surviving.entry(at).or_default().extend(names);
        }
        for (at, names) in lifetime.authorized {
            previous.authorized.entry(at).or_default().extend(names);
        }
    } else {
        retained.push(RetainedCallLoan {
            position,
            path,
            lifetime,
        });
    }
}

#[derive(Clone, Copy)]
pub struct CallLifetimeIndex<'a> {
    pub predecessors: &'a crate::descriptor_lifetimes::FlowPredecessors<'a>,
    pub cutoff: Option<&'a crate::descriptor_lifetimes::DeadTailCutoff>,
    pub at: crate::descriptor_lifetimes::DescriptorPosition,
    pub include_surviving: bool,
}

fn indexed_descriptor_lifetime(
    flow: &std::collections::BTreeMap<
        crate::descriptor_lifetimes::DescriptorPosition,
        crate::descriptor_lifetimes::DescriptorFlowStep,
    >,
    at: crate::descriptor_lifetimes::DescriptorPosition,
    descriptor: &crate::descriptor_lifetimes::DescriptorLocation,
    index: Option<CallLifetimeIndex<'_>>,
) -> crate::descriptor_lifetimes::DescriptorLifetime {
    match index {
        Some(index) if index.include_surviving => {
            crate::descriptor_lifetimes::descriptor_location_lifetime_indexed(
                flow,
                index.predecessors,
                index.cutoff,
                at,
                descriptor,
            )
        }
        Some(index) => {
            crate::descriptor_lifetimes::descriptor_location_lifetime_with_contents_indexed(
                flow,
                index.predecessors,
                index.cutoff,
                at,
                descriptor,
                |_, _| false,
            )
        }
        None => crate::descriptor_lifetimes::descriptor_location_lifetime(flow, at, descriptor),
    }
}

#[allow(clippy::too_many_arguments)]
fn retain_provider_inputs(
    module: &terminal_psi::TerminalModule,
    result: &terminal_psi::StructuralOperationResult,
    parameters: &[terminal_psi::StructuralParameterDeclaration],
    arguments: &[terminal_psi::StructuralArgument],
    slots: &[crate::descriptor_results::DescriptorResultSlot],
    at: crate::descriptor_lifetimes::DescriptorPosition,
    flow: &std::collections::BTreeMap<
        crate::descriptor_lifetimes::DescriptorPosition,
        crate::descriptor_lifetimes::DescriptorFlowStep,
    >,
    index: Option<CallLifetimeIndex<'_>>,
    retained: &mut Vec<RetainedCallLoan>,
) -> Result<(), &'static str> {
    use crate::descriptor_results::DescriptorStoredOrigin;
    for slot in slots {
        slot.visit(
            result.structural_type,
            &mut Vec::new(),
            &mut |path, containing_type, slot| {
                let declared = crate::descriptor_lifetimes::descriptor_location_type(
                    &module.types.structural_types,
                    containing_type,
                    &slot.path,
                )
                .and_then(|ty| module.structural_type(ty));
                if !declared.is_some_and(|row| row.shape.is_borrowed_descriptor())
                    && borrowed_descriptor_shape(module, containing_type, &slot.path).is_none()
                {
                    return Ok(());
                }
                let descriptor = crate::descriptor_lifetimes::DescriptorLocation {
                    root: result.place,
                    path: path.to_vec(),
                };
                let mut pending = Vec::new();
                for origin in &slot.origins {
                    if let DescriptorStoredOrigin::Provider(provider) = origin {
                        pending.extend(
                            provider
                                .lifetime_dependencies
                                .iter()
                                .map(|debt| &debt.origin),
                        );
                    }
                }
                if pending.is_empty() {
                    return Ok(());
                }
                let lifetime = indexed_descriptor_lifetime(flow, at, &descriptor, index);
                if lifetime.live.contains(&at) {
                    return Err("returned provider dependency crosses its own call formation");
                }
                retain_result_dependencies(
                    module, parameters, arguments, pending, &lifetime, retained,
                )?;
                Ok::<(), &'static str>(())
            },
        )?;
    }
    Ok(())
}

fn retain_result_dependencies(
    module: &terminal_psi::TerminalModule,
    parameters: &[terminal_psi::StructuralParameterDeclaration],
    arguments: &[terminal_psi::StructuralArgument],
    mut pending: Vec<&crate::descriptor_results::DescriptorStoredOrigin>,
    lifetime: &crate::descriptor_lifetimes::DescriptorLifetime,
    retained: &mut Vec<RetainedCallLoan>,
) -> Result<(), &'static str> {
    use crate::descriptor_results::DescriptorStoredOrigin;
    if parameters.len() != arguments.len() {
        return Err("returned value differs from its input contract");
    }
    // Keeping an input live grants no authority to use its loan.
    let mut inputs = Vec::new();
    while let Some(origin) = pending.pop() {
        let (position, stored) = match origin {
            DescriptorStoredOrigin::Provider(provider) => {
                pending.extend(
                    provider
                        .lifetime_dependencies
                        .iter()
                        .map(|debt| &debt.origin),
                );
                continue;
            }
            DescriptorStoredOrigin::ProducedContents(contents) => {
                pending.extend(
                    contents
                        .lifetime_dependencies
                        .iter()
                        .map(|debt| &debt.origin),
                );
                continue;
            }
            DescriptorStoredOrigin::Parameter { position, .. } => (*position, false),
            DescriptorStoredOrigin::IncomingSlot { position, .. }
            | DescriptorStoredOrigin::IncomingContents { position, .. } => (*position, true),
            DescriptorStoredOrigin::Literal { .. } | DescriptorStoredOrigin::Empty => {
                continue;
            }
            DescriptorStoredOrigin::Invocation { .. }
            | DescriptorStoredOrigin::AccessFormation { .. }
            | DescriptorStoredOrigin::Carried { .. } => {
                return Err("returned value dependency needs its admitted retention resource");
            }
        };
        let formal = parameters
            .get(position)
            .ok_or("returned value dependency names no input formal")?;
        let argument = arguments
            .get(position)
            .ok_or("returned value dependency names no actual argument")?;
        if argument.access != formal.access {
            return Err("returned value dependency changes its input access");
        }
        if argument.access == StructuralAccess::Owned {
            // A moved descriptor carries its original permission;
            // the call did not form a new loan of its storage.
            continue;
        }
        let formal_type = module
            .structural_type(formal.structural_type)
            .ok_or("returned value dependency has no typed input formal")?;
        if formal_type.shape.is_view_descriptor() {
            // The view carries its original payload loan through
            // descriptor flow; this is not a loan of its storage.
            continue;
        }
        if stored
            && !crate::descriptor_results::is_descriptor(
                module.types.structural_types.iter(),
                formal.structural_type,
            )
        {
            // A reference held inside an input retains its own
            // original loan through descriptor flow, not a new
            // borrow of the enclosing record's storage.
            continue;
        }
        if !inputs.contains(&position) {
            inputs.push(position);
        }
    }
    for position in inputs {
        let argument = arguments
            .get(position)
            .ok_or("returned value dependency names no actual argument")?;
        retain_call_lifetime(
            retained,
            position,
            argument.path.clone(),
            crate::descriptor_lifetimes::DescriptorLifetime {
                returned_origin: None,
                recurring_authority: None,
                live: lifetime.live.clone(),
                demanded: lifetime.demanded.clone(),
                surviving: lifetime.surviving.clone(),
                authorized: Default::default(),
            },
        );
    }
    Ok(())
}

/// A reference or view returned by a call keeps its input loan until
/// every transported result descriptor ends (lifetimes.md, "Returned
/// views": the source must "remain live for the result's use").
/// Reconstruct the callee's actual origins before using the existing call
/// binding; a result header alone cannot extend a loan or supply an
/// unrelated backing resource.
pub fn reference_call_lifetimes(
    module: &terminal_psi::TerminalModule,
    caller: &terminal_psi::TerminalMachine,
    operation: &terminal_psi::Operation,
    analysis: &mut crate::descriptor_results::DescriptorAnalysis<'_>,
) -> Result<Vec<RetainedCallLoan>, crate::descriptor_results::DescriptorGraphError> {
    reference_call_lifetimes_with_index(module, caller, operation, analysis, None)
}

pub fn reference_call_lifetimes_indexed(
    module: &terminal_psi::TerminalModule,
    caller: &terminal_psi::TerminalMachine,
    operation: &terminal_psi::Operation,
    analysis: &mut crate::descriptor_results::DescriptorAnalysis<'_>,
    index: CallLifetimeIndex<'_>,
) -> Result<Vec<RetainedCallLoan>, crate::descriptor_results::DescriptorGraphError> {
    reference_call_lifetimes_with_index(module, caller, operation, analysis, Some(index))
}

fn reference_call_lifetimes_with_index(
    module: &terminal_psi::TerminalModule,
    caller: &terminal_psi::TerminalMachine,
    operation: &terminal_psi::Operation,
    analysis: &mut crate::descriptor_results::DescriptorAnalysis<'_>,
    index: Option<CallLifetimeIndex<'_>>,
) -> Result<Vec<RetainedCallLoan>, crate::descriptor_results::DescriptorGraphError> {
    let refuse = |detail| {
        crate::descriptor_results::DescriptorGraphError::at_operation(
            module, caller.id, operation, detail,
        )
    };
    use super::reference_origins::{
        ReferenceCallContract, ReferenceCallOrigin, bind_reference_call_results,
    };
    use crate::descriptor_lifetimes::{DescriptorLocation, descriptor_location, descriptor_path};

    let invocation = operation.declared_invocation();
    let runtime = operation.kind.callable_invocation();
    if invocation.is_none()
        && runtime.is_none()
        && !matches!(
            operation.kind,
            terminal_psi::OperationKind::CallDynamic { .. }
        )
    {
        return Ok(Vec::new());
    }
    let Some(result) = operation.result.as_structural().filter(|result| {
        terminal_psi::structural_type_has_borrowed_storage_by(
            |id| module.structural_type(id),
            result.structural_type,
        )
    }) else {
        return Ok(Vec::new());
    };
    let has_reference_contents = terminal_psi::structural_type_contains_reference_by(
        |id| module.structural_type(id),
        result.structural_type,
    );
    let result_inventory = crate::reference_inventory(
        module.types.structural_types.iter(),
        result.structural_type,
        usize::MAX,
    )
    .ok_or_else(|| refuse("returned contents have no complete reference inventory"))?;
    // An opaque value packet carries its existing loans. It does not
    // form a new reference to the storage that previously held the value.
    let returns_reference = !result_inventory.references.is_empty();
    let arguments = operation.kind.structural_arguments();
    let (declared_parameters, slots, actual, public) = if let Some(call) = runtime {
        let contract = module
            .execution
            .callables
            .contract(call.contract)
            .ok_or_else(|| refuse("returned runtime value has no exact public contract"))?;
        let declared = super::callable_reference_result(&module.types.structural_types, contract)
            .map_err(refuse)?;
        if !contract.result.matches_machine_result(
            &terminal_psi::TerminalMachineResult::Structural(
                terminal_psi::StructuralResultDeclaration {
                    place: result.place,
                    structural_type: result.structural_type,
                    multiplicity: result.multiplicity,
                    qualifications: result.qualifications.clone(),
                    projected_qualifications: result.projected_qualifications.clone(),
                    reference_sources: Vec::new(),
                },
            ),
        ) {
            return Err(refuse(
                "returned runtime value changes its public result carrier",
            ));
        }
        let slots = crate::descriptor_results::public_result_slots(
            &module.types.structural_types,
            crate::descriptor_results::DescriptorSlotKind::Views,
            &declared,
            result.place,
        )
        .ok_or_else(|| refuse("returned runtime views have no complete declared frontier"))?;
        (
            contract.structural_parameters.as_slice(),
            slots,
            Vec::new(),
            Some(declared),
        )
    } else {
        let Some(call) = invocation else {
            return Err(refuse(if returns_reference {
                "returned dynamic reference needs its admitted completion resource"
            } else {
                "returned dynamic view needs its declared lifetime contract"
            }));
        };
        match call.callee {
            terminal_psi::TargetMachineCallable::Boundary(boundary) => {
                let declaration = module
                    .boundary_machine(boundary)
                    .ok_or_else(|| refuse("returned boundary borrow has no exact declaration"))?;
                let terminal_psi::BoundaryMachineResult::Structural(expected) = &declaration.result
                else {
                    return Err(refuse("returned borrow has no structural boundary result"));
                };
                if result.qualifications != expected.qualifications
                    || result.projected_qualifications != expected.projected_qualifications
                {
                    return Err(refuse(
                        "returned borrow differs from its boundary result contract",
                    ));
                }
                let slots = crate::boundary_result_slots(
                    &module.types.structural_types,
                    declaration,
                    crate::descriptor_results::DescriptorSlotKind::BorrowedStorage,
                    result,
                )
                .map_err(refuse)?;
                (
                    declaration.structural_parameters.as_slice(),
                    slots,
                    Vec::new(),
                    None,
                )
            }
            terminal_psi::TargetMachineCallable::Machine(callee) => {
                let callee = module
                    .machine(callee)
                    .ok_or_else(|| refuse("returned reference call has no actual callee body"))?;
                let signature = callee.result.structural().ok_or_else(|| {
                    refuse("returned reference call has no structural result contract")
                })?;
                if result.structural_type != signature.structural_type
                    || result.multiplicity != signature.multiplicity
                {
                    return Err(refuse(
                        "returned reference call changes its declared result carrier",
                    ));
                }
                // A rigid call reaches a bodiless requirement: its declared
                // lifetimes are the only origins, and no body can narrow them.
                // It binds through the same public account a selected call reads.
                if let Some(requirement) =
                    super::requirement_reference_result(module, callee.id).map_err(refuse)?
                {
                    (
                        callee.structural_parameters.as_slice(),
                        Vec::new(),
                        Vec::new(),
                        Some(requirement),
                    )
                } else {
                    let contents = analysis.result_contents(callee.id).ok_or_else(|| {
                    // A dependency can fail before its return sources are
                    // exported. Keep that flow's actual machine and site.
                    analysis.flow(callee.id).err().unwrap_or_else(|| {
                        refuse("returned reference callee has no complete actual source correspondence")
                    })
                })?;
                    let actual = if has_reference_contents {
                        super::reference_origins::reference_result_contents(
                            &module.types.structural_types,
                            &callee.structural_parameters,
                            signature,
                            &contents,
                        )
                        .map_err(refuse)?
                    } else {
                        Vec::new()
                    };
                    if actual != signature.reference_sources {
                        return Err(refuse(
                            "returned reference header differs from its actual body origins",
                        ));
                    }
                    let public = call
                        .required_contract
                        .filter(|_| returns_reference)
                        .map(|required| {
                            super::selected_reference_result(module, caller.id, callee.id, required)
                        })
                        .transpose()
                        .map_err(refuse)?;
                    if let Some(public) = &public {
                        public
                            .validate_actual_sources(&callee.structural_parameters, &actual)
                            .map_err(refuse)?;
                    }
                    (
                        callee.structural_parameters.as_slice(),
                        contents.slots,
                        actual,
                        public,
                    )
                }
            }
        }
    };
    let parameters = public.as_ref().map_or(declared_parameters, |public| {
        public.contract().structural_parameters.as_slice()
    });
    // An admitted boundary establishes its own result provenance. Its
    // logical input debts below retain loans without claiming that the
    // returned bytes lie inside an input's storage.
    let admitted_boundary = invocation.is_some_and(|call| {
        matches!(
            call.callee,
            terminal_psi::TargetMachineCallable::Boundary(_)
        )
    });
    let bindings = if !returns_reference || admitted_boundary {
        Vec::new()
    } else if let Some(public) = &public {
        super::reference_origins::bind_public_reference_call_results(
            module.types.structural_types.iter(),
            public,
            arguments,
        )
        .map_err(refuse)?
    } else {
        bind_reference_call_results(
            module.types.structural_types.iter(),
            ReferenceCallContract::Machine {
                parameters,
                sources: &actual,
            },
            arguments,
        )
        .map_err(refuse)?
    };
    let at = if let Some(at) = index.map(|i| i.at) {
        at
    } else {
        let mut positions = caller.blocks.iter().flat_map(|block| {
            block
                .operations
                .iter()
                .enumerate()
                .filter_map(move |(ordinal, candidate)| {
                    (candidate.id == operation.id && candidate == operation)
                        .then_some((block.id, ordinal))
                })
        });
        let (block, ordinal) = positions
            .next()
            .ok_or_else(|| refuse("returned reference has no exact caller operation"))?;
        if positions.next().is_some() {
            return Err(refuse(
                "returned reference has ambiguous caller operation coordinates",
            ));
        }
        (
            block,
            u32::try_from(ordinal).map_err(|_| refuse("call ordinal overflows"))?,
        )
    };
    let flow = analysis.flow_arc(caller.id)?;
    let compute_lifetime =
        |descriptor: &DescriptorLocation| indexed_descriptor_lifetime(&flow, at, descriptor, index);
    let mut retained = Vec::new();
    for (mapping_index, binding) in bindings.into_iter().enumerate() {
        let source_positions = if let Some(public) = &public {
            use terminal_psi::ReferenceResultShape;
            public
                .reference_regions(&binding.path)
                .map(|region| {
                    let region = region.map_err(refuse)?;
                    let terminal_psi::TerminalDynamicReferenceOrigin::Parameter { argument_index } =
                        region.origin
                    else {
                        return Err(refuse(
                            "selected reference input unexpectedly uses provider storage",
                        ));
                    };
                    usize::try_from(argument_index).map(Some).map_err(|_| {
                        refuse("selected reference input ordinal exceeds host positions")
                    })
                })
                .collect::<Result<Vec<_>, _>>()?
        } else {
            actual
                .get(mapping_index)
                .ok_or_else(|| refuse("returned reference binding lost its body mapping"))?
                .sources
                .iter()
                .map(|origin| {
                    origin
                        .parameter()
                        .map(|formal| {
                            terminal_psi::structural_parameter_position(parameters, formal.place)
                                .ok_or_else(|| refuse("returned reference names no formal input"))
                        })
                        .transpose()
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        if source_positions.len() != binding.sources.len() {
            return Err(refuse(
                "returned reference binding differs from its exact source ordinals",
            ));
        }
        let descriptor = descriptor_location(
            &module.types.structural_types,
            result.place,
            result.structural_type,
            &binding.path,
            None,
        )
        .ok_or_else(|| refuse("returned reference has no exact result descriptor path"))?;
        let alternatives = binding.sources.len();
        let input_choice = alternatives > 1
            && binding.sources.iter().all(|source| {
                matches!(
                    source.origin,
                    ReferenceCallOrigin::Parameter {
                        transfers_existing: false,
                        ..
                    }
                )
            });
        for (ordinal, (position, bound)) in source_positions
            .into_iter()
            .zip(binding.sources)
            .enumerate()
        {
            let (position, source, transfers_existing) = match (position, bound.origin) {
                (
                    Some(position),
                    ReferenceCallOrigin::Parameter {
                        source,
                        transfers_existing,
                    },
                ) => (position, source, transfers_existing),
                // The producing call establishes its own address. Its reconstructed
                // provider debts below still retain every required input loan.
                (None, ReferenceCallOrigin::Invocation { .. }) => continue,
                _ => return Err(refuse("returned reference binding changes its origin")),
            };
            if transfers_existing {
                // The existing descriptor flow transports that input
                // occurrence. It does not form a new loan of its backing.
                continue;
            }
            let argument = arguments
                .get(position)
                .ok_or_else(|| refuse("returned reference input has no actual argument"))?;
            if argument.access == StructuralAccess::Owned
                || source.place != argument.place
                || !source.path.starts_with(&argument.path)
            {
                return Err(refuse(
                    "returned reference does not retain an actual input loan",
                ));
            }
            let mut lifetime = compute_lifetime(&descriptor);
            if input_choice {
                lifetime.returned_origin =
                    Some(Box::new(crate::descriptor_lifetimes::ReturnedLoanOrigin {
                        leaf: (at, descriptor.clone()),
                        alternatives,
                        source: ordinal,
                    }));
            }
            if lifetime.live.contains(&at) {
                return Err(refuse(
                    "returned reference loan crosses its own call formation",
                ));
            }
            retain_call_lifetime(
                &mut retained,
                position,
                retained_call_path(argument, &source.path),
                lifetime,
            );
        }
    }
    // A view leaf uses actual body slots for a direct call and the
    // declared input union for a runtime callable. A view into a borrowed formal's
    // own storage keeps the argument's loan for the view's use. One that
    // reaches through a reference held by the input points into that
    // reference's original backing, whose own loan the descriptor flow
    // already transports; it is not a borrow of the caller's carrier.
    let is_view = |shape: &StructuralTypeShape| {
        matches!(
            shape,
            StructuralTypeShape::ElementView { .. }
                | StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                        | terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                )
        )
    };
    if !result_inventory.unknown_contents.is_empty() {
        let packets = analysis
            .call_result_contents(caller.id, at, operation.id)
            .ok_or_else(|| refuse("generic call result has no checked contents account"))?;
        for packet in packets {
            let dependencies = packet
                .origins
                .iter()
                .filter_map(|origin| match origin {
                    crate::descriptor_results::DescriptorStoredOrigin::ProducedContents(
                        contents,
                    ) => Some(&contents.lifetime_dependencies),
                    _ => None,
                })
                .flatten()
                .map(|debt| &debt.origin)
                .collect::<Vec<_>>();
            if dependencies.is_empty() {
                continue;
            }
            let location = DescriptorLocation {
                root: result.place,
                path: packet.path,
            };
            let lifetime = compute_lifetime(&location);
            if lifetime.live.contains(&at) {
                return Err(refuse("returned contents cross their own call formation"));
            }
            retain_result_dependencies(
                module,
                parameters,
                arguments,
                dependencies,
                &lifetime,
                &mut retained,
            )
            .map_err(refuse)?;
        }
    }
    for slot in &slots {
        slot.visit(
            result.structural_type,
            &mut Vec::new(),
            &mut |path, containing_type, slot| {
                descriptor_path(&module.types.structural_types, containing_type, &slot.path)
                    .ok_or_else(|| refuse("returned view slot has no typed result leaf"))?;
                if borrowed_descriptor_shape(module, containing_type, &slot.path).is_none() {
                    return Ok(());
                }
                let descriptor = DescriptorLocation {
                    root: result.place,
                    path: path.to_vec(),
                };
                for origin in &slot.origins {
                    let crate::descriptor_results::DescriptorStoredOrigin::Parameter {
                        position,
                        projection,
                    } = origin
                    else {
                        continue;
                    };
                    // A formal that is itself a view hands back the descriptor
                    // it was given, or a window of it: that view's backing is
                    // the original one, not the argument's storage.
                    let formal = parameters
                        .get(*position)
                        .and_then(|formal| module.structural_type(formal.structural_type))
                        .ok_or_else(|| refuse("returned view input has no typed formal"))?;
                    if is_view(&formal.shape)
                        || projection
                            .path
                            .contains(&terminal_psi::StructuralPathSegment::Referent)
                    {
                        continue;
                    }
                    let argument = arguments
                        .get(*position)
                        .ok_or_else(|| refuse("returned view input has no actual argument"))?;
                    if argument.access == StructuralAccess::Owned {
                        // An owned input moves into the callee, so a view it
                        // returns is a descriptor the input carried, as when a
                        // view passed by value comes back. The descriptor flow
                        // transports that view's own loan; the call lends none.
                        continue;
                    }
                    let lifetime = compute_lifetime(&descriptor);
                    if lifetime.live.contains(&at) {
                        return Err(refuse("returned view loan crosses its own call formation"));
                    }
                    retain_call_lifetime(&mut retained, *position, argument.path.clone(), lifetime);
                }
                Ok::<(), crate::descriptor_results::DescriptorGraphError>(())
            },
        )?;
    }
    retain_provider_inputs(
        module,
        result,
        parameters,
        arguments,
        &slots,
        at,
        &flow,
        index,
        &mut retained,
    )
    .map_err(refuse)?;
    if retained
        .iter()
        .any(|loan| loan.position >= parameters.len())
    {
        return Err(refuse("returned borrow dependency has no input position"));
    }
    // Consumers walk the call's arguments in order: each input's own
    // region first, then its narrower ones in path order.
    retained.sort_by(|left, right| {
        (left.position, left.path.len(), &left.path).cmp(&(
            right.position,
            right.path.len(),
            &right.path,
        ))
    });
    Ok(retained)
}

#[cfg(test)]
#[test]
fn owned_cleanup_disposes_only_the_transferred_descriptor_subtree() {
    let carrier = PlaceId::new(1).unwrap();
    let field = StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(1));
    let actions = [TerminalAffineCleanupAction::InvokeOwned(
        terminal_psi::OwnedCleanupInvocation {
            subject: terminal_psi::StructuralArgument {
                place: carrier,
                path: vec![field.clone()],
                access: StructuralAccess::Owned,
            },
            structural_type: StructuralTypeId::new(1).unwrap(),
            cleanup_machine: semantic_vocabulary::MachineId::new(1).unwrap(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
        },
    )];
    assert!(cleanup_disposes_reference(
        &actions,
        carrier,
        std::slice::from_ref(&field)
    ));
    assert!(!cleanup_disposes_reference(&actions, carrier, &[]));
    assert!(!cleanup_disposes_reference(
        &actions,
        PlaceId::new(2).unwrap(),
        std::slice::from_ref(&field)
    ));
    assert!(!cleanup_disposes_reference(
        &actions,
        carrier,
        &[field, StructuralPathSegment::Referent]
    ));
}

pub(crate) fn cleanup_disposes_reference(
    actions: &[TerminalAffineCleanupAction],
    carrier: PlaceId,
    path: &[StructuralPathSegment],
) -> bool {
    if path.contains(&StructuralPathSegment::Referent) {
        return false;
    }
    actions.iter().any(|action| match action {
        TerminalAffineCleanupAction::DiscardRoot(place) => *place == carrier,
        TerminalAffineCleanupAction::DiscardResidual(discard) => {
            discard.place == carrier
                && path.starts_with(&discard.path)
                && !discard.path.contains(&StructuralPathSegment::Referent)
        }
        TerminalAffineCleanupAction::InvokeOwned(cleanup) => {
            cleanup.subject.access == StructuralAccess::Owned
                && cleanup.subject.place == carrier
                && path.starts_with(&cleanup.subject.path)
                && !cleanup
                    .subject
                    .path
                    .contains(&StructuralPathSegment::Referent)
        }
        TerminalAffineCleanupAction::RetireHome(_) => false,
    })
}
