//! Exact structural scalar-field preservation after independent Terminal verification.
//! Bounded reads use the declared integer carrier after constructor/entry range
//! proof reconstruction. A read captures a value; it does not relax the declared
//! invariant, so stores without their own establishment proof remain rejected.

use abstract_operations::{AbstractOperation, AbstractResult};
use semantic_vocabulary::{
    PlaceId, ScalarType, StructuralFieldId, StructuralPlaceKind, StructuralTypeId,
};
use terminal_psi::{
    BindingRelevance, Block, Operation, OperationKind, OperationResult, StructuralAccess,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
    is_bounded_structural_scalar_store_path,
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
        OperationKind::IndexedPrimitiveRead {
            source,
            path,
            index,
            obligation,
        } => lower_indexed_read(
            operation,
            block,
            machine,
            structural_types,
            *source,
            path,
            *index,
            *obligation,
        ),
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
    let destination = exact_parameter(machine, destination).ok_or_else(invalid)?;
    let scalar_type =
        dominating_scalar_type(machine, block, operation.id, value).ok_or_else(invalid)?;
    if operation.result != OperationResult::Unit
        || !matches!(
            destination.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        )
        || !matches!(
            destination.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        )
        || !has_empty_structural_custody(machine, destination.place)
        || !is_bounded_structural_scalar_store_path(path)
    {
        return Err(invalid());
    }
    let parent_type = resolve_structural_path(structural_types, destination.structural_type, path)
        .ok_or_else(invalid)?;
    let declaration = structural_types
        .iter()
        .find(|candidate| candidate.id == parent_type)
        .ok_or_else(invalid)?;
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
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

fn lower_integer_read(
    operation: &Operation,
    machine: &TerminalMachine,
    structural_types: &[StructuralTypeDeclaration],
    source: PlaceId,
    path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
    field: StructuralFieldId,
) -> Result<AbstractOperation, LoweringError> {
    let invalid = || LoweringError::InvalidIntegerStructuralField(operation.id);
    let structural_type = readable_source_type(machine, source).ok_or_else(invalid)?;
    let carrier =
        terminal_semantics::record_field_carrier(structural_types.iter(), structural_type, path)
            .ok_or_else(invalid)?;
    let result = operation.result.scalar().ok_or_else(invalid)?;
    if !matches!(result.scalar_type, ScalarType::Integer(_))
        || direct_relevant_scalar_field(structural_types, carrier.structural_type, field, true)
            != Some(result.scalar_type)
    {
        return Err(invalid());
    }
    Ok(AbstractOperation::IntegerStructuralField {
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

/// One element of a fixed-array leaf read at a verified runtime position: the
/// path ends at the array, the result is its element type, and the dominating
/// index is the `u64` position the bounds obligation constrains.
#[allow(clippy::too_many_arguments)]
fn lower_indexed_read(
    operation: &Operation,
    block: &Block,
    machine: &TerminalMachine,
    structural_types: &[StructuralTypeDeclaration],
    source: PlaceId,
    path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
    index: semantic_vocabulary::ValueId,
    obligation: semantic_vocabulary::ObligationId,
) -> Result<AbstractOperation, LoweringError> {
    let invalid = || LoweringError::InvalidIndexedPrimitiveRead(operation.id);
    let structural_type = readable_source_type(machine, source).ok_or_else(invalid)?;
    let (element, _extent) =
        terminal_semantics::fixed_array_place_shape(structural_types.iter(), structural_type, path)
            .ok_or_else(invalid)?;
    let result = operation.result.scalar().ok_or_else(invalid)?;
    let index_type =
        dominating_scalar_type(machine, block, operation.id, index).ok_or_else(invalid)?;
    let unsigned_64 =
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
            .map_err(|_| invalid())?;
    if result.scalar_type != element || index_type != ScalarType::Integer(unsigned_64) {
        return Err(invalid());
    }
    Ok(AbstractOperation::IndexedPrimitiveRead {
        psi_operation: operation.id,
        result: AbstractResult {
            value: result.id,
            scalar_type: result.scalar_type,
        },
        source,
        path: path.to_vec(),
        index: AbstractResult {
            value: index,
            scalar_type: index_type,
        },
        obligation,
    })
}

fn readable_source_type(machine: &TerminalMachine, source: PlaceId) -> Option<StructuralTypeId> {
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
        .then_some(parameter.structural_type);
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
    .then_some(result.structural_type)
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

fn dominating_scalar_type(
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
            _ => return None,
        };
    }
    Some(structural_type)
}

fn direct_relevant_scalar_field(
    structural_types: &[StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
    field: StructuralFieldId,
    allow_bounded_integer: bool,
) -> Option<ScalarType> {
    let declaration = exact_structural_type(structural_types, structural_type)?;
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return None;
    };
    let mut matching = fields.iter().filter(|candidate| {
        candidate.id == field && candidate.relevance == BindingRelevance::Relevant
    });
    let field = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    match &field.field_type {
        StructuralFieldType::Scalar(scalar_type) => Some(*scalar_type),
        StructuralFieldType::IeeeFloat(format) => Some(ScalarType::IeeeFloat(*format)),
        StructuralFieldType::BoundedInteger(bounds) if allow_bounded_integer => {
            Some(ScalarType::Integer(bounds.integer_type()))
        }
        StructuralFieldType::ByteSequence(_)
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
        has_empty_structural_custody, resolve_structural_path,
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
    fn dominating_scalar_type_reads_the_single_entry_declaration() {
        let machine = machine(vec![scalar_parameter(5)], Vec::new(), Vec::new());
        let block = test_block(Vec::new());
        assert_eq!(
            dominating_scalar_type(
                &machine,
                &block,
                OperationId::new(1).unwrap(),
                ValueId::new(5).unwrap(),
            ),
            Some(i32_type())
        );
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
    fn dominating_scalar_type_uses_only_prior_block_definitions() {
        let machine = machine(Vec::new(), Vec::new(), Vec::new());
        // The defining operation precedes the store.
        let block = test_block(vec![producer(3, 5), marker(4)]);
        assert_eq!(
            dominating_scalar_type(
                &machine,
                &block,
                OperationId::new(4).unwrap(),
                ValueId::new(5).unwrap(),
            ),
            Some(i32_type())
        );
        // The definition sits after the queried operation: not yet in scope.
        let block = test_block(vec![marker(2), producer(3, 5)]);
        assert_eq!(
            dominating_scalar_type(
                &machine,
                &block,
                OperationId::new(2).unwrap(),
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
}
