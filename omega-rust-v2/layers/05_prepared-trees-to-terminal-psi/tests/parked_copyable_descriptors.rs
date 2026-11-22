//! A field loan can need a copyable home whose other field contains a loan.
#[path = "support/source.rs"]
mod source;

use omega_lowering::{
    CallSite, EmbeddedLoanOrigin, EmbeddedLoanSource, LoweredPackage, MachineBody, OperationKind, Parameter, PlaceOrigin, PlaceProjection, ReferenceAccess,
    TypeShape, ValueOperand,
};
use psi_handoff_consumer::parked_copyable_homes;

const TYPES: &str = "data Item<'r> [copy] {key:u64; other:u64; link:@'r u8;} machine wait(k:@u64) suspends {}";

fn body(pkg: &LoweredPackage) -> &MachineBody {
    &pkg.machines
        .iter()
        .find(|m| m.declaration.path == "f")
        .unwrap()
        .body
}

/// Read the call and descriptor rows independently. The two scalar fields
/// have the same type, as do the two root formals.
fn check_field(
    pkg: &LoweredPackage,
    slot: usize,
    field: &str,
) -> Result<(), &'static str> {
    let body = body(pkg);
    let [site] = body.suspension_sites.as_slice() else {
        return Err("expected one suspension");
    };
    let Parameter::Structural(expected) = body.parameters[slot] else {
        return Err("expected structural formal");
    };
    let homes = parked_copyable_homes(pkg, body, site)?;
    let [home] = homes.as_slice() else { return Err("expected one home") };
    if home.place != expected || !home.needed_by_callee || home.needed_after_completion {
        return Err("wrong retained home");
    }
    let CallSite::Operation(id) = site.call else {
        return Err("expected ordinary call");
    };
    let OperationKind::Call { arguments, .. } = &body.operations[id.index()].kind else {
        return Err("wrong call");
    };
    let [
        ValueOperand::Borrow {
            place,
            access: ReferenceAccess::Shared,
        },
    ] = arguments.as_slice()
    else {
        return Err("wrong loan operand");
    };
    let source = match site.retained_loans.as_slice() {
        [] => place,
        [retained]
            if retained.descriptor == place.root
                && place.path == [PlaceProjection::Referent]
                && retained.needed_by_callee
                && retained.access == ReferenceAccess::Shared =>
        {
            let PlaceOrigin::Operation(op) = body.places[place.root.index()].origin else {
                return Err("loan has no source");
            };
            let OperationKind::BorrowPlace {
                source,
                access: ReferenceAccess::Shared,
            } = &body.operations[op.index()].kind
            else {
                return Err("wrong reborrow");
            };
            source
        }
        _ => return Err("wrong named loan"),
    };
    if source.root != expected || !matches!(source.path.as_slice(), [PlaceProjection::Field(f)] if f.path == field) {
        return Err("wrong borrowed field");
    }
    let [row] = site.retained_homes.as_slice() else { return Err("missing home") };
    let [loan] = row.embedded_loans.as_slice() else {
        return Err("missing stored loan");
    };
    let Some(TypeShape::Nominal(data)) = pkg.types.get(row.ty) else {
        return Err("wrong aggregate");
    };
    let link = pkg.data[data.get() - 1]
        .fields
        .iter()
        .find(|f| f.declaration.path == "Item::link")
        .unwrap();
    let path = vec![PlaceProjection::Field(link.declaration.clone())];
    if loan.path != path || loan.ty != link.carrier || loan.access != ReferenceAccess::Shared {
        return Err("wrong stored field");
    }
    if loan.origins
        != [EmbeddedLoanOrigin {
            arrival: vec![],
            reborrows: vec![],
            source: EmbeddedLoanSource::Formal { formal: expected, path },
        }]
    {
        return Err("wrong stored loan origin");
    }
    Ok(())
}

#[test]
fn the_selected_scalar_and_the_stored_descriptor_keep_their_distinct_places() {
    for (owner, slot) in [("a", 0), ("b", 1)] {
        for (field, member) in [("key", "Item::key"), ("other", "Item::other")] {
            for named in [false, true] {
                let call = if named {
                    format!("let k = @{owner}.{field}; suspend wait(k)")
                } else {
                    format!("suspend wait(@{owner}.{field})")
                };
                let program = format!("{TYPES} machine f<'r>(a:Item<'r>,b:Item<'r>) suspends {{ {call} }}");
                let mut pkg = source::lower(&[("field.omg", &program)]).unwrap();
                spec_corpus::observe_handoff!(psi "psi-output/copyable-home-descriptor-backing", assert_eq!(check_field(&pkg,slot,member), Ok(())));
                let body = &mut pkg
                    .machines
                    .iter_mut()
                    .find(|m| m.declaration.path == "f")
                    .unwrap()
                    .body;
                let Parameter::Structural(other) = body.parameters[1 - slot] else {
                    unreachable!()
                };
                let loan = &mut body.suspension_sites[0].retained_homes[0].embedded_loans[0];
                let EmbeddedLoanSource::Formal { formal, .. } = &mut loan.origins[0].source else {
                    unreachable!()
                };
                *formal = other;
                assert_eq!(check_field(&pkg, slot, member), Err("wrong stored loan origin"));
            }
        }
    }
}

#[test]
fn redirecting_the_borrow_to_an_equal_typed_field_changes_the_handoff() {
    let program = format!("{TYPES} machine f<'r>(a:Item<'r>,b:Item<'r>) suspends {{ suspend wait(@a.key) }}");
    let mut pkg = source::lower(&[("field.omg", &program)]).unwrap();
    let other = pkg
        .data
        .iter()
        .find(|d| d.declaration.path == "Item")
        .unwrap()
        .fields
        .iter()
        .find(|f| f.declaration.path == "Item::other")
        .unwrap()
        .declaration
        .clone();
    let body = &mut pkg
        .machines
        .iter_mut()
        .find(|m| m.declaration.path == "f")
        .unwrap()
        .body;
    let CallSite::Operation(id) = body.suspension_sites[0].call else {
        unreachable!()
    };
    let OperationKind::Call { arguments, .. } = &mut body.operations[id.index()].kind else {
        unreachable!()
    };
    let [ValueOperand::Borrow { place, .. }] = arguments.as_mut_slice() else {
        unreachable!()
    };
    place.path[0] = PlaceProjection::Field(other);
    assert_eq!(check_field(&pkg, 0, "Item::key"), Err("wrong borrowed field"));
}

#[test]
fn a_constructed_copyable_home_keeps_the_selected_descriptor_initializer() {
    for (input, slot) in [("first", 0), ("second", 1)] {
        let program =
            format!("{TYPES} machine f(first:@u8,second:@u8) suspends {{ let item=Item{{key:1u64,other:2u64,link:{input}}}; suspend wait(@item.key) }}");
        let pkg = source::lower(&[("local.omg", &program)]).unwrap();
        let body = body(&pkg);
        let [row] = body.suspension_sites[0].retained_homes.as_slice() else {
            panic!("one caller home")
        };
        let [loan] = row.embedded_loans.as_slice() else {
            panic!("one stored descriptor")
        };
        let Parameter::Structural(formal) = body.parameters[slot] else {
            unreachable!()
        };
        assert!(matches!(loan.path.as_slice(),[PlaceProjection::Field(f)] if f.path=="Item::link"));
        assert_eq!(
            loan.origins,
            [EmbeddedLoanOrigin {
                arrival: vec![],
                reborrows: vec![],
                source: EmbeddedLoanSource::Formal { formal, path: vec![] }
            }]
        );
    }
}

#[test]
fn a_continuation_retains_the_nested_descriptor_at_the_incoming_path() {
    let text = "data Inner<'r> [copy] {link:@'r u8;} data Outer<'r> [copy] {inner:Inner<'r>;key:u64;}
                machine wait()->u8 suspends {1u8}
                machine f<'r>(x:Outer<'r>)->u64 suspends { let a:u8 = suspend wait(); x.key }";
    let pkg = source::lower(&[("later.omg", text)]).unwrap();
    let body = body(&pkg);
    let [site] = body.suspension_sites.as_slice() else { panic!("one suspension") };
    let [home] = site.retained_homes.as_slice() else {
        panic!("one retained home")
    };
    let [Parameter::Structural(root)] = body.parameters.as_slice() else {
        panic!("one input home")
    };
    assert_eq!((home.place, home.needed_by_callee, home.needed_after_completion), (*root, false, true));
    let [loan] = home.embedded_loans.as_slice() else {
        panic!("one nested descriptor")
    };
    assert!(matches!(loan.path.as_slice(),[PlaceProjection::Field(a),PlaceProjection::Field(b)] if a.path=="Outer::inner" && b.path=="Inner::link"));
    assert_eq!(
        loan.origins,
        [EmbeddedLoanOrigin {
            arrival: vec![],
            reborrows: vec![],
            source: EmbeddedLoanSource::Formal {
                formal: *root,
                path: loan.path.clone()
            }
        }]
    );
}
