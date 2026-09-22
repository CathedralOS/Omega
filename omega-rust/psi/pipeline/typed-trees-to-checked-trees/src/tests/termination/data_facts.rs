use crate::tests::front_end::typed_program;
/// R2 rung 2 slice 2: the admitted zero-satisfying default-domain facts
/// travel to the TYPED data definition -- rung 3's consumer substrate.
#[test]
fn data_where_facts_propagate_to_typed() {
    let source = r#"
    data Ledger
    where
        count <= len,
    {
        len: u32;
        count: u32;
    }

    data Main { ledger: Ledger; }

    machine Main::main(&mut self) -> u64 { 7 }
    "#;

    let typed = typed_program(source);

    let ledger = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Ledger")
        .expect("Ledger data");
    assert_eq!(typed.proof_facts.span_or_empty(ledger.where_facts).len(), 1);
}
