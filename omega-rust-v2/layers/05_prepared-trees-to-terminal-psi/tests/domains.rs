#[path = "support/source.rs"]
mod sources;

use omega_lowering::{PredicateOperator, PredicateStep, ScalarType, TypeShape};

#[test]
fn impossible_domain_preserves_both_bounds_without_establishing_a_value() {
    use PredicateOperator::{And, GreaterThan, LessThan};
    use PredicateStep::{Apply, Integer, SelfValue};

    let lower = |second_comparison| {
        let source = format!("public domain i32::Impossible requires self > 0 && self {second_comparison} 0;");
        sources::lower(&[("bounds.omg", &source)]).unwrap()
    };
    let impossible = lower("<");
    let satisfiable = lower(">");
    let [domain] = impossible.domains.as_slice() else {
        panic!("expected the uninhabited domain declaration");
    };
    assert!(impossible.machines.is_empty());
    assert_eq!(domain.declaration.package, "app");
    assert_eq!(domain.declaration.path, "i32::Impossible");
    assert_eq!(domain.declaration.kind, omega_psi::DeclarationKind::Domain);
    assert!(domain.public);
    assert_eq!(domain.site.file, "bounds.omg");
    let carrier = TypeShape::PrimitiveScalar(ScalarType::Integer { signed: true, width: 32 });
    assert_eq!(impossible.types.get(domain.carrier), Some(&carrier));
    let [clause] = domain.requires.as_slice() else {
        panic!("expected the authored requirement");
    };
    let expected_predicate = |second| {
        [
            SelfValue,
            Integer(0.into()),
            Apply(GreaterThan),
            SelfValue,
            Integer(0.into()),
            Apply(second),
            Apply(And),
        ]
    };
    assert_eq!(clause.predicate, expected_predicate(LessThan));
    assert_eq!(clause.site.file, "bounds.omg");
    let satisfiable_domain = &satisfiable.domains[0];
    assert_eq!(satisfiable_domain.declaration, domain.declaration);
    assert_eq!(satisfiable.types.get(satisfiable_domain.carrier), Some(&carrier));
    assert_eq!(satisfiable_domain.requires[0].predicate, expected_predicate(GreaterThan));
}

#[test]
fn equal_predicates_preserve_distinct_domains_and_authored_requirements() {
    let first = ("first.omg", "public domain i32::First requires true;");
    let second = ("second.omg", "domain i32::Second requires true;");
    let tag = ("tag.omg", "domain i32::Tag;");
    let package = sources::lower(&[second, tag, first]).unwrap();
    assert_eq!(package, sources::lower(&[first, second, tag]).unwrap());
    let [first, second, tag] = package.domains.as_slice() else {
        panic!("expected all three named classifications");
    };

    assert_eq!(first.declaration.path, "i32::First");
    assert_eq!(second.declaration.path, "i32::Second");
    assert_eq!(tag.declaration.path, "i32::Tag");
    spec_corpus::observe_handoff!(
        psi "psi-output/distinct-domains",
        assert_ne!(first.declaration, second.declaration),
    );
    assert!(first.public);
    assert!(!second.public);
    assert_eq!(first.carrier, second.carrier);
    assert_eq!(second.carrier, tag.carrier);
    assert_eq!(first.requires[0].predicate, [PredicateStep::Boolean(true)]);
    assert_eq!(second.requires[0].predicate, first.requires[0].predicate);
    assert!(tag.requires.is_empty());
}

#[test]
fn unused_domains_keep_each_carrier_when_their_predicates_do_not_read_self() {
    let source = "domain i32::Empty requires false; domain u32::Empty requires false;";
    let package = sources::lower(&[("empty.omg", source)]).unwrap();
    let [signed, unsigned] = package.domains.as_slice() else {
        panic!("expected both named domains");
    };
    for (domain, path, signed) in [(signed, "i32::Empty", true), (unsigned, "u32::Empty", false)] {
        assert_eq!(domain.declaration.path, path);
        assert_eq!(domain.requires[0].predicate, [PredicateStep::Boolean(false)]);
        spec_corpus::observe_handoff!(
            "normalized-domain-carriers/consumer",
            assert_eq!(
                package.types.get(domain.carrier),
                Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed, width: 32 }))
            )
        );
    }
}

#[test]
fn domain_clause_applies_the_selected_predicate_at_its_occurrence() {
    let source = "machine allowed()->bool reaches none crashes none terminates {true}
                  domain i32::Guarded requires true && allowed();";
    let package = sources::lower(&[("guard.omg", source)]).unwrap();
    let domain = &package.domains[0];
    let allowed = &package.machines[0].declaration;
    assert_eq!((domain.declaration.path.as_str(), allowed.path.as_str()), ("i32::Guarded", "allowed"));
    spec_corpus::observe_handoff!(
        psi "psi-output/named-domain-call",
        assert!(matches!(domain.requires[0].predicate.as_slice(), [PredicateStep::Boolean(true), PredicateStep::MachineApplication {callee, site, ..}, PredicateStep::Apply(PredicateOperator::And)] if callee == allowed && site.span.slice(source.as_bytes()) == Some(b"allowed()"))),
    );
}

#[test]
fn parenthesized_and_explicitly_sized_bounds_lower_to_the_same_predicate() {
    let lower = |predicate| {
        let source = format!("domain u64::AlignmentSize requires {predicate};");
        sources::lower(&[("alignment.omg", &source)]).unwrap()
    };
    let plain = lower("self >= 1 && self <= 8");
    let explicit = lower("((self >= 1u64) && (self <= 8u64))");
    let a = &plain.domains[0];
    let b = &explicit.domains[0];
    assert_eq!(a.declaration, b.declaration);
    assert_eq!(plain.types.get(a.carrier), explicit.types.get(b.carrier));
    assert_eq!(a.requires[0].predicate, b.requires[0].predicate);
}

#[test]
fn a_carrier_generic_domain_names_its_own_type_slot() {
    use omega_psi::{TemplateBinder, TypeExpressionStep};
    for (source, slot) in [("domain<T> T::Tag;", 0), ("domain<U,T> T::Tag;", 1)] {
        let package = sources::lower(&[("domain.omg", source)]).unwrap();
        let [family] = package.domain_templates.as_slice() else { panic!("expected one carrier family") };
        assert!(package.domains.is_empty() && family.requires.is_empty() && family.index_binders.is_empty());
        assert_eq!(family.declaration.path, "T::Tag");
        assert_eq!(family.parameters.len(), slot + 1);
        assert!(family.parameters.iter().all(|p| matches!(p, TemplateBinder::Type { multiplicity: None })));
        spec_corpus::observe_handoff!(psi "psi-output/domain-family-carrier", assert_eq!(family.carrier.0,
            [TypeExpressionStep::GenericParameter { owner: family.declaration.clone(), index: slot }]));
    }
}

#[test]
fn a_record_tag_names_its_exact_nominal_subject() {
    let original = "data Item { valid:bool; } domain Item::Tag;";
    for (source, name) in [(original, "Item"), ("data Item { valid:bool; } data Other { valid:bool; } domain Other::Tag;", "Other")] {
        let package = sources::lower(&[("domain.omg", source)]).unwrap();
        let [domain] = package.domains.as_slice() else { panic!("expected the record tag") };
        let record = package.data.iter().find(|r| r.declaration.path == name).unwrap();
        assert_eq!(domain.declaration.path, format!("{name}::Tag"));
        assert!(domain.requires.is_empty() && domain.parameters.is_empty());
        spec_corpus::observe_handoff!(psi "psi-output/record-tag-carrier", assert_eq!(package.types.get(domain.carrier), Some(&TypeShape::Nominal(record.id))));
    }
}

#[test]
fn indexed_alias_keeps_each_applied_atom_and_canonical_argument() {
    use omega_psi::{DeclarationTerm, DomainAtom, TemplateBinder, TypeExpression, TypeExpressionStep};
    let byte = ScalarType::Integer { signed: false, width: 8 };
    let original = "domain u8::Five = u8::Bound<5>; domain<const N:u8> u8::Bound<N>;";
    for (source, values) in [
        (original.to_owned(), vec![5]),
        (original.replace("u8::Bound<5>;", "u8::Bound<6> & u8::Bound<5> & u8::Bound<6>;"), vec![5, 6]),
    ] {
        let files = [("domain.omg", source.as_str())];
        let resolved = sources::resolve(&files);
        let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
        let (_, members) = prepared.typed().domain_alias_members.iter().find(|(id, _)| id.package == "app" && id.path == "u8::Five").unwrap();
        assert_eq!(members.len(), values.len());
        for (member, &n) in members.iter().zip(&values) {
            assert_eq!(member.declaration.path, "u8::Bound");
            assert_eq!(member.arguments, [omega_typed::ValueType::StaticInteger {
                carrier: Box::new(omega_typed::ValueType::Integer { signed: false, width: Some(8) }), value: n.into(),
            }]);
        }
        let package = sources::lower_prepared(&prepared, &files).unwrap();
        let [bound] = package.domains.as_slice() else {
            panic!("expected only the atom")
        };
        let [alias] = package.domain_aliases.as_slice() else {
            panic!("expected the transparent alias")
        };
        assert_eq!(alias.declaration.path, "u8::Five");
        assert_eq!(bound.declaration.path, "u8::Bound");
        assert_eq!(alias.carrier, bound.carrier);
        assert_eq!(package.types.get(alias.carrier), Some(&TypeShape::PrimitiveScalar(byte)));
        assert_eq!(
            bound.parameters,
            [TemplateBinder::Const(TypeExpression(vec![
                TypeExpressionStep::Scalar(byte)
            ]))]
        );
        assert_eq!(bound.index_binders, [0]);
        spec_corpus::observe_handoff!(psi "psi-output/indexed-alias-atoms", assert_eq!(
            alias.members,
            values
                .into_iter()
                .map(|n| DomainAtom {
                    declaration: bound.declaration.clone(),
                    arguments: vec![DeclarationTerm::StaticInteger {
                        carrier: byte,
                        value: n.into()
                    }],
                })
                .collect::<Vec<_>>()
        ));
    }
}

#[test]
fn indexed_alias_forwards_its_own_binders_in_member_argument_order() {
    use omega_psi::{DeclarationTerm, TypeExpression, TypeExpressionStep};
    let source = "domain<const X:u8, const Y:u8> u8::Pair<Y,X>; domain<const A:u8,const B:u8> u8::Swapped<A,B> = u8::Pair<B,A>;";
    let package = sources::lower(&[("domain.omg", source)]).unwrap();
    let [pair] = package.domains.as_slice() else { panic!("expected the family") };
    let [alias] = package.domain_aliases.as_slice() else {
        panic!("expected the alias")
    };
    assert_eq!(pair.declaration.path, "u8::Pair");
    assert_eq!(alias.declaration.path, "u8::Swapped");
    assert_eq!(pair.index_binders, [1, 0]);
    assert_eq!(alias.index_binders, [0, 1]);
    assert_eq!(alias.parameters.len(), 2);
    let [atom] = alias.members.as_slice() else {
        panic!("expected the applied family")
    };
    assert_eq!(atom.declaration, pair.declaration);
    assert_eq!(
        atom.arguments,
        [1, 0].map(|index| DeclarationTerm::Type(TypeExpression(vec![TypeExpressionStep::GenericParameter {
            owner: alias.declaration.clone(),
            index
        }])))
    );
}

#[test]
fn inherited_domain_requires_the_qualified_parent_subject() {
    let original = "domain i32::Positive requires self > 0; domain i32::Positive::Small requires true;";
    let files = [("domain.omg", original)];
    let resolved = sources::resolve(&files);
    let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
    let (_, subject) = prepared.typed().domain_carriers.iter().find(|(id, _)| id.package == "app" && id.path == "i32::Positive::Small").unwrap();
    let [inherited] = subject.domains() else { panic!("expected one prepared parent qualification") };
    assert_eq!(inherited.declaration.path, "i32::Positive");
    assert!(inherited.arguments.is_empty());
    assert_eq!(prepared.typed().domain_carriers.get(&inherited.declaration), Some(subject.carrier()));
    let package = sources::lower_prepared(&prepared, &files).unwrap();
    let [parent, child] = package.domains.as_slice() else {
        panic!("expected parent and child declarations")
    };
    assert_eq!(parent.declaration.path, "i32::Positive");
    assert_eq!(child.declaration.path, "i32::Positive::Small");
    let Some(TypeShape::QualifiedScalar { carrier, domains }) = package.types.get(child.carrier) else {
        panic!("child subject lost its parent qualification")
    };
    assert_eq!(*carrier, ScalarType::Integer { signed: true, width: 32 });
    assert_eq!(package.types.get(parent.carrier), Some(&TypeShape::PrimitiveScalar(*carrier)));
    spec_corpus::observe_handoff!(
        psi "psi-output/inherited-domain-subject",
        assert_eq!(package.types.domain_set(*domains).unwrap(), std::slice::from_ref(&parent.declaration)),
    );
    let [base] = parent.requires.as_slice() else {
        panic!("missing parent predicate")
    };
    assert_eq!(
        base.predicate,
        [
            PredicateStep::SelfValue,
            PredicateStep::Integer(0.into()),
            PredicateStep::Apply(PredicateOperator::GreaterThan)
        ]
    );
    let [own] = child.requires.as_slice() else {
        panic!("missing child predicate")
    };
    assert_eq!(own.predicate, [PredicateStep::Boolean(true)]);
}

#[test]
fn inherited_subject_selects_the_named_parent_among_equal_predicates() {
    for parent in ["A", "B"] {
        let text = format!("domain i32::A requires self > 0; domain i32::B requires self > 0; domain i32::{parent}::Child;");
        let package = sources::lower(&[("domain.omg", &text)]).unwrap();
        let domain = |path: &str| package.domains.iter().find(|d| d.declaration.path == path).unwrap();
        let child = domain(&format!("i32::{parent}::Child"));
        assert!(child.requires.is_empty()); // No authored requirement; not a bare tag.
        let Some(TypeShape::QualifiedScalar { domains, .. }) = package.types.get(child.carrier) else { panic!("missing inherited subject") };
        let expected = &domain(&format!("i32::{parent}")).declaration;
        assert_eq!(package.types.domain_set(*domains).unwrap(), std::slice::from_ref(expected));
    }
}

#[test]
fn inherited_subject_cast_reads_the_parent_qualified_value_in_the_child_predicate() {
    use PredicateOperator::{Equal, GreaterThan, LessThan};
    use PredicateStep::{Apply, Formal, Integer, Result, SelfValue};
    // `-Positive` removes the declared domain but not the value: the child's
    // clause compares its own subject, which still carries the parent.
    let text = "domain i32::Positive requires self > 0; domain i32::Positive::Small requires (self as -Positive) < 100;
                machine keep(x:i32 in Positive::Small)->i32 in Positive::Small {x}";
    let package = sources::lower(&[("domain.omg", text)]).unwrap();
    let [parent, child] = package.domains.as_slice() else {
        panic!("expected parent and child declarations")
    };
    let Some(TypeShape::QualifiedScalar { domains, .. }) = package.types.get(child.carrier) else {
        panic!("child subject lost its parent qualification")
    };
    assert_eq!(package.types.domain_set(*domains).unwrap(), std::slice::from_ref(&parent.declaration));
    let [own] = child.requires.as_slice() else {
        panic!("missing child predicate")
    };
    assert_eq!(own.site.span.slice(text.as_bytes()), Some(b"requires (self as -Positive) < 100".as_slice()));
    spec_corpus::observe_handoff!(
        psi "psi-output/inherited-subject-cast",
        assert_eq!(own.predicate, [SelfValue, Integer(100.into()), Apply(LessThan)]),
    );
    // A forwarder binds both the parent's clause and the child's cast-read clause.
    let keep = &package.machines[0];
    for (clauses, subject) in [
        (&keep.contract.requires, Formal(0)),
        (&keep.contract.ensures, Result),
    ] {
        assert_eq!(
            clauses
                .iter()
                .map(|c| (&c.site, &c.predicate[..]))
                .collect::<Vec<_>>(),
            [
                (&parent.requires[0].site, &[subject.clone(), Integer(0.into()), Apply(GreaterThan)][..]),
                (&own.site, &[subject, Integer(100.into()), Apply(LessThan)][..]),
            ]
        );
    }

    // The same read applies to a qualified formal in a machine's own guarantee.
    let text = "domain u16::Small; machine greater(left:u16 in Small, right:u16)->bool
                ensures result == ((left as -Small) > right) { (left as -Small) > right }";
    let package = sources::lower(&[("formal.omg", text)]).unwrap();
    let [guarantee] = package.machines[0].contract.ensures.as_slice() else {
        panic!("expected the authored guarantee")
    };
    assert_eq!(guarantee.predicate, [Result, Formal(0), Formal(1), Apply(GreaterThan), Apply(Equal)]);
}

#[test]
fn the_returned_empty_issuer_list_is_published_on_its_domain() {
    let source = include_str!("fixtures/domain_access/empty-issuers/policy.omg");
    let (_, lowered) = sources::lower_built_fixture("domain_access/empty-issuers", "policy.omg", source);
    let [domain] = lowered.domains.as_slice() else { panic!("one policy domain") };
    assert_eq!(domain.declaration.path, "i32::Locked");
    spec_corpus::observe_handoff!(psi "psi-output/executed-empty-domain",
        assert_eq!((domain.declaration.path.as_str(), domain.issuers), ("i32::Locked", omega_psi::DomainIssuers::Nobody)),
    );
}

#[test]
fn unused_domain_aliases_expand_to_exact_atoms_without_creating_a_domain() {
    let atoms = ("atoms.omg", "public domain u8::A; public domain u8::B; domain u8::C;");
    let names = (
        "names.omg",
        "public domain u8::Pair = u8::B & u8::A; public domain u8::Nested = u8::Pair & u8::B; domain u8::Other = u8::C & u8::A;",
    );
    let package = sources::lower(&[atoms, names]).unwrap();
    assert_eq!(package, sources::lower(&[names, atoms]).unwrap());
    let [a, b, c] = &package.domains[..] else {
        panic!("expected three domain atoms")
    };
    assert_eq!(
        [&a.declaration.path, &b.declaration.path, &c.declaration.path].map(String::as_str),
        ["u8::A", "u8::B", "u8::C"]
    );
    let [nested, other, pair] = &package.domain_aliases[..] else {
        panic!("expected three aliases")
    };
    for (alias, path, second) in [
        (nested, "u8::Nested", b),
        (other, "u8::Other", c),
        (pair, "u8::Pair", b),
    ] {
        assert_eq!(alias.declaration.path, path);
        assert_eq!(alias.public, path != "u8::Other");
        assert_eq!(alias.carrier, a.carrier);
        assert_eq!(
            alias.members,
            [a, second].map(|atom| omega_psi::DomainAtom {
                declaration: atom.declaration.clone(),
                arguments: vec![]
            })
        );
    }
}

#[test]
fn an_alias_cast_delivers_the_qualifications_of_its_current_expansion() {
    use omega_lowering::{BlockTerminator, QualificationDirection, ScalarQualificationChange};
    for (expansion, expected) in [
        ("u8::B & u8::A", ["u8::A", "u8::B"]),
        ("u8::C & u8::B", ["u8::B", "u8::C"]),
    ] {
        let [first, second] = expected.map(|path| path.trim_start_matches("u8::"));
        let text = format!(
            "domain u8::A; domain u8::B; domain u8::C; domain u8::Both = {expansion};
             machine f(x:u8)->u8 {{ let both = x as +Both; let only = both as -{first} ={second}; only as -{second} }}"
        );
        let package = sources::lower(&[("alias.omg", &text)]).unwrap();
        let body = &package.machines[0].body;
        let BlockTerminator::Jump(edge) = &body.blocks[0].terminator else {
            panic!("expected the alias cast edge")
        };
        assert_eq!(
            edge.scalar_qualification_changes,
            [ScalarQualificationChange {
                argument_index: 0,
                direction: QualificationDirection::Add
            }]
        );
        let [destination] = body.blocks[edge.target.get() - 1].scalar_parameters[..] else {
            panic!("expected one qualified parameter")
        };
        let Some(TypeShape::QualifiedScalar { domains, .. }) = package.types.get(body.values[destination.index()].ty) else {
            panic!("expected a qualified destination")
        };
        let delivered: Vec<_> = package
            .types
            .domain_set(*domains)
            .unwrap()
            .iter()
            .map(|id| id.path.as_str())
            .collect();
        spec_corpus::observe_handoff!("type-and-domain-aliases/consumer", assert_eq!(delivered, expected));
        let [alias] = &package.domain_aliases[..] else { panic!("expected the alias") };
        assert_eq!(
            alias
                .members
                .iter()
                .map(|atom| atom.declaration.path.as_str())
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn qualification_edges_preserve_each_source_and_deliver_the_exact_destination_set() {
    use omega_lowering::{
        BlockTerminator, Parameter,
        QualificationDirection::{Add, Erase},
        ReturnEdge, ScalarQualificationChange, ValueOrigin,
    };
    let text = "domain u8::A; domain u8::B; domain u8::Both = u8::A & u8::B;
                machine f(x:u8)->u8 { let both = x as +Both; let only = both as -A =B; only as -B }";
    let package = sources::lower(&[("cast.omg", text)]).unwrap();
    let body = &package.machines[0].body;
    let [Parameter::Scalar(mut input)] = body.parameters[..] else {
        panic!("expected a bare input")
    };
    assert!(body.operations.is_empty());
    assert_eq!(body.blocks.len(), 4);
    for (block, (direction, names, occurrence)) in body.blocks.iter().zip([
        (Add, vec!["u8::A", "u8::B"], "x as +Both"),
        (Erase, vec!["u8::B"], "both as -A =B"),
        (Erase, vec![], "only as -B"),
    ]) {
        let BlockTerminator::Jump(edge) = &block.terminator else {
            panic!("expected a qualification edge")
        };
        assert_eq!(edge.arguments, [input]);
        assert_eq!(
            edge.scalar_qualification_changes,
            [ScalarQualificationChange {
                argument_index: 0,
                direction
            }]
        );
        assert_eq!(
            body.diagnostics.edges[edge.id.get() - 1]
                .span
                .slice(text.as_bytes()),
            Some(occurrence.as_bytes())
        );
        let [destination] = body.blocks[edge.target.get() - 1].scalar_parameters[..] else {
            panic!("expected one fresh parameter")
        };
        assert_ne!(input, destination);
        let value = &body.values[destination.index()];
        assert_eq!(value.origin, ValueOrigin::BlockParameter(edge.target));
        let domains = match package.types.get(value.ty).unwrap() {
            TypeShape::QualifiedScalar { domains, .. } => package.types.domain_set(*domains).unwrap(),
            TypeShape::PrimitiveScalar(_) => &[][..],
            other => panic!("unexpected scalar {other:?}"),
        };
        // Each cast's accounting decides the exact set the next block receives.
        spec_corpus::observe_handoff!(
            "checked-domain-casts/consumer",
            assert_eq!(domains.iter().map(|id| id.path.as_str()).collect::<Vec<_>>(), names)
        );
        input = destination;
    }
    assert!(matches!(body.blocks.last().unwrap().terminator, BlockTerminator::Return {result: ReturnEdge::Scalar(result), ..} if result == input));
}

#[test]
fn carrier_conversion_and_literal_landing_precede_domain_addition() {
    use omega_lowering::{BlockTerminator, OperationKind, OperationResult, ScalarLiteral};
    for (args, cast) in [("n:u16", "n as u8 +Tag"), ("", "7 as +Tag")] {
        let source = format!("domain u8::Tag; machine f({args})->u8 in Tag {{ {cast} }}");
        let package = sources::lower(&[("cast.omg", &source)]).unwrap();
        let body = &package.machines[0].body;
        let [operation] = body.operations.as_slice() else {
            panic!("expected a bare integer value")
        };
        if args.is_empty() {
            assert_eq!(operation.kind, OperationKind::Constant(ScalarLiteral::Integer(7u8.into())));
        } else {
            let omega_lowering::Parameter::Scalar(n) = body.parameters[0] else {
                panic!("expected an integer parameter")
            };
            assert_eq!(operation.kind, OperationKind::ExactNumericCast { operand: n });
        }
        let OperationResult::Scalar(value) = operation.result else {
            panic!("expected scalar operation")
        };
        assert_eq!(body.values[value.index()].ty, package.domains[0].carrier);
        let BlockTerminator::Jump(edge) = &body.blocks[0].terminator else {
            panic!("expected a qualification edge")
        };
        assert_eq!(edge.arguments, [value]);
    }
}

#[test]
fn retaining_membership_forwards_the_value_and_replacing_membership_refuses_at_the_cast() {
    use omega_lowering::{BlockTerminator, Parameter, ReturnEdge};
    let declarations = "domain u8::A; domain u8::B;";
    let program = format!("{declarations} machine keep(a:u8 in A)->u8 in A {{a as =A}}");
    let package = sources::lower(&[("cast.omg", &program)]).unwrap();
    let body = &package.machines[0].body;
    let [Parameter::Scalar(a)] = body.parameters[..] else {
        panic!("expected a qualified input")
    };
    assert_eq!(body.blocks.len(), 1);
    assert!(body.operations.is_empty());
    assert!(matches!(body.blocks[0].terminator, BlockTerminator::Return {result: ReturnEdge::Scalar(v), ..} if v == a));
    let program = format!("{declarations} machine replace(a:u8 in A)->u8 in B {{a as -A +B}}");
    let message = "Psi has no specified edge for simultaneous domain removal and addition";
    let error = sources::refuse(&[("cast.omg", &program)], "cast.omg", sources::unique_span(&program, "a as -A +B"), message);
    assert_eq!(error.site.span.slice(program.as_bytes()), Some(b"a as -A +B".as_slice()));
    assert_eq!(error.message, message);
}

#[test]
fn only_the_cast_arm_changes_qualification_before_an_ordinary_join() {
    use omega_lowering::{BlockTerminator, Parameter, QualificationDirection, ReturnEdge};
    let source = "domain bool::Enabled; machine choose(flag:bool, bare:bool, held:bool in Enabled)->bool in Enabled {
        let result = match flag {true -> bare as +Enabled, false -> held,}; result
    }";
    let package = sources::lower(&[("arms.omg", source)]).unwrap();
    let body = &package.machines[0].body;
    let [
        Parameter::Scalar(flag),
        Parameter::Scalar(bare),
        Parameter::Scalar(held),
    ] = body.parameters[..]
    else {
        panic!("expected scalar inputs")
    };
    let BlockTerminator::Conditional {
        condition,
        when_true,
        when_false,
    } = &body.blocks[0].terminator
    else {
        panic!("expected dispatch")
    };
    assert_eq!(*condition, flag);
    assert!(when_true.scalar_qualification_changes.is_empty() && when_false.scalar_qualification_changes.is_empty());
    let jump = |id: omega_lowering::BlockId| {
        let BlockTerminator::Jump(edge) = &body.blocks[id.get() - 1].terminator else {
            panic!("expected successor")
        };
        edge
    };
    let change = jump(when_true.target);
    assert_eq!(change.arguments, [bare]);
    assert_eq!(change.scalar_qualification_changes[0].direction, QualificationDirection::Add);
    let [tagged] = body.blocks[change.target.get() - 1].scalar_parameters[..] else {
        panic!("expected qualified result")
    };
    let from_true = jump(change.target);
    let from_false = jump(when_false.target);
    assert_eq!(from_true.target, from_false.target);
    assert_eq!(from_true.arguments, [tagged]);
    assert_eq!(from_false.arguments, [held]);
    assert!(from_true.scalar_qualification_changes.is_empty() && from_false.scalar_qualification_changes.is_empty());
    let join = &body.blocks[from_true.target.get() - 1];
    let [result] = join.scalar_parameters[..] else {
        panic!("expected a join parameter")
    };
    assert!(matches!(join.terminator,BlockTerminator::Return{result:ReturnEdge::Scalar(v),..} if v == result));
}

#[test]
fn a_forwarded_predicate_binds_the_formal_and_result_and_is_required_at_each_call() {
    use omega_lowering::{CallRequirement, CallSite, OperationKind};
    let text = "domain u8::Positive requires self > 0u8;
        machine keep(other:u8, value:u8 in Positive)->u8 in Positive {value}
        machine relay(x:u8 in Positive)->u8 in Positive {keep(1u8,x)}";
    let package = sources::lower(&[("domain.omg", text)]).unwrap();
    let [keep, relay] = package.machines.as_slice() else {
        panic!("expected both machines")
    };
    let domain_clause = &package.domains[0].requires[0];
    for (clauses, subject) in [
        (&keep.contract.requires, PredicateStep::Formal(1)),
        (&relay.contract.requires, PredicateStep::Formal(0)),
        (&keep.contract.ensures, PredicateStep::Result),
        (&relay.contract.ensures, PredicateStep::Result),
    ] {
        let [clause] = clauses.as_slice() else {
            panic!("expected one domain predicate")
        };
        assert_eq!(
            clause.predicate,
            [
                subject,
                PredicateStep::Integer(0.into()),
                PredicateStep::Apply(PredicateOperator::GreaterThan)
            ]
        );
        assert_eq!(clause.site, domain_clause.site);
    }
    let [_, call] = relay.body.operations.as_slice() else {
        panic!("expected the literal and call")
    };
    assert!(
        matches!(&call.kind, OperationKind::Call{callee: omega_lowering::CallTarget::Machine(callee),requirements,..} if *callee==keep.id && requirements == &[CallRequirement {site: CallSite::Operation(call.id), clause:0}])
    );
}

#[test]
fn a_predicate_completion_cannot_stand_in_for_other_origins_or_required_memberships() {
    for (source, cast, message) in [
        ("domain u8::P; domain u8::P::Child requires true; machine make(x:u8 in P)->u8 in P::Child {x as =P +P::Child}",
         "x as =P +P::Child", "predicate completion does not support a domain requiring parent membership"),
        // A vacuous origin needs empty required memberships, and a tag child requires its parent.
        ("domain u8::P requires true; domain u8::T; domain u8::T::C; machine make(x:u8)->u8 { let q = x as +P +T +T::C; q as -P -T -T::C }",
         "x as +P +T +T::C", "vacuous completion does not support a domain requiring parent membership"),
    ] {
        sources::refuse(&[("limit.omg", source)], "limit.omg", sources::unique_span(source, cast), message);
    }
}

#[test]
fn predicate_cast_requests_completion_and_binds_the_arrival_invariant() {
    use omega_psi::{BlockTerminator, CompletionOrigin, DomainAtom, DomainCompletion, Parameter, ReturnEdge, ScalarDomainInvariant};
    let original = "domain u8::Positive requires self > 0u8; machine f(x:u8)->u8 in Positive {x as +Positive}";
    let twin = "domain u8::Positive requires self > 0u8; domain u8::Other requires self > 0u8; machine f(x:u8,y:u8)->u8 in Other {y as +Other}";
    for (text, cast, path, slot) in [(original, "x as +Positive", "u8::Positive", 0), (twin, "y as +Other", "u8::Other", 1)] {
        let files = [("domain.omg", text)];
        let resolved = sources::resolve(&files);
        let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
        let debts: Vec<_> = prepared.typed().obligations.iter().filter_map(|o| match &o.kind {
            omega_typed::ObligationKind::EstablishDomain(d) => Some((o.site.span.slice(text.as_bytes()), d.declaration.path.as_str(), d.arguments.len())),
            _ => None,
        }).collect();
        assert_eq!(debts, [(Some(cast.as_bytes()), path, 0)]);
        let pkg = sources::lower_prepared(&prepared, &files).unwrap();
        drop(prepared);
        let domain = pkg.domains.iter().find(|d| d.declaration.path == path).unwrap();
        assert_eq!(domain.requires.len(), 1);
        assert_eq!(domain.requires[0].predicate, [PredicateStep::SelfValue, PredicateStep::Integer(0.into()), PredicateStep::Apply(PredicateOperator::GreaterThan)]);
        let body = &pkg.machines[0].body;
        let Parameter::Scalar(input) = body.parameters[slot] else { panic!("expected scalar formal") };
        let [entry, arrival] = body.blocks.as_slice() else { panic!("expected one completion") };
        let BlockTerminator::Jump(edge) = &entry.terminator else { panic!("missing cast completion") };
        assert_eq!(body.diagnostics.edges[edge.id.get() - 1].span.slice(text.as_bytes()), Some(cast.as_bytes()));
        assert!(edge.scalar_qualification_changes.is_empty());
        assert!(entry.scalar_domain_invariants.is_empty());
        let [dest] = arrival.scalar_parameters[..] else { panic!("expected one destination") };
        assert_ne!(input, dest);
        assert_eq!(body.values[dest.index()].origin, omega_psi::ValueOrigin::BlockParameter(arrival.id));
        assert!(matches!(pkg.types.get(body.values[input.index()].ty), Some(TypeShape::PrimitiveScalar(_))));
        let Some(TypeShape::QualifiedScalar { domains, .. }) = pkg.types.get(body.values[dest.index()].ty) else { panic!("lost result domain") };
        assert_eq!(pkg.types.domain_set(*domains).unwrap(), std::slice::from_ref(&domain.declaration));
        assert!(matches!(arrival.terminator, BlockTerminator::Return { result: ReturnEdge::Scalar(v), .. } if v == dest));
        let atom = DomainAtom { declaration: domain.declaration.clone(), arguments: vec![] };
        spec_corpus::observe_handoff!(psi "psi-output/predicate-cast-completion", assert_eq!(
            (edge.target, edge.arguments.as_slice(), edge.domain_completions.as_slice(), arrival.scalar_domain_invariants.as_slice()),
            (arrival.id, &[input][..], &[DomainCompletion { argument_index: 0, domain: atom.clone(), origin: CompletionOrigin::Proved }][..],
             &[ScalarDomainInvariant { subject: dest, domain: atom }][..])
        ));
    }
}

/// The original `x as +P +Tag` program: one jump establishes both domains, each once,
/// with its own origin. `requires true` is a predicate, so `P` is proved, not vacuous.
#[test]
fn a_cast_adding_a_tag_and_a_predicate_names_each_origin_on_one_completion() {
    use omega_psi::{BlockTerminator, CompletionOrigin, DomainAtom, DomainCompletion, QualificationDirection, ScalarDomainInvariant, ScalarQualificationChange};
    let original = "domain u8::P requires true; domain u8::Tag; machine make(x:u8)->u8 { let q = x as +P +Tag; q as -P -Tag }";
    let entries = original.replace("x as +P +Tag; q as -P -Tag", "x as +Tag +P; q as -Tag -P");
    let declarations = original.replace("domain u8::P requires true; domain u8::Tag;", "domain u8::Tag; domain u8::P requires true;");
    let unrelated = original.replace("domain u8::Tag;", "domain u8::Tag; domain u8::Other requires true; domain u8::Spare;");
    let predicate = original.replace("requires true", "requires self > 0u8");
    // L5 keeps a contradictory predicate for the checker; it is still a proved origin.
    let contradiction = original.replace("requires true", "requires false");
    let truth = vec![PredicateStep::Boolean(true)];
    let positive = vec![PredicateStep::SelfValue, PredicateStep::Integer(0.into()), PredicateStep::Apply(PredicateOperator::GreaterThan)];
    let falsity = vec![PredicateStep::Boolean(false)];
    for (text, cast, clause) in [
        (original, "x as +P +Tag", &truth),
        (entries.as_str(), "x as +Tag +P", &truth),
        (declarations.as_str(), "x as +P +Tag", &truth),
        (unrelated.as_str(), "x as +P +Tag", &truth),
        (predicate.as_str(), "x as +P +Tag", &positive),
        (contradiction.as_str(), "x as +P +Tag", &falsity),
    ] {
        let files = [("limit.omg", text)];
        let resolved = sources::resolve(&files);
        let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
        let mut debts: Vec<_> = prepared.typed().obligations.iter().filter_map(|o| match &o.kind {
            omega_typed::ObligationKind::EstablishDomain(d) => Some((o.site.span.slice(text.as_bytes()), d.declaration.path.as_str())),
            _ => None,
        }).collect();
        debts.sort();
        assert_eq!(debts, [(Some(cast.as_bytes()), "u8::P"), (Some(cast.as_bytes()), "u8::Tag")]);
        let pkg = sources::lower_prepared(&prepared, &files).unwrap();
        drop(prepared);
        let domain = |path| pkg.domains.iter().find(|d| d.declaration.path == path).unwrap();
        let (p, tag) = (domain("u8::P"), domain("u8::Tag"));
        assert_eq!((p.requires.len(), &p.requires[0].predicate, tag.requires.is_empty()), (1, clause, true));
        let body = &pkg.machines[0].body;
        let [entry, arrival, _erased] = body.blocks.as_slice() else { panic!("expected the completion and the erasure") };
        let BlockTerminator::Jump(edge) = &entry.terminator else { panic!("missing cast completion") };
        assert_eq!(body.diagnostics.edges[edge.id.get() - 1].span.slice(text.as_bytes()), Some(cast.as_bytes()));
        let [dest] = arrival.scalar_parameters[..] else { panic!("expected one destination") };
        let BlockTerminator::Jump(erase) = &arrival.terminator else { panic!("missing erasure") };
        assert_eq!(erase.scalar_qualification_changes, [ScalarQualificationChange { argument_index: 0, direction: QualificationDirection::Erase }]);
        assert!(erase.domain_completions.is_empty());
        let atom = |d: &omega_psi::LoweredDomain| DomainAtom { declaration: d.declaration.clone(), arguments: vec![] };
        let row = |d, origin| DomainCompletion { argument_index: 0, domain: atom(d), origin };
        spec_corpus::observe_handoff!(psi "psi-output/mixed-cast-completion", assert_eq!(
            (edge.scalar_qualification_changes.as_slice(), edge.domain_completions.as_slice(), arrival.scalar_domain_invariants.as_slice()),
            (&[][..], &[row(p, CompletionOrigin::Proved), row(tag, CompletionOrigin::Vacuous)][..],
             &[ScalarDomainInvariant { subject: dest, domain: atom(p) }][..])
        ));
        assert_eq!(psi_handoff_consumer::cast_completions(&pkg, body, edge, 0),
                   Ok(vec![(&atom(p), CompletionOrigin::Proved), (&atom(tag), CompletionOrigin::Vacuous)]));
    }

    // The consumer refuses each Psi that misstates one origin, row or arrival obligation.
    type Mutation = fn(&mut omega_psi::SuccessorEdge, &mut Vec<ScalarDomainInvariant>);
    let mutations: [(Mutation, &str); 6] = [
        (|e, _| e.domain_completions.swap(0, 1), "completion rows do not name each added domain once in destination order"),
        (|e, _| { e.domain_completions.pop(); }, "completion rows do not name each added domain once in destination order"),
        (|e, _| e.domain_completions[0].origin = CompletionOrigin::Vacuous, "a vacuous origin names a domain with predicates"),
        (|e, _| e.domain_completions[1].origin = CompletionOrigin::Proved, "a proved origin names a predicate-free domain"),
        (|e, _| e.scalar_qualification_changes.push(ScalarQualificationChange { argument_index: 0, direction: QualificationDirection::Add }),
         "an argument with completions also carries a qualification change"),
        (|_, i| i.clear(), "the arrival invariants differ from the proved completions"),
    ];
    for (mutate, expected) in mutations {
        let mut pkg = sources::lower(&[("limit.omg", original)]).unwrap();
        let (entry, rest) = pkg.machines[0].body.blocks.split_first_mut().unwrap();
        let BlockTerminator::Jump(edge) = &mut entry.terminator else { panic!("missing cast completion") };
        mutate(edge, &mut rest[0].scalar_domain_invariants);
        let body = &pkg.machines[0].body;
        let BlockTerminator::Jump(edge) = &body.blocks[0].terminator else { unreachable!() };
        assert_eq!(psi_handoff_consumer::cast_completions(&pkg, body, edge, 0), Err(expected));
    }

    // Dropping +Tag keeps one proved completion. Dropping +P keeps the ordinary Add,
    // which names no completion row.
    let d = "domain u8::P requires true; domain u8::Tag;";
    for (text, rows) in [
        (format!("{d} machine make(x:u8)->u8 {{ let q = x as +P; q as -P }}"), Some(vec![("u8::P", CompletionOrigin::Proved)])),
        (format!("{d} machine make(x:u8)->u8 {{ let q = x as +Tag; q as -Tag }}"), None),
    ] {
        let pkg = sources::lower(&[("limit.omg", &text)]).unwrap();
        let body = &pkg.machines[0].body;
        let BlockTerminator::Jump(edge) = &body.blocks[0].terminator else { panic!("missing cast") };
        let read = psi_handoff_consumer::cast_completions(&pkg, body, edge, 0).map(|r| r.into_iter().map(|(d, o)| (d.declaration.path.as_str(), o)).collect::<Vec<_>>());
        match rows {
            Some(rows) => assert_eq!(read, Ok(rows)),
            None => {
                assert_eq!(edge.scalar_qualification_changes, [ScalarQualificationChange { argument_index: 0, direction: QualificationDirection::Add }]);
                assert!(edge.domain_completions.is_empty());
                assert_eq!(read, Err("an argument with completions also carries a qualification change"));
            }
        }
    }

    // Omitting either entry leaves the result type unmet, so the typer refuses first.
    for (cast, found) in [("x as +P", "found u8 in app\\u8::P"), ("x as +Tag", "found u8 in app\\u8::Tag")] {
        let text = format!("{d} machine make(x:u8)->u8 in P & Tag {{{cast}}}");
        let resolved = sources::resolve(&[("limit.omg", &text)]);
        let error = omega_preparation::prepare_source_package(&resolved).expect_err("the typer refuses the omitted domain");
        assert_eq!((error.file.as_str(), error.span.slice(text.as_bytes())), ("limit.omg", Some(cast.as_bytes())));
        assert!(error.message.ends_with(found), "{error:?}");
    }
}

/// The original applied-domain casts. `Five` expands to one applied atom, and each
/// completion row and proved invariant names that exact atom with its closed argument,
/// beside the unapplied domain of the other origin. `+Five` alone keeps the argument too.
#[test]
fn an_applied_domain_completion_keeps_its_closed_arguments_beside_the_other_origin() {
    use omega_psi::{BlockTerminator, CompletionOrigin, DeclarationTerm, DomainAtom, DomainCompletion, QualificationDirection, ScalarDomainInvariant, ScalarQualificationChange};
    use CompletionOrigin::{Proved, Vacuous};
    let byte = ScalarType::Integer { signed: false, width: 8 };
    let tag_original = "domain<const N:u8> u8::Bound<N>; domain u8::Five = u8::Bound<5>; domain u8::P requires true; machine make(x:u8)->u8 in Five & P {x as +Five +P}";
    let predicate_original = "domain<const N:u8> u8::Above<N> requires self > N; domain u8::Five = u8::Above<5>; domain u8::Tag; machine make(x:u8)->u8 in Five & Tag {x as +Five +Tag}";
    let alone = "domain<const N:u8> u8::Above<N> requires self > N; domain u8::Five = u8::Above<5>; machine make(x:u8)->u8 in Five {x as +Five}";
    type Row = (&'static str, Option<u8>, CompletionOrigin);
    let cases: [(String, &str, Vec<Row>); 5] = [
        (tag_original.to_owned(), "x as +Five +P", vec![("u8::Bound", Some(5), Vacuous), ("u8::P", None, Proved)]),
        (tag_original.replace("Bound<5>", "Bound<6>"), "x as +Five +P", vec![("u8::Bound", Some(6), Vacuous), ("u8::P", None, Proved)]),
        (predicate_original.to_owned(), "x as +Five +Tag", vec![("u8::Above", Some(5), Proved), ("u8::Tag", None, Vacuous)]),
        (predicate_original.replace("Above<5>", "Above<6>"), "x as +Five +Tag", vec![("u8::Above", Some(6), Proved), ("u8::Tag", None, Vacuous)]),
        (alone.to_owned(), "x as +Five", vec![("u8::Above", Some(5), Proved)]),
    ];
    for (text, cast, expected) in cases {
        let files = [("limit.omg", text.as_str())];
        let resolved = sources::resolve(&files);
        let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
        let mut debts: Vec<_> = prepared.typed().obligations.iter().filter_map(|o| match &o.kind {
            omega_typed::ObligationKind::EstablishDomain(d) => Some((o.site.span.slice(text.as_bytes()), d.declaration.path.as_str(), d.arguments.len())),
            _ => None,
        }).collect();
        debts.sort();
        let mut owed: Vec<_> = expected.iter().map(|&(path, n, _)| (Some(cast.as_bytes()), path, usize::from(n.is_some()))).collect();
        owed.sort();
        assert_eq!(debts, owed);
        let pkg = sources::lower_prepared(&prepared, &files).unwrap();
        drop(prepared);
        let atom = |(path, n, _): &Row| DomainAtom {
            declaration: pkg.domains.iter().find(|d| d.declaration.path == *path).unwrap().declaration.clone(),
            arguments: n.iter().map(|&v| DeclarationTerm::StaticInteger { carrier: byte, value: v.into() }).collect(),
        };
        let rows: Vec<_> = expected.iter().map(|row| DomainCompletion { argument_index: 0, domain: atom(row), origin: row.2 }).collect();
        let body = &pkg.machines[0].body;
        let [entry, arrival] = body.blocks.as_slice() else { panic!("expected one completion") };
        let BlockTerminator::Jump(edge) = &entry.terminator else { panic!("missing cast completion") };
        assert_eq!(body.diagnostics.edges[edge.id.get() - 1].span.slice(text.as_bytes()), Some(cast.as_bytes()));
        let [dest] = arrival.scalar_parameters[..] else { panic!("expected one destination") };
        let invariants: Vec<_> = expected.iter().filter(|row| row.2 == Proved).map(|row| ScalarDomainInvariant { subject: dest, domain: atom(row) }).collect();
        spec_corpus::observe_handoff!(psi "psi-output/applied-cast-completion", assert_eq!(
            (edge.scalar_qualification_changes.as_slice(), edge.domain_completions.as_slice(), arrival.scalar_domain_invariants.as_slice()),
            (&[][..], rows.as_slice(), invariants.as_slice())
        ));
        let read: Vec<_> = expected.iter().map(|row| (atom(row), row.2)).collect();
        assert_eq!(psi_handoff_consumer::cast_completions(&pkg, body, edge, 0),
                   Ok(read.iter().map(|(a, o)| (a, *o)).collect::<Vec<_>>()));
    }

    // The consumer refuses a row or invariant whose argument differs from the destination atom.
    type Mutation = fn(&mut omega_psi::SuccessorEdge, &mut Vec<ScalarDomainInvariant>);
    fn six(carrier: ScalarType) -> Vec<omega_psi::DeclarationTerm> {
        vec![omega_psi::DeclarationTerm::StaticInteger { carrier, value: 6.into() }]
    }
    let mutations: [(Mutation, &str); 3] = [
        (|e, _| e.domain_completions[0].domain.arguments = six(ScalarType::Integer { signed: false, width: 8 }),
         "completion rows do not name each added domain once in destination order"),
        (|e, _| e.domain_completions[0].domain.arguments.clear(), "completion rows do not name each added domain once in destination order"),
        (|_, i| i[0].domain.arguments = six(ScalarType::Integer { signed: false, width: 8 }), "the arrival invariants differ from the proved completions"),
    ];
    for (mutate, expected) in mutations {
        let mut pkg = sources::lower(&[("limit.omg", predicate_original)]).unwrap();
        let (entry, rest) = pkg.machines[0].body.blocks.split_first_mut().unwrap();
        let BlockTerminator::Jump(edge) = &mut entry.terminator else { panic!("missing cast completion") };
        mutate(edge, &mut rest[0].scalar_domain_invariants);
        let body = &pkg.machines[0].body;
        let BlockTerminator::Jump(edge) = &body.blocks[0].terminator else { unreachable!() };
        assert_eq!(psi_handoff_consumer::cast_completions(&pkg, body, edge, 0), Err(expected));
    }

    // The applied tag alone is still an ordinary Add, which names no completion.
    let tag_alone = "domain<const N:u8> u8::Bound<N>; domain u8::Five = u8::Bound<5>; machine make(x:u8)->u8 in Five {x as +Five}";
    let pkg = sources::lower(&[("limit.omg", tag_alone)]).unwrap();
    let BlockTerminator::Jump(edge) = &pkg.machines[0].body.blocks[0].terminator else { panic!("missing cast") };
    assert_eq!(edge.scalar_qualification_changes, [ScalarQualificationChange { argument_index: 0, direction: QualificationDirection::Add }]);
    assert!(edge.domain_completions.is_empty());
}

#[test]
fn a_schema_argument_without_a_policy_result_cannot_supply_issuance() {
    let template = "use omega\\language\\core;
                    use omega\\language\\core\\reflect;
                    use omega\\language\\core\\access;
                    domain i32::Locked access Rule;
                    Rule satisfies AccessPolicy {
                        machine plan<'t>(schema:TypeSchema<'t>)->AccessPlan<'t> reaches none crashes none terminates {
                            new_plan::<'t>(@schema, ISSUANCE)
                        }
                    }";
    for issuance in ["Issuance::Unrestricted{}", "Issuance::Only{routes: []}"] {
        let policy = template.replace("ISSUANCE", issuance);
        let source = format!("{policy}
            machine issue(x:i32)->i32 in Locked {{ x as +Locked }}
            machine forward(x:i32 in Locked)->i32 in Locked {{ x }}");
        let mut package = sources::resolve(&[("policy.omg", &source)]);
        package.bind_package_key("app", [0x42; 32]).unwrap();
        let prepared = omega_preparation::prepare_source_package(&package).unwrap();
        let [selection] = &prepared.typed().access_policies[..] else {
            panic!("one selected domain")
        };
        assert!(matches!(
            prepared.pending_access_plan(&selection.domain).unwrap().schema,
            omega_prepared::PolicySchemaInput::Ready { .. }
        ));
        // Only the cast asks to introduce the qualification. Forwarding requires
        // the formal to carry it already, regardless of which policy was selected.
        let establishments: Vec<_> = prepared.typed().obligations.iter().filter_map(|obligation| {
            let omega_typed::ObligationKind::EstablishDomain(domain) = &obligation.kind else { return None };
            (domain.declaration == selection.domain).then_some(obligation.site.span.slice(source.as_bytes()).unwrap())
        }).collect();
        assert_eq!(establishments, [b"x as +Locked".as_slice()]);
        let error = omega_lowering::lower_package(&prepared).unwrap_err();
        assert_eq!(error.message, r"domain access policy needs checked establishment of `language\core\AccessPlan` in `language\core\access\new_plan`");
        assert_eq!(error.site, selection.site);
    }
}

#[test]
fn domain_cleanup_follows_the_owned_occurrences_qualification() {
    let case = include_str!("fixtures/package_cases/domain-selected-cleanup/case/case.omg");
    let control = include_str!("fixtures/package_cases/domain-selected-cleanup/control/case.omg");
    let qualified = case.replace("hold(r:R)", "hold(r:R in Live)");
    let substitution = qualified.replace("domain R::Live dropped by end", "machine finish(r:R){let R{v as _}=r;} domain R::Live dropped by finish");
    let packages = [case, control, qualified.as_str(), substitution.as_str()].map(|text| sources::lower(&[("case.omg", text)]).unwrap());
    spec_corpus::observe_handoff!(psi "psi-output/record-domain-cleanup", assert_eq!(
        packages.each_ref().map(|pkg| psi_handoff_consumer::returned_domain_cleanup(pkg, "hold", "R::Live").unwrap()),
        [(Some("end"), vec![], vec![]), (None, vec![], vec![]),
         (Some("end"), vec!["R::Live".into()], vec!["end"]), (Some("finish"), vec!["R::Live".into()], vec!["finish"])]
    ));
}
