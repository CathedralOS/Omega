#[path = "support/returns.rs"]
mod returns;
#[path = "support/source.rs"]
mod sources;

use omega_lowering::{
    FieldInitializer, LoweredPackage, MachineBody, OperationKind, OperationResult, PlaceId, PlaceOrigin, ReturnEdge, ScalarLiteral, ValueOperand, ValueOrigin,
};

#[test]
fn returned_literal_tracks_the_selected_constant_declaration() {
    // Same carrier for both declarations. Moving one declaration does not change
    // either read; replacing just its initializer must change only its own read.
    for (declarations, left) in [
        ("const LEFT:u8=17u8; const RIGHT:u8=42u8;", 17),
        ("const RIGHT:u8=42u8; const LEFT:u8=17u8;", 17),
        ("const RIGHT:u8=42u8; const LEFT:u8=91u8;", 91),
    ] {
        for (read, expected) in [("LEFT", left), ("RIGHT", 42)] {
            let source = format!("{declarations} machine get()->u8{{{read}}}");
            let psi = sources::lower(&[("named.omg", &source)]).unwrap();
            let body = &psi.machines[0].body;
            let ReturnEdge::Scalar(result) = returns::sole_return(body) else {
                panic!("expected scalar return")
            };
            let [op] = body.operations.as_slice() else { panic!("expected one constant") };
            spec_corpus::observe_handoff!(
                "named-constant-initializers/consumer",
                assert_eq!(
                    (op.result, &op.kind),
                    (
                        OperationResult::Scalar(result),
                        &OperationKind::Constant(ScalarLiteral::Integer(expected.into()))
                    ),
                    "{source}"
                )
            );
        }
    }
}

fn returned_place(body: &MachineBody) -> PlaceId {
    let ReturnEdge::Structural { source, result } = returns::sole_return(body) else {
        panic!("expected structural return")
    };
    assert_eq!(body.places[result.index()].origin, PlaceOrigin::Result);
    assert_eq!(body.places[result.index()].ty, body.places[source.index()].ty);
    source
}

fn definition(
    body: &MachineBody,
    place: PlaceId,
) -> &OperationKind {
    let PlaceOrigin::Operation(id) = body.places[place.index()].origin else {
        panic!("expected constructed place")
    };
    let op = &body.operations[id.index()];
    assert_eq!(op.result, OperationResult::Structural(place));
    &op.kind
}

// Follow the returned place into its own concrete schema (not just a matching operation elsewhere).
fn slots<'a>(
    psi: &LoweredPackage,
    body: &'a MachineBody,
    place: PlaceId,
    owner: &str,
    active: Option<&str>,
) -> &'a [FieldInitializer] {
    let schema = psi
        .data
        .iter()
        .find(|s| s.carrier == body.places[place.index()].ty)
        .unwrap();
    assert_eq!((schema.declaration.package.as_str(), schema.declaration.path.as_str()), ("app", owner));
    let (fields, roster) = match (active, definition(body, place)) {
        (None, OperationKind::EstablishRecord { fields }) => (fields, &schema.fields),
        (Some(expected), OperationKind::EstablishCase { case, fields }) => {
            let selected = schema
                .cases
                .iter()
                .find(|c| c.declaration.path == expected)
                .unwrap();
            assert_eq!(case, &selected.declaration);
            (fields, &selected.fields)
        }
        other => panic!("wrong returned construction: {other:?}"),
    };
    assert_eq!(fields.len(), roster.len());
    for (field, slot) in fields.iter().zip(roster) {
        assert_eq!(field.field, slot.declaration);
        let ty = match field.operand {
            ValueOperand::Scalar(v) => body.values[v.index()].ty,
            ValueOperand::CopyWhole(p) => body.places[p.index()].ty,
            _ => panic!("expected scalar or copyable frozen field"),
        };
        assert_eq!(ty, slot.carrier);
    }
    fields
}

fn literal(
    body: &MachineBody,
    field: &FieldInitializer,
) -> ScalarLiteral {
    let ValueOperand::Scalar(v) = field.operand else {
        panic!("expected scalar slot")
    };
    let ValueOrigin::Operation(id) = body.values[v.index()].origin else {
        panic!("expected initialized scalar")
    };
    let op = &body.operations[id.index()];
    assert_eq!(op.result, OperationResult::Scalar(v));
    let OperationKind::Constant(value) = &op.kind else {
        panic!("expected constant")
    };
    value.clone()
}

#[test]
fn returned_nominal_has_the_frozen_declaration_slots_and_active_case() {
    let record = "data Pair [copy] {first:u8; second:u8;}";
    let sum = "data Pair [copy] {case A(first:u8,second:u8); case B(first:u8,second:u8);}";
    for (schema, initializer, expected_case, expected_fields) in [
        (record, "Pair{first:2,second:9}", None, [2, 9]),
        (record, "Pair{second:9,first:2}", None, [2, 9]),
        (record, "Pair{second:2,first:9}", None, [9, 2]),
        (sum, "Pair::A{second:9,first:2}", Some("Pair::A"), [2, 9]),
        (sum, "Pair::B{second:9,first:2}", Some("Pair::B"), [2, 9]),
        (sum, "Pair::B{second:2,first:9}", Some("Pair::B"), [9, 2]),
    ] {
        let source = format!("{schema} const VALUE:Pair={initializer}; machine get()->Pair{{VALUE}}");
        let psi = sources::lower(&[("slots.omg", &source)]).unwrap();
        let body = &psi.machines[0].body;
        let fields = slots(&psi, body, returned_place(body), "Pair", expected_case);
        let prefix = expected_case.unwrap_or("Pair");
        let expected = ["first", "second"]
            .into_iter()
            .zip(expected_fields)
            .map(|(name, n)| (format!("{prefix}::{name}"), ScalarLiteral::Integer(n.into())))
            .collect::<Vec<_>>();
        spec_corpus::observe_handoff!(
            "frozen-nominal-slots/consumer",
            assert_eq!(
                fields
                    .iter()
                    .map(|f| (f.field.path.clone(), literal(body, f)))
                    .collect::<Vec<_>>(),
                expected,
                "{source}"
            )
        );
    }
}

#[test]
fn fieldless_constant_returned_through_an_alias_keeps_its_owner() {
    let source = "data Marker [copy] {} data Alias = Marker;
        const VALUE:Marker=Marker{}; machine get()->Alias{VALUE}";
    let psi = sources::lower(&[("empty.omg", source)]).unwrap();
    let body = &psi.machines[0].body;
    assert!(slots(&psi, body, returned_place(body), "Marker", None).is_empty());
}

#[test]
fn returned_array_preserves_the_variant_in_each_record() {
    let source = "data Choice [copy] {case Selected(value:u8); case Empty;}
        data Entry [copy] {choice:Choice;}
        const ROWS:[Entry;2]=[Entry{choice:Choice::Empty}, Entry{choice:Choice::Selected{value:7}}];
        machine get()->[Entry;2]{ROWS}";
    let psi = sources::lower(&[("nested.omg", source)]).unwrap();
    let body = &psi.machines[0].body;
    let OperationKind::EstablishStructuralArray { elements } = definition(body, returned_place(body)) else {
        panic!("expected array")
    };
    assert_eq!(elements.len(), 2);
    for (&entry, case) in elements.iter().zip(["Choice::Empty", "Choice::Selected"]) {
        let [choice] = slots(&psi, body, entry, "Entry", None) else {
            panic!("expected one field")
        };
        let ValueOperand::CopyWhole(place) = choice.operand else {
            panic!("nested value must copy its whole place")
        };
        let payload = slots(&psi, body, place, "Choice", Some(case));
        if case == "Choice::Empty" {
            assert!(payload.is_empty());
        } else {
            let [value] = payload else { panic!("expected selected value") };
            spec_corpus::observe_handoff!(
                psi "psi-output/frozen-case-in-record-array",
                assert_eq!(literal(body, value), ScalarLiteral::Integer(7.into()))
            );
        }
    }
}

#[test]
fn applied_case_payload_uses_its_concrete_argument() {
    let source = "data Choice<T [copy]> [copy] {case Some(item:T); case Empty;}
        const BYTE:Choice<u8> = Choice::<u8>::Some{item:42};
        const FLAG:Choice<bool> = Choice::<bool>::Some{item:true};
        machine byte()->Choice<u8>{BYTE} machine flag()->Choice<bool>{FLAG}";
    let psi = sources::lower(&[("applied.omg", source)]).unwrap();
    for (name, expected) in [
        ("byte", ScalarLiteral::Integer(42.into())),
        ("flag", ScalarLiteral::Boolean(true)),
    ] {
        let body = &psi
            .machines
            .iter()
            .find(|m| m.declaration.path == name)
            .unwrap()
            .body;
        let place = returned_place(body);
        let [item] = slots(&psi, body, place, "Choice", Some("Choice::Some")) else {
            panic!("expected applied payload")
        };
        assert_eq!(literal(body, item), expected);
        let ValueOperand::Scalar(v) = item.operand else {
            panic!("expected scalar argument")
        };
        let schema = psi
            .data
            .iter()
            .find(|s| s.carrier == body.places[place.index()].ty)
            .unwrap();
        assert_eq!(schema.type_arguments, [omega_psi::DataArgument::Type(body.values[v.index()].ty)]);
    }
}
