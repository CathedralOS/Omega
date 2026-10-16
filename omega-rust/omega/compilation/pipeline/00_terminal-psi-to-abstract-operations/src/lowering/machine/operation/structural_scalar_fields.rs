//! Exact structural scalar-field preservation from admitted Terminal declarations.
//!
//! Scalar definitions belong to the machine's SSA roster, not just the block
//! containing a store. RHS evaluation may introduce continuations after a
//! destination selector was captured. Its dominating definition remains the
//! same value; requiring a local declaration would reject that ordinary flow.
//! This projection consumes producer-established SSA. The shared roster supplies
//! types, not dominance evidence; it does not impose a second, block-local rule.
//! Refined reads use the declared scalar carrier after constructor/entry predicate
//! proof reconstruction. A read captures a value; it does not relax the declared
//! invariant, so stores without their own establishment proof remain rejected.
//! A scalar read may also end its carrier with one `Case` step naming a
//! payload field of that declared case; the payload overlays the sum's own
//! place, and the carrier and leaf are rebuilt from declarations here.

use std::collections::BTreeMap;

use abstract_operations::{AbstractOperation, AbstractResult, StructuralScalarStoreDestination};
use semantic_vocabulary::{
    PlaceId, ScalarType, StructuralFieldId, StructuralPlaceKind, StructuralTypeId,
};
use terminal_psi::{
    BindingRelevance, Operation, OperationKind, OperationResult, StructuralAccess,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
    find_structural_type,
};

use crate::lowering::LoweringError;

pub(super) fn lower(
    operation: &Operation,
    machine: &TerminalMachine,
    structural_types: &[StructuralTypeDeclaration],
    module: &terminal_psi::TerminalModule,
    value_types: &BTreeMap<semantic_vocabulary::ValueId, ScalarType>,
) -> Result<AbstractOperation, LoweringError> {
    match &operation.kind {
        OperationKind::StructuralScalarFieldStore {
            destination,
            path,
            field,
            value,
            range_obligation,
        } => lower_store(
            operation,
            machine,
            structural_types,
            module,
            value_types,
            *destination,
            path,
            *field,
            *value,
            *range_obligation,
        ),
        OperationKind::ScalarStructuralField {
            source,
            path,
            field,
        } => lower_scalar_read(operation, machine, structural_types, *source, path, *field),
        _ => unreachable!("structural scalar-field router is exhaustive"),
    }
}

/// Preserve the exact owned or borrowed root for every scalar store form.
/// Address presentation never turns an owned state home into an entry loan.
pub(super) fn store_destination(
    machine: &TerminalMachine,
    destination: PlaceId,
) -> Option<StructuralScalarStoreDestination> {
    exact_parameter(machine, destination)
        .or_else(|| {
            let mut matches = machine.blocks.iter().flat_map(|block| {
                block
                    .structural_parameters
                    .iter()
                    .enumerate()
                    .filter(move |(_, parameter)| parameter.place == destination)
                    .map(move |(position, parameter)| (block.id, position, parameter))
            });
            let (owner, position, parameter) = matches.next()?;
            let expected_kind = StructuralPlaceKind::BlockParameter {
                block: owner,
                position: parameter.position,
            };
            if matches.next().is_some()
                || owner == machine.entry
                || parameter.position as usize != position
                // Named-state write-only arrivals retain store authority just
                // as mutable arrivals do; neither permits descriptor reseating.
                || !matches!(parameter.access,
                    StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow)
                || machine
                    .structural_parameters
                    .iter()
                    .any(|row| row.place == destination)
            {
                return None;
            }
            let mut places = machine
                .structural_places
                .iter()
                .filter(|row| row.id == destination);
            if places.next()?.kind != expected_kind || places.next().is_some() {
                return None;
            }
            Some(parameter.clone())
        })
        .map(StructuralScalarStoreDestination::Parameter)
        .or_else(|| owned_storage_destination(machine, destination))
        // An owned arrival writes the home its block entry established, the
        // one every incoming edge moved its whole source into; Terminal
        // verification admits an owned block parameter as write authority.
        .or_else(|| {
            let (block, parameter) =
                super::primitive_storage::owned_state_storage_parameter(machine, destination)?;
            (block != machine.entry).then(|| {
                StructuralScalarStoreDestination::OwnedBlockParameter {
                    block,
                    parameter: parameter.clone(),
                }
            })
        })
}

#[allow(clippy::too_many_arguments)]
fn lower_store(
    operation: &Operation,
    machine: &TerminalMachine,
    structural_types: &[StructuralTypeDeclaration],
    module: &terminal_psi::TerminalModule,
    value_types: &BTreeMap<semantic_vocabulary::ValueId, ScalarType>,
    destination: PlaceId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    value: semantic_vocabulary::ValueId,
    range_obligation: Option<semantic_vocabulary::ObligationId>,
) -> Result<AbstractOperation, LoweringError> {
    let invalid = |reason| LoweringError::InvalidStructuralScalarFieldStore {
        machine: machine.id,
        operation: operation.id,
        reason,
    };
    // A state arrival is a declared loan of the same backing storage, not an
    // entry ABI parameter. Preserve its own declaration whether transport uses
    // an address or a view descriptor; edge verification checked exclusivity.
    let destination = store_destination(machine, destination)
        .or_else(|| owned_storage_destination(machine, destination))
        .ok_or_else(|| invalid("destination has no exact writable storage declaration"))?;
    let scalar_type = value_types
        .get(&value)
        .copied()
        .ok_or_else(|| invalid("stored value has no scalar declaration"))?;
    // A returned reference is a permission carrier, not a new record home.
    // Resolve the final explicit crossing for geometry; preserve the root and
    // full path below for independent ownership and native custody replay.
    let (storage_type, storage_path) = if let Some(crossing) = path
        .iter()
        .rposition(|segment| *segment == StructuralPathSegment::Referent)
    {
        let suffix = &path[crossing + 1..];
        // Any owned or loaned carrier whose declaration is a reference
        // type peels the same explicit dereference: an operation result,
        // an entry parameter, or a join-completed block parameter all
        // carry the loan's authority on the type, not on a producer
        // identity only an operation result has.
        let carrier_type = destination.structural_type();
        let Some(StructuralTypeShape::Reference {
            referent,
            access: StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow,
        }) = resolve_structural_path(structural_types, carrier_type, &path[..crossing])
            .and_then(|descriptor| find_structural_type(structural_types, descriptor))
            .map(|declaration| &declaration.shape)
        else {
            return Err(invalid(
                "referent crossing has no exclusive reference carrier",
            ));
        };
        if find_structural_type(structural_types, *referent).is_none()
            || !terminal_semantics::reference_source_projection_accessible(
                structural_types.iter(),
                carrier_type,
                destination.multiplicity(),
                destination
                    .parameter()
                    .map_or(StructuralAccess::Owned, |parameter| parameter.access),
                path,
                StructuralAccess::WriteOnlyBorrow,
            )
        {
            return Err(invalid("referent projection has no write authority"));
        }
        (*referent, suffix)
    } else {
        (destination.structural_type(), path)
    };
    if !terminal_semantics::scalar_field_store_preserves_qualifications(
        structural_types.iter(),
        module.structural_domains.iter(),
        &module.identities,
        destination.structural_type(),
        destination.qualifications(),
        destination.projected_qualifications(),
        path,
        field,
    ) {
        return Err(invalid(
            "field write overlaps a domain predicate or content definition",
        ));
    }
    if !terminal_semantics::scalar_field_store_preserves_content(
        structural_types.iter(),
        module.structural_domains.iter(),
        &module.identities,
        destination.structural_type(),
        destination.place(),
        machine.content_entry_claims.iter(),
        path,
        field,
    ) {
        return Err(invalid("field write changes conserved entry content"));
    }
    if operation.result != OperationResult::Unit
        || destination.parameter().is_some_and(|parameter| {
            !matches!(
                parameter.access,
                StructuralAccess::Owned
                    | StructuralAccess::MutableBorrow
                    | StructuralAccess::WriteOnlyBorrow
            )
        })
        || !terminal_psi::is_structural_scalar_store_path(storage_path)
        || !path
            .iter()
            .filter_map(StructuralPathSegment::runtime_index)
            .all(|(index, _)| {
                super::primitive_projection::runtime_selector(value_types.get(&index).copied())
            })
    {
        return Err(invalid(
            "store result, access, path or selector type is incompatible",
        ));
    }
    let declaration = terminal_semantics::projected_field_declaration(
        structural_types.iter(),
        storage_type,
        storage_path,
        field,
    )
    .ok_or_else(|| invalid("selected path has no exact field declaration"))?;
    if terminal_semantics::scalar_field_requires_evidence(&declaration.field_type)
        != range_obligation.is_some()
        || declaration.field_type.scalar_type() != Some(scalar_type)
    {
        return Err(invalid(
            "stored scalar type or receiving obligation differs from the field",
        ));
    }
    Ok(AbstractOperation::StructuralScalarFieldStore {
        psi_operation: operation.id,
        destination,
        path: path.to_vec(),
        field,
        value: AbstractResult { value, scalar_type },
        range_obligation,
    })
}

/// Resolve completed storage identity without deciding a mutation's evidence
/// obligations. Scalar stores and restoration windows apply their own joins.
pub(super) fn owned_storage_destination(
    machine: &TerminalMachine,
    place: PlaceId,
) -> Option<StructuralScalarStoreDestination> {
    let mut declarations = machine
        .structural_places
        .iter()
        .filter(|row| row.id == place);
    let declaration = declarations.next()?;
    if declarations.next().is_some() {
        return None;
    }
    let (producer, structural_type) = declaration.kind.as_operation_result()?;
    let mut operations = machine
        .operations()
        .filter(|operation| operation.id == producer);
    let operation = operations.next()?;
    if operations.next().is_some() {
        return None;
    }
    let result = operation.result.structural()?;
    (result.place == place
        && result.structural_type == structural_type
        && machine
            .content_entry_claims
            .iter()
            .all(|claim| claim.input.root != place))
    .then(|| StructuralScalarStoreDestination::OwnedResult {
        producer,
        result: result.clone(),
    })
}

fn lower_scalar_read(
    operation: &Operation,
    machine: &TerminalMachine,
    structural_types: &[StructuralTypeDeclaration],
    source: PlaceId,
    path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
    field: StructuralFieldId,
) -> Result<AbstractOperation, LoweringError> {
    let invalid = || LoweringError::InvalidScalarStructuralField(operation.id);
    let (structural_type, access) =
        readable_source_type(machine, structural_types, source).ok_or_else(invalid)?;
    let result = operation.result.scalar().ok_or_else(invalid)?;
    // A call result's `Reference` shell keeps the loan's access on the type;
    // the observed record is its referent, the same peel the verifying and
    // codec-side validators apply to this read.
    let (observed_root, access) = match find_structural_type(structural_types, structural_type)
        .map(|declaration| &declaration.shape)
    {
        Some(StructuralTypeShape::Reference { referent, access }) => (*referent, *access),
        _ => (structural_type, access),
    };
    let access = terminal_semantics::observed_field_backing_access(
        structural_types.iter(),
        observed_root,
        path,
        access,
    )
    .ok_or_else(invalid)?;
    let declaration = terminal_semantics::observed_field_declaration(
        structural_types.iter(),
        observed_root,
        path,
        field,
    )
    .ok_or_else(invalid)?;
    if matches!(declaration.field_type, StructuralFieldType::Atomic(_))
        && !matches!(access, terminal_psi::mutating_access_pair_pattern!())
    {
        return Err(invalid());
    }
    if observed_scalar_field(structural_types, observed_root, path, field)
        != Some(result.scalar_type)
    {
        return Err(invalid());
    }
    Ok(AbstractOperation::StructuralScalarFieldRead {
        psi_operation: operation.id,
        result: result.into(),
        source,
        path: path.to_vec(),
        field,
    })
}

fn readable_source_type(
    machine: &TerminalMachine,
    structural_types: &[StructuralTypeDeclaration],
    source: PlaceId,
) -> Option<(StructuralTypeId, StructuralAccess)> {
    // An observation retains the owner's obligation at its existing home.
    // Reference and view carriers retain their separate claim-free contract;
    // Terminal replay already checked current ownership and loan availability.
    let owned_storage = |structural_type, access| {
        access == StructuralAccess::Owned
            && structural_types.iter().any(|declaration| {
                declaration.id == structural_type && (declaration.shape).is_composite_shape()
            })
    };
    // A content-bearing entry claim is an entry claim for observation: owned
    // composite storage may be read under it (a partition provider reads the
    // magnitude it conserves), while borrowed or linear non-owned sources
    // keep the narrower rule. The snapshot neither consumes nor discharges it.
    let has_entry_claim = machine
        .entry_claims
        .iter()
        .any(|claim| claim.input == source)
        || machine
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == source);
    if let Some(parameter) = machine.parameter(source) {
        return (parameter.access != StructuralAccess::WriteOnlyBorrow
            && (owned_storage(parameter.structural_type, parameter.access)
                || (parameter.multiplicity != StructuralMultiplicity::Linear
                    && !has_entry_claim)))
            .then_some((parameter.structural_type, parameter.access));
    }
    // The verified producer and availability checks retain the actual result root;
    // it never acquires an incoming-parameter identity merely to be observed.
    let result = machine
        .operations()
        .filter_map(|operation| operation.result.structural())
        .find(|result| result.place == source)?;
    (owned_storage(result.structural_type, StructuralAccess::Owned)
        || (result.multiplicity != StructuralMultiplicity::Linear
            && result.claims.is_empty()
            && !has_entry_claim))
        .then_some((result.structural_type, StructuralAccess::Owned))
}

pub(super) fn exact_parameter(
    machine: &TerminalMachine,
    place: PlaceId,
) -> Option<StructuralParameterDeclaration> {
    let mut parameters = machine
        .structural_parameters
        .iter()
        .filter(|parameter| parameter.place == place);
    let parameter = parameters.next()?;
    if parameters.next().is_some() {
        return None;
    }
    let mut places = machine
        .structural_places
        .iter()
        .filter(|declaration| declaration.id == place);
    let declaration = places.next()?;
    if places.next().is_some()
        || !matches!(
            declaration.kind,
            StructuralPlaceKind::Parameter { position }
                if position == parameter.position
        )
    {
        return None;
    }
    Some(parameter.clone())
}

fn resolve_structural_path(
    structural_types: &[StructuralTypeDeclaration],
    mut structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    for segment in path {
        let declaration = exact_structural_type(structural_types, structural_type)?;
        structural_type = match (segment, &declaration.shape) {
            (
                StructuralPathSegment::Field(identity),
                StructuralTypeShape::Record { fields, .. },
            ) => {
                let mut matching = fields.iter().filter(|field| {
                    field.identity == *identity && field.relevance == BindingRelevance::Relevant
                });
                let field = matching.next()?;
                if matching.next().is_some() {
                    return None;
                }
                let next = field.field_type.as_structural()?;
                next
            }
            (
                StructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) if index < length => *element,
            // The element's bound is the store's own verified obligation.
            (
                StructuralPathSegment::RuntimeIndex { .. },
                StructuralTypeShape::FixedArray { element, .. },
            ) => *element,
            // A view's extent is its stored runtime length — the same
            // verified obligation, just carried by the loan rather than
            // the type's literal.
            (
                StructuralPathSegment::FixedIndex(_),
                StructuralTypeShape::ElementView { element, .. },
            ) => *element,
            (
                StructuralPathSegment::RuntimeIndex { .. },
                StructuralTypeShape::ElementView { element, .. },
            ) => *element,
            _ => return None,
        };
    }
    Some(structural_type)
}

/// The scalar one field observation reads beneath a `root` of that type: a
/// relevant field of the record `path` reaches, or a relevant payload field of
/// the declared case a final `Case` step names. The `Case` step is a claim
/// about which overlay is live, not storage; Terminal verification proved it
/// before this read, so only the declared carrier and leaf are rebuilt here.
fn observed_scalar_field(
    structural_types: &[StructuralTypeDeclaration],
    root: StructuralTypeId,
    path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
    field: StructuralFieldId,
) -> Option<ScalarType> {
    if let Some(carrier) =
        terminal_semantics::case_payload_carrier(structural_types.iter(), root, path)
    {
        exact_structural_type(structural_types, carrier.sum)?;
        return unique_relevant_scalar_field(carrier.fields, field, true);
    }
    // Common fields sit beside the tag regardless of the active case.
    let carrier = terminal_semantics::record_field_carrier(structural_types.iter(), root, path)?;
    let declaration = exact_structural_type(structural_types, carrier.structural_type)?;
    let Some(fields) = declaration.shape.direct_fields() else {
        return None;
    };
    unique_relevant_scalar_field(fields, field, true)
}

fn unique_relevant_scalar_field(
    fields: &[terminal_psi::StructuralFieldDeclaration],
    field: StructuralFieldId,
    allow_refined_scalar: bool,
) -> Option<ScalarType> {
    let mut matching = fields.iter().filter(|candidate| {
        candidate.id == field && candidate.relevance == BindingRelevance::Relevant
    });
    let field = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    match &field.field_type {
        StructuralFieldType::Scalar(scalar_type) | StructuralFieldType::Atomic(scalar_type) => {
            Some(*scalar_type)
        }
        StructuralFieldType::IeeeFloat(format) => Some(ScalarType::IeeeFloat(*format)),
        StructuralFieldType::BoundedInteger(bounds) if allow_refined_scalar => {
            Some(ScalarType::Integer(bounds.integer_type()))
        }
        StructuralFieldType::RefinedScalar(refinement) if allow_refined_scalar => {
            Some(refinement.carrier())
        }
        StructuralFieldType::ByteSequence(_)
        | StructuralFieldType::PredicateByteSequence { .. }
        | StructuralFieldType::RefinedScalar(_)
        | StructuralFieldType::BoundedInteger(_)
        | StructuralFieldType::Structural(_)
        | StructuralFieldType::Erased { .. } => None,
    }
}

fn exact_structural_type(
    structural_types: &[StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
) -> Option<&StructuralTypeDeclaration> {
    let mut matching = structural_types
        .iter()
        .filter(|declaration| declaration.id == structural_type);
    let declaration = matching.next()?;
    matching.next().is_none().then_some(declaration)
}
