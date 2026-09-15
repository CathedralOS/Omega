use crate::{
    ExternalBindingId, ExternalBindingMechanism, MachineSupplyMode, MachineTerminationPlan,
    RankingViewId, RankingWitness, TerminationGuarantee, TerminationInterface,
};

#[test]
fn machine_supply_queries_keep_checked_and_boundary_tiers_distinct() {
    assert!(MachineSupplyMode::CheckedBody.is_checked_body());
    assert!(!MachineSupplyMode::CheckedBody.is_boundary_declaration());

    assert!(MachineSupplyMode::Boundary.is_boundary_declaration());
    assert!(MachineSupplyMode::AdmissionClaim.is_boundary_declaration());
    assert!(!MachineSupplyMode::AdmissionClaim.is_checked_body());
    assert!(MachineSupplyMode::TopLevelRequirement.is_boundary_declaration());
    assert!(!MachineSupplyMode::TopLevelRequirement.is_checked_body());

    for mode in [
        MachineSupplyMode::Requirement,
        MachineSupplyMode::ExternalRealization {
            binding: Some(ExternalBindingId(1)),
            mechanism: Some(ExternalBindingMechanism::CompilerIntrinsic),
        },
    ] {
        assert!(!mode.is_checked_body());
        assert!(!mode.is_boundary_declaration());
    }
}

#[test]
fn termination_guarantee_default_is_no_guarantee() {
    // Exported omission normalizes to NoGuarantee — never an implied
    // promise.
    assert_eq!(
        TerminationGuarantee::default(),
        TerminationGuarantee::NoGuarantee
    );
}

#[test]
fn witness_stays_out_of_the_published_half() {
    // The plan SHAPE enforces the firewall: the witness lives beside
    // the published guarantee, never inside it — equality of two plans'
    // published halves is witness-blind by construction.
    let with_witness = MachineTerminationPlan {
        interface: TerminationInterface::Published(TerminationGuarantee::NoGuarantee),
        checked_summary: TerminationGuarantee::NoGuarantee,
        implementation_witness: Some(RankingWitness::default()),
    };
    let without_witness = MachineTerminationPlan {
        implementation_witness: None,
        ..with_witness.clone()
    };
    assert_eq!(with_witness.interface, without_witness.interface);
}

#[test]
fn canonical_view_catalog_round_trips() {
    // Fixed, deterministic ids: the catalog may enter proof-cache keys,
    // so a builtin's id and spelling must round-trip exactly.
    for id in [
        RankingViewId::NAT_DESCENDING,
        RankingViewId::NAT_BOUNDED_DISTANCE,
        RankingViewId::SLICE_LENGTH,
        RankingViewId::NAT_INCREASING_TO,
    ] {
        assert!(id.is_valid());
        let path = id.canonical_path().expect("builtin has a spelling");
        assert_eq!(RankingViewId::canonical(path), Some(id));
    }
    // Declared measures are NOT canonical builtins.
    assert_eq!(RankingViewId::canonical("Card::PowerOrder"), None);
    assert_eq!(RankingViewId::NULL.canonical_path(), None);
}
