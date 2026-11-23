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
fn an_applied_predicate_binds_its_closed_argument_at_each_forwarding_formal_and_result() {
    use PredicateOperator::{And, GreaterThan, LessThan};
    use PredicateStep::{Apply, Formal, Integer, Result, SelfValue, ValueIndex};
    use omega_psi::{CallRequirement, CallSite, CallTarget, OperationKind};
    // Forwarding `Bound<5>` owes `self > N` with N closed at 5 on entry and on
    // the result, and a caller owes that entry clause at its call.
    let declaration = "domain<const N:u8> u8::Bound<N> requires self > N;";
    let src = format!(
        "{declaration} domain u8::Five = u8::Bound<5>; machine keep(x:u8 in Five)->u8 in Five {{x}}
         machine relay(y:u8 in Bound<5>)->u8 in Bound<5> {{keep(y)}}"
    );
    let pkg = sources::lower(&[("predicate.omg", &src)]).unwrap();
    let [bound] = pkg.domains.as_slice() else {
        panic!("expected the indexed family")
    };
    let [clause] = bound.requires.as_slice() else {
        panic!("expected the authored predicate")
    };
    // The catalog keeps the open predicate; only uses specialize it.
    assert_eq!(clause.predicate, [SelfValue, ValueIndex(0), Apply(GreaterThan)]);
    assert_eq!(clause.site.span.slice(src.as_bytes()), Some(b"requires self > N".as_slice()));
    let set = forwarded(&pkg, "keep");
    assert_eq!(
        pkg.types.domain_atoms(set).unwrap().collect::<Vec<_>>(),
        [(
            &bound.declaration,
            &[DeclarationTerm::StaticInteger {
                carrier: BYTE,
                value: 5.into()
            }][..]
        )]
    );
    let [keep, relay] = pkg.machines.as_slice() else {
        panic!("expected both machines")
    };
    // `Five` and `Bound<5>` spell one qualified type.
    assert_eq!(relay.parameter_types, keep.parameter_types);
    assert_eq!(relay.result_type, keep.result_type);
    let bound_at = |subject| (&clause.site, vec![subject, Integer(5.into()), Apply(GreaterThan)]);
    let applied = [keep, relay]
        .into_iter()
        .flat_map(|m| [&m.contract.requires, &m.contract.ensures])
        .flat_map(|clauses| clauses.iter().map(|c| (&c.site, c.predicate.clone())))
        .collect::<Vec<_>>();
    // Each list holds exactly the specialized domain predicate, owned by the family's clause.
    spec_corpus::observe_handoff!(
        psi "psi-output/applied-domain-predicate",
        assert_eq!(applied, [Formal(0), Result, Formal(0), Result].map(bound_at)),
    );
    let [call] = relay.body.operations.as_slice() else {
        panic!("expected the forwarding call")
    };
    assert!(matches!(
        &call.kind,
        OperationKind::Call { callee: CallTarget::Machine(callee), requirements, .. }
            if *callee == keep.id && requirements == &[CallRequirement { site: CallSite::Operation(call.id), clause: 0 }]
    ));

    // Each binder reads its own head position, not its declaration position.
    let src = "domain<const X:u8, const Y:u8> u8::Gap<Y,X> requires self > X && self < Y;
               machine gap(x:u8 in Gap<9,2>)->u8 in Gap<9,2> {x}";
    let pkg = sources::lower(&[("permuted.omg", src)]).unwrap();
    assert_eq!(pkg.domains[0].index_binders, [1, 0]);
    let m = &pkg.machines[0];
    for (clauses, subject) in [(&m.contract.requires, Formal(0)), (&m.contract.ensures, Result)] {
        let [applied] = clauses.as_slice() else {
            panic!("expected the specialized predicate")
        };
        assert_eq!(
            applied.predicate,
            [
                subject.clone(),
                Integer(2.into()),
                Apply(GreaterThan),
                subject,
                Integer(9.into()),
                Apply(LessThan),
                Apply(And)
            ]
        );
    }
}

// Compare L3's occurrence-specific debts before dropping the prepared input. In the public
// product follow the cast edge, not the alias table or just the declared return type.
fn tag_cast(source: &str, cast: &str, expected: &[(&str, Option<u8>)]) -> (LoweredPackage, DomainSetId) {
    let files = [("cast.omg", source)];
    let resolved = sources::resolve(&files);
    let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
    let debts: Vec<_> = prepared.typed().obligations.iter().filter_map(|o| match &o.kind {
        omega_typed::ObligationKind::EstablishDomain(d) => {
            assert_eq!(o.site.span.slice(source.as_bytes()), Some(cast.as_bytes()));
            Some(d)
        }
        _ => None,
    }).collect();
    assert_eq!(debts.len(), expected.len());
    for (path, index) in expected {
        let debt = debts.iter().find(|d| d.declaration.path == *path).unwrap();
        let args: Vec<_> = index.iter().map(|&n| omega_typed::ValueType::StaticInteger {
            carrier: Box::new(omega_typed::ValueType::Integer { signed: false, width: Some(8) }), value: n.into(),
        }).collect();
        assert_eq!(debt.arguments, args);
    }
    let pkg = sources::lower_prepared(&prepared, &files).unwrap();
    drop(prepared);
    let body = &pkg.machines[0].body;
    let [Parameter::Scalar(input)] = body.parameters[..] else { panic!("expected bare scalar") };
    assert_eq!(pkg.types.get(body.values[input.index()].ty), Some(&TypeShape::PrimitiveScalar(BYTE)));
    let [entry, arrival] = body.blocks.as_slice() else { panic!("expected one tag edge") };
    let BlockTerminator::Jump(edge) = &entry.terminator else { panic!("missing cast") };
    assert_eq!(body.diagnostics.edges[edge.id.get() - 1].span.slice(source.as_bytes()), Some(cast.as_bytes()));
    assert_eq!(edge.target, arrival.id);
    assert_eq!(edge.arguments, [input]);
    assert_eq!(edge.scalar_qualification_changes, [omega_psi::ScalarQualificationChange { argument_index: 0, direction: omega_psi::QualificationDirection::Add }]);
    assert!(edge.domain_completions.is_empty());
    let [dest] = arrival.scalar_parameters[..] else { panic!("expected one destination") };
    assert_eq!(body.values[dest.index()].origin, omega_psi::ValueOrigin::BlockParameter(arrival.id));
    assert!(matches!(arrival.terminator, BlockTerminator::Return { result: ReturnEdge::Scalar(v), .. } if v == dest));
    let Some(TypeShape::QualifiedScalar { carrier: BYTE, domains }) = pkg.types.get(body.values[dest.index()].ty) else { panic!("expected qualified byte") };
    let set = *domains;
    (pkg, set)
}

#[test]
fn cast_adds_the_expanded_family_and_exact_index() {
    let original = "domain<const N:u8> u8::Bound<N>; domain u8::Five = u8::Bound<5>; machine make(x:u8)->u8 in Five {x as +Five}";
    for n in [5, 6] {
        let source = original.replace("Bound<5>", &format!("Bound<{n}>"));
        let (pkg, set) = tag_cast(&source, "x as +Five", &[("u8::Bound", Some(n))]);
        let [bound] = pkg.domains.as_slice() else { panic!("missing family") };
        assert_eq!(bound.declaration.path, "u8::Bound");
        let args = [DeclarationTerm::StaticInteger { carrier: BYTE, value: n.into() }];
        spec_corpus::observe_handoff!(psi "psi-output/applied-domain-cast", assert_eq!(
            pkg.types.domain_atoms(set).unwrap().collect::<Vec<_>>(), [(&bound.declaration, args.as_slice())]
        ));
    }
}

#[test]
fn cast_adds_the_parent_and_the_selected_child() {
    let original = "domain u8::Parent; domain u8::Parent::Child; machine make(x:u8)->u8 in Parent::Child {x as +Parent +Parent::Child}";
    for child in ["Child", "Sibling"] {
        let source = original.replace("Child", child);
        let path = format!("u8::Parent::{child}");
        let (pkg, set) = tag_cast(&source, &format!("x as +Parent +Parent::{child}"), &[("u8::Parent", None), (&path, None)]);
        let [parent, child] = pkg.domains.as_slice() else { panic!("expected both domains") };
        assert_eq!(parent.declaration.path, "u8::Parent");
        assert_eq!(child.declaration.path, path);
        let Some(TypeShape::QualifiedScalar { domains, .. }) = pkg.types.get(child.carrier) else { panic!("missing parent subject") };
        assert_eq!(pkg.types.domain_set(*domains).unwrap(), std::slice::from_ref(&parent.declaration));
        spec_corpus::observe_handoff!(psi "psi-output/inherited-domain-cast", assert_eq!(
            pkg.types.domain_atoms(set).unwrap().collect::<Vec<_>>(), [(&parent.declaration, &[][..]), (&child.declaration, &[][..])]
        ));
    }
}

#[test]
fn a_record_formal_retains_its_whole_value_domain_and_nominal_schema() {
    let case = include_str!("fixtures/package_cases/record-domain-qualified-formal/case/case.omg");
    let control = include_str!("fixtures/package_cases/record-domain-qualified-formal/control/case.omg");
    let package = sources::lower(&[("case.omg", case)]).unwrap();
    let plain = sources::lower(&[("case.omg", control)]).unwrap();
    let selected = psi_handoff_consumer::record_formal;
    let [live] = &package.domains[..] else { panic!("expected Live") };
    assert_eq!(live.declaration.path, "R::Live");
    let (record, domains) = selected(&package, "hold", 0).unwrap();
    let (control_record, control_domains) = selected(&plain, "hold", 0).unwrap();
    spec_corpus::observe_handoff!(psi "psi-output/record-domain-formal", assert_eq!(
        (record.declaration.path.as_str(), domains, control_record.declaration.path.as_str(), control_domains),
        ("R", vec![omega_psi::DomainAtom { declaration: live.declaration.clone(), arguments: vec![] }], "R", vec![])
    ));
    let alternative = "data R {v:u8;} data S {v:u8;} domain R::Live; domain S::Live; machine hold(r:S in Live){}";
    let twin = sources::lower(&[("case.omg", alternative)]).unwrap();
    let (record, domains) = selected(&twin, "hold", 0).unwrap();
    assert_eq!(record.declaration.path, "S");
    assert_eq!(domains.len(), 1);
    assert_eq!(domains[0].declaration.path, "S::Live");
}

#[test]
fn a_combined_numeric_cast_publishes_its_plain_result_before_adding_the_core_domain() {
    use omega_psi::{OperationKind, OperationResult, QualificationDirection, ScalarQualificationChange};
    for (from, to, signed, width) in [
        ("u32", "u64", false, 64),
        ("u16", "u8", false, 8),
        ("f64", "i32", true, 32),
    ] {
        let destination = ScalarType::Integer { signed, width };
        for expression in [
            format!("x as {to} +Wrapping"),
            format!("(x as {to}) as +Wrapping"),
        ] {
            let text = format!(r"use omega\core; machine change(x:{from})->{to} in Wrapping {{ {expression} }}");
            let pkg = sources::lower(&[("cast.omg", &text)]).unwrap();
            let machine = pkg
                .machines
                .iter()
                .find(|m| m.declaration.path == "change")
                .unwrap();
            let body = &machine.body;
            let [Parameter::Scalar(input)] = body.parameters[..] else {
                panic!("scalar formal")
            };
            let [op] = &body.operations[..] else { panic!("exactly one conversion") };
            assert!(matches!(op.kind, OperationKind::ExactNumericCast {operand} if operand==input));
            let OperationResult::Scalar(converted) = op.result else {
                panic!("converted value")
            };
            assert_eq!(pkg.types.get(body.values[converted.index()].ty), Some(&TypeShape::PrimitiveScalar(destination)));
            let [entry, qualified] = &body.blocks[..] else {
                panic!("one domain establishment")
            };
            let BlockTerminator::Jump(edge) = &entry.terminator else {
                panic!("qualification edge")
            };
            assert_eq!(edge.arguments, [converted]);
            assert_eq!(edge.target, qualified.id);
            assert_eq!(
                edge.scalar_qualification_changes,
                [ScalarQualificationChange {
                    argument_index: 0,
                    direction: QualificationDirection::Add
                }]
            );
            let [tagged] = qualified.scalar_parameters[..] else {
                panic!("qualified value")
            };
            assert_eq!(Some(body.values[tagged.index()].ty), machine.result_type);
            let Some(TypeShape::QualifiedScalar { carrier, domains }) = pkg.types.get(body.values[tagged.index()].ty) else {
                panic!("qualified destination")
            };
            assert_eq!(*carrier, destination);
            let atoms: Vec<_> = pkg
                .types
                .domain_atoms(*domains)
                .unwrap()
                .map(|(id, _)| id.path.as_str())
                .collect();
            assert_eq!(atoms, [if signed { r"core\i::Wrapping" } else { r"core\u::Wrapping" }]);
            assert!(matches!(qualified.terminator,BlockTerminator::Return{result:ReturnEdge::Scalar(out),..} if out==tagged));
        }
    }
}
