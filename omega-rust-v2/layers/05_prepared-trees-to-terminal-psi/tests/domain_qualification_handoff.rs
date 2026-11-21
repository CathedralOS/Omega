#[path = "support/source.rs"]
mod sources;

use omega_psi::{
    BlockTerminator, DeclarationTerm, DomainSetId, LoweredPackage, Parameter, PredicateOperator, PredicateStep, ReturnEdge, ScalarType, TypeShape,
};

const BYTE: ScalarType = ScalarType::Integer { signed: false, width: 8 };

fn forwarded(
    package: &LoweredPackage,
    name: &str,
) -> DomainSetId {
    let m = package
        .machines
        .iter()
        .find(|m| m.declaration.path == name)
        .unwrap();
    let [formal] = m.parameter_types[..] else { panic!("expected one formal") };
    assert_eq!(m.result_type, Some(formal));
    let Some(TypeShape::QualifiedScalar { carrier: BYTE, domains }) = package.types.get(formal) else {
        panic!("lost qualified byte type")
    };
    let [Parameter::Scalar(input)] = m.body.parameters[..] else {
        panic!("expected a scalar input")
    };
    assert_eq!(m.body.values[input.index()].ty, formal);
    assert!(m.body.operations.is_empty());
    let [block] = m.body.blocks.as_slice() else {
        panic!("unexpected qualification change")
    };
    assert!(matches!(block.terminator, BlockTerminator::Return {result: ReturnEdge::Scalar(v), ..} if v == input));
    *domains
}

#[test]
fn alias_use_forwards_the_applied_atom_not_the_alias_or_the_bare_carrier() {
    let source = "domain<const N:u8> u8::Bound<N>;\ndomain u8::Five = u8::Bound<5>;\nmachine forward(x:u8 in Five)->u8 in Five {x}\n";
    let files = [("alias-use.omg", source)];
    let resolved = sources::resolve(&files);
    let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
    let sig = prepared
        .typed()
        .signatures
        .iter()
        .find(|s| s.machine.path == "forward")
        .unwrap();
    assert_eq!(sig.result, sig.parameters[0].1);
    let [bound] = sig.result.domains() else {
        panic!("expected the prepared expansion")
    };
    assert_eq!(bound.declaration.path, "u8::Bound");
    assert_eq!(
        bound.arguments,
        [omega_typed::ValueType::StaticInteger {
            carrier: Box::new(omega_typed::ValueType::Integer { signed: false, width: Some(8) }),
            value: 5.into()
        }]
    );
    let pkg = sources::lower_prepared(&prepared, &files).unwrap();
    drop(prepared);
    let set = forwarded(&pkg, "forward");
    let [(id, args)] = pkg.types.domain_atoms(set).unwrap().collect::<Vec<_>>()[..] else {
        panic!("expected one atom")
    };
    assert_eq!(id, &pkg.domains[0].declaration);
    assert_eq!(id, &pkg.domain_aliases[0].members[0].declaration);
    assert_eq!(args, pkg.domain_aliases[0].members[0].arguments);
    assert_eq!(
        args,
        [DeclarationTerm::StaticInteger {
            carrier: BYTE,
            value: 5.into()
        }]
    );
    assert!(pkg.types.domain_set(set).is_none()); // the declaration-only view cannot erase arguments
}

#[test]
fn equal_payloads_preserve_distinct_indices_and_declarations_in_one_table() {
    let src = "domain<const N:u8> u8::Bound<N>; domain<const N:u8> u8::Other<N>;
        domain u8::Five = u8::Bound<5>;
        machine alias(x:u8 in Five)->u8 in Five {x}
        machine five(x:u8 in Bound<5>)->u8 in Bound<5> {x}
        machine six(x:u8 in Bound<6>)->u8 in Bound<6> {x}
        machine other(x:u8 in Other<5>)->u8 in Other<5> {x}";
    let pkg = sources::lower(&[("indices.omg", src)]).unwrap();
    assert_eq!(forwarded(&pkg, "alias"), forwarded(&pkg, "five"));
    for (name, path, n) in [
        ("five", "u8::Bound", 5),
        ("six", "u8::Bound", 6),
        ("other", "u8::Other", 5),
    ] {
        let set = forwarded(&pkg, name);
        let atoms = pkg.types.domain_atoms(set).unwrap().collect::<Vec<_>>();
        let [(id, args)] = atoms[..] else { panic!("expected one applied atom") };
        assert_eq!(id.path, path);
        assert_eq!(
            args,
            [DeclarationTerm::StaticInteger {
                carrier: BYTE,
                value: n.into()
            }]
        );
    }
}

#[test]
fn a_conjunction_keeps_repeated_family_ids_and_each_ordered_tuple() {
    let src = "domain<const X:u8, const Y:u8> u8::Pair<X,Y>;
        domain u8::Both = u8::Pair<2,1> & u8::Pair<1,2>;
        machine keep(x:u8 in Both)->u8 in Both {x}";
    let pkg = sources::lower(&[("tuple.omg", src)]).unwrap();
    let atoms = pkg
        .types
        .domain_atoms(forwarded(&pkg, "keep"))
        .unwrap()
        .collect::<Vec<_>>();
    assert_eq!(atoms.len(), 2);
    for ((id, args), values) in atoms.into_iter().zip([[1, 2], [2, 1]]) {
        assert_eq!(id, &pkg.domains[0].declaration);
        assert_eq!(
            args,
            values.map(|n| DeclarationTerm::StaticInteger {
                carrier: BYTE,
                value: n.into()
            })
        );
    }
}

#[test]
fn inherited_use_forwards_both_atoms_with_the_predicate_owned_by_mark() {
    let src = "domain u8::Mark requires self > 0;\ndomain u8::Mark::Child;\nmachine forward(x:u8 in Mark::Child)->u8 in Mark::Child {x}\n";
    let files = [("inherited-use.omg", src)];
    let resolved = sources::resolve(&files);
    let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
    let sig = prepared
        .typed()
        .signatures
        .iter()
        .find(|s| s.machine.path == "forward")
        .unwrap();
    assert_eq!(sig.result, sig.parameters[0].1);
    assert_eq!(
        sig.result
            .domains()
            .iter()
            .map(|a| a.declaration.path.as_str())
            .collect::<Vec<_>>(),
        ["u8::Mark", "u8::Mark::Child"]
    );
    let pkg = sources::lower_prepared(&prepared, &files).unwrap();
    drop(prepared);
    let set = forwarded(&pkg, "forward");
    let [parent, child] = pkg.domains.as_slice() else {
        panic!("expected the two named domains")
    };
    assert_eq!(
        [
            parent.declaration.path.as_str(),
            child.declaration.path.as_str()
        ],
        ["u8::Mark", "u8::Mark::Child"]
    );
    assert_eq!(
        pkg.types.domain_atoms(set).unwrap().collect::<Vec<_>>(),
        [(&parent.declaration, &[][..]), (&child.declaration, &[][..])]
    );
    assert!(child.requires.is_empty());
    let [clause] = parent.requires.as_slice() else {
        panic!("expected one authored predicate")
    };
    let m = &pkg.machines[0];
    for (clauses, subject) in [
        (&m.contract.requires, PredicateStep::Formal(0)),
        (&m.contract.ensures, PredicateStep::Result),
        (&parent.requires, PredicateStep::SelfValue),
    ] {
        let [bound] = clauses.as_slice() else {
            panic!("expected exactly the parent predicate")
        };
        assert_eq!(bound.site, clause.site);
        assert_eq!(
            bound.predicate,
            [
                subject,
                PredicateStep::Integer(0.into()),
                PredicateStep::Apply(PredicateOperator::GreaterThan)
            ]
        );
    }
}

#[test]
fn an_applied_predicate_cannot_be_silently_dropped_from_the_forwarder() {
    let declaration = "domain<const N:u8> u8::Bound<N> requires self > N;";
    let src = format!("{declaration} domain u8::Five = u8::Bound<5>; machine keep(x:u8 in Five)->u8 in Five {{x}}");
    sources::refuse(
        &[("predicate.omg", &src)],
        "predicate.omg",
        sources::unique_span(&src, declaration),
        "applied scalar qualification requires a closed predicate-free domain application",
    );
}

#[test]
fn transport_does_not_enable_tag_cast_establishment() {
    for (source, cast) in [
        (
            "domain<const N:u8> u8::Bound<N>; domain u8::Five = u8::Bound<5>; machine make(x:u8)->u8 in Five {x as +Five}",
            "x as +Five",
        ),
        (
            "domain u8::Parent; domain u8::Parent::Child; machine make(x:u8)->u8 in Parent::Child {x as +Parent +Parent::Child}",
            "x as +Parent +Parent::Child",
        ),
    ] {
        sources::refuse(
            &[("cast.omg", source)],
            "cast.omg",
            sources::unique_span(source, cast),
            "changing an applied or inherited scalar qualification is not supported",
        );
    }
}
