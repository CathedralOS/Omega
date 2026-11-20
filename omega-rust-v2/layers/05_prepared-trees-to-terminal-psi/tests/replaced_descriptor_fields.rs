#[path = "support/known_gap.rs"]
mod known_gap;
#[path = "support/source.rs"]
mod source;

use omega_lowering::{MachineBody, OperationKind, OperationResult, Parameter, PlaceId, PlaceOrigin, PlaceProjection, ReferenceAccess, ValueOperand};

const UNTRACED: &str = "parking this owner requires the backing or carry evidence of its embedded descriptors";

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
        let err = source::lower(&[("replaced.omg", &text)]).unwrap_err();
        assert_eq!(err.message, UNTRACED);
        known_gap::report(&format!("suspend after replacing {initial} with {latest}"), "The parked record needs the latest descriptor's origin", &text, &err);
        assert!(
            err.site
                .span
                .slice(text.as_bytes())
                .unwrap()
                .ends_with(b"wait()")
        );
    }
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
