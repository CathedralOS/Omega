use crate::tests::front_end::checked_program;

#[test]
fn linear_judgment_reads_canonical_permission_events() {
    let checked = checked_program(
        r#"
        data Receipt [linear] { code: i32; }
        boundary requirement Receipt::ack(self);
        data Main {}
        machine Main::run() -> i32 {
            let issued: Receipt = Receipt { code: 7 };
            let forwarded: Receipt = issued;
            Receipt::ack(forwarded);
            0
        }
        "#,
    );
    crate::checks::validate_linear_permission_events(&checked.typed, &checked.facts)
        .expect("semantic permission events are sufficient for the judgment");
}

#[test]
fn consuming_call_that_returns_an_obligation_transfers_its_origin() {
    let checked = checked_program(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::forward(self) -> Receipt { self }
        boundary requirement Receipt::ack(self);
        data Main {}
        machine Main::run() -> i32 {
            let issued: Receipt = Receipt { code: 7 };
            let returned: Receipt = Receipt::forward(issued);
            Receipt::ack(returned);
            0
        }
        "#,
    );

    use language_semantics::{PermissionAccess, PermissionEventKind};
    // `forward`'s owned receiver and `ack`'s contract receiver each establish
    // an ordinary entry claim in their own frames; this test follows the
    // caller's claim in `run`, which still rides `issued` through the
    // `forward` call and the `returned` binding into the `ack` consume.
    let run_machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::run")
        .map(|machine| machine.symbol)
        .expect("run machine");
    let events = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| {
            event.access == PermissionAccess::Owned && event.machine_symbol == run_machine
        })
        .collect::<Vec<_>>();
    assert_eq!(
        events.iter().map(|event| event.kind).collect::<Vec<_>>(),
        [
            PermissionEventKind::Establish,
            PermissionEventKind::Transfer,
            PermissionEventKind::Establish,
            PermissionEventKind::Consume,
        ]
    );
    assert!(
        events
            .iter()
            .all(|event| event.provenance == events[0].provenance)
    );
}
