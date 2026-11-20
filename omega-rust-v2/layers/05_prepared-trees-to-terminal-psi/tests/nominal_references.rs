#[path = "support/source.rs"]
mod sources;
const SOURCE: &str = "data Command {
    command_id: u64;
    op: data Operation { case GetUser(user_id: u64); case CountUsers; };
}
machine targets_one_user(op: @Command::Operation) -> bool {
    match op {
        Command::Operation::GetUser { user_id as _ } -> true,
        Command::Operation::CountUsers -> false,
    }
}
machine inspect(command: @Command) -> bool { targets_one_user(@command.op) }";
#[test]
fn projected_calls_observe_the_discriminator_of_the_original_field() {
    use omega_lowering::{BlockTerminator, CallArgument, OperationKind, OperationResult, Parameter, PlaceProjection, ReturnEdge, ScalarLiteral, TypeShape};
    let project = |field: &str| {
        let text = SOURCE
            .replace("op: data Operation", "other: Operation; op: data Operation")
            .replace("@command.op)", &format!("@command.{field})"));
        sources::lower(&[("pat.omg", &text)]).unwrap()
    };
    for field in ["op", "other"] {
        let pkg = project(field);
        let [inspect, targets] = pkg.machines.as_slice() else {
            panic!("expected both machines")
        };
        let [Parameter::Structural(command)] = inspect.body.parameters[..] else {
            panic!("expected the borrowed record")
        };
        let [call] = inspect.body.operations.as_slice() else {
            panic!("expected a call without a snapshot")
        };
        let OperationKind::Call {
            callee: omega_lowering::CallTarget::Machine(callee),
            arguments,
            ..
        } = &call.kind
        else {
            panic!("expected direct call")
        };
        assert_eq!(*callee, targets.id);
        let [
            CallArgument::Borrow {
                place,
                access: omega_lowering::ReferenceAccess::Shared,
            },
        ] = arguments.as_slice()
        else {
            panic!("expected shared access")
        };
        assert_eq!(place.root, command);
        let [PlaceProjection::Referent, PlaceProjection::Field(selected)] = place.path.as_slice() else {
            panic!("expected reference crossing and field")
        };
        assert_eq!(selected.path, format!("Command::{field}"));
        let [Parameter::Structural(op)] = targets.body.parameters[..] else {
            panic!("expected the borrowed sum")
        };
        let [observation, yes, no] = targets.body.operations.as_slice() else {
            panic!("expected tag test and both arm results")
        };
        assert!(
            matches!(&observation.kind,OperationKind::StructuralCaseMembership{source,case} if source.root==op && source.path==[PlaceProjection::Referent] && case.path=="Command::Operation::GetUser")
        );
        assert_eq!(yes.kind, OperationKind::Constant(ScalarLiteral::Boolean(true)));
        assert_eq!(no.kind, OperationKind::Constant(ScalarLiteral::Boolean(false)));
        let BlockTerminator::Conditional {
            condition,
            when_true,
            when_false,
        } = &targets.body.blocks[0].terminator
        else {
            panic!("expected case dispatch")
        };
        assert_eq!(observation.result, OperationResult::Scalar(*condition));
        for (edge, arm) in [(when_true, yes), (when_false, no)] {
            assert!(
                matches!(targets.body.blocks[edge.target.get()-1].terminator,BlockTerminator::Return{result:ReturnEdge::Scalar(v),..}if arm.result==OperationResult::Scalar(v))
            );
        }
        assert!(
            matches!(pkg.types.get(targets.parameter_types[0]),Some(TypeShape::NominalReference{referent,access:omega_lowering::ReferenceAccess::Shared}) if pkg.data[referent.get()-1].declaration.path=="Command::Operation")
        );
    }
}
#[test]
fn a_payload_comparison_cannot_be_lowered_as_a_whole_case_match() {
    let source = "data Choice {case A(value:u8);case B;} machine first(x:@Choice)->u8 {
        match x {Choice::A {value:1u8} -> 1u8, _ -> 0u8,}
    }";
    let err = sources::lower(&[("case.omg", source)]).unwrap_err();
    assert!(err.message.contains("payload comparison"), "{err:?}");
}

#[test]
fn mutable_call_writes_the_original_field_and_exits_with_the_affine_local_disposed() {
    use omega_lowering::{BlockTerminator, CallArgument, OperationKind, OperationResult, Parameter, PlaceProjection, ReferenceAccess, ReturnEdge, TypeShape};
    let source = "data Counter { first:u64; second:u64; } machine clear(c:@mut Counter) { c.second = 0; } machine example() {let mut counter:Counter;clear(@mut counter);}";
    let pkg = sources::lower(&[("counter.omg", source)]).unwrap();
    let [clear, example] = pkg.machines.as_slice() else {
        panic!("expected clear and example")
    };
    let body = &example.body;
    let [establish, call] = body.operations.as_slice() else {
        panic!("expected establishment and call")
    };
    assert_eq!(establish.kind, OperationKind::EstablishZeroedStructural);
    let OperationResult::Structural(home) = establish.result else {
        panic!("expected the owned local")
    };
    assert_eq!(body.mutable_homes, [home]);
    assert!(
        matches!(&call.kind,OperationKind::Call{arguments,callee: omega_lowering::CallTarget::Machine(callee),..} if *callee==clear.id && matches!(arguments.as_slice(),[CallArgument::Borrow{place,access:ReferenceAccess::Mutable}] if place.root==home && place.path.is_empty()))
    );
    assert!(
        matches!(&body.blocks[0].terminator,BlockTerminator::Return{result:ReturnEdge::Unit,cleanup,..} if cleanup==&[omega_lowering::CleanupAction::DiscardRoot(home)])
    );
    assert!(matches!(
        pkg.types.get(clear.parameter_types[0]),
        Some(TypeShape::NominalReference {
            access: ReferenceAccess::Mutable,
            ..
        })
    ));
    let [Parameter::Structural(c)] = clear.body.parameters[..] else {
        panic!("expected the caller's referent")
    };
    let [zero, store] = clear.body.operations.as_slice() else {
        panic!("expected the zero and store")
    };
    assert!(
        matches!(&store.kind, OperationKind::StructuralScalarFieldStore{destination,field,value} if destination.root==c && destination.path==[PlaceProjection::Referent] && field.path=="Counter::second" && OperationResult::Scalar(*value)==zero.result)
    );
}

#[test]
fn zero_provisioning_cannot_skip_an_unsatisfied_record_predicate() {
    let src = "data Nonzero requires self.value > 0 {value:u8;} machine f(){let mut n:Nonzero;}";
    let error = sources::lower(&[("zero.omg", src)]).unwrap_err();
    assert_eq!(error.message, "zero initialization requires record qualification or cleanup evidence");
}
