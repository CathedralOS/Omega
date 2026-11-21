#[path = "support/source.rs"]
mod source;

use omega_lowering::{
    EmbeddedLoanOrigin, MachineBody, OperationKind, OperationResult, Parameter, PlaceId, PlaceOrigin, PlaceProjection, ReferenceAccess, ValueOperand,
};

fn forward(package: &omega_lowering::LoweredPackage) -> &MachineBody {
    &package
        .machines
        .iter()
        .find(|m| m.declaration.path == "forward")
        .unwrap()
        .body
}

/// Every constructed record place with its sole `Holder::r` initializer, in body order.
fn records(body: &MachineBody) -> Vec<(PlaceId, ValueOperand)> {
    body.operations
        .iter()
        .filter_map(|op| match (&op.kind, op.result) {
            (OperationKind::EstablishRecord { fields }, OperationResult::Structural(place)) => {
                let [field] = fields.as_slice() else { panic!("one initializer") };
                assert_eq!(field.field.path, "Holder::r");
                Some((place, field.operand.clone()))
            }
            _ => None,
        })
        .collect()
}

fn formal(
    body: &MachineBody,
    slot: usize,
) -> PlaceId {
    let Parameter::Structural(place) = body.parameters[slot] else {
        panic!("reference formal")
    };
    place
}

/// The home first holds `initial`, then the whole record is replaced by one holding `latest`.
fn program(
    initial: &str,
    latest: &str,
    suspending: bool,
) -> String {
    let (effect, call) = if suspending { (" suspends", "suspend wait()") } else { ("", "wait()") };
    format!(
        "data Holder{{r:@u8;}} machine wait(){effect}{{}} machine forward(first:@u8,second:@u8){effect}{{let mut h=Holder{{r:{initial}}};h=Holder{{r:{latest}}};{call}}}"
    )
}

#[test]
fn an_ordinary_scope_stores_the_replacement_descriptor_into_the_original_home() {
    for (initial, latest, slots) in [("first", "second", (0, 1)), ("second", "first", (1, 0))] {
        let text = program(initial, latest, false);
        let package = source::lower(&[("replaced.omg", &text)]).unwrap();
        let body = forward(&package);
        assert!(body.suspension_sites.is_empty());
        let records = records(body);
        let [(home, original), (replacement, latest_operand)] = records.as_slice() else {
            panic!("two constructions")
        };
        let (home, replacement) = (*home, *replacement);
        assert_eq!(*original, ValueOperand::CopyWhole(formal(body, slots.0)));
        assert_eq!(*latest_operand, ValueOperand::CopyWhole(formal(body, slots.1)));
        let stores: Vec<_> = body
            .operations
            .iter()
            .filter_map(|op| match &op.kind {
                OperationKind::StructuralStore { destination, value, .. } => Some((destination, *value)),
                _ => None,
            })
            .collect();
        let [(destination, value)] = stores.as_slice() else {
            panic!("one whole replacement")
        };
        assert!(destination.path.is_empty());
        assert_eq!((destination.root, *value), (home, replacement));
    }
}

#[test]
fn a_replaced_home_never_parks_with_its_first_descriptor() {
    for (initial, latest) in [("first", "second"), ("second", "first")] {
        let text = program(initial, latest, true);
        let files = [("replaced.omg", text.as_str())];
        let resolved = source::resolve(&files);
        let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
        let tree = prepared.typed().trees().next().unwrap();
        let file = tree.resolved().file;
        let mut inputs = Vec::new();
        for node in 0..file.nodes.len() {
            if let Some(construction) = tree.construction(node) {
                let [input] = construction.initializers.as_slice() else { panic!("one initializer") };
                assert_eq!(input.position, 0);
                inputs.push(file.span(input.expression).slice(text.as_bytes()).unwrap());
            }
        }
        assert_eq!(inputs, [initial.as_bytes(), latest.as_bytes()]);
        let package = source::lower_prepared(&prepared, &files).unwrap();
        drop(prepared);
        drop(resolved);
        let body = forward(&package);
        let records = records(body);
        let [(home, original), (rhs, installed)] = records.as_slice() else { panic!("two record results") };
        assert_ne!(home, rhs);
        assert_eq!(body.mutable_homes, [*home]);
        let latest_slot = usize::from(latest == "second");
        assert_eq!(*original, ValueOperand::CopyWhole(formal(body, 1 - latest_slot)));
        assert_eq!(*installed, ValueOperand::CopyWhole(formal(body, latest_slot)));
        let parked_origin = psi_handoff_consumer::parked_replacement_origin(&package).expect("one shared field in the parked destination");
        let incoming = |slot| EmbeddedLoanOrigin { formal: formal(body, slot), path: vec![], reborrows: vec![] };
        if latest == "second" {
            spec_corpus::observe_handoff!(psi "psi-output/replaced-home-second", assert_eq!(*parked_origin, incoming(1)));
        } else {
            spec_corpus::observe_handoff!(psi "psi-output/replaced-home-first", assert_eq!(*parked_origin, incoming(0)));
        }
    }
}

#[test]
fn successive_crossings_snapshot_the_same_home_between_stores() {
    let text = program("first", "second", true)
        .replace(";h=", ";suspend wait();h=")
        .replace(";suspend wait()}", ";suspend wait();h=Holder{r:first};suspend wait()}");
    let package = source::lower(&[("snapshots.omg", &text)]).unwrap();
    let body = forward(&package);
    let [home] = body.mutable_homes.as_slice() else { panic!("one home") };
    let origins: Vec<_> = body.suspension_sites.iter().map(|site| {
        let [owner] = site.retained_owners.as_slice() else { panic!("one owner") };
        let [loan] = owner.embedded_loans.as_slice() else { panic!("one embedded reference") };
        (owner.place, loan.origin.formal)
    }).collect();
    assert_eq!(origins, [(*home, formal(body, 0)), (*home, formal(body, 1)), (*home, formal(body, 0))]);
}

#[test]
fn an_ordinary_constructor_copies_the_descriptor_captured_from_a_field_borrow() {
    for statements in [
        "let loan:@Token=@h.token;let w=Ref{r:loan};",
        "let w=Ref{r:@h.token};",
    ] {
        let text = format!("data Token{{v:u8;}} data Holder{{token:Token;}} data Ref{{r:@Token;}} machine forward(h:Holder){{{statements}}}");
        let package = source::lower(&[("captured.omg", &text)]).unwrap();
        let body = forward(&package);
        assert!(body.suspension_sites.is_empty());
        let holder = formal(body, 0);
        let borrows: Vec<_> = body
            .operations
            .iter()
            .filter(|op| matches!(op.kind, OperationKind::BorrowPlace { .. }))
            .collect();
        let [borrow] = borrows.as_slice() else {
            panic!("exactly one captured borrow")
        };
        let OperationKind::BorrowPlace { source, access } = &borrow.kind else {
            unreachable!()
        };
        assert_eq!((source.root, *access), (holder, ReferenceAccess::Shared));
        assert!(matches!(source.path.as_slice(), [PlaceProjection::Field(field)] if field.path == "Holder::token"));
        let OperationResult::Structural(descriptor) = borrow.result else {
            panic!("borrowed descriptor")
        };
        assert_eq!(body.places[descriptor.index()].origin, PlaceOrigin::Operation(borrow.id));
        let fields = body
            .operations
            .iter()
            .find_map(|op| match &op.kind {
                OperationKind::EstablishRecord { fields } => Some(fields),
                _ => None,
            })
            .expect("constructed Ref");
        let [field] = fields.as_slice() else { panic!("one initializer") };
        assert_eq!(field.field.path, "Ref::r");
        assert_eq!(field.operand, ValueOperand::CopyWhole(descriptor));
    }
}
