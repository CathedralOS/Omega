use psi_language_semantics::{
    BlockingSummary, ServiceReachRowId, ServiceReachRowTable, ServiceReachSummary,
    ServiceReachTable, SuspensionSummary,
};

pub(crate) fn service_names(
    services: &ServiceReachTable,
    rows: &ServiceReachRowTable,
    row: ServiceReachRowId,
) -> Vec<String> {
    rows.services(row)
        .iter()
        .filter_map(|service| services.definition(*service))
        .map(|definition| definition.name.clone())
        .collect()
}

pub(crate) fn append_reach_and_operation_lines(
    label: &mut String,
    services: &ServiceReachTable,
    rows: &ServiceReachRowTable,
    reach: ServiceReachSummary,
    suspension: SuspensionSummary,
    blocking: BlockingSummary,
) {
    label.push_str("\ndirect service reach: ");
    append_service_row(label, services, rows, reach.direct);
    label.push_str("\nreached service reach: ");
    append_service_row(label, services, rows, reach.transitive);
    label.push_str("\nsuspension: direct ");
    label.push_str(yes_no(suspension.direct_may_suspend));
    label.push_str(", reached ");
    label.push_str(yes_no(suspension.transitive_may_suspend));
    label.push_str("\nblocking: direct ");
    label.push_str(yes_no(blocking.direct_may_block));
    label.push_str(", reached ");
    label.push_str(yes_no(blocking.transitive_may_block));
}

fn append_service_row(
    label: &mut String,
    services: &ServiceReachTable,
    rows: &ServiceReachRowTable,
    row: ServiceReachRowId,
) {
    let names = service_names(services, rows, row);
    if names.is_empty() {
        label.push_str("<none>");
    } else {
        label.push_str(&names.join(" + "));
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

#[cfg(test)]
mod tests {
    use super::append_reach_and_operation_lines;
    use psi_language_semantics::{
        BlockingSummary, ServiceReachRowTable, ServiceReachSummary, ServiceReachTable,
        SuspensionSummary,
    };
    use psi_symbols::SymbolHandle;

    #[test]
    fn rendering_keeps_services_and_operational_axes_independent() {
        let mut services = ServiceReachTable::default();
        let control = services.intern(SymbolHandle::from_arena_index(1), "MachineControl");
        let ports = services.intern(SymbolHandle::from_arena_index(2), "PortIo");
        let mut rows = ServiceReachRowTable::default();
        let direct = rows.intern(vec![ports]);
        let transitive = rows.intern(vec![control, ports]);
        let mut label = String::new();

        append_reach_and_operation_lines(
            &mut label,
            &services,
            &rows,
            ServiceReachSummary { direct, transitive },
            SuspensionSummary {
                direct_may_suspend: false,
                transitive_may_suspend: true,
            },
            BlockingSummary {
                direct_may_block: true,
                transitive_may_block: true,
            },
        );

        assert!(label.contains("direct service reach: PortIo"));
        assert!(label.contains("reached service reach: MachineControl + PortIo"));
        assert!(label.contains("suspension: direct no, reached yes"));
        assert!(label.contains("blocking: direct yes, reached yes"));
        assert!(!label.contains("0x"));
    }
}
