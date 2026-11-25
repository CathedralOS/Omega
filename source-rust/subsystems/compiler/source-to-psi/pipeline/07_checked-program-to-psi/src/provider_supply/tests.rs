use super::*;
use crate::DeclaredRequirementApplication;
use crate::provider_choice_inputs::{ProviderApplicationId, ProviderSelectionIdentity};
use symbols::SymbolHandle;

#[test]
fn a_provider_for_one_receiver_does_not_supply_another_receiver_or_the_template() {
    let slot = SymbolHandle::from_parts(1, 1);
    let template = SymbolHandle::from_parts(2, 1);
    let reached = |instance, application| ReachedBoundaryRequirement {
        boundary: BoundaryMachineId::new(1).unwrap(),
        requirement: SymbolHandle::from_parts(instance, 1),
        slot,
        name: "Reading::total".into(),
        application,
    };
    let closed = |receiver: &str, callable: &str| {
        Some(DeclaredRequirementApplication {
            template,
            receiving_trait: receiver.into(),
            callable: callable.into(),
        })
    };
    let first = reached(3, closed("Reading<17>", "Reading<17>::total"));
    let second = reached(4, closed("Reading<29>", "Reading<29>::total"));
    let open = reached(2, None);
    let applied = |symbol, identity: &str| ProviderSelectionIdentity {
        symbol,
        application: ProviderApplicationId::from_parts(1, 1),
        applied: true,
        application_identity: Some(identity.into()),
        package: None,
        canonical_path: String::new(),
        authored_path: String::new(),
    };
    for subject in [
        ProviderSelectionSubject::BoundaryTrait(applied(slot, "Reading<17>")),
        ProviderSelectionSubject::BoundaryRequirement(applied(template, "Reading<17>::total")),
    ] {
        assert!(names_requirement(&subject, &first));
        assert!(!names_requirement(&subject, &second));
        assert!(!names_requirement(&subject, &open));
    }
}
