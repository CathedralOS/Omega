use crate::{
    BlockingInterface, BlockingPlan, ServiceReachId, ServiceReachRowId, ServiceReachRowTable,
    ServiceReachTable, SuspensionInterface, SuspensionPlan,
};

#[test]
fn service_rows_exclude_operational_axes_and_normalize_as_sets() {
    let readable = ServiceReachId(2);
    let queryable = ServiceReachId(3);
    let mut rows = ServiceReachRowTable::default();
    assert_eq!(rows.intern(Vec::new()), ServiceReachRowTable::EMPTY_ROW);
    let combined = rows.intern(vec![queryable, readable]);
    assert_eq!(rows.intern(vec![readable, queryable, readable]), combined);
    assert_eq!(rows.services(combined), &[readable, queryable]);
    assert_eq!(
        rows.services(ServiceReachRowId::NULL),
        &[] as &[ServiceReachId]
    );
}

#[test]
fn service_row_prefix_preserves_ids_while_new_sets_append() {
    let readable = ServiceReachId(2);
    let queryable = ServiceReachId(3);
    let mut retained = ServiceReachRowTable::default();
    let readable_row = retained.intern(vec![readable]);
    let queryable_row = retained.intern(vec![queryable]);
    let mut extended = retained.clone();
    let combined = extended.intern(vec![queryable, readable, queryable]);
    assert!(extended.starts_with(&retained));
    assert!(retained.starts_with(&retained));
    assert!(!retained.starts_with(&extended));
    assert_eq!(extended.services(readable_row), &[readable]);
    assert_eq!(extended.services(queryable_row), &[queryable]);
    assert_eq!(extended.services(combined), &[readable, queryable]);
    assert_eq!(extended.intern(Vec::new()), ServiceReachRowTable::EMPTY_ROW);

    let mut reordered = ServiceReachRowTable::default();
    reordered.intern(vec![queryable]);
    reordered.intern(vec![readable]);
    assert!(!reordered.starts_with(&retained));
    let mut changed = retained.clone();
    changed.rows[readable_row.0 as usize - 1] = vec![readable, queryable];
    assert!(!changed.starts_with(&retained));
    let mut truncated = retained.clone();
    truncated.rows.pop();
    assert!(!truncated.starts_with(&retained));
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

#[test]
fn operational_plans_distinguish_private_inference_from_public_omission() {
    assert_eq!(
        SuspensionPlan::default().interface,
        SuspensionInterface::InternalInferred
    );
    assert_eq!(
        BlockingPlan::default().interface,
        BlockingInterface::InternalInferred
    );

    let public_non_suspending = SuspensionPlan {
        interface: SuspensionInterface::PublishedMaySuspend(false),
        checked_may_suspend: false,
    };
    let public_non_blocking = BlockingPlan {
        interface: BlockingInterface::PublishedMayBlock(false),
        checked_may_block: false,
    };
    assert_ne!(public_non_suspending, SuspensionPlan::default());
    assert_ne!(public_non_blocking, BlockingPlan::default());

    let independently_suspending = SuspensionPlan {
        interface: SuspensionInterface::PublishedMaySuspend(true),
        checked_may_suspend: true,
    };
    assert!(independently_suspending.checked_may_suspend);
    assert!(!public_non_blocking.checked_may_block);
}
