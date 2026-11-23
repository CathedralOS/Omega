//! The affected domain comes from the destination's schema. A store and a
//! later use of that schema have distinct program points in Terminal Psi.
#[path = "support/source.rs"]
mod sources;

use omega_lowering::{
    BlockTerminator, LoweredPackage, OperationKind, OperationResult, Parameter, PlaceProjection, PredicateOperator, PredicateStep, ReferenceAccess,
    ScalarLiteral, TypeShape, ValueOrigin,
};
use std::collections::BTreeMap;

const PAIR: &str = "data Pair requires self.lo <= self.hi { lo:u8; hi:u8; spare:u8; }";

/// For these straight-line bodies, replay each actual SSA constant into the
/// mutable formal's referent. Evaluate the predicate published by its schema
/// after each write and at every call and handback. No source is used here.
fn observations(package: &LoweredPackage) -> Vec<(&'static str, bool)> {
    let machine = package
        .machines
        .iter()
        .find(|m| m.declaration.path == "store")
        .unwrap();
    let body = &machine.body;
    let [Parameter::Structural(root)] = body.parameters[..] else {
        panic!("one mutable formal")
    };
    let TypeShape::NominalReference {
        referent,
        access: ReferenceAccess::Mutable,
    } = package.types.get(body.places[root.index()].ty).unwrap()
    else {
        panic!("expected a mutable record")
    };
    let schema = &package.data[referent.get() - 1];
    let [clause] = &schema.requires[..] else { panic!("missing default domain") };
    let [
        PredicateStep::SelfValue,
        PredicateStep::Field(lo),
        PredicateStep::SelfValue,
        PredicateStep::Field(hi),
        PredicateStep::Apply(PredicateOperator::LessOrEqual),
    ] = &clause.predicate[..]
    else {
        panic!("missing ordered field relation")
    };
    let mut contents: BTreeMap<_, u8> = schema
        .fields
        .iter()
        .map(|f| (f.declaration.clone(), 0))
        .collect();
    let mut observations = Vec::new();
    let [block] = &body.blocks[..] else { panic!("expected straight-line body") };
    for op in &body.operations[block.operations.clone()] {
        let event = match &op.kind {
            OperationKind::Constant(_) => continue,
            OperationKind::StructuralScalarFieldStore { destination, field, value } => {
                assert_eq!((destination.root, destination.path.as_slice()), (root, &[PlaceProjection::Referent][..]));
                let ValueOrigin::Operation(definition) = body.values[value.index()].origin else {
                    panic!("expected constant source")
                };
                let producer = &body.operations[definition.index()];
                assert_eq!(producer.result, OperationResult::Scalar(*value));
                assert!(definition.index() < op.id.index());
                let OperationKind::Constant(ScalarLiteral::Integer(n)) = &producer.kind else {
                    panic!("integer value")
                };
                *contents.get_mut(field).expect("field belongs to this record") = n.try_into().unwrap();
                "write"
            }
            OperationKind::Call { callee, arguments, .. } => {
                let see = package
                    .machines
                    .iter()
                    .find(|m| m.declaration.path == "see")
                    .unwrap();
                assert_eq!(callee, &omega_lowering::CallTarget::Machine(see.id));
                assert!(
                    matches!(arguments.as_slice(), [omega_lowering::ValueOperand::Borrow { place, access: ReferenceAccess::Mutable }]
                    if place.root == root && place.path == [PlaceProjection::Referent]),
                    "expected a mutable reborrow: {arguments:?}"
                );
                "call"
            }
            step => panic!("unexpected operation {step:?}"),
        };
        observations.push((event, contents[lo] <= contents[hi]));
    }
    assert!(matches!(block.terminator, BlockTerminator::Return { .. }));
    observations.push(("handback", contents[lo] <= contents[hi]));
    observations
}

#[test]
fn scalar_writes_retain_the_facts_needed_to_distinguish_repair_from_invalid_consumption() {
    for (statements, expected) in [
        ("p.spare=3u8;", vec![("write", true), ("handback", true)]),
        ("p.lo=9u8; p.hi=9u8;", vec![("write", false), ("write", true), ("handback", true)]),
        ("p.hi=0u8; p.lo=9u8;", vec![("write", true), ("write", false), ("handback", false)]),
        (
            "p.lo=9u8; see(p); p.hi=9u8;",
            vec![
                ("write", false),
                ("call", false),
                ("write", true),
                ("handback", true),
            ],
        ),
    ] {
        let text = format!("{PAIR} machine see(p:@mutable Pair){{}} machine store(p:@mutable Pair){{{statements}}}");
        let package = sources::lower(&[("window.omg", &text)]).unwrap();
        assert_eq!(observations(&package), expected, "{statements}");
        let machine = package
            .machines
            .iter()
            .find(|m| m.declaration.path == "store")
            .unwrap();
        let body = &machine.body;
        let sites: Vec<_> = body
            .operations
            .iter()
            .filter(|op| matches!(op.kind, OperationKind::StructuralScalarFieldStore { .. }))
            .map(|op| {
                body.diagnostics.operations[op.id.index()]
                    .span
                    .slice(text.as_bytes())
                    .unwrap()
            })
            .collect();
        let written: Vec<_> = statements
            .split(';')
            .filter_map(|stmt| stmt.trim().split_once('=').map(|(field, _)| field.as_bytes()))
            .collect();
        assert_eq!(sites, written);
    }
}

#[test]
fn a_nested_field_write_identifies_both_affected_instances() {
    let text = format!(
        "{PAIR} data Box requires self.p.lo <= self.cap {{ p:Pair; cap:u8; }}
                        machine store(b:@mutable Box){{ b.p.lo=9u8; b.p.hi=9u8; b.cap=9u8; }}"
    );
    let package = sources::lower(&[("nested.omg", &text)]).unwrap();
    let body = &package.machines[0].body;
    let [Parameter::Structural(root)] = body.parameters[..] else {
        panic!("formal")
    };
    let TypeShape::NominalReference { referent, .. } = package.types.get(body.places[root.index()].ty).unwrap() else {
        panic!("box")
    };
    let outer = &package.data[referent.get() - 1];
    let [
        PredicateStep::SelfValue,
        PredicateStep::Field(p),
        PredicateStep::Field(nested_lo),
        PredicateStep::SelfValue,
        PredicateStep::Field(cap),
        PredicateStep::Apply(PredicateOperator::LessOrEqual),
    ] = outer.requires[0].predicate.as_slice()
    else {
        panic!("outer relation")
    };
    let p_field = outer.fields.iter().find(|f| f.declaration == *p).unwrap();
    let TypeShape::Nominal(id) = package.types.get(p_field.carrier).unwrap() else {
        panic!("inner pair")
    };
    let inner = &package.data[id.get() - 1];
    let [
        PredicateStep::SelfValue,
        PredicateStep::Field(lo),
        PredicateStep::SelfValue,
        PredicateStep::Field(hi),
        PredicateStep::Apply(PredicateOperator::LessOrEqual),
    ] = inner.requires[0].predicate.as_slice()
    else {
        panic!("inner relation")
    };
    assert_eq!(lo, nested_lo);
    let stores: Vec<_> = body
        .operations
        .iter()
        .filter_map(|o| match &o.kind {
            OperationKind::StructuralScalarFieldStore { destination, field, .. } => Some((destination, field)),
            _ => None,
        })
        .collect();
    let inner_path = &[PlaceProjection::Referent, PlaceProjection::Field(p.clone())][..];
    let expected = [
        (inner_path, lo),
        (inner_path, hi),
        (&[PlaceProjection::Referent][..], cap),
    ];
    assert_eq!(stores.len(), expected.len());
    for ((place, field), (path, member)) in stores.into_iter().zip(expected) {
        assert_eq!((place.root, place.path.as_slice(), field), (root, path, member));
    }
}
