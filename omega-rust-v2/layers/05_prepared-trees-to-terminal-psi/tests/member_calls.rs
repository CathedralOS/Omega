#[path = "support/source.rs"]
mod source;
use omega_lowering::{CallArgument, CallTarget, MemberRealization, OperationKind, StaticArgument};

#[test]
fn boundary_demand_preserves_the_requirement_and_authored_argument_interleave() {
    let src = "boundary trait Device { machine read(n:u8, bytes:@[u8], last:u8)->u8 reaches Device crashes none; machine other(n:u8,bytes:@[u8],last:u8)->u8 reaches Device crashes none; } machine run(n:u8,b:@[u8],last:u8)->u8 reaches Device { Device::read(last,b,n) }";
    for member in ["read", "other"] {
        let program = src.replace("Device::read(", &format!("Device::{member}("));
        let pkg = source::lower(&[("device.omg", &program)]).unwrap();
        let run = &pkg.machines[0];
        let [op] = run.body.operations.as_slice() else {
            panic!("expected a single boundary call")
        };
        let (selection, arguments) = psi_handoff_consumer::member_call(&pkg, &op.kind).expect("lost boundary selection");
        assert_eq!(selection.requirement.declaration.path, format!("Device::{member}"));
        assert_eq!(selection.owner.declaration, pkg.traits[0].declaration);
        assert_eq!(selection.target, MemberRealization::Boundary { direction: None });
        assert_eq!(run.contract.realized_reach[0].declaration, pkg.traits[0].declaration);
        assert_eq!(
            selection.parameters,
            [
                omega_lowering::ParameterType::Scalar(run.parameter_types[0]),
                omega_lowering::ParameterType::Structural(run.parameter_types[1]),
                omega_lowering::ParameterType::Scalar(run.parameter_types[2])
            ]
        );
        let declaration = pkg.traits[0]
            .requirements
            .iter()
            .find(|r| r.declaration == selection.requirement.declaration)
            .unwrap();
        assert_eq!(declaration.parameter_types, selection.parameters);
        let [
            omega_lowering::Parameter::Scalar(n),
            omega_lowering::Parameter::Structural(bytes),
            omega_lowering::Parameter::Scalar(last),
        ] = run.body.parameters.as_slice()
        else {
            panic!("unexpected parameter lanes")
        };
        assert_eq!(
            arguments,
            &[
                CallArgument::Scalar(*last),
                CallArgument::CopyWhole(*bytes),
                CallArgument::Scalar(*n)
            ]
        );
    }
}

#[test]
fn machine_reach_uses_each_closed_type_argument() {
    let src = "boundary trait Device<T>{} machine pass<T>(x:T)->T reaches Device<T>{x} machine small(x:u8)->u8{pass::<u8>(x)} machine flag(x:bool)->bool{pass::<bool>(x)}";
    let pkg = source::lower(&[("reach.omg", src)]).unwrap();
    let mut rows = pkg
        .machines
        .iter()
        .filter(|m| m.declaration.path == "pass")
        .map(|m| {
            let [row] = m.contract.reach.as_deref().unwrap() else {
                panic!("expected exactly one service")
            };
            assert_eq!(row.declaration, pkg.traits[0].declaration);
            let [argument] = row.arguments.as_slice() else {
                panic!("lost applied service argument")
            };
            let [omega_lowering::TypeExpressionStep::Scalar(carrier)] = argument.0.as_slice() else {
                panic!("open service argument on a closed machine")
            };
            carrier
        });
    assert_ne!(rows.next().unwrap(), rows.next().unwrap());
    assert!(rows.next().is_none());
}

#[test]
fn conformance_invocations_keep_the_public_precondition_and_exact_selected_body() {
    let src = "trait Clock { machine tick(v:u8)->bool requires v!=0; } machine loose(v:u8)->bool{true} Local satisfies Clock { Clock::tick=loose; } machine use_clock<const E:Clock>(n:u8)->bool{E::tick(n)} machine call(n:u8)->bool{use_clock::<Local>(n)}";
    let package = source::lower(&[("contract.omg", src)]).unwrap();
    let wrapper = package
        .machines
        .iter()
        .find(|m| m.declaration.path == "use_clock")
        .unwrap();
    let [op] = wrapper.body.operations.as_slice() else {
        panic!("missing applied call")
    };
    let OperationKind::Call { requirements, .. } = &op.kind else {
        panic!("missing call obligations")
    };
    assert_eq!(
        requirements.as_slice(),
        [omega_lowering::CallRequirement {
            site: omega_lowering::CallSite::Operation(op.id),
            clause: 0
        }]
    );
    let (selection, _) = psi_handoff_consumer::member_call(&package, &op.kind).unwrap();
    let MemberRealization::Conformance { machine, .. } = selection.target else {
        panic!("lost conformance")
    };
    let selected = &package.machines[machine.get() as usize - 1];
    assert_eq!(selected.declaration.path, "loose");
    assert!(selected.contract.requires.is_empty());
    let requirement = &package.traits[0].requirements[0];
    assert_eq!(requirement.declaration, selection.requirement.declaration);
    assert_eq!(requirement.requires.len(), 1);
}

#[test]
fn conformance_of_an_empty_trait_still_records_its_applied_identity() {
    let src = "trait Stamp<T>{} Byte satisfies Stamp<u8>{} Flag satisfies Stamp<bool>{}";
    let package = source::lower(&[("empty.omg", src)]).unwrap();
    let [byte, flag] = package.conformances.as_slice() else {
        panic!("missing empty conformances")
    };
    assert!(byte.members.is_empty() && flag.members.is_empty());
    assert_eq!(byte.implemented.declaration, flag.implemented.declaration);
    assert_ne!(byte.implemented.arguments, flag.implemented.arguments);
}

#[test]
fn equal_signature_conformances_select_distinct_bodies_and_keep_the_same_requirement() {
    let src = "trait Render<T>{machine paint(x:T)->bool;} machine red(x:u8)->bool{true} machine blue(x:u8)->bool{false} Red satisfies Render<u8>{Render::paint=red;} Blue satisfies Render<u8>{Render::paint=blue;} machine use_map<T,const E:Render<T>>(x:T)->bool{E::paint(x)} machine one(x:u8)->bool{use_map::<u8,Red>(x)} machine two(x:u8)->bool{use_map::<u8,Blue>(x)}";
    let pkg = source::lower(&[("render.omg", src)]).unwrap();
    let lookup = |name| {
        pkg.machines
            .iter()
            .find(|m| m.declaration.path == name)
            .unwrap()
    };
    let selected = |caller| {
        let [select] = lookup(caller).body.operations.as_slice() else {
            panic!("missing call")
        };
        let OperationKind::Call {
            callee: CallTarget::Machine(id),
            ..
        } = select.kind
        else {
            panic!("wrapper must be closed")
        };
        let wrapper = &pkg.machines[id.get() as usize - 1];
        let [_, StaticArgument::Conformance { declaration: owner, bound }] = wrapper.arguments.as_slice() else {
            panic!("missing conformance argument")
        };
        let [op] = wrapper.body.operations.as_slice() else {
            panic!("missing member call")
        };
        let (selection, _) = psi_handoff_consumer::member_call(&pkg, &op.kind).expect("missing member selection");
        let MemberRealization::Conformance { declaration, machine } = &selection.target else {
            panic!("wrong selection")
        };
        assert_eq!(declaration, owner);
        assert_eq!(bound, &selection.owner);
        assert!(pkg.conformances.iter().any(|c| &c.declaration == owner
            && c.members[0].requirement == selection.requirement
            && c.members[0].implementation == pkg.machines[machine.get() as usize - 1].declaration));
        (&selection.requirement, *machine)
    };
    let (first, red) = selected("one");
    let (other, blue) = selected("two");
    assert_eq!(first, other);
    assert_eq!(first.declaration.path, "Render::paint");
    assert_eq!(red, lookup("red").id);
    assert_eq!(blue, lookup("blue").id);
}
