use semantic_vocabulary::ServiceId;
use terminal_codec::PccDependency;
use terminal_psi::{InstallationReachDependency, ServiceDeclaration};

#[test]
fn installation_dependency_commitments_follow_service_meaning_not_local_numbers() {
    let service = ServiceId::new(1).unwrap();
    let mut dependency = InstallationReachDependency {
        requirement_identity: "test::observe".into(),
        upper_bound: vec![service],
    };
    let mut services = vec![ServiceDeclaration {
        id: service,
        identity: "Console".into(),
        parents: Vec::new(),
    }];
    let original = PccDependency::from_installation_reach(&dependency, &services).unwrap();
    services[0].identity = "Filesystem".into();
    assert_ne!(
        original,
        PccDependency::from_installation_reach(&dependency, &services).unwrap()
    );
    services[0].identity = "Console".into();
    services[0].id = ServiceId::new(20).unwrap();
    dependency.upper_bound[0] = services[0].id;
    assert_eq!(
        original,
        PccDependency::from_installation_reach(&dependency, &services).unwrap()
    );
    assert!(PccDependency::from_installation_reach(&dependency, &[]).is_err());
}
