//! Borrowed structural installation transported into abstract custody.
//! Stores retain the distinction between vacant restoration and intact
//! freely discardable replacement. Only restoration requires readable mutable
//! custody; replacement uses write-only access and independently checks that
//! the exact value carries no conserved or retained permission resources.
//!
//! `MoveStructuralField` transfers one declared subtree from its retained
//! storage root; `StoreStructuralField` installs an owned value of that type.
//! Borrowed backing opens restoration debt keyed by its canonical field path;
//! no operation, edge, or case dispatch may observe the absent subtree, and
//! every normal exit closes that debt. An owned home instead retains its
//! remaining complement and moves the selected subtree's existing claims.
//! Psi checking and emission establish those distinct dynamic invariants. This leg joins the static shape
//! retained in Terminal — root authority, declared field
//! type, and the exact result/value custody rows — and retains the complete
//! root declaration so no consumer reconstructs borrow authority from
//! physical ABI shape. The dynamic debt itself needs no replay here: the
//! spelled pairs retain the checked program's operation identities.
//! A first-class mutable record reference keeps its own producer/result row
//! and explicit leading `Referent`; only geometry uses the referent-relative
//! suffix. Owning that carrier never grants ownership of its referent.
//!
//! A root window (`field: None`, empty path) vacates and reseats the whole
//! referent of an exclusive loan: its root must be a mutable-borrowed
//! parameter, as the verifier admitted.

use abstract_operations::{
    AbstractOperation, StructuralScalarStoreDestination, StructuralWindowRoot,
};
use semantic_vocabulary::{PlaceId, StructuralFieldId, StructuralPlaceKind, StructuralTypeId};
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralAccess, StructuralArgument,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
};

use crate::lowering::LoweringError;

use super::structural_scalar_fields::{exact_parameter, owned_storage_destination};

pub(super) fn lower(
    operation: &Operation,
    machine: &TerminalMachine,
    structural_types: &[StructuralTypeDeclaration],
) -> Result<AbstractOperation, LoweringError> {
    match &operation.kind {
        OperationKind::MoveStructuralField {
            source,
            path,
            field,
        } => lower_move(operation, machine, structural_types, *source, path, *field),
        OperationKind::StoreStructuralField {
            disposition,
            destination,
            path,
            field,
            value,
        } => lower_store(
            operation,
            machine,
            structural_types,
            *destination,
            *disposition,
            path,
            *field,
            value,
        ),
        _ => unreachable!("borrowed-window router is exhaustive"),
    }
}

fn lower_move(
    operation: &Operation,
    machine: &TerminalMachine,
    structural_types: &[StructuralTypeDeclaration],
    source: PlaceId,
    path: &[StructuralPathSegment],
    field: Option<StructuralFieldId>,
) -> Result<AbstractOperation, LoweringError> {
    let invalid = |reason: &str| LoweringError::InvalidBorrowedStorageWindow {
        machine: machine.id,
        operation: operation.id,
        place: source,
        reason: reason.to_owned(),
    };
    let result = operation
        .result
        .structural()
        .ok_or_else(|| invalid("window extraction has no structural result"))?;
    // The moved subtree is a fresh operation result under exact-once custody:
    // its complete declared claims move with it, independently of the
    // original place's restoration debt. Memberships follow that exact
    // selected subtree, not the enclosing owner's predicate or sibling.
    if !matches!(
        machine.structural_place(result.place)
            .map(|place| place.kind),
        Some(StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        }) if producer == operation.id && structural_type == result.structural_type
    ) {
        return Err(invalid(
            "extracted result does not name its exact producer and carrier",
        ));
    }
    let source = borrowed_window_root(machine, source, StructuralAccess::MutableBorrow)
        .ok_or_else(|| {
            invalid(&format!(
                "extraction root has no admitted mutable storage: declaration={:?}, signature={:?}, block={:?}, block_claims={:?}",
                machine.structural_place(source),
                machine.structural_parameter(source),
                block_parameter(machine, source),
                machine.block_entry_claims.iter().filter(|claim| claim.input == source).collect::<Vec<_>>(),
            ))
        })?;
    let projection = declared_window_projection(
        structural_types,
        &source,
        path,
        field,
        StructuralAccess::MutableBorrow,
    )
    .ok_or_else(|| invalid("extraction path has no admitted structural window"))?;
    let signature = source_signature(machine, source.place())
        .ok_or_else(|| invalid("extraction root has no declared source signature"))?;
    let (qualifications, projected) = terminal_psi::StructuralPathQualification::select_restorable(
        signature.qualifications,
        signature.projected_qualifications,
        &projection.path,
    )
    .ok_or_else(|| invalid("extraction memberships have no exact restorable projection"))?;
    if projection.structural_type != result.structural_type {
        return Err(invalid(&format!(
            "extracted carrier {:?} differs from window carrier {:?}",
            result.structural_type, projection.structural_type,
        )));
    }
    if result.qualifications != qualifications {
        return Err(invalid(
            "extracted memberships differ from the declared window",
        ));
    }
    if result.projected_qualifications != projected {
        return Err(invalid(
            "extracted child memberships differ from the declared window",
        ));
    }
    if !result.qualification_establishments.is_empty() {
        return Err(invalid("extraction attempts to establish new memberships"));
    }
    if !root_window_loan(&source, path, field) {
        return Err(invalid(
            "extraction has no admitted child or whole-referent loan",
        ));
    }
    Ok(AbstractOperation::MoveStructuralField {
        psi_operation: operation.id,
        result: result.clone(),
        source,
        path: path.to_vec(),
        field,
    })
}

fn lower_store(
    operation: &Operation,
    machine: &TerminalMachine,
    structural_types: &[StructuralTypeDeclaration],
    destination: PlaceId,
    disposition: terminal_psi::StructuralStoreDisposition,
    path: &[StructuralPathSegment],
    field: Option<StructuralFieldId>,
    value: &StructuralArgument,
) -> Result<AbstractOperation, LoweringError> {
    let invalid = |reason: &str| LoweringError::InvalidBorrowedStorageWindow {
        machine: machine.id,
        operation: operation.id,
        place: destination,
        reason: reason.to_owned(),
    };
    if operation.result != OperationResult::Unit {
        return Err(invalid("window store must produce Unit"));
    }
    if !terminal_semantics::structural_store_value_admitted(
        machine,
        structural_types.iter(),
        disposition,
        value.place,
    ) {
        return Err(invalid(
            "stored value has no admitted custody for this disposition",
        ));
    }
    if value.access != StructuralAccess::Owned {
        return Err(invalid("window store must consume an owned value"));
    }
    if !value.path.is_empty() {
        return Err(invalid(
            "window store must consume a complete value occurrence",
        ));
    }
    let destination = borrowed_window_root(machine, destination, disposition.required_access())
        .ok_or_else(|| invalid("store root has no admitted storage access"))?;
    let projection = declared_window_projection(
        structural_types,
        &destination,
        path,
        field,
        disposition.required_access(),
    )
    .ok_or_else(|| invalid("store path has no admitted structural window"))?;
    let signature = source_signature(machine, destination.place())
        .ok_or_else(|| invalid("store root has no declared source signature"))?;
    let (required, required_projected) =
        terminal_psi::StructuralPathQualification::select_restorable(
            signature.qualifications,
            signature.projected_qualifications,
            &projection.path,
        )
        .ok_or_else(|| invalid("store memberships have no exact restorable projection"))?;
    let Some(source) = source_signature(machine, value.place) else {
        return Err(invalid("stored value has no declared source signature"));
    };
    if source.structural_type != projection.structural_type {
        return Err(invalid(&format!(
            "stored carrier {:?} differs from window carrier {:?}",
            source.structural_type, projection.structural_type,
        )));
    }
    if !required
        .iter()
        .all(|domain| source.qualifications.contains(domain))
    {
        return Err(invalid("stored value lacks required window memberships"));
    }
    if !required_projected
        .iter()
        .all(|row| source.projected_qualifications.contains(row))
    {
        return Err(invalid("stored value lacks required child memberships"));
    }
    if !root_window_loan(&destination, path, field) {
        return Err(invalid(
            "store has no admitted child or whole-referent loan",
        ));
    }
    Ok(AbstractOperation::StoreStructuralField {
        disposition,
        psi_operation: operation.id,
        destination,
        path: path.to_vec(),
        field,
        value: value.clone(),
    })
}

/// The root a window operation may name: a mutable-borrowed entry or block parameter
/// (borrow authority, a transferable multiplicity, and no entry claims,
/// resolved through the exact parameter row), or an owned local
/// home resolved through its exact operation-result storage declaration.
/// Terminal admission separately establishes completed payload custody or a
/// returned reference's loan authority. Here the shared storage query retains
/// that producer and result; field geometry never treats an owned reference
/// carrier as ownership of its referent.
fn borrowed_window_root(
    machine: &TerminalMachine,
    place: PlaceId,
    required_access: StructuralAccess,
) -> Option<StructuralWindowRoot> {
    // An owned signature parameter is its own ABI home. A block arrival
    // retains its own declaration and place: edge verification transfers the
    // loan; it does not turn that arrival into an ABI parameter.
    if let Some(parameter) =
        exact_parameter(machine, place).or_else(|| block_parameter(machine, place).cloned())
        && ((parameter.access == StructuralAccess::Owned
            && exact_parameter(machine, place).is_some())
            || (required_access.admits_referent(parameter.access)
                && matches!(
                    parameter.multiplicity,
                    terminal_psi::nonlinear_multiplicity_pair_pattern!()
                )
                && !machine
                    .entry_claims
                    .iter()
                    .any(|claim| claim.input == place)
                && !machine
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == place)))
    {
        return Some(StructuralWindowRoot::Parameter(parameter));
    }
    // An owned arrival supplies its exact home independently of its generic
    // claim frontier. The retained block and moved result keep those bindings;
    // they do not turn an owned case payload into borrowed backing. Content
    // entry custody still requires its separate conservation transfer protocol.
    if let Some((block, parameter)) =
        super::primitive_storage::owned_state_storage_parameter(machine, place)
        && block != machine.entry
        && !machine
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == place)
    {
        return Some(StructuralWindowRoot::OwnedBlockParameter {
            block,
            parameter: parameter.clone(),
        });
    }
    // Rejoin the unique operation-result declaration rather than recognize
    // how its producer was spelled. The verified window and the field-shape
    // check below retain authority: a view descriptor is not owned storage,
    // and a reference carrier crosses only an explicit `Referent`.
    let StructuralScalarStoreDestination::OwnedResult { producer, result } =
        owned_storage_destination(machine, place)?
    else {
        return None;
    };
    Some(StructuralWindowRoot::OwnedResult { producer, result })
}

/// A whole-root window needs actual mutable backing custody. A reference
/// crossing alone does not turn its whole referent into a child.
fn root_window_loan(
    root: &StructuralWindowRoot,
    path: &[StructuralPathSegment],
    field: Option<StructuralFieldId>,
) -> bool {
    terminal_semantics::structural_window_selects_child(path, field)
        // This is the declared route, not live authority. Current loan replay
        // authenticates all of its owned or exclusive ingress origins.
        || path.contains(&StructuralPathSegment::Referent)
        || (path.is_empty() && root.parameter().is_some())
}

/// The declared `Structural` type of the field a window operation names,
/// resolved beneath `root` through the spelled path; a root window names the
/// whole referent with no path. `None` means the place does not host a
/// structural field there.
fn declared_window_projection(
    structural_types: &[StructuralTypeDeclaration],
    root: &StructuralWindowRoot,
    path: &[StructuralPathSegment],
    field: Option<StructuralFieldId>,
    required_access: StructuralAccess,
) -> Option<terminal_semantics::StructuralWindowProjection> {
    // The carrier owns its permission, not the referent bytes. Only the
    // explicit crossing changes geometry; keep the original root and path on
    // the emitted operation for loan replay and native pointer transport.
    if let Some(crossing) = path
        .iter()
        .rposition(|segment| *segment == StructuralPathSegment::Referent)
    {
        let carrier = terminal_semantics::structural_window_projection(
            structural_types.iter(),
            root.structural_type(),
            &path[..crossing],
            None,
        )?;
        let StructuralTypeShape::Reference { access, .. } =
            terminal_psi::find_structural_type(structural_types, carrier.structural_type)?.shape
        else {
            return None;
        };
        if !required_access.admits_referent(access)
            || matches!(root, StructuralWindowRoot::OwnedResult { result, .. } if !result.qualification_establishments.is_empty())
        {
            return None;
        }
    }
    terminal_semantics::structural_window_projection(
        structural_types.iter(),
        root.structural_type(),
        path,
        field,
    )
}

/// The declared signature a spelled structural place carries: machine
/// parameter, block parameter resolved through its place row, then operation
/// result, or an explicitly established literal carrier. Literal storage has a
/// declaration and a Unit-producing establishment, not a structural result row.
pub(super) struct SourceSignature<'declaration> {
    pub(super) structural_type: StructuralTypeId,
    access: StructuralAccess,
    multiplicity: StructuralMultiplicity,
    qualifications: &'declaration [semantic_vocabulary::StructuralDomainId],
    projected_qualifications: &'declaration [terminal_psi::StructuralPathQualification],
}

impl SourceSignature<'_> {
    /// Descriptor ownership retains the value's declared backing access;
    /// a borrowed parameter or literal already publishes its own access.
    pub(super) fn descriptor_access(
        &self,
        structural_types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> StructuralAccess {
        if self.access != StructuralAccess::Owned {
            return self.access;
        }
        terminal_semantics::descriptor_results::descriptor_value_access(
            structural_types.iter(),
            self.structural_type,
            self.multiplicity,
        )
        .unwrap_or(self.access)
    }
}

pub(super) fn source_signature(
    machine: &TerminalMachine,
    place: PlaceId,
) -> Option<SourceSignature<'_>> {
    if let Some(parameter) = machine.structural_parameter(place) {
        return Some(SourceSignature {
            structural_type: parameter.structural_type,
            access: parameter.access,
            multiplicity: parameter.multiplicity,
            qualifications: &parameter.qualifications,
            projected_qualifications: &parameter.projected_qualifications,
        });
    }
    if let Some(parameter) = block_parameter(machine, place) {
        return Some(SourceSignature {
            structural_type: parameter.structural_type,
            access: parameter.access,
            multiplicity: parameter.multiplicity,
            qualifications: &parameter.qualifications,
            projected_qualifications: &parameter.projected_qualifications,
        });
    }
    if let StructuralPlaceKind::ByteSequenceLiteral {
        structural_type, ..
    } = machine.structural_place(place)?.kind
    {
        let mut establishments = machine.operations().filter_map(|operation| {
            let OperationKind::EstablishByteSequenceLiteral {
                destination,
                qualifications,
                ..
            } = &operation.kind
            else {
                return None;
            };
            (*destination == place && matches!(operation.result, OperationResult::Unit))
                .then_some(qualifications)
        });
        let qualifications = establishments.next()?;
        return establishments.next().is_none().then_some(SourceSignature {
            structural_type,
            access: StructuralAccess::SharedBorrow,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications,
            projected_qualifications: &[],
        });
    }
    machine.operations().find_map(|operation| {
        operation
            .result
            .structural()
            .filter(|result| result.place == place)
            .map(|result| SourceSignature {
                structural_type: result.structural_type,
                access: StructuralAccess::Owned,
                multiplicity: result.multiplicity,
                qualifications: &result.qualifications,
                projected_qualifications: &result.projected_qualifications,
            })
    })
}

/// Resolve one block-structural parameter through its `BlockParameter` place
/// row: the row's block and position select the declaration, which must
/// still spell the same place and position and cannot be `self`.
pub(super) fn block_parameter(
    machine: &TerminalMachine,
    place: PlaceId,
) -> Option<&StructuralParameterDeclaration> {
    let declaration = machine.structural_place(place)?;
    let StructuralPlaceKind::BlockParameter { block, position } = declaration.kind else {
        return None;
    };
    machine
        .block(&block)?
        .structural_parameters
        .get(position as usize)
        .filter(|parameter| parameter.place == place && parameter.position == position)
}
