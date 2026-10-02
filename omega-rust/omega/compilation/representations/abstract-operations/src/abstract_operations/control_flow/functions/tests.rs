use super::{AbstractBlockEntry, AbstractFunction};
use crate::AbstractFunctionResult;
use semantic_vocabulary::{BlockId, MachineId, PlaceId, StructuralTypeId};
use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration};

fn parameter(
    place: u64,
    position: u32,
    is_self: bool,
    access: StructuralAccess,
) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: PlaceId::new(place).unwrap(),
        position,
        is_self,
        structural_type: StructuralTypeId::new(1).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

fn block(
    block: u64,
    structural_parameters: Vec<StructuralParameterDeclaration>,
) -> AbstractBlockEntry {
    AbstractBlockEntry {
        block: BlockId::new(block).unwrap(),
        structural_parameters,
        parameters: Vec::new(),
        operation_offset: 0,
    }
}

fn function(
    structural_parameters: Vec<StructuralParameterDeclaration>,
    block_entries: Vec<AbstractBlockEntry>,
) -> AbstractFunction {
    AbstractFunction {
        machine: MachineId::new(1).unwrap(),
        attachment: None,
        entry: BlockId::new(1).unwrap(),
        parameters: Vec::new(),
        structural_parameters,
        result: AbstractFunctionResult::Unit,
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries,
        operations: Vec::new(),
    }
}

#[test]
fn arrival_parameter_rejoins_entry_and_block_declarations_with_their_access() {
    let entry = parameter(1, 0, false, StructuralAccess::WriteOnlyBorrow);
    let arrival = parameter(2, 0, false, StructuralAccess::MutableBorrow);
    let function = function(vec![entry], vec![block(2, vec![arrival])]);

    assert_eq!(
        function
            .arrival_parameter(PlaceId::new(1).unwrap())
            .map(|parameter| parameter.access),
        Some(StructuralAccess::WriteOnlyBorrow)
    );
    assert_eq!(
        function
            .arrival_parameter(PlaceId::new(2).unwrap())
            .map(|parameter| parameter.access),
        Some(StructuralAccess::MutableBorrow)
    );
}

#[test]
fn arrival_parameter_fails_closed_for_duplicate_place_identity() {
    let duplicated = parameter(1, 0, false, StructuralAccess::MutableBorrow);
    let function = function(vec![duplicated.clone()], vec![block(2, vec![duplicated])]);

    assert!(
        function
            .arrival_parameter(PlaceId::new(1).unwrap())
            .is_none()
    );
}

#[test]
fn arrival_parameter_rejects_entry_block_formals() {
    let function = function(
        Vec::new(),
        vec![block(
            1,
            vec![parameter(1, 0, false, StructuralAccess::MutableBorrow)],
        )],
    );

    assert!(
        function
            .arrival_parameter(PlaceId::new(1).unwrap())
            .is_none()
    );
}

#[test]
fn arrival_parameter_rejects_nondense_block_positions() {
    let function = function(
        Vec::new(),
        vec![block(
            2,
            vec![parameter(1, 1, false, StructuralAccess::MutableBorrow)],
        )],
    );

    assert!(
        function
            .arrival_parameter(PlaceId::new(1).unwrap())
            .is_none()
    );
}

#[test]
fn arrival_parameter_rejects_block_receiver_identity() {
    let function = function(
        Vec::new(),
        vec![block(
            2,
            vec![parameter(1, 0, true, StructuralAccess::MutableBorrow)],
        )],
    );

    assert!(
        function
            .arrival_parameter(PlaceId::new(1).unwrap())
            .is_none()
    );
}

#[test]
fn arrival_parameter_rejects_an_unknown_place() {
    let function = function(
        vec![parameter(1, 0, false, StructuralAccess::MutableBorrow)],
        Vec::new(),
    );

    assert!(
        function
            .arrival_parameter(PlaceId::new(2).unwrap())
            .is_none()
    );
}
