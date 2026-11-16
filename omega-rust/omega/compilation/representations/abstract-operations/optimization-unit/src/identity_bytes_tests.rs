use super::{access_view_catalog_identity_bytes, scalar_refinement_identity_bytes};
use semantic_vocabulary::{
    IeeeFloatFormat, IntegerSign, IntegerType, MachineId, Proposition, ScalarRefinement,
    ScalarTerm, ScalarType, ValueId, WidthParameter,
};
use terminal_psi::{StructuralAccess, TerminalAccessViewInput, TerminalIdentityIndex};

#[test]
fn scalar_and_access_catalog_bytes_keep_the_existing_encoding() {
    for (carrier, expected) in [
        (ScalarType::Boolean, vec![1, 1]),
        (
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
            vec![3, 1, 1],
        ),
        (
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
            vec![3, 2, 1],
        ),
        (
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 13).unwrap()),
            vec![2, 1, 13, 0, 1],
        ),
        (
            ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap()),
            vec![2, 2, 64, 0, 1],
        ),
    ] {
        let refinement = ScalarRefinement::new(carrier, Proposition::Truth).unwrap();
        assert_eq!(scalar_refinement_identity_bytes(&refinement), expected);
    }
    let nested = ScalarRefinement::new(
        ScalarType::Boolean,
        Proposition::Conjunction(vec![
            Proposition::Truth,
            Proposition::Equal(ScalarTerm::Boolean(true), ScalarTerm::Boolean(false)),
        ]),
    )
    .unwrap();
    assert_eq!(
        scalar_refinement_identity_bytes(&nested),
        [1, 7, 2, 0, 0, 0, 0, 0, 0, 0, 1, 4, 2, 1, 2, 0],
    );
    let symbolic = IntegerType::parameterized(
        IntegerSign::Unsigned,
        WidthParameter {
            owner: MachineId::new(29).unwrap(),
            value: ValueId::new(31).unwrap(),
        },
    );
    let mut encoded = super::CanonicalBytes::collect();
    super::carrier_encoding::encode_scalar_type(&mut encoded, ScalarType::Integer(symbolic));
    let mut expected = vec![2, 1, 255, 255];
    expected.extend_from_slice(&29_u64.to_le_bytes());
    expected.extend_from_slice(&31_u64.to_le_bytes());
    assert_eq!(encoded.finish(), expected);

    let catalog =
        abstract_operations::StructuralTypeCatalog::default().with_access_view_inputs(vec![
            TerminalAccessViewInput {
                machine: MachineId::new(3).unwrap(),
                position: 5,
                runtime_position: 7,
                carrier_identity: TerminalIdentityIndex(9),
                source_machine_identity: TerminalIdentityIndex(10),
                source_state_identity: TerminalIdentityIndex(11),
                source_parameter_identity: TerminalIdentityIndex(12),
                view_identity: TerminalIdentityIndex(13),
                policy_identity: TerminalIdentityIndex(14),
                policy_plan_machine_identity: TerminalIdentityIndex(15),
                schema_identity: TerminalIdentityIndex(16),
                access: StructuralAccess::SharedBorrow,
                binding_is_const: true,
                binding_is_mutable: false,
                placement_report_fingerprint: 17,
                placement_commitment: [19; 32],
            },
        ]);
    let mut expected = b"placed-formal-demands-v1".to_vec();
    expected.extend_from_slice(&1_u64.to_le_bytes());
    expected.extend_from_slice(&3_u64.to_le_bytes());
    expected.extend_from_slice(&5_u32.to_le_bytes());
    expected.extend_from_slice(&7_u32.to_le_bytes());
    for identity in 9_u32..=16 {
        expected.extend_from_slice(&identity.to_le_bytes());
    }
    expected.extend_from_slice(&[2, 1, 0]);
    expected.extend_from_slice(&17_u64.to_le_bytes());
    expected.extend_from_slice(&[19; 32]);
    assert_eq!(access_view_catalog_identity_bytes(&catalog), expected);
}
