use super::super::module_wire::encode_raw;
use super::super::target_family_tests::{identity_table, unit_machine};
use crate::{decode_module, encode_module, semantic_fingerprint};
use semantic_vocabulary::{
    IntegerSign, IntegerType, MachineId, PlaceId, ScalarType, StructuralPlaceKind,
    StructuralTypeId, ValueId,
};
use terminal_psi::{
    BoundaryParameterKind::{Scalar, Structural},
    StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralRecordStorage, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalIdentityIndex, TerminalModule, ValueDeclaration,
};

fn module() -> TerminalModule {
    let identity = MachineId::new(1).unwrap();
    let mut module = TerminalModule::for_entry(identity);
    module.identities = identity_table(4);
    let mut machine = unit_machine(identity);
    machine.declaration.as_mut().unwrap().parameter_types = vec![TerminalIdentityIndex(3); 4];
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap());
    machine.parameters = [1, 2]
        .map(|value| ValueDeclaration {
            id: ValueId::new(value).unwrap(),
            scalar_type,
            qualifications: Default::default(),
        })
        .to_vec();
    let place = PlaceId::new(1).unwrap();
    let structural_type = StructuralTypeId::new(1).unwrap();
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place,
            position: 0,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::Parameter { position: 0 },
    });
    module.structural_types.push(StructuralTypeDeclaration {
        quotient_carrier: None,
        owns_linear_claim: false,
        no_code_affine_cleanup: false,
        descriptor_cleanup: false,
        copyable: true,
        scalar_restriction: None,
        id: structural_type,
        identity: TerminalIdentityIndex(3),
        shape: StructuralTypeShape::Record {
            fields: Vec::new(),
            storage: StructuralRecordStorage::Packed,
        },
    });
    module.native_layouts.insert(
        structural_type,
        TerminalIdentityIndex(3),
        TerminalIdentityIndex::ABSENT,
        TerminalIdentityIndex::ABSENT,
        [],
        [],
    );
    machine.parameter_order = vec![Scalar, Structural, Scalar];
    module.machines.push(machine);
    module
}

#[test]
fn machine_parameter_order_round_trip_and_bind_identity() {
    let original = module();
    let bytes = encode_module(&original).unwrap();
    let decoded = decode_module(&bytes).unwrap();
    assert_eq!(decoded, original);
    assert_eq!(
        decoded.machines[0].runtime_parameter_formals().unwrap(),
        [0, 2, 1]
    );
    let mut different = original.clone();
    different.machines[0].parameter_order = vec![Structural, Scalar, Scalar];
    assert_eq!(
        different.machines[0].runtime_parameter_formals().unwrap(),
        [1, 2, 0]
    );
    assert_ne!(
        semantic_fingerprint(&original).unwrap(),
        semantic_fingerprint(&different).unwrap()
    );
    assert_eq!(
        decode_module(&encode_module(&different).unwrap()).unwrap(),
        different
    );
}

#[test]
fn machine_parameter_order_rejects_missing_extra_and_wrong_lane() {
    for order in [
        vec![],
        vec![Scalar, Structural],
        vec![Scalar; 3],
        vec![Scalar, Structural, Structural],
        vec![Scalar, Structural, Scalar, Scalar],
    ] {
        let mut invalid = module();
        invalid.machines[0].parameter_order = order;
        assert!(invalid.machines[0].validate_parameter_order().is_err());
        assert!(decode_module(&encode_raw(&invalid).unwrap()).is_err());
    }
}

#[test]
fn machine_parameter_order_has_no_erased_formal_lane() {
    let original = module();
    // The checked declaration has four source formals but only three runtime
    // parameters. No ABI position is reserved for the erased formal.
    assert_eq!(
        original.machines[0]
            .declaration
            .as_ref()
            .unwrap()
            .parameter_types
            .len(),
        4
    );
    assert_eq!(original.machines[0].parameter_order.len(), 3);
    assert_eq!(
        original.machines[0].runtime_parameter_formals().unwrap(),
        [0, 2, 1]
    );
}

fn rank_tuple_fixture() -> terminal_psi::TerminalRankedScc {
    use terminal_psi::{
        TerminalBlockNaturalRank, TerminalIntegerRankValue, TerminalNaturalCycle,
        TerminalNaturalRankComparison, TerminalNaturalRankEdge, TerminalRankedScc,
    };
    let narrow = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let wide = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let value = |integer_type, identity| TerminalIntegerRankValue {
        integer_type,
        value: ValueId::new(identity).unwrap(),
    };
    let mut observations = arena::Arena::new();
    // Allocation history is not a portable coordinate. The codec copies only
    // the complete typed rows and reconstructs its own arena ownership.
    observations.insert(value(wide, 999));
    let header = observations.insert_many([value(narrow, 1), value(wide, 2)]);
    let arrival = observations.insert_many([value(narrow, 3), value(wide, 4)]);
    let block = semantic_vocabulary::BlockId::new(1).unwrap();
    TerminalRankedScc::Natural(vec![TerminalNaturalCycle {
        observations,
        ranks: vec![TerminalBlockNaturalRank {
            block,
            values: header,
        }],
        edges: vec![TerminalNaturalRankEdge {
            edge: semantic_vocabulary::EdgeId::new(1).unwrap(),
            source: block,
            target: block,
            successor_ranks: arrival,
            comparison: TerminalNaturalRankComparison::Strict,
        }],
    }])
}

#[test]
fn rank_tuple_wire_retains_order_types_and_ssa_values_without_arena_coordinates() {
    let original = rank_tuple_fixture();
    let mut writer = super::Writer::default();
    super::encode_ranked_scc(&mut writer, Some(&original)).unwrap();
    let bytes = writer.finish();
    assert_eq!(bytes[0], 3);
    assert_eq!(
        super::decode_ranked_scc(&mut super::Reader::new(&bytes)).unwrap(),
        Some(original)
    );
    assert_eq!(
        super::decode_ranked_scc(&mut super::Reader::new(&[2])),
        Err(crate::CodecError::InvalidTag("TerminalRankedScc", 2))
    );
}

#[test]
fn rank_tuple_wire_rejects_a_dropped_successor_suffix() {
    let mut original = rank_tuple_fixture();
    let terminal_psi::TerminalRankedScc::Natural(components) = &mut original;
    let component = &mut components[0];
    let first = component
        .rank_values(component.edges[0].successor_ranks)
        .unwrap()[0];
    component.edges[0].successor_ranks = component.observations.insert_many([first]);
    let mut writer = super::Writer::default();
    assert_eq!(
        super::encode_ranked_scc(&mut writer, Some(&original)),
        Err(crate::CodecError::MalformedRanking(
            "incomplete or differently typed rank tuple"
        ))
    );

    // A foreign producer need not use our writer. Pin the decoder's own arity
    // check with a complete byte row whose successor has lost one component.
    let mut writer = super::Writer::default();
    writer.u8(3);
    writer.len("components", 1).unwrap();
    writer.len("blocks", 1).unwrap();
    writer.id(semantic_vocabulary::BlockId::new(1).unwrap());
    writer.len("rank", 2).unwrap();
    for identity in [1, 2] {
        super::encode_integer_type(
            &mut writer,
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        );
        writer.id(ValueId::new(identity).unwrap());
    }
    writer.len("edges", 1).unwrap();
    writer.id(semantic_vocabulary::EdgeId::new(1).unwrap());
    writer.id(semantic_vocabulary::BlockId::new(1).unwrap());
    writer.id(semantic_vocabulary::BlockId::new(1).unwrap());
    writer.len("successor", 1).unwrap();
    super::encode_integer_type(
        &mut writer,
        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
    );
    writer.id(ValueId::new(3).unwrap());
    writer.u8(2);
    let bytes = writer.finish();
    assert_eq!(
        super::decode_ranked_scc(&mut super::Reader::new(&bytes)),
        Err(crate::CodecError::MalformedRanking(
            "incomplete or differently typed rank tuple"
        ))
    );
}

#[test]
fn rank_tuple_wire_rejects_reordered_differently_typed_components() {
    let mut original = rank_tuple_fixture();
    let terminal_psi::TerminalRankedScc::Natural(components) = &mut original;
    let component = &mut components[0];
    let mut values = component
        .rank_values(component.edges[0].successor_ranks)
        .unwrap()
        .to_vec();
    values.swap(0, 1);
    component.edges[0].successor_ranks = component.observations.insert_many(values);
    let mut writer = super::Writer::default();
    assert_eq!(
        super::encode_ranked_scc(&mut writer, Some(&original)),
        Err(crate::CodecError::MalformedRanking(
            "incomplete or differently typed rank tuple"
        ))
    );
}
