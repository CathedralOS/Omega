#[path = "support/source.rs"]
mod source;

use omega_lowering::{EmbeddedLoanOrigin, EmbeddedLoanSource, Parameter};
use psi_handoff_consumer::check_constructed_field;

#[test]
fn parked_local_keeps_the_descriptor_used_to_initialize_its_field() {
    for (selected, slot, terminal) in [
        ("first", 0, false),
        ("second", 1, false),
        ("first", 0, true),
        ("second", 1, true),
    ] {
        let call = if terminal { "transition{_ -> suspend wait(),}" } else { "suspend wait()" };
        let program =
            format!("data Holder<'r>{{r:@'r u8;}} machine wait() suspends{{}} machine forward(first:@u8,second:@u8) suspends{{let h=Holder{{r:{selected}}};{call}}}");
        let resolved = source::resolve(&[("constructed.omg", &program)]);
        let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
        let tree = prepared.typed().trees().next().unwrap();
        let construction = (0..tree.resolved().file.nodes.len())
            .find_map(|node| tree.construction(node))
            .unwrap();
        let [input] = construction.initializers.as_slice() else {
            panic!("one prepared field")
        };
        assert_eq!(input.position, 0);
        assert_eq!(
            tree.resolved()
                .file
                .span(input.expression)
                .slice(program.as_bytes()),
            Some(selected.as_bytes())
        );
        let mut package = omega_lowering::lower_package(&prepared).unwrap();
        drop(prepared);
        drop(resolved);
        assert_eq!(check_constructed_field(&package, slot, omega_lowering::ReferenceAccess::Shared), Ok(()));
        let body = &mut package
            .machines
            .iter_mut()
            .find(|m| m.declaration.path == "forward")
            .unwrap()
            .body;
        assert_eq!(matches!(body.suspension_sites[0].call, omega_lowering::CallSite::Edge(_)), terminal);
        let Parameter::Structural(other) = body.parameters[1 - slot] else {
            panic!("other input descriptor")
        };
        body.suspension_sites[0].retained_owners[0].embedded_loans[0].origins = vec![EmbeddedLoanOrigin {
            arrival: Vec::new(),
            reborrows: Vec::new(),
            source: EmbeddedLoanSource::Formal { formal: other, path: vec![] },
        }];
        assert_eq!(check_constructed_field(&package, slot, omega_lowering::ReferenceAccess::Shared), Err("wrong initializer origin"));
    }
}

#[test]
fn a_selected_case_stores_its_shared_descriptor_in_the_payload_slot() {
    use omega_lowering::{OperationKind, ValueOperand};
    let text = "data Reply<'r>{case Ready(r:@'r u8);case Empty;} machine build(x:@u8){let item=Reply::Ready{r:x};}";
    let pkg = source::lower(&[("payload.omg", text)]).unwrap();
    let body = &pkg.machines[0].body;
    let Parameter::Structural(input) = body.parameters[0] else {
        panic!("incoming loan")
    };
    let [establish] = body.operations.as_slice() else {
        panic!("one case construction")
    };
    let OperationKind::EstablishCase { case, fields } = &establish.kind else {
        panic!("selected case")
    };
    let [field] = fields.as_slice() else { panic!("one initialized field") };
    assert_eq!(case.path, "Reply::Ready");
    assert_eq!(field.field.path, "Reply::Ready::r");
    assert_eq!(field.operand, ValueOperand::CopyWhole(input));
}

#[test]
fn parking_a_reborrow_preserves_its_formation_and_its_incoming_parent() {
    use omega_lowering::{OperationKind, PlaceProjection, ReferenceAccess};
    let text =
        "data Cell{mark:u8;} data Holder<'r>{r:@'r u8;} machine wait() suspends{} machine forward(cell:@Cell) suspends{let h=Holder{r:@cell.mark};suspend wait()}";
    let package = source::lower(&[("reborrow.omg", text)]).unwrap();
    let body = &package
        .machines
        .iter()
        .find(|m| m.declaration.path == "forward")
        .unwrap()
        .body;
    let Parameter::Structural(cell) = body.parameters[0] else {
        panic!("incoming parent descriptor")
    };
    let [loan] = body.suspension_sites[0].retained_owners[0]
        .embedded_loans
        .as_slice()
    else {
        panic!("one stored reborrow")
    };
    let [origin] = loan.origins.as_slice() else { panic!("one origin") };
    let [definition] = origin.reborrows.as_slice() else {
        panic!("one reborrow formation")
    };
    assert_eq!(origin.source, EmbeddedLoanSource::Formal { formal: cell, path: vec![] });
    assert!(origin.arrival.is_empty());
    let op = &body.operations[definition.index()];
    let OperationKind::BorrowPlace { source, access } = &op.kind else {
        panic!("borrow operation")
    };
    assert_eq!((source.root, *access), (cell, ReferenceAccess::Shared));
    assert!(matches!(&source.path[..], [PlaceProjection::Referent, PlaceProjection::Field(field)] if field.path=="Cell::mark"));
}

#[test]
fn an_exclusive_field_takes_the_incoming_descriptor_only_once() {
    use omega_lowering::{OperationKind, ValueOperand};
    for access in ["mutable", "write"] {
        for (shape, constructor) in [("{r:@'r ACCESS Cell;}", "Holder"), ("{case Some(r:@'r ACCESS Cell); case None;}", "Holder::Some")] {
            let text = format!("data Cell{{v:u8;}} data Holder<'r>{} machine build(x:@{access} Cell){{let h={constructor}{{r:x}};}}", shape.replace("ACCESS", access));
            let package = source::lower(&[("transfer.omg", &text)]).unwrap();
            let body = &package.machines[0].body;
            let [Parameter::Structural(input)] = body.parameters[..] else { panic!("one incoming descriptor") };
            let [op] = body.operations.as_slice() else { panic!("construction takes the input directly") };
            let fields = match &op.kind {
                OperationKind::EstablishRecord { fields } if constructor == "Holder" => fields,
                OperationKind::EstablishCase { case, fields } if case.path == constructor => fields,
                _ => panic!("expected the selected constructor"),
            };
            let [field] = fields.as_slice() else { panic!("one stored descriptor") };
            assert_eq!(field.field.path, format!("{constructor}::r"));
            assert_eq!(field.operand, ValueOperand::MoveWhole(input));
        }
    }
}

#[test]
fn constructed_exclusive_loan_names_the_field_it_borrowed() {
    use omega_lowering::{OperationKind, OperationResult, PlaceProjection, ReferenceAccess, ValueOperand};
    for (spelling, expected) in [("mutable", ReferenceAccess::Mutable), ("write", ReferenceAccess::WriteOnly)] {
        let text = format!("data Cell{{v:u8;}} data Holder<'r>{{r:@'r {spelling} u8;}} machine build(cell:@mutable Cell){{let h=Holder{{r:@{spelling} cell.v}};}}");
        let package = source::lower(&[("new_loan.omg", &text)]).unwrap();
        let body = &package.machines[0].body;
        let [Parameter::Structural(parent)] = body.parameters[..] else { panic!("one parent loan") };
        let [borrow, establish] = body.operations.as_slice() else { panic!("one loan and its destination") };
        let OperationKind::BorrowPlace { source, access } = &borrow.kind else { panic!("explicit borrow") };
        assert_eq!((source.root, *access), (parent, expected));
        assert!(matches!(&source.path[..], [PlaceProjection::Referent, PlaceProjection::Field(field)] if field.path == "Cell::v"));
        let OperationResult::Structural(descriptor) = borrow.result else { panic!("descriptor") };
        let OperationKind::EstablishRecord { fields } = &establish.kind else { panic!("record construction") };
        assert!(matches!(&fields[..], [field] if field.field.path == "Holder::r" && field.operand == ValueOperand::MoveWhole(descriptor)));
    }
}

#[test]
fn parked_owner_retains_the_exclusive_loan_that_moved_into_its_field() {
    use omega_lowering::ReferenceAccess;
    for (spelling, access) in [("mutable", ReferenceAccess::Mutable), ("write", ReferenceAccess::WriteOnly)] {
        let text = format!(
            "data Holder<'r>{{r:@'r {spelling} u8;}} machine wait() suspends{{}}
             machine forward(x:@{spelling} u8) suspends{{let h=Holder{{r:x}}; suspend wait()}}"
        );
        let package = source::lower(&[("parked.omg", &text)]).unwrap();
        assert_eq!(check_constructed_field(&package, 0, access), Ok(()));
    }
}
