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

    // The bound's set is the abstract row's ceiling; the row may narrow at
    // the bound fit check but never exceeds it.
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
