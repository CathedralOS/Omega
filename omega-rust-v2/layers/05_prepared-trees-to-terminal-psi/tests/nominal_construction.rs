#[path = "support/source.rs"]
mod sources;
use omega_lowering::{BlockTerminator, OperationKind, OperationResult, ScalarLiteral};
fn scalar(input: &omega_lowering::ValueOperand) -> omega_lowering::ValueId {
    match input {
        omega_lowering::ValueOperand::Scalar(value) => *value,
        _ => panic!("expected a scalar field"),
    }
}

#[test]
fn fields_evaluate_in_source_order_and_publish_into_declaration_slots() {
    let src = "data Reply {case Pair(left:u8,right:u8); case End;}
    machine make(){let pair = Reply::Pair {right:2,left:1}; let end = Reply::End;}";
    let pkg = sources::lower(&[("pair.omg", src)]).unwrap();
    let body = &pkg.machines[0].body;
    let [right, left, pair, end] = body.operations.as_slice() else {
        panic!("expected two inputs and two completed cases")
    };
    assert_eq!(right.kind, OperationKind::Constant(ScalarLiteral::Integer(2.into())));
    assert_eq!(left.kind, OperationKind::Constant(ScalarLiteral::Integer(1.into())));
    let OperationKind::EstablishCase { case, fields } = &pair.kind else {
        panic!("expected case construction")
    };
    assert_eq!(case.path, "Reply::Pair");
    let [a, b] = fields.as_slice() else { panic!("expected both fields") };
    assert_eq!(
        (&a.field.path[..], OperationResult::Scalar(scalar(&a.operand))),
        ("Reply::Pair::left", left.result)
    );
    assert_eq!(
        (&b.field.path[..], OperationResult::Scalar(scalar(&b.operand))),
        ("Reply::Pair::right", right.result)
    );
    assert!(matches!(&end.kind,OperationKind::EstablishCase{case,fields} if case.path=="Reply::End" && fields.is_empty()));
    let BlockTerminator::Return { cleanup, .. } = &body.blocks[0].terminator else {
        panic!("expected a normal exit")
    };
    assert_eq!(
        cleanup
            .iter()
            .map(|action| match action {
                omega_lowering::CleanupAction::DiscardRoot(id) => OperationResult::Structural(*id),
                _ => panic!("expected intact local"),
            })
            .collect::<Vec<_>>(),
        [end.result, pair.result]
    );
}

#[test]
fn constructed_case_uses_the_applied_nested_carrier() {
    let source="data Envelope<T>{body:data Body{case Value(value:T);case Empty;};}
    machine make(){let body: Envelope<u32>::Body = Envelope::<u32>::Body::Value {value:7}; let flag: Envelope<bool>::Body=Envelope::<bool>::Body::Value {value:true};}";
    let pkg = sources::lower(&[("envelope.omg", source)]).unwrap();
    let body = &pkg.machines[0].body;
    let [input, construct, flag, other] = body.operations.as_slice() else {
        panic!("expected two inputs and two constructors")
    };
    let OperationKind::EstablishCase { case, fields } = &construct.kind else {
        panic!("expected case construction")
    };
    assert_eq!(case.path, "Envelope::Body::Value");
    assert!(
        matches!(fields.as_slice(),[field] if field.field.path=="Envelope::Body::Value::value" && OperationResult::Scalar(scalar(&field.operand))==input.result)
    );
    let OperationResult::Structural(place) = construct.result else {
        panic!("expected owned result")
    };
    let schema = pkg
        .data
        .iter()
        .find(|d| d.carrier == body.places[place.index()].ty)
        .unwrap();
    let payload = schema.cases.iter().find(|c| &c.declaration == case).unwrap();
    assert_eq!(payload.fields[0].carrier, body.values[scalar(&fields[0].operand).index()].ty);
    let OperationResult::Structural(other_place) = other.result else {
        panic!("expected second owned result")
    };
    assert_ne!(body.places[place.index()].ty, body.places[other_place.index()].ty);
    let other_schema = pkg
        .data
        .iter()
        .find(|d| d.carrier == body.places[other_place.index()].ty)
        .unwrap();
    assert_ne!(schema.type_arguments, other_schema.type_arguments);
    let OperationKind::EstablishCase {
        case: other_case,
        fields: other_fields,
    } = &other.kind
    else {
        panic!("expected second constructor")
    };
    assert_eq!(other_case, case);
    assert_eq!(other_fields[0].field, fields[0].field);
    assert_eq!(OperationResult::Scalar(scalar(&other_fields[0].operand)), flag.result);
    assert_ne!(
        body.values[scalar(&fields[0].operand).index()].ty,
        body.values[scalar(&other_fields[0].operand).index()].ty
    );
    assert!(
        matches!(&body.blocks[0].terminator,BlockTerminator::Return{cleanup,..} if cleanup==&[omega_lowering::CleanupAction::DiscardRoot(other_place),omega_lowering::CleanupAction::DiscardRoot(place)])
    );
}

#[test]
fn a_record_constructor_cannot_drop_its_independent_establishment_requirement() {
    let src = "data Positive requires self.n > 0 {n:u8;} machine make(){let n=Positive{n:0};}";
    let err = sources::refuse(
        &[("pred.omg", src)],
        "pred.omg",
        sources::unique_span(src, "Positive{n:0}"),
        "lowering cannot establish the data predicate for `Positive` at construction",
    );
    assert_eq!(err.site.span.slice(src.as_bytes()), Some(b"Positive{n:0}".as_slice()));
}

#[test]
fn copy_record_is_writable_only_when_bound_mutably_and_needs_no_discard() {
    let src = "data Point [copy] {x:u8;} machine make(){let mut p=(Point{x:1});p.x=2;}";
    let pkg = sources::lower(&[("point.omg", src)]).unwrap();
    let body = &pkg.machines[0].body;
    let [one, construct, two, store] = body.operations.as_slice() else {
        panic!("expected a constructor and replacement")
    };
    let OperationResult::Structural(home) = construct.result else {
        panic!("expected a fresh local")
    };
    assert_eq!(body.mutable_homes, [home]);
    assert!(
        matches!(&construct.kind,OperationKind::EstablishRecord{fields} if matches!(fields.as_slice(),[field] if field.field.path=="Point::x" && OperationResult::Scalar(scalar(&field.operand))==one.result))
    );
    assert!(
        matches!(&store.kind,OperationKind::StructuralScalarFieldStore{destination,field,value} if destination.root==home && destination.path.is_empty() && field.path=="Point::x" && OperationResult::Scalar(*value)==two.result)
    );
    assert!(matches!(&body.blocks[0].terminator,BlockTerminator::Return{cleanup,..} if cleanup.is_empty()));

    let linear = src.replace("[copy]", "[linear]");
    // The implicit return belongs to the body, not to p's last use.
    sources::refuse(
        &[("linear.omg", &linear)],
        "linear.omg",
        sources::unique_span(&linear, "{let mut p=(Point{x:1});p.x=2;}"),
        "normal exit needs an authorized disposition for owned storage",
    );
}

#[test]
fn an_interned_applied_record_remains_a_structural_parameter() {
    let source = "data Cell<T>{value:T;} machine build(){let cell=Cell::<u8>{value:7};} machine accept(cell:Cell<u8>){}";
    let package = sources::lower(&[("cell.omg", source)]).unwrap();
    let accept = package
        .machines
        .iter()
        .find(|m| m.declaration.path == "accept")
        .unwrap();
    assert!(matches!(accept.body.parameters[..], [omega_lowering::Parameter::Structural(_)]));
}
