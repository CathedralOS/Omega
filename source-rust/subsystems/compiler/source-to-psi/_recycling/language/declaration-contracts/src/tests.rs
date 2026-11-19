/*
We verify carry policies, external bindings, machine termination plans, semantic
domains, service reach tables, array extent identities, and canonical type
identity encoding.
*/

mod carry_policies {
    use crate::{
        CarryAddress, CarryCpu, CarryHostThread, CarryPermission, CarryPolicy, CarrySuspension,
    };

    #[test]
    fn carry_axes_compose_independently_and_fail_closed() {
        let cpu_local = CarryPolicy {
            suspension: CarrySuspension::Allowed,
            cpu: CarryCpu::Origin,
            host_thread: CarryHostThread::Any,
            address: CarryAddress::Movable,
        };
        let pinned = CarryPolicy {
            suspension: CarrySuspension::Allowed,
            cpu: CarryCpu::Any,
            host_thread: CarryHostThread::Any,
            address: CarryAddress::Stable,
        };

        assert_eq!(
            cpu_local.intersect(pinned),
            CarryPolicy {
                suspension: CarrySuspension::Allowed,
                cpu: CarryCpu::Origin,
                host_thread: CarryHostThread::Any,
                address: CarryAddress::Stable,
            }
        );
        assert!(CarryPolicy::PERMISSIVE.permits(cpu_local));
        assert!(!CarryPolicy::STRICT.permits(cpu_local));
        assert_eq!(CarryPolicy::default(), CarryPolicy::STRICT);
    }

    #[test]
    fn carry_permissions_are_closed_named_positive_relaxations() {
        let mut policy = CarryPolicy::STRICT;
        for permission in CarryPermission::ALL {
            assert_eq!(
                CarryPermission::from_name(permission.name()),
                Some(permission)
            );
            policy = permission.relax(policy);
        }
        assert_eq!(policy, CarryPolicy::PERMISSIVE);
        assert_eq!(CarryPermission::from_name("Carry::Portable"), None);
        assert_eq!(CarryPermission::from_name("Carry::Anywhere"), None);
    }
}

mod semantic_domains {
    use crate::{DomainEstablishmentRoute, DomainPredicatePresence, QualificationEvidenceOrigin};

    #[test]
    fn establishment_routes_keep_source_and_requirement_identity_independent() {
        let checked_trait = symbols::SymbolHandle::from_arena_index(9);
        let checked_requirement = symbols::SymbolHandle::from_arena_index(10);
        let checked = DomainEstablishmentRoute::CheckedRequirement {
            trait_definition: checked_trait,
            requirement: checked_requirement,
        };
        assert_eq!(checked.kind_name(), "checked_requirement");
        assert_eq!(checked.source_symbol(), checked_trait);
        assert_eq!(checked.requirement_symbol(), checked_requirement);

        let boundary_trait = symbols::SymbolHandle::from_arena_index(11);
        let requirement = symbols::SymbolHandle::from_arena_index(12);
        let route = DomainEstablishmentRoute::BoundaryRequirement {
            boundary_trait,
            requirement,
        };
        assert_eq!(route.kind_name(), "boundary_requirement");
        assert_eq!(route.source_symbol(), boundary_trait);
        assert_eq!(route.requirement_symbol(), requirement);
    }

    #[test]
    fn domain_predicate_presence_distinguishes_bodyless_from_explicit_predicates() {
        assert_eq!(
            DomainPredicatePresence::default(),
            DomainPredicatePresence::Absent
        );
        assert!(!DomainPredicatePresence::Absent.is_present());
        assert!(DomainPredicatePresence::Present.is_present());
        assert_eq!(DomainPredicatePresence::Absent.as_str(), "bodyless");
        assert_eq!(DomainPredicatePresence::Present.as_str(), "present");
    }

    #[test]
    fn qualification_evidence_origins_have_stable_public_names() {
        use QualificationEvidenceOrigin as Origin;

        assert_eq!(Origin::default(), Origin::None);
        assert_eq!(Origin::Prover.as_str(), "prover");
        assert_eq!(Origin::CheckedValidation.as_str(), "checked_validation");
        assert_eq!(
            Origin::AuthorizedRouteEstablishment.as_str(),
            "authorized_route_establishment"
        );
        assert_eq!(
            Origin::CheckedTransformation.as_str(),
            "checked_transformation"
        );
        assert_eq!(Origin::AdmittedReceipt.as_str(), "admitted_receipt");
        assert_eq!(Origin::Propagated.as_str(), "propagated");
        assert_eq!(
            Origin::VacuousQualification.as_str(),
            "vacuous_qualification"
        );
    }
}
