mod external_bindings {
    use super::super::{ExternalBindingId, ExternalBindingIdentity, ExternalBindingTable};

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

mod service_reach {
    use super::super::{
        ServiceReachId, ServiceReachRowId, ServiceReachRowTable, ServiceReachTable,
    };

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

mod domains {
    use super::super::{SemanticDomainId, SemanticDomainTable};
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
}
