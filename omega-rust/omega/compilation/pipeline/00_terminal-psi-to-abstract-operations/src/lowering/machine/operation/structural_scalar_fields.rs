//! Exact structural scalar-field preservation after independent Terminal verification.
//! Bounded reads use the declared integer carrier after constructor/entry range
//! proof reconstruction. A read captures a value; it does not relax the declared
//! invariant, so stores without their own establishment proof remain rejected.
//! An integer read may also end its carrier with one `Case` step naming a
//! payload field of that declared case; the payload overlays the sum's own
//! place, and the carrier and leaf are rebuilt from declarations here.

use abstract_operations::{AbstractOperation, AbstractResult, StructuralScalarStoreDestination};
use semantic_vocabulary::{
    PlaceId, ScalarType, StructuralFieldId, StructuralPlaceKind, StructuralTypeId,
};
use terminal_psi::{
    BindingRelevance, Block, Operation, OperationKind, OperationResult, StructuralAccess,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
};

use crate::lowering::LoweringError;

pub(super) fn lower(
    operation: &Operation,
    block: &Block,
    machine: &TerminalMachine,
    structural_types: &[StructuralTypeDeclaration],
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
            block,
            machine,
            structural_types,
            *destination,
            path,
            *field,
            *value,
            *range_obligation,
        ),
        OperationKind::IntegerStructuralField {
            source,
            path,
            field,
        } => lower_integer_read(operation, machine, structural_types, *source, path, *field),
        _ => unreachable!("structural scalar-field router is exhaustive"),
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_store(
    operation: &Operation,
    block: &Block,
    machine: &TerminalMachine,
    structural_types: &[StructuralTypeDeclaration],
    destination: PlaceId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    value: semantic_vocabulary::ValueId,
    range_obligation: Option<semantic_vocabulary::ObligationId>,
) -> Result<AbstractOperation, LoweringError> {
    let invalid = || LoweringError::InvalidStructuralScalarFieldStore(operation.id);
    // A state arrival is a declared loan of the same backing storage, not an
    // entry ABI parameter. Preserve its own declaration whether transport uses
    // an address or a view descriptor; edge verification checked exclusivity.
    let destination = exact_parameter(machine, destination)
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
                || parameter.is_self
                || parameter.position as usize != position
                || parameter.access != StructuralAccess::MutableBorrow
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
        .or_else(|| owned_store_destination(machine, destination))
        // An owned arrival writes the home its block entry established, the
        // one every incoming edge moved its whole source into; Terminal
        // verification admits an owned block parameter as write authority.
        .or_else(|| {
            let (block, parameter) =
                super::primitive_storage::owned_state_parameter(machine, destination)?;
            (block != machine.entry).then(|| {
                StructuralScalarStoreDestination::OwnedBlockParameter {
                    block,
                    parameter: parameter.clone(),
                }
            })
        })
        .ok_or_else(invalid)?;
    let scalar_type =
        dominating_scalar_type(machine, block, operation.id, value).ok_or_else(invalid)?;
    // A returned reference is a permission carrier, not a new record home.
    // Peel only the explicit dereference for geometry; preserve the shell and
    // full path below for independent ownership and native custody replay.
    let (storage_type, storage_path) =
        if let Some((StructuralPathSegment::Referent, suffix)) = path.split_first() {
            let StructuralScalarStoreDestination::OwnedResult { result, .. } = &destination else {
                return Err(invalid());
            };
            let Some(StructuralTypeShape::Reference {
                referent,
                access: StructuralAccess::MutableBorrow,
            }) = structural_types
                .iter()
                .find(|declaration| declaration.id == result.structural_type)
                .map(|declaration| &declaration.shape)
            else {
                return Err(invalid());
            };
            if result.multiplicity != StructuralMultiplicity::Affine
                || !matches!(
                    structural_types
                        .iter()
                        .find(|declaration| declaration.id == *referent)
                        .map(|declaration| &declaration.shape),
                    Some(StructuralTypeShape::Record { .. })
                )
            {
                return Err(invalid());
            }
            (*referent, suffix)
        } else {
            (destination.structural_type(), path)
        };
    if operation.result != OperationResult::Unit
        || !matches!(
            destination.multiplicity(),
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        )
        || destination.parameter().is_some_and(|parameter| {
            !matches!(
                parameter.access,
                StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
            ) || !has_empty_structural_custody(machine, parameter.place)
        })
        || !terminal_psi::is_structural_scalar_store_path(storage_path)
        || !path
            .iter()
            .filter_map(StructuralPathSegment::runtime_index)
            .all(|(index, _)| {
                super::primitive_projection::runtime_selector(dominating_scalar_type(
                    machine,
                    block,
                    operation.id,
                    index,
                ))
            })
    {
        return Err(invalid());
    }
    let parent_type = resolve_structural_path(structural_types, storage_type, storage_path)
        .ok_or_else(invalid)?;
    let declaration = structural_types
        .iter()
        .find(|candidate| candidate.id == parent_type)
        .ok_or_else(invalid)?;
    // A mixed shape's common field is a direct field at one offset in every
    // case, stored without case knowledge exactly as a record field.
    let (StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. }) =
        &declaration.shape
    else {
        return Err(invalid());
    };
    let declaration = fields
        .iter()
        .find(|candidate| candidate.id == field)
        .ok_or_else(invalid)?;
    if matches!(
        declaration.field_type,
        StructuralFieldType::BoundedInteger(_)
    ) != range_obligation.is_some()
        || direct_relevant_scalar_field(
            structural_types,
            parent_type,
            field,
            range_obligation.is_some(),
        ) != Some(scalar_type)
    {
        return Err(invalid());
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

/// Retain the exact owned producer instead of manufacturing a parameter row.
pub(super) fn owned_store_destination(
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
    let StructuralPlaceKind::OperationResult {
        producer,
        structural_type,
    } = declaration.kind
    else {
        return None;
    };
    let mut operations = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| operation.id == producer);
    let operation = operations.next()?;
    if operations.next().is_some() {
        return None;
    }
    let result = operation.result.structural()?;
    (result.place == place
        && result.structural_type == structural_type
        && matches!(
            result.multiplicity,
            StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
        )
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty()
        && machine
            .entry_claims
            .iter()
            .all(|claim| claim.input != place)
        && machine
            .content_entry_claims
            .iter()
            .all(|claim| claim.input.root != place))
    .then(|| StructuralScalarStoreDestination::OwnedResult {
        producer,
        result: result.clone(),
    })
}

fn lower_integer_read(
    operation: &Operation,
    machine: &TerminalMachine,
    structural_types: &[StructuralTypeDeclaration],
    source: PlaceId,
    path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
    field: StructuralFieldId,
) -> Result<AbstractOperation, LoweringError> {
    let invalid = || LoweringError::InvalidIntegerStructuralField(operation.id);
    let (structural_type, access) = readable_source_type(machine, source).ok_or_else(invalid)?;
    let result = operation.result.scalar().ok_or_else(invalid)?;
    // A call result's `Reference` shell keeps the loan's access on the type;
    // the observed record is its referent, the same peel the verifying and
    // codec-side validators apply to this read.
    let (observed_root, access) = match structural_types
        .iter()
        .find(|declaration| declaration.id == structural_type)
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
        && !matches!(
            access,
            StructuralAccess::Owned | StructuralAccess::MutableBorrow
        )
    {
        return Err(invalid());
    }
    if !matches!(result.scalar_type, ScalarType::Integer(_))
        || observed_scalar_field(structural_types, observed_root, path, field)
            != Some(result.scalar_type)
    {
        return Err(invalid());
    }
    Ok(AbstractOperation::StructuralScalarFieldRead {
        psi_operation: operation.id,
        result: AbstractResult {
            value: result.id,
            scalar_type: result.scalar_type,
        },
        source,
        path: path.to_vec(),
        field,
    })
}

fn readable_source_type(
    machine: &TerminalMachine,
    source: PlaceId,
) -> Option<(StructuralTypeId, StructuralAccess)> {
    if machine
        .entry_claims
        .iter()
        .any(|claim| claim.input == source)
        || machine
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == source)
    {
        return None;
    }
    if let Some(parameter) = machine
        .structural_parameters
        .iter()
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .find(|parameter| parameter.place == source)
    {
        return (parameter.access != StructuralAccess::WriteOnlyBorrow
            && parameter.multiplicity != StructuralMultiplicity::Linear
            && parameter.qualifications.is_empty()
            && parameter.projected_qualifications.is_empty())
        .then_some((parameter.structural_type, parameter.access));
    }
    // The verified producer and availability checks retain the actual result root;
    // it never acquires an incoming-parameter identity merely to be observed.
    let result = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| operation.result.structural())
        .find(|result| result.place == source)?;
    (result.multiplicity != StructuralMultiplicity::Linear
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty())
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
            StructuralPlaceKind::Parameter { position, is_self }
                if position == parameter.position && is_self == parameter.is_self
        )
    {
        return None;
    }
    Some(parameter.clone())
}

pub(super) fn dominating_scalar_type(
    machine: &TerminalMachine,
    block: &Block,
    operation: semantic_vocabulary::OperationId,
    value: semantic_vocabulary::ValueId,
) -> Option<ScalarType> {
    let entry_declarations = machine
        .parameters
        .iter()
        .chain(block.parameters.iter())
        .filter(|declaration| declaration.id == value)
        .map(|declaration| declaration.scalar_type)
        .collect::<Vec<_>>();
    if let [scalar_type] = entry_declarations.as_slice() {
        return Some(*scalar_type);
    }
    if !entry_declarations.is_empty() {
        return None;
    }

    let mut definition = None;
    let mut found_operation = false;
    for candidate in &block.operations {
        if candidate.id == operation {
            if found_operation {
                return None;
            }
            found_operation = true;
            continue;
        }
        if !found_operation
            && let Some(result) = candidate.result.scalar()
            && result.id == value
            && definition.replace(result.scalar_type).is_some()
        {
            return None;
        }
    }
    found_operation.then_some(definition).flatten()
}

pub(super) fn has_empty_structural_custody(machine: &TerminalMachine, place: PlaceId) -> bool {
    machine
        .structural_parameters
        .iter()
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .find(|parameter| parameter.place == place)
        .is_some_and(|parameter| {
            parameter.qualifications.is_empty() && parameter.projected_qualifications.is_empty()
        })
        && machine
            .entry_claims
            .iter()
            .all(|claim| claim.input != place)
        && machine
            .content_entry_claims
            .iter()
            .all(|claim| claim.input.root != place)
}

fn resolve_structural_path(
    structural_types: &[StructuralTypeDeclaration],
    mut structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    for segment in path {
        let declaration = exact_structural_type(structural_types, structural_type)?;
        structural_type = match (segment, &declaration.shape) {
            (StructuralPathSegment::Field(identity), StructuralTypeShape::Record { fields }) => {
                let mut matching = fields.iter().filter(|field| {
                    field.identity == *identity && field.relevance == BindingRelevance::Relevant
                });
                let field = matching.next()?;
                if matching.next().is_some() {
                    return None;
                }
                let StructuralFieldType::Structural(next) = field.field_type else {
                    return None;
                };
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
                StructuralTypeShape::ElementView { element },
            ) => *element,
            (
                StructuralPathSegment::RuntimeIndex { .. },
                StructuralTypeShape::ElementView { element },
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
    // A read also reaches a mixed shape's common field, which sits on the
    // carrier beside the tag whatever case is active; a store reaches the same
    // common fields through `direct_relevant_scalar_field`.
    let carrier = terminal_semantics::record_field_carrier(structural_types.iter(), root, path)?;
    let declaration = exact_structural_type(structural_types, carrier.structural_type)?;
    let (StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. }) =
        &declaration.shape
    else {
        return None;
    };
    unique_relevant_scalar_field(fields, field, true)
}

fn direct_relevant_scalar_field(
    structural_types: &[StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
    field: StructuralFieldId,
    allow_bounded_integer: bool,
) -> Option<ScalarType> {
    let declaration = exact_structural_type(structural_types, structural_type)?;
    let (StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. }) =
        &declaration.shape
    else {
        return None;
    };
    unique_relevant_scalar_field(fields, field, allow_bounded_integer)
}

fn unique_relevant_scalar_field(
    fields: &[terminal_psi::StructuralFieldDeclaration],
    field: StructuralFieldId,
    allow_bounded_integer: bool,
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
        StructuralFieldType::BoundedInteger(bounds) if allow_bounded_integer => {
            Some(ScalarType::Integer(bounds.integer_type()))
        }
        StructuralFieldType::ByteSequence(_)
        | StructuralFieldType::PredicateByteSequence { .. }
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

#[cfg(test)]
mod tests {
    use super::{
        direct_relevant_scalar_field, dominating_scalar_type, exact_parameter,
        has_empty_structural_custody, observed_scalar_field, resolve_structural_path,
    };
    use semantic_vocabulary::{
        BlockId, ContractId, EdgeId, IntegerSign, IntegerType, MachineId, OperationId, PlaceId,
        ScalarType, StructuralFieldId, StructuralPlaceKind, StructuralTypeId, ValueId,
    };
    use terminal_psi::{
        BindingRelevance, Block, MachineContract, Operation, OperationKind, OperationResult,
        StructuralAccess, StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
        StructuralParameterDeclaration, StructuralPathSegment, StructuralPlaceDeclaration,
        StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine, TerminalMachineResult,
        Terminator, ValueDeclaration,
    };

    fn i32_type() -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap())
    }

    fn scalar_parameter(id: u64) -> ValueDeclaration {
        ValueDeclaration {
            qualifications: Default::default(),
            id: ValueId::new(id).unwrap(),
            scalar_type: i32_type(),
        }
    }

    fn structural_parameter(
        place: u64,
        position: u32,
        multiplicity: StructuralMultiplicity,
        access: StructuralAccess,
    ) -> StructuralParameterDeclaration {
        StructuralParameterDeclaration {
            place: PlaceId::new(place).unwrap(),
            position,
            is_self: false,
            structural_type: StructuralTypeId::new(1).unwrap(),
            multiplicity,
            access,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }
    }

    fn machine(
        parameters: Vec<ValueDeclaration>,
        structural_parameters: Vec<StructuralParameterDeclaration>,
        structural_places: Vec<StructuralPlaceDeclaration>,
    ) -> TerminalMachine {
        TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(1).unwrap(),
            attachment: None,
            parameters,
            structural_parameters,
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(2).unwrap(),
            blocks: Vec::new(),
            contract: MachineContract {
                erased_proof_formals: Vec::new(),
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(9).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }
    }

    fn test_block(operations: Vec<Operation>) -> Block {
        Block {
            erased_proof_formals: Vec::new(),
            erased_scalar_formals: Vec::new(),
            id: BlockId::new(2).unwrap(),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations,
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(8).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        }
    }

    /// A no-result marker: the dominance walk only needs the operation's id
    /// to exist in the block to bound "prior definitions".
    fn marker(id: u64) -> Operation {
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: OperationId::new(id).unwrap(),
            result: OperationResult::Unit,
            kind: OperationKind::BooleanConstant { value: false },
        }
    }

    fn producer(id: u64, value: u64) -> Operation {
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: OperationId::new(id).unwrap(),
            result: OperationResult::Scalar(scalar_parameter(value)),
            kind: OperationKind::BooleanConstant { value: true },
        }
    }

    fn record_type(id: u64, fields: Vec<StructuralFieldDeclaration>) -> StructuralTypeDeclaration {
        StructuralTypeDeclaration {
            id: StructuralTypeId::new(id).unwrap(),
            identity: format!("test::T{id}"),
            shape: StructuralTypeShape::Record { fields },
        }
    }

    fn field(
        id: u64,
        identity: &str,
        field_type: StructuralFieldType,
    ) -> StructuralFieldDeclaration {
        StructuralFieldDeclaration {
            id: StructuralFieldId::new(id).unwrap(),
            identity: identity.to_owned(),
            relevance: BindingRelevance::Relevant,
            field_type,
        }
    }

    #[test]
    fn dominating_scalar_type_rejects_conflicting_entry_declarations() {
        // Machine and block parameters both claiming one value id is exactly
        // the ambiguity the store check must not guess through.
        let machine = machine(vec![scalar_parameter(5)], Vec::new(), Vec::new());
        let mut block = test_block(Vec::new());
        block.parameters.push(scalar_parameter(5));
        assert_eq!(
            dominating_scalar_type(
                &machine,
                &block,
                OperationId::new(1).unwrap(),
                ValueId::new(5).unwrap(),
            ),
            None
        );
    }

    #[test]
    fn dominating_scalar_type_rejects_redefined_or_undeclared_values() {
        let machine = machine(Vec::new(), Vec::new(), Vec::new());
        let block = test_block(vec![producer(3, 5), producer(4, 5), marker(5)]);
        assert_eq!(
            dominating_scalar_type(
                &machine,
                &block,
                OperationId::new(5).unwrap(),
                ValueId::new(5).unwrap(),
            ),
            None
        );
        assert_eq!(
            dominating_scalar_type(
                &machine,
                &block,
                OperationId::new(5).unwrap(),
                ValueId::new(7).unwrap(),
            ),
            None
        );
    }

    #[test]
    fn resolve_structural_path_walks_record_fields_and_fixed_indexes() {
        let types = vec![
            record_type(
                1,
                vec![field(
                    10,
                    "inner",
                    StructuralFieldType::Structural(StructuralTypeId::new(2).unwrap()),
                )],
            ),
            StructuralTypeDeclaration {
                id: StructuralTypeId::new(2).unwrap(),
                identity: "test::Array".into(),
                shape: StructuralTypeShape::FixedArray {
                    element: StructuralTypeId::new(3).unwrap(),
                    length: 4,
                },
            },
            record_type(3, Vec::new()),
        ];
        let root = StructuralTypeId::new(1).unwrap();
        assert_eq!(
            resolve_structural_path(
                &types,
                root,
                &[
                    StructuralPathSegment::Field("inner".into()),
                    StructuralPathSegment::FixedIndex(3),
                ],
            ),
            Some(StructuralTypeId::new(3).unwrap())
        );
        // An out-of-range index and a field on a non-record both fail closed.
        assert_eq!(
            resolve_structural_path(
                &types,
                root,
                &[
                    StructuralPathSegment::Field("inner".into()),
                    StructuralPathSegment::FixedIndex(4),
                ],
            ),
            None
        );
        assert_eq!(
            resolve_structural_path(
                &types,
                root,
                &[StructuralPathSegment::Field("missing".into())],
            ),
            None
        );
        assert_eq!(resolve_structural_path(&types, root, &[]), Some(root));
    }

    #[test]
    fn exact_parameter_requires_one_matching_place_declaration() {
        let place = PlaceId::new(20).unwrap();
        let mut machine = machine(
            Vec::new(),
            vec![structural_parameter(
                20,
                0,
                StructuralMultiplicity::Affine,
                StructuralAccess::MutableBorrow,
            )],
            vec![StructuralPlaceDeclaration {
                id: place,
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            }],
        );
        assert!(exact_parameter(&machine, place).is_some());

        // A place row declaring a different position desyncs the pair.
        machine.structural_places[0].kind = StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        };
        assert_eq!(exact_parameter(&machine, place), None);

        // Missing place row or a second parameter on the place fail closed.
        machine.structural_places.clear();
        assert_eq!(exact_parameter(&machine, place), None);
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        });
        machine.structural_parameters.push(structural_parameter(
            20,
            1,
            StructuralMultiplicity::Affine,
            StructuralAccess::MutableBorrow,
        ));
        assert_eq!(exact_parameter(&machine, place), None);
    }

    #[test]
    fn empty_structural_custody_rejects_claims_and_qualifications() {
        let place = PlaceId::new(20).unwrap();
        let mut machine = machine(
            Vec::new(),
            vec![structural_parameter(
                20,
                0,
                StructuralMultiplicity::Affine,
                StructuralAccess::MutableBorrow,
            )],
            Vec::new(),
        );
        assert!(has_empty_structural_custody(&machine, place));
        machine.structural_parameters[0].qualifications =
            vec![semantic_vocabulary::StructuralDomainId::new(30).unwrap()];
        assert!(!has_empty_structural_custody(&machine, place));
        machine.structural_parameters[0].qualifications.clear();
        machine.entry_claims.push(terminal_psi::EntryClaim {
            claim: semantic_vocabulary::ClaimId::new(31).unwrap(),
            input: place,
            path: Vec::new(),
        });
        assert!(!has_empty_structural_custody(&machine, place));
    }

    #[test]
    fn direct_relevant_scalar_field_maps_declared_types_once() {
        let types = vec![record_type(
            1,
            vec![
                field(10, "count", StructuralFieldType::Scalar(i32_type())),
                field(
                    11,
                    "bounded",
                    StructuralFieldType::BoundedInteger(
                        semantic_vocabulary::BoundedIntegerType::new(
                            IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                            semantic_vocabulary::IntegerValue::Unsigned(0),
                            semantic_vocabulary::IntegerValue::Unsigned(200),
                        )
                        .unwrap(),
                    ),
                ),
                field(
                    12,
                    "nested",
                    StructuralFieldType::Structural(StructuralTypeId::new(9).unwrap()),
                ),
            ],
        )];
        let root = StructuralTypeId::new(1).unwrap();
        assert_eq!(
            direct_relevant_scalar_field(&types, root, StructuralFieldId::new(10).unwrap(), false),
            Some(i32_type())
        );
        // Bounded integers are only readable once their range obligation is
        // reconstructed; the flag is the admission, not a lookup detail.
        assert_eq!(
            direct_relevant_scalar_field(&types, root, StructuralFieldId::new(11).unwrap(), false),
            None
        );
        assert_eq!(
            direct_relevant_scalar_field(&types, root, StructuralFieldId::new(11).unwrap(), true),
            Some(ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 8).unwrap()
            ))
        );
        assert_eq!(
            direct_relevant_scalar_field(&types, root, StructuralFieldId::new(12).unwrap(), true),
            None
        );
        assert_eq!(
            direct_relevant_scalar_field(&types, root, StructuralFieldId::new(13).unwrap(), true),
            None
        );
    }

    #[test]
    fn case_qualified_reads_resolve_only_the_named_case_payload() {
        use semantic_vocabulary::{CanonicalStructuralPathSegment as Segment, StructuralCaseId};
        let u8_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
        let sum = StructuralTypeId::new(2).unwrap();
        let types = vec![
            record_type(
                1,
                vec![field(5, "mode", StructuralFieldType::Structural(sum))],
            ),
            StructuralTypeDeclaration {
                id: sum,
                identity: "test::Mode".into(),
                shape: StructuralTypeShape::Sum {
                    cases: vec![
                        terminal_psi::StructuralCaseDeclaration {
                            id: StructuralCaseId::new(1).unwrap(),
                            identity: "Idle".into(),
                            fields: vec![field(
                                1,
                                "level",
                                StructuralFieldType::Scalar(i32_type()),
                            )],
                        },
                        terminal_psi::StructuralCaseDeclaration {
                            id: StructuralCaseId::new(2).unwrap(),
                            identity: "Run".into(),
                            fields: vec![field(1, "speed", StructuralFieldType::Scalar(u8_type))],
                        },
                    ],
                },
            },
        ];
        let root = StructuralTypeId::new(1).unwrap();
        let path = |case| {
            vec![
                Segment::Field(StructuralFieldId::new(5).unwrap()),
                Segment::Case(StructuralCaseId::new(case).unwrap()),
            ]
        };
        let first = StructuralFieldId::new(1).unwrap();
        // Both cases declare field 1; the case step alone selects its type.
        assert_eq!(
            observed_scalar_field(&types, root, &path(1), first),
            Some(i32_type())
        );
        assert_eq!(
            observed_scalar_field(&types, root, &path(2), first),
            Some(u8_type)
        );
        // An undeclared case, an absent payload field, and a case step that
        // is not final all fail closed.
        assert_eq!(observed_scalar_field(&types, root, &path(3), first), None);
        assert_eq!(
            observed_scalar_field(&types, root, &path(1), StructuralFieldId::new(2).unwrap()),
            None
        );
        let mut trailing = path(1);
        trailing.push(Segment::Field(first));
        assert_eq!(observed_scalar_field(&types, root, &trailing, first), None);
        // Without the case step the sum is no record carrier.
        assert_eq!(
            observed_scalar_field(&types, root, &path(1)[..1], first),
            None
        );
    }

    /// One owned record arrival at block 4 (not the entry, block 2) whose
    /// field 1 a store writes from a value defined earlier in that block.
    fn owned_arrival_store(
        access: StructuralAccess,
        multiplicity: StructuralMultiplicity,
    ) -> (TerminalMachine, Vec<StructuralTypeDeclaration>, Operation) {
        let place = PlaceId::new(20).unwrap();
        let store = Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: OperationId::new(11).unwrap(),
            result: OperationResult::Unit,
            kind: OperationKind::StructuralScalarFieldStore {
                destination: place,
                path: Vec::new(),
                field: StructuralFieldId::new(1).unwrap(),
                value: ValueId::new(5).unwrap(),
                range_obligation: None,
            },
        };
        let mut arrival = test_block(vec![producer(10, 5), store.clone()]);
        arrival.id = BlockId::new(4).unwrap();
        arrival
            .structural_parameters
            .push(structural_parameter(20, 0, multiplicity, access));
        let mut machine = machine(
            Vec::new(),
            Vec::new(),
            vec![StructuralPlaceDeclaration {
                id: place,
                kind: StructuralPlaceKind::BlockParameter {
                    block: BlockId::new(4).unwrap(),
                    position: 0,
                },
            }],
        );
        machine.blocks = vec![test_block(Vec::new()), arrival];
        let types = vec![record_type(
            1,
            vec![field(1, "total", StructuralFieldType::Scalar(i32_type()))],
        )];
        (machine, types, store)
    }

    fn lower_arrival_store(
        machine: &TerminalMachine,
        types: &[StructuralTypeDeclaration],
        store: &Operation,
    ) -> Result<abstract_operations::AbstractOperation, crate::lowering::LoweringError> {
        super::lower(store, &machine.blocks[1], machine, types)
    }

    fn mutable_view_arrival_store(
        selector: StructuralPathSegment,
    ) -> (TerminalMachine, Vec<StructuralTypeDeclaration>, Operation) {
        let (mut machine, mut types, mut store) = owned_arrival_store(
            StructuralAccess::MutableBorrow,
            StructuralMultiplicity::Unrestricted,
        );
        let view_type = StructuralTypeId::new(2).unwrap();
        types.push(StructuralTypeDeclaration {
            id: view_type,
            identity: "test::Counters".into(),
            shape: StructuralTypeShape::ElementView {
                element: StructuralTypeId::new(1).unwrap(),
            },
        });
        machine.blocks[1].structural_parameters[0].structural_type = view_type;
        if let Some((value, _)) = selector.runtime_index() {
            machine.blocks[1].parameters.push(ValueDeclaration {
                id: value,
                scalar_type: i32_type(),
                qualifications: Default::default(),
            });
        }
        let OperationKind::StructuralScalarFieldStore { path, .. } = &mut store.kind else {
            panic!("the fixture constructs a scalar field store");
        };
        *path = vec![selector];
        machine.blocks[1].operations[1] = store.clone();
        (machine, types, store)
    }

    #[test]
    fn mutable_view_arrival_store_preserves_descriptor_and_selector() {
        for selector in [
            StructuralPathSegment::FixedIndex(0),
            StructuralPathSegment::RuntimeIndex {
                index: ValueId::new(6).unwrap(),
                obligation: semantic_vocabulary::ObligationId::new(12).unwrap(),
            },
        ] {
            let (machine, types, store) = mutable_view_arrival_store(selector.clone());
            let lowered = lower_arrival_store(&machine, &types, &store).unwrap();
            let abstract_operations::AbstractOperation::StructuralScalarFieldStore {
                destination,
                path,
                ..
            } = lowered
            else {
                panic!("a field store lowers to a field store");
            };
            assert_eq!(
                destination,
                abstract_operations::StructuralScalarStoreDestination::Parameter(
                    machine.blocks[1].structural_parameters[0].clone()
                )
            );
            assert_eq!(path, vec![selector]);
        }
    }

    #[test]
    fn mutable_view_arrival_store_rejects_lost_authority_and_custody() {
        let (original, types, store) =
            mutable_view_arrival_store(StructuralPathSegment::FixedIndex(0));
        let refused = |machine: &TerminalMachine| {
            assert!(matches!(
                lower_arrival_store(machine, &types, &store),
                Err(crate::lowering::LoweringError::InvalidStructuralScalarFieldStore(_))
            ));
        };
        let mut changed = original.clone();
        changed.blocks[1].structural_parameters[0].access = StructuralAccess::SharedBorrow;
        refused(&changed);
        let mut changed = original.clone();
        changed.blocks[1].structural_parameters[0].multiplicity = StructuralMultiplicity::Linear;
        refused(&changed);
        let mut changed = original.clone();
        changed.blocks[1].structural_parameters[0].qualifications =
            vec![semantic_vocabulary::StructuralDomainId::new(30).unwrap()];
        refused(&changed);
        let mut changed = original.clone();
        changed.blocks[1].structural_parameters[0].projected_qualifications =
            vec![terminal_psi::StructuralPathQualification {
                path: vec![StructuralPathSegment::FixedIndex(0)],
                domain: semantic_vocabulary::StructuralDomainId::new(30).unwrap(),
            }];
        refused(&changed);
        let mut changed = original;
        changed.entry_claims.push(terminal_psi::EntryClaim {
            claim: semantic_vocabulary::ClaimId::new(31).unwrap(),
            input: PlaceId::new(20).unwrap(),
            path: Vec::new(),
        });
        refused(&changed);
    }

    #[test]
    fn mutable_view_arrival_store_rejects_substituted_or_ambiguous_identity() {
        let (original, types, store) =
            mutable_view_arrival_store(StructuralPathSegment::FixedIndex(0));
        let refused = |machine: &TerminalMachine| {
            assert!(matches!(
                lower_arrival_store(machine, &types, &store),
                Err(crate::lowering::LoweringError::InvalidStructuralScalarFieldStore(_))
            ));
        };
        let mut changed = original.clone();
        changed.structural_places[0].kind = StructuralPlaceKind::BlockParameter {
            block: BlockId::new(2).unwrap(),
            position: 0,
        };
        refused(&changed);
        let mut changed = original.clone();
        changed.structural_places[0].kind = StructuralPlaceKind::BlockParameter {
            block: BlockId::new(4).unwrap(),
            position: 1,
        };
        refused(&changed);
        let mut changed = original.clone();
        changed
            .structural_places
            .push(changed.structural_places[0].clone());
        refused(&changed);
        let mut changed = original.clone();
        let duplicate = changed.blocks[1].structural_parameters[0].clone();
        changed.blocks[1].structural_parameters.push(duplicate);
        refused(&changed);
        let mut changed = original;
        changed.blocks[1].structural_parameters[0].is_self = true;
        refused(&changed);
    }

    #[test]
    fn owned_block_arrival_is_its_own_store_destination() {
        for multiplicity in [
            StructuralMultiplicity::Affine,
            StructuralMultiplicity::Unrestricted,
        ] {
            let (machine, types, store) =
                owned_arrival_store(StructuralAccess::Owned, multiplicity);
            let lowered = lower_arrival_store(&machine, &types, &store).unwrap();
            let abstract_operations::AbstractOperation::StructuralScalarFieldStore {
                destination,
                ..
            } = lowered
            else {
                panic!("a field store lowers to a field store");
            };
            // The arrival keeps its block as establishment identity and its
            // exact declaration; it never becomes a loan.
            assert_eq!(
                destination,
                abstract_operations::StructuralScalarStoreDestination::OwnedBlockParameter {
                    block: BlockId::new(4).unwrap(),
                    parameter: machine.blocks[1].structural_parameters[0].clone(),
                }
            );
            assert!(destination.parameter().is_none());
        }
    }

    #[test]
    fn shared_linear_and_claimed_arrivals_are_not_store_destinations() {
        let refused = |machine: &TerminalMachine, types: &[StructuralTypeDeclaration], store| {
            assert!(matches!(
                lower_arrival_store(machine, types, store),
                Err(crate::lowering::LoweringError::InvalidStructuralScalarFieldStore(_))
            ));
        };
        // A shared loan grants no write, and a linear arrival is not plain
        // record storage.
        let (machine, types, store) = owned_arrival_store(
            StructuralAccess::SharedBorrow,
            StructuralMultiplicity::Affine,
        );
        refused(&machine, &types, &store);
        let (machine, types, store) =
            owned_arrival_store(StructuralAccess::Owned, StructuralMultiplicity::Linear);
        refused(&machine, &types, &store);
        // Qualified or claimed storage carries custody this write would break.
        let (mut machine, types, store) =
            owned_arrival_store(StructuralAccess::Owned, StructuralMultiplicity::Affine);
        machine.blocks[1].structural_parameters[0].qualifications =
            vec![semantic_vocabulary::StructuralDomainId::new(30).unwrap()];
        refused(&machine, &types, &store);
        let (mut machine, types, store) =
            owned_arrival_store(StructuralAccess::Owned, StructuralMultiplicity::Affine);
        machine.entry_claims.push(terminal_psi::EntryClaim {
            claim: semantic_vocabulary::ClaimId::new(31).unwrap(),
            input: PlaceId::new(20).unwrap(),
            path: Vec::new(),
        });
        refused(&machine, &types, &store);
    }

    #[test]
    fn copied_entry_parameters_and_substituted_places_are_not_store_destinations() {
        let refused = |machine: &TerminalMachine, types: &[StructuralTypeDeclaration], store| {
            assert!(matches!(
                lower_arrival_store(machine, types, store),
                Err(crate::lowering::LoweringError::InvalidStructuralScalarFieldStore(_))
            ));
        };
        // An unrestricted record copied in by value is the caller's ABI
        // placement, not a block home this route writes.
        let (mut machine, types, store) = owned_arrival_store(
            StructuralAccess::Owned,
            StructuralMultiplicity::Unrestricted,
        );
        let copied = machine.blocks[1].structural_parameters.remove(0);
        machine.structural_parameters.push(copied);
        machine.structural_places[0].kind = StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        };
        refused(&machine, &types, &store);
        // A place row naming another block, or another position, does not
        // describe this arrival.
        let (mut machine, types, store) =
            owned_arrival_store(StructuralAccess::Owned, StructuralMultiplicity::Affine);
        machine.structural_places[0].kind = StructuralPlaceKind::BlockParameter {
            block: BlockId::new(2).unwrap(),
            position: 0,
        };
        refused(&machine, &types, &store);
        let (mut machine, types, store) =
            owned_arrival_store(StructuralAccess::Owned, StructuralMultiplicity::Affine);
        machine.structural_places[0].kind = StructuralPlaceKind::BlockParameter {
            block: BlockId::new(4).unwrap(),
            position: 1,
        };
        refused(&machine, &types, &store);
        // An arrival declared on the entry block has no edge that moved a
        // value into it.
        let (mut machine, types, store) =
            owned_arrival_store(StructuralAccess::Owned, StructuralMultiplicity::Affine);
        machine.entry = BlockId::new(4).unwrap();
        refused(&machine, &types, &store);
    }
}
