#[path = "support/source.rs"]
mod source;

use omega_psi::{
    EmbeddedLoanOrigin, EmbeddedLoanSource, LoweredPackage, OwnedCustody, Parameter,
    PlaceId, PlaceProjection, ReferenceAccess, RetainedOwner, TypeShape,
};

const CASES: &str = "data Choice<'r> { case Plain(v:u8); case Left(r:@'r u8); case Right(r:@'r u8); }
    machine wait() suspends {}";

fn lower(machine: &str) -> LoweredPackage {
    source::lower(&[("sum.omg", &format!("{CASES}\n{machine}"))]).unwrap()
}

fn retained(package: &LoweredPackage) -> (&[Parameter], &RetainedOwner) {
    let body = &package.machines.iter().find(|machine| machine.declaration.path == "forward").unwrap().body;
    let [site] = body.suspension_sites.as_slice() else { panic!("one crossing") };
    let [owner] = site.retained_owners.as_slice() else { panic!("one sum owner") };
    assert_eq!(owner.custody, OwnedCustody::Caller);
    assert!(owner.moved.is_empty() && site.retained_loans.is_empty());
    (&body.parameters, owner)
}

fn payload_path(package: &LoweredPackage, owner: &RetainedOwner, name: &str) -> Vec<PlaceProjection> {
    let choice = package.data.iter().find(|data| data.declaration.path == "Choice").unwrap();
    assert_eq!(owner.ty, choice.carrier);
    let case = choice.cases.iter().find(|case| case.declaration.path == format!("Choice::{name}")).unwrap();
    let [field] = case.fields.as_slice() else { panic!("one payload field") };
    assert_eq!(field.declaration.path, format!("Choice::{name}::r"));
    assert!(matches!(package.types.get(field.carrier), Some(TypeShape::ScalarReference { access: ReferenceAccess::Shared, .. })));
    vec![PlaceProjection::Case(case.declaration.clone()), PlaceProjection::Field(field.declaration.clone())]
}

fn assert_formal(origin: &EmbeddedLoanOrigin, expected: PlaceId, path: &[PlaceProjection]) {
    assert!(origin.reborrows.is_empty());
    assert_eq!(origin.source, EmbeddedLoanSource::Formal { formal: expected, path: path.to_vec() });
}

#[test]
fn incoming_sum_qualifies_each_possible_loan_with_its_own_case() {
    let package = lower("machine forward<'r>(value:Choice<'r>)->Choice<'r> suspends { suspend wait(); value }");
    let (parameters, owner) = retained(&package);
    assert_eq!(parameters, [Parameter::Structural(owner.place)]);
    let [left, right] = owner.embedded_loans.as_slice() else { panic!("only the two reference arms") };
    for (loan, name) in [(left, "Left"), (right, "Right")] {
        let path = payload_path(&package, owner, name);
        assert_eq!(loan.path, path);
        assert_eq!(loan.access, ReferenceAccess::Shared);
        assert!(matches!(package.types.get(loan.ty), Some(TypeShape::ScalarReference { access: ReferenceAccess::Shared, .. })));
        let [origin] = loan.origins.as_slice() else { panic!("one formal origin") };
        assert!(origin.arrival.is_empty());
        assert_formal(origin, owner.place, &path);
    }
}

#[test]
fn a_known_constructor_excludes_inactive_same_shape_payloads() {
    for (name, argument) in [("Left", "first"), ("Right", "second"), ("Plain", "7")] {
        let field = if name == "Plain" { "v" } else { "r" };
        let package = lower(&format!("machine forward<'r>(first:@'r u8,second:@'r u8)->Choice<'r> suspends {{
            let value:Choice<'r> = Choice::{name}{{{field}:{argument}}}; suspend wait(); value }}"));
        let (parameters, owner) = retained(&package);
        if name == "Plain" {
            assert!(owner.embedded_loans.is_empty());
            continue;
        }
        let [loan] = owner.embedded_loans.as_slice() else { panic!("only the active arm contains a loan") };
        assert_eq!(loan.path, payload_path(&package, owner, name));
        let [origin] = loan.origins.as_slice() else { panic!("one descriptor initializer") };
        assert!(origin.arrival.is_empty());
        let slot = usize::from(name == "Right");
        let Parameter::Structural(formal) = parameters[slot] else { panic!("reference formal") };
        assert_formal(origin, formal, &[]);
    }
}

#[test]
fn an_inactive_exclusive_payload_needs_no_backing() {
    let declarations = "data Choice<'r> { case Plain(v:u8); case Packed(r:@'r mutable u8); }
        machine wait() suspends {}";
    let plain = format!("{declarations}
        machine forward<'r>(r:@'r u8)->Choice<'r> suspends {{
            let value:Choice<'r> = Choice::Plain{{v:7}}; suspend wait(); value
        }}");
    let package = source::lower(&[("exclusive.omg", &plain)]).unwrap();
    let (_, owner) = retained(&package);
    let choice = package.data.iter().find(|data| data.declaration.path == "Choice").unwrap();
    assert_eq!(owner.ty, choice.carrier);
    assert!(owner.embedded_loans.is_empty());

    let unknown_case = format!("{declarations}
        machine forward<'r>(incoming:Choice<'r>)->Choice<'r> suspends {{
            let value = move incoming; suspend wait(); value
        }}");
    let package = source::lower(&[("exclusive.omg", &unknown_case)]).unwrap();
    let (parameters, owner) = retained(&package);
    let [Parameter::Structural(formal)] = parameters else { panic!("one incoming sum") };
    let [loan] = owner.embedded_loans.as_slice() else { panic!("one possible exclusive descriptor") };
    assert_ne!(owner.place, *formal);
    assert_eq!(loan.access, ReferenceAccess::Mutable);
    assert!(matches!(loan.path.as_slice(), [PlaceProjection::Case(case), PlaceProjection::Field(field)]
        if case.path == "Choice::Packed" && field.path == "Choice::Packed::r"));
    let [origin] = loan.origins.as_slice() else { panic!("one origin in the moved formal") };
    assert_formal(origin, *formal, &loan.path);

}

#[test]
fn a_completed_call_preserves_its_exact_result_path_and_applied_region() {
    let text = "data Choice<'r> { case Plain(v:u8); case Packed(r:@'r mutable u8); }
        machine wait() suspends {}
        machine relay<'r>(incoming:Choice<'r>)->Choice<'r> { incoming }
        machine consume<'r>(incoming:Choice<'r>) {}
        machine forward<'a,'b>(first:Choice<'a>,second:Choice<'b>)->Choice<'b> suspends {
            let unused = relay::<'a>(first); consume::<'a>(unused);
            let value = relay::<'b>(second); suspend wait(); value
        }";
    let package = source::lower(&[("exclusive.omg", text)]).unwrap();
    let (parameters, owner) = retained(&package);
    let [_, Parameter::Structural(second)] = parameters else { panic!("two source formals") };
    let body = &package.machines.iter().find(|machine| machine.declaration.path == "forward").unwrap().body;
    let [loan] = owner.embedded_loans.as_slice() else { panic!("one possible exclusive result descriptor") };
    let [origin] = loan.origins.as_slice() else { panic!("one producing call") };
    let EmbeddedLoanSource::CallResult { operation, result, path } = &origin.source else { panic!("origin in a completed result") };
    let producing = &body.operations[operation.index()];
    assert_eq!(producing.result, omega_psi::OperationResult::Structural(*result));
    assert_eq!(path, &loan.path);
    assert!(matches!(path.as_slice(), [PlaceProjection::Case(case), PlaceProjection::Field(field)]
        if case.path == "Choice::Packed" && field.path == "Choice::Packed::r"));
    assert_eq!(loan.access, ReferenceAccess::Mutable);
    let omega_psi::OperationKind::Call { callee, arguments, .. } = &producing.kind else { panic!("call result") };
    let [omega_psi::ValueOperand::MoveWhole(actual)] = arguments.as_slice() else { panic!("one transferred input") };
    let omega_psi::PlaceOrigin::Operation(transfer) = body.places[actual.index()].origin else { panic!("transferred source") };
    assert!(matches!(&body.operations[transfer.index()].kind,
        omega_psi::OperationKind::MoveStructural { source } if source.root == *second && source.path.is_empty()));
    let [first_call, second_call] = body.operations.iter().filter(|op| matches!(&op.kind,
        omega_psi::OperationKind::Call { callee: selected, .. } if selected == callee)).collect::<Vec<_>>()[..]
        else { panic!("two calls of the same machine") };
    assert_ne!(first_call.id, *operation);
    assert_eq!(second_call.id, *operation);
    assert_eq!(body.call_regions.iter().find(|row| row.call == omega_psi::CallSite::Operation(*operation)).unwrap().arguments,
        [omega_psi::CallRegion::Binder(1)]);
}

#[test]
fn alternative_arrivals_keep_each_origin_on_its_own_edge() {
    let package = lower("machine forward<'r>(flag:bool, first:@'r u8, second:@'r u8)->Choice<'r> suspends {
        let value:Choice<'r> = match flag {
            true -> Choice::Left{r:first}, false -> Choice::Right{r:second},
        }; suspend wait(); value }");
    let (parameters, owner) = retained(&package);
    let [_, Parameter::Structural(first), Parameter::Structural(second)] = parameters else { panic!("two reference formals") };
    let [left, right] = owner.embedded_loans.as_slice() else { panic!("both possible payloads") };
    let body = &package.machines.iter().find(|machine| machine.declaration.path == "forward").unwrap().body;
    for (loan, name, formal) in [(left, "Left", first), (right, "Right", second)] {
        assert_eq!(loan.path, payload_path(&package, owner, name));
        let [origin] = loan.origins.as_slice() else { panic!("one origin for this arm") };
        assert_formal(origin, *formal, &[]);
        let [arrival] = origin.arrival.as_slice() else { panic!("the predecessor that built this case") };
        let producing_block = body.blocks.iter().find(|block| matches!(&block.terminator, omega_psi::BlockTerminator::Jump(edge) if edge.id == *arrival)).unwrap();
        let constructed = body.operations[producing_block.operations.clone()].iter().find_map(|op| match &op.kind {
            omega_psi::OperationKind::EstablishCase { case, .. } => Some(case.path.as_str()),
            _ => None,
        });
        spec_corpus::observe_handoff!(psi "psi-output/parked-sum-payload-origins",
            assert_eq!((&loan.path, &origin.source, constructed),
                (&payload_path(&package, owner, name), &EmbeddedLoanSource::Formal { formal: *formal, path: vec![] },
                    Some(format!("Choice::{name}").as_str()))),
        );
    }
}

#[test]
fn a_returned_sum_moves_the_same_formal_with_or_without_a_park() {
    for (effect, wait) in [("", ""), ("suspends", "suspend wait();")] {
        for value in ["value", "move value"] {
            let text = format!("{CASES}\nmachine forward<'r>(value:Choice<'r>)->Choice<'r> {effect} {{ {wait} {value} }}");
            let resolved = source::resolve(&[("return.omg", &text)]);
            let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
            let return_loan = prepared.typed().obligations.iter().find(|row| matches!(&row.kind,
                omega_typed::ObligationKind::ReturnBorrow { machine } if machine.path == "forward")).unwrap();
            if value == "move value" {
                assert!(prepared.typed().obligations.iter().any(|row| row.node == return_loan.node
                    && matches!(&row.kind, omega_typed::ObligationKind::Move { ty, .. }
                        if ty.nominal_parts().is_some_and(|(data, _)| data.path == "Choice"))));
            }
            let package = omega_lowering::lower_package(&prepared).unwrap();
            let body = &package.machines.iter().find(|machine| machine.declaration.path == "forward").unwrap().body;
            let [Parameter::Structural(formal)] = body.parameters.as_slice() else { panic!("one incoming sum") };
            let result_source = body.blocks.iter().find_map(|block| match &block.terminator {
                omega_psi::BlockTerminator::Return { result: omega_psi::ReturnEdge::Structural { source, .. }, .. } => Some(*source),
                _ => None,
            }).unwrap();
            let returned = body.operations.iter().find(|op| op.result == omega_psi::OperationResult::Structural(result_source)).unwrap();
            assert!(matches!(&returned.kind, omega_psi::OperationKind::MoveStructural { source }
                if source.root == *formal && source.path.is_empty()));
            assert_eq!(body.suspension_sites.len(), usize::from(!wait.is_empty()));
        }
    }
}

#[test]
fn a_callee_owned_sum_keeps_case_qualified_origins_from_the_moved_formal() {
    let package = lower("machine take<'r>(item:Choice<'r>) suspends {}
        machine forward<'r>(value:Choice<'r>) suspends { suspend take(move value); }");
    let body = &package.machines.iter().find(|machine| machine.declaration.path == "forward").unwrap().body;
    let [Parameter::Structural(formal)] = body.parameters.as_slice() else { panic!("one formal") };
    let [site] = body.suspension_sites.as_slice() else { panic!("one crossing") };
    let [owner] = site.retained_owners.as_slice() else { panic!("the callee owns the moved sum") };
    assert_eq!(owner.custody, OwnedCustody::CalleeArgument(0));
    let [left, right] = owner.embedded_loans.as_slice() else { panic!("one loan in either reference payload") };
    for (loan, name) in [(left, "Left"), (right, "Right")] {
        let path = payload_path(&package, owner, name);
        assert_eq!(loan.path, path);
        let [origin] = loan.origins.as_slice() else { panic!("one origin") };
        assert_formal(origin, *formal, &path);
    }
}
