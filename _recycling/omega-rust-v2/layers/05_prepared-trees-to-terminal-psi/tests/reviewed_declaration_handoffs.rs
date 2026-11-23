#[path = "support/source.rs"]
mod sources;

use omega_psi::{DeclarationTerm, DomainAtom, PredicateStep, ScalarType, TypeShape};
use omega_typed::{CallUse, ValueType};

const BYTE: ScalarType = ScalarType::Integer { signed: false, width: 8 };
const BIT: ScalarType = ScalarType::Boolean;

#[test]
fn reversing_actuals_selects_the_backed_overload_and_its_premise_range() {
    let helpers = "machine p(n:u8,b:bool)->bool requires n > 0 reaches none crashes none terminates {b}
                   machine p(b:bool,n:u8)->bool requires b requires n > 1 reaches none crashes none terminates {false}";
    for (application, slots, types, premises) in [
        ("p(x,y)", [0, 1], [BYTE, BIT], 1),
        ("p(y,x)", [1, 0], [BIT, BYTE], 2),
    ] {
        let text = format!("{helpers}\nmachine gate(x:u8,y:bool) requires {application} {{}}");
        let files = [("swap.omg", text.as_str())];
        let (psi, site) = {
            let input = sources::resolve(&files);
            let prepared = omega_preparation::prepare_source_package(&input).unwrap();
            let typed = prepared.typed();
            let call = typed.calls.iter().find(|c| c.site.file == "swap.omg").unwrap();
            assert_eq!(call.use_site, CallUse::Fact);
            assert_eq!(call.caller.as_ref().unwrap().path, "gate");
            assert_eq!(call.machine.path, "p");
            assert_eq!(call.site.span.slice(text.as_bytes()), Some(application.as_bytes()));
            let sig = typed.signatures.iter().find(|s| s.machine == call.machine).unwrap();
            let formals = [ValueType::Integer { signed: false, width: Some(8) }, ValueType::Bool];
            assert!(sig.parameters.iter().map(|(_, ty)| ty).eq(slots.map(|i| &formals[i])));
            (sources::lower_prepared(&prepared, &files).unwrap(), call.site.clone())
        };

        let gate = psi.machines.iter().find(|m| m.declaration.path == "gate").unwrap();
        assert!(gate.body.operations.is_empty(), "a fact call is not an executable call");
        let [clause] = gate.contract.requires.as_slice() else { panic!("expected one gate requirement") };
        use PredicateStep::{Formal, MachineApplication};
        let [Formal(a), Formal(b), MachineApplication { callee, operands, result, requirements, site: at, .. }] = clause.predicate.as_slice() else {
            panic!("expected two formal actuals and their application: {clause:?}")
        };
        assert_eq!([*a, *b], slots);
        assert_eq!(operands, &types.map(omega_psi::TypeExpression::from));
        assert_eq!(*result, BIT);
        assert_eq!(at, &site);
        assert_eq!(*requirements, 0..premises);

        // Read through the full callee identity, not a lookup of the shared name.
        let helper = psi.machines.iter().find(|m| m.declaration == *callee).unwrap();
        assert_eq!(helper.declaration.path, "p");
        let parameters: Vec<_> = helper.parameter_types.iter().map(|&t| psi.types.get(t).unwrap()).collect();
        assert_eq!(parameters, types.map(TypeShape::PrimitiveScalar).iter().collect::<Vec<_>>());
        assert_eq!(helper.contract.requires.len(), premises);
    }
}

#[test]
fn landing_a_constant_actual_does_not_discharge_the_helpers_premise() {
    for (actual, value) in [("0", 0), ("(2 + 3)", 5)] {
        let text = format!("machine p(x:i8)->bool requires x != 0 reaches none crashes none terminates {{true}}
                            machine gate() requires p({actual}) {{}}");
        let psi = sources::lower(&[("constant.omg", &text)]).unwrap();
        let gate = psi.machines.iter().find(|m| m.declaration.path == "gate").unwrap();
        let helper = psi.machines.iter().find(|m| m.declaration.path == "p").unwrap();
        let [clause] = gate.contract.requires.as_slice() else { panic!("expected one gate requirement") };
        let [PredicateStep::Integer(landed), PredicateStep::MachineApplication { callee, requirements, .. }] = clause.predicate.as_slice() else {
            panic!("expected a landed constant passed to p: {clause:?}")
        };
        assert_eq!(*landed, value.into());
        assert_eq!(callee, &helper.declaration);
        assert_eq!(helper.contract.requires.len(), 1);
        assert_eq!(*requirements, 0..1);
    }
}

#[test]
fn closing_a_reordered_alias_keeps_each_index_tuple_with_its_family() {
    // Hold the head, closed actuals and both families fixed; swap only the family names.
    for (rhs, tuples) in [
        ("u8::Left<B,A> & u8::Right<A,B>", [[2, 9], [9, 2]]),
        ("u8::Right<B,A> & u8::Left<A,B>", [[9, 2], [2, 9]]),
    ] {
        let text = format!("domain<const X:u8,const Y:u8> u8::Left<Y,X> requires self > X;
                            domain<const X:u8,const Y:u8> u8::Right<X,Y> requires self > X;
                            domain<const A:u8,const B:u8> u8::Select<B,A> = {rhs};
                            domain u8::Closed = u8::Select<2,9>;");
        let files = [("domain.omg", text.as_str())];
        let expected = [("u8::Left", tuples[0]), ("u8::Right", tuples[1])];
        let psi = {
            let input = sources::resolve(&files);
            let prepared = omega_preparation::prepare_source_package(&input).unwrap();
            let (_, members) = prepared.typed().domain_alias_members.iter().find(|(id, _)| id.path == "u8::Closed").unwrap();
            assert_eq!(members.len(), expected.len());
            for (atom, (family, values)) in members.iter().zip(expected) {
                assert_eq!(atom.declaration.path, family);
                assert_eq!(atom.arguments, values.map(|n| ValueType::StaticInteger {
                    carrier: Box::new(ValueType::Integer { signed: false, width: Some(8) }), value: n.into(),
                }));
            }
            sources::lower_prepared(&prepared, &files).unwrap()
        };

        let closed = psi.domain_aliases.iter().find(|a| a.declaration.path == "u8::Closed").unwrap();
        assert_eq!(closed.site.file, "domain.omg");
        assert_eq!(closed.site.span.slice(text.as_bytes()), Some(b"domain u8::Closed = u8::Select<2,9>;".as_slice()));
        assert_eq!(closed.members, expected.map(|(path, values)| {
            let family = psi.domains.iter().find(|d| d.declaration.path == path).unwrap();
            DomainAtom {
                declaration: family.declaration.clone(),
                arguments: values.map(|n| DeclarationTerm::StaticInteger { carrier: BYTE, value: n.into() }).into(),
            }
        }));
    }
}
