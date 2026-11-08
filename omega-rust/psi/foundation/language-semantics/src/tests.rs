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

mod external_bindings {
    use crate::{ExternalBindingId, ExternalBindingIdentity, ExternalBindingTable};

    #[test]
    fn external_binding_interner_uses_structural_identity() {
        let mut bindings = ExternalBindingTable::default();
        let first = bindings.intern(ExternalBindingIdentity::VtableField {
            field: "invoke".to_owned(),
        });
        assert_eq!(
            bindings.intern(ExternalBindingIdentity::VtableField {
                field: "invoke".to_owned(),
            }),
            first,
            "equal structural bindings must share one identity"
        );
        assert_ne!(
            bindings.intern(ExternalBindingIdentity::VtableField {
                field: "invoke_later".to_owned(),
            }),
            first,
            "payload values must remain identity-bearing"
        );
        assert_ne!(
            bindings.intern(ExternalBindingIdentity::TableFunction {
                field: "invoke".to_owned(),
            }),
            first,
            "an equal payload under a different mechanism must not collapse"
        );
        let intrinsic = bindings.intern(ExternalBindingIdentity::CompilerIntrinsic);
        assert_ne!(
            intrinsic, first,
            "mechanism tags must remain identity-bearing"
        );
        assert_eq!(
            bindings.intern(ExternalBindingIdentity::CompilerIntrinsic),
            intrinsic,
            "payloadless intrinsic values must share one structural binding identity"
        );
        assert_eq!(
            bindings.identity(first),
            Some(&ExternalBindingIdentity::VtableField {
                field: "invoke".to_owned(),
            })
        );
        assert_eq!(bindings.identity(ExternalBindingId(0)), None);
        assert_eq!(bindings.identity(ExternalBindingId(u32::MAX)), None);
    }
}

mod machine_termination {
    use crate::RankingViewId;

    #[test]
    fn canonical_view_catalog_round_trips() {
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
        assert_eq!(RankingViewId::canonical("Card::PowerOrder"), None);
        assert_eq!(RankingViewId::NULL.canonical_path(), None);
    }
}

mod semantic_domains {
    use crate::{
        DomainEstablishmentRoute, DomainPredicatePresence, QualificationEvidenceOrigin,
        SemanticDomainId, SemanticDomainTable,
    };

    #[test]
    fn semantic_domain_table_is_deterministic_and_idempotent() {
        let mut table = SemanticDomainTable::default();
        assert_eq!(table.len(), 3);
        assert!(!table.is_empty());
        assert_eq!(
            table.lookup("Wrapping"),
            Some(SemanticDomainTable::WRAPPING)
        );
        assert_eq!(
            table.lookup("Saturating"),
            Some(SemanticDomainTable::SATURATING)
        );
        assert_eq!(
            table.lookup("Trapping"),
            Some(SemanticDomainTable::TRAPPING)
        );
        let kilometres = table.intern("Km");
        assert_eq!(kilometres, SemanticDomainId(4));
        assert_eq!(table.intern("Km"), kilometres);
        assert_eq!(table.name(kilometres), Some("Km"));
        assert_eq!(table.intern("Wrapping"), SemanticDomainTable::WRAPPING);
        assert_eq!(table.len(), 4);
        assert!(!table.is_empty());
        assert_eq!(table.name(SemanticDomainId::NULL), None);
        assert_eq!(table.lookup("Miles"), None);
    }

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

mod service_reach {
    use crate::{ServiceReachId, ServiceReachRowId, ServiceReachRowTable, ServiceReachTable};

    #[test]
    fn row_table_emptiness_counts_interned_rows_not_their_services() {
        let mut rows = ServiceReachRowTable::default();
        assert_eq!(rows.len(), 0);
        assert!(rows.is_empty());

        let empty_row = rows.intern(Vec::new());
        assert_eq!(empty_row, ServiceReachRowTable::EMPTY_ROW);
        assert!(rows.services(empty_row).is_empty());
        assert_eq!(rows.len(), 1);
        assert!(!rows.is_empty());

        assert_eq!(rows.intern(Vec::new()), empty_row);
        assert_eq!(rows.len(), 1);
        rows.intern(vec![ServiceReachId(2)]);
        assert_eq!(rows.len(), 2);
        assert!(!rows.is_empty());
    }

    #[test]
    fn abstract_rows_intern_per_bound_and_report_the_bound_as_ceiling() {
        let readable = ServiceReachId(2);
        let queryable = ServiceReachId(3);
        let mut rows = ServiceReachRowTable::default();
        let readable_row = rows.intern(vec![readable]);
        let queryable_row = rows.intern(vec![queryable]);

        let abstract_readable = rows.intern_abstract(readable_row);
        let abstract_queryable = rows.intern_abstract(queryable_row);
        assert_ne!(abstract_readable, abstract_queryable);
        assert_ne!(abstract_readable, readable_row);
        assert_eq!(rows.intern_abstract(readable_row), abstract_readable);

        assert!(rows.is_abstract(abstract_readable));
        assert!(!rows.is_abstract(readable_row));
        assert!(!rows.is_abstract(ServiceReachRowId::NULL));
        assert_eq!(rows.abstract_bound(abstract_readable), Some(readable_row));
        assert_eq!(rows.abstract_bound(readable_row), None);

        assert_eq!(rows.services(abstract_readable), &[readable]);
        assert_eq!(rows.services(abstract_queryable), &[queryable]);
        assert_eq!(
            rows.services(ServiceReachRowId::NULL),
            &[] as &[ServiceReachId]
        );
    }

    #[test]
    fn abstract_rows_preserve_row_id_prefix_stability() {
        let readable = ServiceReachId(2);
        let mut retained = ServiceReachRowTable::default();
        retained.intern(vec![readable]);
        let mut extended = retained.clone();
        let abstract_row = extended.intern_abstract(ServiceReachRowTable::EMPTY_ROW);
        assert!(extended.starts_with(&retained));
        assert!(!retained.starts_with(&extended));
        assert_eq!(
            extended.abstract_bound(abstract_row),
            Some(ServiceReachRowTable::EMPTY_ROW)
        );
        assert_eq!(extended.services(abstract_row), &[] as &[ServiceReachId]);
    }

    #[test]
    fn service_table_resolves_exact_canonical_names() {
        let mut services = ServiceReachTable::default();
        let machine_control =
            services.intern(symbols::SymbolHandle::from_parts(7, 1), "MachineControl");
        assert_eq!(
            services.id_for_name("MachineControl"),
            Some(machine_control)
        );
        assert_eq!(services.id_for_name("machine_control"), None);
        assert_eq!(services.id_for_name("PortIo"), None);
    }
}

mod array_extent_identity {
    use crate::type_identity::{
        TypeIdentityPackageOwnerVisitor, TypeIdentityVisitError, visit_type_identity_package_owners,
    };

    #[derive(Default)]
    struct Owners {
        allowed: Option<[u8; 32]>,
        seen: Vec<[u8; 32]>,
        depth: usize,
    }
    impl TypeIdentityPackageOwnerVisitor for Owners {
        fn enter(&mut self) -> Result<(), TypeIdentityVisitError> {
            if self.depth == 32 {
                return Err(TypeIdentityVisitError::ResourceLimitExceeded);
            }
            self.depth += 1;
            Ok(())
        }
        fn leave(&mut self) {
            self.depth -= 1;
        }
        fn reserve(&mut self, _: usize) -> Result<(), TypeIdentityVisitError> {
            Ok(())
        }
        fn package_owner(&mut self, owner: [u8; 32]) -> Result<(), TypeIdentityVisitError> {
            self.seen.push(owner);
            if self.allowed == Some(owner) {
                Ok(())
            } else {
                Err(TypeIdentityVisitError::UnknownPackage)
            }
        }
        fn embedded_name(&mut self, _: &str) -> Result<(), TypeIdentityVisitError> {
            Ok(())
        }
    }
    fn call_extent() -> String {
        let nominal = format!("nominal(package-owner(32:{}),path(size))", "ab".repeat(32));
        let escaped = nominal
            .replace('\\', "\\\\")
            .replace('(', "\\(")
            .replace(')', "\\)")
            .replace(',', "\\,");
        format!(
            "array(compiler-type(atom(u8)),const-expression(call(target({escaped}),integer(3))))"
        )
    }
    #[test]
    fn extent_call_visits_exact_package_owner_and_refuses_unknown_owner() {
        let mut owners = Owners {
            allowed: Some([0xab; 32]),
            ..Default::default()
        };
        assert_eq!(
            visit_type_identity_package_owners(&call_extent(), &mut owners),
            Ok(())
        );
        assert_eq!(owners.seen, vec![[0xab; 32]]);
        assert_eq!(owners.depth, 0);
        let mut unknown = Owners::default();
        assert_eq!(
            visit_type_identity_package_owners(&call_extent(), &mut unknown),
            Err(TypeIdentityVisitError::UnknownPackage)
        );
        assert_eq!(unknown.depth, 0);
    }
    #[test]
    fn extent_cast_visits_target_owner_and_rejects_unknown_policy() {
        let identity = format!(
            "array(compiler-type(atom(u8)),const-expression(cast(integer(3),nominal(package-owner(32:{}),path(Count)),domain(Wrapping))))",
            "ab".repeat(32)
        );
        let mut owners = Owners {
            allowed: Some([0xab; 32]),
            ..Default::default()
        };
        assert_eq!(
            visit_type_identity_package_owners(&identity, &mut owners),
            Ok(())
        );
        assert_eq!(owners.seen, vec![[0xab; 32]]);
        assert_eq!(
            visit_type_identity_package_owners(
                &identity.replace("Wrapping", "Invented"),
                &mut owners
            ),
            Err(TypeIdentityVisitError::MalformedIdentity)
        );
        assert_eq!(owners.depth, 0);
    }

    #[test]
    fn extent_static_operands_visit_nested_type_and_selection_owners() {
        let nominal = format!(
            "nominal(package-owner(32:{}),path(Selected))",
            "ab".repeat(32)
        );
        let escaped = nominal
            .replace('\\', "\\\\")
            .replace('(', "\\(")
            .replace(')', "\\)")
            .replace(',', "\\,");
        let identity = format!(
            "array(compiler-type(atom(u8)),const-expression(call(target(count),static-arguments({nominal},static-application(const-name({escaped}),integer(7))),integer(3))))"
        );
        let mut owners = Owners {
            allowed: Some([0xab; 32]),
            ..Default::default()
        };
        assert_eq!(
            visit_type_identity_package_owners(&identity, &mut owners),
            Ok(())
        );
        assert_eq!(owners.seen, vec![[0xab; 32], [0xab; 32]]);
        assert_eq!(owners.depth, 0);
        let mut unknown = Owners::default();
        assert_eq!(
            visit_type_identity_package_owners(&identity, &mut unknown),
            Err(TypeIdentityVisitError::UnknownPackage)
        );
        assert_eq!(unknown.depth, 0);
        assert_eq!(
            visit_type_identity_package_owners(
                &identity.replace("integer(7)", "hidden-owner(7)"),
                &mut owners
            ),
            Err(TypeIdentityVisitError::MalformedIdentity)
        );
    }

    #[test]
    fn generic_arguments_visit_type_and_const_owners_without_widening_type_roots() {
        let nominal = format!("nominal(package-owner(32:{}),path(count))", "ab".repeat(32));
        let escaped = nominal
            .replace('\\', "\\\\")
            .replace('(', "\\(")
            .replace(')', "\\)")
            .replace(',', "\\,");
        for application in ["generic", "dynamic-trait"] {
            let identity = format!(
                "{application}(name(Container),compiler-type(atom(u8)),integer(7),call(target({escaped}),integer(3)))"
            );
            let mut owners = Owners {
                allowed: Some([0xab; 32]),
                ..Default::default()
            };
            assert_eq!(
                visit_type_identity_package_owners(&identity, &mut owners),
                Ok(())
            );
            assert_eq!(owners.seen, vec![[0xab; 32]]);
            assert_eq!(owners.depth, 0);
            let mut unknown = Owners::default();
            assert_eq!(
                visit_type_identity_package_owners(&identity, &mut unknown),
                Err(TypeIdentityVisitError::UnknownPackage)
            );
            assert_eq!(unknown.depth, 0);
            assert_eq!(
                visit_type_identity_package_owners(
                    &identity.replace("integer(7)", "hidden-owner(7)"),
                    &mut owners,
                ),
                Err(TypeIdentityVisitError::MalformedIdentity)
            );
            assert_eq!(owners.depth, 0);
            assert_eq!(
                visit_type_identity_package_owners("integer(7)", &mut owners),
                Err(TypeIdentityVisitError::MalformedIdentity)
            );
            assert_eq!(owners.depth, 0);
        }
    }
}

mod type_identity {
    use crate::type_identity::{
        TypeIdentityPackageOwnerVisitor, TypeIdentityVisitError, malformed_identity_offset,
        visit_type_identity_package_owners,
    };

    struct PackageOwnerCounter {
        owners: usize,
    }

    impl TypeIdentityPackageOwnerVisitor for PackageOwnerCounter {
        fn enter(&mut self) -> Result<(), TypeIdentityVisitError> {
            Ok(())
        }
        fn leave(&mut self) {}
        fn reserve(&mut self, _: usize) -> Result<(), TypeIdentityVisitError> {
            Ok(())
        }
        fn package_owner(&mut self, _: [u8; 32]) -> Result<(), TypeIdentityVisitError> {
            self.owners += 1;
            Ok(())
        }
        fn embedded_name(&mut self, _: &str) -> Result<(), TypeIdentityVisitError> {
            Ok(())
        }
    }

    #[test]
    fn dynamic_trait_identity_reads_generic_arguments() {
        let identity = concat!(
            r"dynamic-trait(name(nominal\(package-owner\(32:",
            "7f51c408406b55784a8d8130b3ddf511c938d99257c59be2ea74046ebc7bd4e2",
            r"\)\,path\(src::structures::structs::symbolic_struct_resolver::SymbolicStructResolverHooks\)\)),",
            "named(name(type-parameter:0)))",
        );
        let mut counter = PackageOwnerCounter { owners: 0 };
        visit_type_identity_package_owners(identity, &mut counter)
            .expect("a dyn trait with one generic argument is a complete identity");
        assert_eq!(counter.owners, 1);
        let bare = r"dynamic-trait(name(compiler-type\(atom\(u8\)\)))";
        visit_type_identity_package_owners(bare, &mut counter)
            .expect("a dyn trait without arguments is a complete identity");
        let malformed = r"dynamic-trait(name(compiler-type\(atom\(u8\)\)),conformance(x))";
        assert!(visit_type_identity_package_owners(malformed, &mut counter).is_err());
    }

    #[test]
    fn generic_identity_reads_const_index_arguments() {
        let identity = concat!(
            r"generic(name(nominal\(toolchain-source-owner\(32:",
            "9ced29a4dddb319ba059ac16618d7e595ebb8d643d3a3b9f51c56245361ce613",
            r"\)\,path\(ForeignBinding\)\)),integer(50),integer(18),integer(0))",
        );
        let mut counter = PackageOwnerCounter { owners: 0 };
        visit_type_identity_package_owners(identity, &mut counter)
            .expect("const index arguments complete a generic identity");
        let mixed = concat!(
            r"generic(name(compiler-type\(atom\(u8\)\)),",
            r"named(name(type-parameter:0)),integer(3))",
        );
        visit_type_identity_package_owners(mixed, &mut counter)
            .expect("type and const arguments may share one application");
        let malformed = r"generic(name(compiler-type\(atom\(u8\)\)),conformance(x))";
        assert!(visit_type_identity_package_owners(malformed, &mut counter).is_err());
    }

    #[test]
    fn malformed_identity_offset_names_the_byte_the_grammar_stopped_on() {
        let readable = "ref(compiler-type(atom(u8)))";
        assert_eq!(malformed_identity_offset(readable), None);
        let trailing = format!("{readable},elided-region");
        assert_eq!(malformed_identity_offset(&trailing), Some(readable.len()));
        let second_operand = "ref(compiler-type(atom(u8)),elided-region)";
        assert_eq!(
            malformed_identity_offset(second_operand),
            second_operand.find(",elided")
        );
        assert_eq!(malformed_identity_offset(""), Some(0));
        assert_eq!(malformed_identity_offset("unknown(x)"), Some(0));
        assert_eq!(malformed_identity_offset("ref(unknown(x))"), Some(4));
        let scoped = "generic(name(compiler-type\\(atom\\(u8\\)\\)),unresolved-region)";
        assert_eq!(
            malformed_identity_offset(scoped),
            Some(scoped.len() - ")".len())
        );
    }
}

mod type_identity_vocabulary {
    use crate::type_identity::vocabulary::{self, Unadmitted};
    use crate::type_identity::{
        TypeIdentityPackageOwnerVisitor, TypeIdentityVisitError, visit_type_identity_package_owners,
    };

    #[derive(Default)]
    struct Owners {
        seen: Vec<[u8; 32]>,
        embedded: Vec<String>,
        refused: Vec<[u8; 32]>,
        depth: usize,
    }

    impl TypeIdentityPackageOwnerVisitor for Owners {
        fn enter(&mut self) -> Result<(), TypeIdentityVisitError> {
            self.depth += 1;
            Ok(())
        }
        fn leave(&mut self) {
            self.depth -= 1;
        }
        fn reserve(&mut self, _: usize) -> Result<(), TypeIdentityVisitError> {
            Ok(())
        }
        fn package_owner(&mut self, digest: [u8; 32]) -> Result<(), TypeIdentityVisitError> {
            self.seen.push(digest);
            if self.refused.contains(&digest) {
                Err(TypeIdentityVisitError::UnknownPackage)
            } else {
                Ok(())
            }
        }
        fn embedded_name(&mut self, name: &str) -> Result<(), TypeIdentityVisitError> {
            self.embedded.push(name.to_owned());
            Ok(())
        }
    }

    fn escape(text: &str) -> String {
        text.replace('\\', "\\\\")
            .replace('(', "\\(")
            .replace(')', "\\)")
            .replace(',', "\\,")
    }

    fn atom(tag: &str, value: &str) -> String {
        format!("{tag}({})", escape(value))
    }

    fn compound(tag: &str, parts: &[&str]) -> String {
        format!("{tag}({})", parts.join(","))
    }

    fn owner(package: u8) -> String {
        format!("package-owner(32:{})", format!("{package:02x}").repeat(32))
    }

    fn nominal(package: u8, path: &str) -> String {
        compound("nominal", &[&owner(package), &atom("path", path)])
    }

    fn named(package: u8, path: &str) -> String {
        compound("named", &[&atom("name", &nominal(package, path))])
    }

    fn constant(package: u8, path: &str) -> String {
        atom("const-name", &nominal(package, path))
    }

    fn index(expression: &str) -> String {
        compound("index-expression", &[expression])
    }

    fn visit(identity: &str) -> (Result<(), TypeIdentityVisitError>, Owners) {
        let mut owners = Owners::default();
        let result = visit_type_identity_package_owners(identity, &mut owners);
        assert_eq!(owners.depth, 0, "every entered node must be left");
        (result, owners)
    }

    fn reads_owners(identity: &str, expected: &[u8]) {
        let (result, owners) = visit(identity);
        assert_eq!(result, Ok(()), "{identity}");
        let expected = expected
            .iter()
            .map(|package| [*package; 32])
            .collect::<Vec<_>>();
        assert_eq!(owners.seen, expected, "{identity}");
    }

    fn refuses(identity: &str, error: TypeIdentityVisitError) {
        assert_eq!(visit(identity).0, Err(error), "{identity}");
    }

    #[test]
    fn every_spelling_of_the_vocabulary_is_unique_and_lexes_whole() {
        vocabulary::audit();
    }

    #[test]
    fn selected_algebra_reads_the_provider_operand_and_visits_every_owner() {
        let operation = compound(
            "open-index-operation",
            &[
                &atom("symbol", &nominal(3, "add_impl")),
                &atom("contract", "contract"),
            ],
        );
        let algebra = compound(
            "open-index-algebra",
            &[
                &atom("provider", &constant(4, "Provider")),
                &atom("trait", &nominal(5, "Algebra")),
                &atom("requirement", "add"),
            ],
        );
        let licensed = |algebra: &str| {
            index(&compound(
                "add",
                &[
                    &atom("operation", &operation),
                    &atom("algebra", algebra),
                    &atom("const-name", "$parameter0"),
                    "integer(1)",
                ],
            ))
        };
        reads_owners(&licensed(&algebra), &[3, 4, 5]);

        let stale = compound(
            "open-index-algebra",
            &[
                &atom("provider", &constant(4, "Provider")),
                &atom("trait", &nominal(5, "Algebra")),
                &atom("requirement", "add"),
                &atom("alias", "<default>"),
            ],
        );
        refuses(&licensed(&stale), TypeIdentityVisitError::MalformedIdentity);

        let applied = compound(
            "static-application",
            &[&constant(6, "Provider"), "integer(2)"],
        );
        let static_provider = compound(
            "open-index-algebra",
            &[
                &atom("provider", &applied),
                &atom("trait", &nominal(5, "Algebra")),
                &atom("requirement", "add"),
            ],
        );
        reads_owners(&licensed(&static_provider), &[3, 6, 5]);
    }

    #[test]
    fn derived_declarations_expose_their_package_owners() {
        let origin = compound(
            "generic",
            &[
                &atom("name", &nominal(6, "Placed")),
                &named(7, "Policy"),
                &named(8, "Schema"),
            ],
        );
        let application = compound("derived-application", &[&origin]);
        let field = compound("placed-field", &[&origin, &atom("member", "42")]);
        let by_name = compound("placed-field", &[&origin, &atom("field", "word")]);
        let operation = compound(
            "placed-operation",
            &[&field, &atom("operation", "read"), &atom("role", "machine")],
        );
        for derived in [&application, &field, &by_name, &operation] {
            reads_owners(&compound("named", &[&atom("name", derived)]), &[6, 7, 8]);
        }
        let (result, owners) = {
            let mut owners = Owners {
                refused: vec![[7; 32]],
                ..Owners::default()
            };
            let result = visit_type_identity_package_owners(
                &compound("named", &[&atom("name", &application)]),
                &mut owners,
            );
            (result, owners)
        };
        assert_eq!(result, Err(TypeIdentityVisitError::UnknownPackage));
        assert_eq!(owners.seen, [[6; 32], [7; 32]]);

        refuses(
            &compound(
                "named",
                &[&atom(
                    "name",
                    &compound("placed-operation", &[&application]),
                )],
            ),
            TypeIdentityVisitError::MalformedIdentity,
        );
        refuses(
            &compound(
                "named",
                &[&atom(
                    "name",
                    &atom("invalid-derived-origin", "no applied origin"),
                )],
            ),
            TypeIdentityVisitError::Unadmitted(Unadmitted::InvalidIdentity),
        );
    }

    #[test]
    fn every_index_form_reads_with_the_owners_it_carries() {
        let ty = |package: u8| named(package, "Operand");
        let domain = atom("domain", "Wrapping");
        let selected = compound(
            "static-application",
            &[
                &constant(2, "Selected"),
                "integer(7)",
                &atom("lifetime", "a"),
            ],
        );
        let cases: Vec<(String, Vec<u8>)> = vec![
            (
                compound("indexed", &[&constant(1, "table"), "integer(0)"]),
                vec![1],
            ),
            (
                compound(
                    "member",
                    &[
                        &constant(1, "value"),
                        &atom("member", "tag"),
                        &atom("case", "On"),
                    ],
                ),
                vec![1],
            ),
            (compound("borrow", &[&constant(1, "value")]), vec![1]),
            (compound("borrow-mut", &[&constant(1, "value")]), vec![1]),
            (compound("borrow-write", &[&constant(1, "value")]), vec![1]),
            (
                compound("range-inclusive", &["integer(0)", &constant(1, "end")]),
                vec![1],
            ),
            (
                compound("range-exclusive", &["integer(0)", &constant(1, "end")]),
                vec![1],
            ),
            (compound("array-literal", &[]), vec![]),
            (
                compound("array-literal", &["integer(1)", &constant(1, "two")]),
                vec![1],
            ),
            (compound("type-expression", &[&ty(1)]), vec![1]),
            (compound("zero-value", &[&ty(1)]), vec![1]),
            (
                compound(
                    "atomic",
                    &[
                        &constant(1, "cell"),
                        &constant(2, "prior"),
                        &atom("ordering", "SequentiallyConsistent"),
                        &atom("result-custody", "Retained"),
                    ],
                ),
                vec![1, 2],
            ),
            (
                compound(
                    "struct-literal",
                    &[
                        &atom("type", &nominal(1, "Record")),
                        &atom("case", "On"),
                        &compound("field", &[&atom("name", "width"), &constant(2, "width")]),
                    ],
                ),
                vec![1, 2],
            ),
            (
                compound(
                    "cast",
                    &[&constant(1, "value"), &nominal(2, "Count"), &domain],
                ),
                vec![1, 2],
            ),
            (
                compound(
                    "call",
                    &[
                        &atom("target", &nominal(1, "size")),
                        &compound("static-arguments", &[&selected]),
                        &constant(3, "receiver"),
                        "integer(3)",
                    ],
                ),
                vec![1, 2, 3],
            ),
            (
                compound(
                    "saved-subject",
                    &[&atom("depth", "0"), &constant(1, "head")],
                ),
                vec![1],
            ),
            (atom("undetermined-nan", "f64"), vec![]),
            (atom("float", "1.5"), vec![]),
            (atom("boolean", "true"), vec![]),
            (
                compound(
                    "case_membership",
                    &[&constant(1, "subject"), &constant(2, "Case")],
                ),
                vec![1, 2],
            ),
        ];
        for (expression, owners) in cases {
            reads_owners(&index(&expression), &owners);
        }

        let destructure = compound(
            "destructure",
            &[
                &constant(1, "Head"),
                &compound(
                    "fields",
                    &[
                        &compound("field", &[&atom("name", &nominal(2, "Head::left")), "bind"]),
                        &compound(
                            "field",
                            &[
                                &atom("name", &nominal(3, "Head::right")),
                                &compound("equal", &[&constant(4, "Wanted")]),
                            ],
                        ),
                        &compound(
                            "field",
                            &[&atom("name", &nominal(5, "Head::spare")), "waive"],
                        ),
                    ],
                ),
                &atom("rest", "yes"),
                &constant(6, "guard"),
                &compound(
                    "proof-selectors",
                    &[&compound(
                        "proof-selector",
                        &[&atom("field", "left"), &atom("binding", "0")],
                    )],
                ),
            ],
        );
        let unguarded = compound(
            "destructure",
            &[
                &constant(1, "Head"),
                "fields()",
                &atom("rest", "no"),
                "no-guard",
                "proof-selectors()",
            ],
        );
        let dispatch = |pattern: &str| {
            index(&compound(
                "match",
                &[
                    &constant(7, "subject"),
                    &compound("arm", &["wildcard", &constant(8, "otherwise")]),
                    &compound("arm", &[pattern, &constant(9, "chosen")]),
                ],
            ))
        };
        reads_owners(&dispatch(&destructure), &[7, 8, 1, 2, 3, 4, 5, 6, 9]);
        reads_owners(&dispatch(&unguarded), &[7, 8, 1, 9]);
        reads_owners(&dispatch(&constant(1, "Wanted")), &[7, 8, 1, 9]);
    }

    #[test]
    fn refusals_name_why_no_retained_policy_holds_the_production() {
        let region = "region(owner(1:00),ordinal(0))";
        let invalid = TypeIdentityVisitError::Unadmitted(Unadmitted::InvalidIdentity);
        let scoped = TypeIdentityVisitError::Unadmitted(Unadmitted::ApplicationScope);
        let u8_type = "compiler-type(atom(u8))";
        for (identity, error) in [
            (compound("ref", &[u8_type, region]), scoped),
            (compound("ref-mut", &[u8_type, "elided-region"]), scoped),
            (
                compound("generic", &[&atom("name", u8_type), "unresolved-region"]),
                scoped,
            ),
            (
                compound("nominal", &["toolchain-owner", &atom("path", "x")]),
                invalid,
            ),
            (
                compound("nominal", &["unresolved-owner", &atom("path", "x")]),
                invalid,
            ),
            (
                compound("array", &[u8_type, "invalid-array-expression(missing)"]),
                invalid,
            ),
            (index("unsupported-const-substitution"), invalid),
            (index("unbound-match-subject"), invalid),
        ] {
            refuses(&identity, error);
        }
    }

    #[test]
    fn only_binder_replacements_reach_the_consumer() {
        let (result, owners) = visit(r"named(name(type-parameter:0))");
        assert_eq!(result, Ok(()));
        assert_eq!(owners.embedded, ["type-parameter:0"]);
        for production in [r"integer\(5\)", r"name\(x\)", r"generic\(name\(x\)\)"] {
            refuses(
                &format!("named(name({production}))"),
                TypeIdentityVisitError::MalformedIdentity,
            );
        }
        refuses("unknown(x)", TypeIdentityVisitError::MalformedIdentity);
        refuses(
            &index("unknown(integer(1))"),
            TypeIdentityVisitError::MalformedIdentity,
        );
    }

    #[test]
    fn retired_scalar_interval_identity_tags_are_refused() {
        for identity in [
            r"constrained(compiler-type(atom(u64)),range(minimum(integer\(0\)),maximum(exclusive-end\(const-name\($parameter0\)\))))",
            r"constrained(compiler-type(atom(u64)),range(minimum(integer\(0\)),maximum(const-name\($parameter0\))))",
        ] {
            refuses(identity, TypeIdentityVisitError::MalformedIdentity);
        }
        refuses(
            &index(&compound("exclusive-end", &["integer(1)"])),
            TypeIdentityVisitError::MalformedIdentity,
        );
    }
}
