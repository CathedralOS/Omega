//! The declaring trait, requirement binders and implementation each retain their own identity.
#[path = "support/source.rs"]
mod source;
use omega_lowering::{
    CallTarget, DeclarationTerm, LoweredPackage, MemberCallSelection, MemberRealization, OperationKind, ScalarType, StaticArgument, TemplateBinder,
    TypeExpression, TypeExpressionStep, TypeShape,
};

fn scalar(ty: ScalarType) -> DeclarationTerm {
    DeclarationTerm::Type(TypeExpression(vec![TypeExpressionStep::Scalar(ty)]))
}
const U8: ScalarType = ScalarType::Integer { signed: false, width: 8 };

fn machine<'a>(
    pkg: &'a LoweredPackage,
    name: &str,
) -> &'a omega_lowering::LoweredMachine {
    pkg.machines
        .iter()
        .find(|m| m.declaration.path == name)
        .unwrap_or_else(|| panic!("missing machine {name}"))
}

/// The selection made by the only operation of a single-call body.
fn only_member_call<'a>(
    pkg: &'a LoweredPackage,
    body: &'a omega_lowering::LoweredMachine,
) -> &'a MemberCallSelection {
    let [op] = body.body.operations.as_slice() else {
        panic!("expected one member call in {}", body.declaration.path)
    };
    psi_handoff_consumer::member_call(pkg, &op.kind)
        .expect("lost member selection")
        .0
}

#[test]
fn generic_boundary_method_publishes_its_binders_and_each_static_argument_tuple() {
    let src = "boundary trait Codec { machine pass<T>(x:T)->T reaches Codec crashes none; } \
        machine small(x:u8)->u8 reaches Codec { Codec::pass::<u8>(x) } \
        machine flag(x:bool)->bool reaches Codec { Codec::pass::<bool>(x) }";
    let pkg = source::lower(&[("codec.omg", src)]).unwrap();
    let codec = &pkg.traits[0];
    assert!(codec.requirements.is_empty());
    let [pass] = codec.dependent_requirements.as_slice() else {
        panic!("expected the generic requirement")
    };
    assert_eq!(pass.declaration.path, "Codec::pass");
    assert_eq!(pass.parameters, [TemplateBinder::Type { multiplicity: None }]);
    let open = TypeExpression(vec![TypeExpressionStep::GenericParameter {
        owner: pass.declaration.clone(),
        index: 0,
    }]);
    assert_eq!(pass.signature.parameters.as_slice(), std::slice::from_ref(&open));
    assert_eq!(pass.signature.result, Some(open));

    let small = machine(&pkg, "small");
    let flag = machine(&pkg, "flag");
    let (a, b) = (only_member_call(&pkg, small), only_member_call(&pkg, flag));
    for (selection, caller, argument) in [(a, small, scalar(U8)), (b, flag, scalar(ScalarType::Boolean))] {
        assert_eq!(selection.owner.declaration, codec.declaration);
        assert_eq!(selection.requirement.declaration, pass.declaration);
        assert_eq!(selection.target, MemberRealization::Boundary { direction: None });
        assert_eq!(selection.static_arguments, [argument]);
        assert_eq!(selection.parameters, [omega_lowering::ParameterType::Scalar(caller.parameter_types[0])]);
        assert_eq!(selection.result, caller.result_type);
    }
    // Same requirement, distinct closed applications.
    assert_eq!(a.requirement, b.requirement);
    assert_ne!(a.static_arguments, b.static_arguments);
    assert_eq!(pkg.member_calls.len(), 2);
}

#[test]
fn const_indexed_boundary_method_keeps_each_canonical_static_value() {
    let src = "boundary trait Ring { machine slot<const N:u64>(x:u8)->u8 reaches Ring crashes none; } \
        machine low(x:u8)->u8 reaches Ring { Ring::slot::<4>(x) } \
        machine high(x:u8)->u8 reaches Ring { Ring::slot::<9>(x) }";
    let pkg = source::lower(&[("ring.omg", src)]).unwrap();
    let u64_carrier = ScalarType::Integer { signed: false, width: 64 };
    let [slot] = pkg.traits[0].dependent_requirements.as_slice() else {
        panic!("expected the const-indexed requirement")
    };
    assert_eq!(
        slot.parameters,
        [TemplateBinder::Const(TypeExpression(vec![
            TypeExpressionStep::Scalar(u64_carrier)
        ]))]
    );
    for (name, value) in [("low", 4u8), ("high", 9)] {
        let caller = machine(&pkg, name);
        let selection = only_member_call(&pkg, caller);
        assert_eq!(selection.requirement.declaration, slot.declaration);
        assert_eq!(selection.target, MemberRealization::Boundary { direction: None });
        assert_eq!(
            selection.static_arguments,
            [DeclarationTerm::StaticInteger {
                carrier: u64_carrier,
                value: num_bigint::BigInt::from(value),
            }]
        );
        assert_eq!(selection.parameters, [omega_lowering::ParameterType::Scalar(caller.parameter_types[0])]);
    }
}

#[test]
fn generic_conformance_method_runs_the_selected_implementation_for_each_static_argument() {
    let src = "trait Convert { machine keep<T>(x:T)->T; } \
        Good satisfies Convert { machine keep<U>(x:U)->U { x } } \
        Flip satisfies Convert { machine keep<U>(x:U)->U { x } } \
        machine use_it<const E:Convert>(x:u8)->u8 { E::keep::<u8>(x) } \
        machine use_flag<const E:Convert>(x:bool)->bool { E::keep::<bool>(x) } \
        machine one(x:u8)->u8 { use_it::<Good>(x) } \
        machine two(x:u8)->u8 { use_it::<Flip>(x) } \
        machine three(x:bool)->bool { use_flag::<Good>(x) }";
    let pkg = source::lower(&[("convert.omg", src)]).unwrap();
    let [keep] = pkg.traits[0].dependent_requirements.as_slice() else {
        panic!("expected the generic requirement")
    };
    // Follow the root caller into its closed wrapper, then into the member call.
    let selected = |root: &str| {
        let [call] = machine(&pkg, root).body.operations.as_slice() else {
            panic!("missing wrapper call")
        };
        let OperationKind::Call {
            callee: CallTarget::Machine(id),
            ..
        } = call.kind
        else {
            panic!("wrapper must be closed")
        };
        let wrapper = &pkg.machines[id.get() as usize - 1];
        let selection = only_member_call(&pkg, wrapper);
        let MemberRealization::Conformance { declaration, machine } = &selection.target else {
            panic!("expected a conformance selection")
        };
        let conformance = pkg
            .conformances
            .iter()
            .find(|c| &c.declaration == declaration)
            .expect("selected conformance is packaged");
        let implementation = &pkg.machines[machine.get() as usize - 1];
        assert_eq!(implementation.id, *machine);
        assert_eq!(conformance.members[0].requirement, selection.requirement);
        assert_eq!(conformance.members[0].implementation, implementation.declaration);
        assert_eq!(selection.requirement.declaration, keep.declaration);
        assert_eq!(
            selection.parameters,
            [omega_lowering::ParameterType::Scalar(
                implementation.parameter_types[0]
            )]
        );
        assert_eq!(selection.result, implementation.result_type);
        // The implementation body is specialized at the member call's static argument.
        let [StaticArgument::Type(argument)] = implementation.arguments.as_slice() else {
            panic!("implementation lost its static argument")
        };
        (selection, conformance.declaration.path.clone(), implementation, *argument)
    };
    let (one, good, good_u8, one_arg) = selected("one");
    let (two, flip, flip_u8, two_arg) = selected("two");
    let (three, good_again, good_bool, three_arg) = selected("three");
    assert_eq!(one.static_arguments, [scalar(U8)]);
    assert_eq!(one.static_arguments, two.static_arguments);
    assert_eq!(three.static_arguments, [scalar(ScalarType::Boolean)]);
    assert_eq!(pkg.types.get(one_arg), Some(&TypeShape::PrimitiveScalar(U8)));
    assert_eq!(one_arg, two_arg);
    assert_ne!(one_arg, three_arg);
    // Equal signatures and static arguments: only the selected conformance changes the body.
    assert_ne!(good, flip);
    assert_ne!(good_u8.id, flip_u8.id);
    assert_ne!(good_u8.declaration, flip_u8.declaration);
    // Same conformance, different static argument: one declaration, two bodies.
    assert_eq!(good, good_again);
    assert_eq!(good_u8.declaration, good_bool.declaration);
    assert_ne!(good_u8.id, good_bool.id);
}

const SIZED: &str = "trait Sized { machine pick<const N:u64>(x:u8)->u8; } \
        Good satisfies Sized { machine pick<const M:u64>(x:u8)->u8 { x } } \
        machine run<const E:Sized>(x:u8)->u8 { E::pick::<4>(x) } \
        machine go(x:u8)->u8 { run::<Good>(x) }";

/// Follow both executable edges, then cross-check the result against the declaration row.
fn sized_target<'a>(pkg: &'a LoweredPackage, root: &str) -> (&'a MemberCallSelection, &'a omega_lowering::LoweredMachine) {
    let [op] = machine(pkg, root).body.operations.as_slice() else { panic!("missing wrapper call") };
    let OperationKind::Call { callee: CallTarget::Machine(id), .. } = op.kind else { panic!("expected closed wrapper") };
    let wrapper = &pkg.machines[id.get() as usize - 1];
    let selection = only_member_call(pkg, wrapper);
    let MemberRealization::Conformance { declaration, machine } = &selection.target else { panic!("expected Good") };
    let implementation = &pkg.machines[machine.get() as usize - 1];
    let [good] = pkg.conformances.as_slice() else { panic!("expected one conformance") };
    let [row] = good.members.as_slice() else { panic!("expected pick") };
    assert_eq!(declaration.path, "Good");
    assert_eq!(selection.requirement.declaration.path, "Sized::pick");
    assert_eq!(declaration, &good.declaration);
    assert_eq!(selection.owner, good.implemented);
    assert_eq!(selection.requirement, row.requirement);
    assert_eq!(implementation.declaration, row.implementation);
    assert_eq!(implementation.id, *machine);
    (selection, implementation)
}

const U64: ScalarType = ScalarType::Integer { signed: false, width: 64 };

#[test]
fn selected_implementation_keeps_the_requirements_const_integer() {
    let pkg = source::lower(&[("sized.omg", SIZED)]).unwrap();
    let (selection, body) = sized_target(&pkg, "go");
    spec_corpus::observe_handoff!(
        psi "psi-output/selected-implementation-integer",
        assert_eq!(
            (selection.static_arguments.as_slice(), body.arguments.as_slice()),
            ([DeclarationTerm::StaticInteger { carrier: U64, value: 4.into() }].as_slice(),
             [StaticArgument::Integer { carrier: U64, value: 4.into() }].as_slice())
        ),
    );
}

#[test]
fn another_const_integer_selects_a_different_body_of_the_same_implementation() {
    let src = format!("{SIZED} machine run9<const E:Sized>(x:u8)->u8 {{ E::pick::<9>(x) }} \
        machine other(x:u8)->u8 {{ run9::<Good>(x) }}");
    let pkg = source::lower(&[("sized.omg", &src)]).unwrap();
    let (first, four) = sized_target(&pkg, "go");
    let (second, nine) = sized_target(&pkg, "other");
    assert_eq!(first.requirement, second.requirement);
    assert_eq!(four.declaration, nine.declaration);
    assert_ne!(four.id, nine.id);
    assert_eq!(second.static_arguments, [DeclarationTerm::StaticInteger { carrier: U64, value: 9.into() }]);
    spec_corpus::observe_handoff!(
        psi "psi-output/selected-implementation-integer-control",
        assert_eq!(
            (four.arguments.as_slice(), nine.arguments.as_slice()),
            ([StaticArgument::Integer { carrier: U64, value: 4.into() }].as_slice(),
             [StaticArgument::Integer { carrier: U64, value: 9.into() }].as_slice())
        ),
    );
}
