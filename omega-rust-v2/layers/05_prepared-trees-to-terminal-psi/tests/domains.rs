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
    assert_ne!(first.declaration, second.declaration);
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
        assert_eq!(
            package.types.get(domain.carrier),
            Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed, width: 32 }))
        );
    }
}

#[test]
fn unsupported_domain_clause_is_refused_at_its_predicate_call() {
    let source = "machine predicate()->bool reaches none crashes none terminates {true}
                  domain i32::Guarded requires true && predicate();";
    let error = sources::lower(&[("guard.omg", source)]).unwrap_err();
    assert_eq!(error.site.file, "guard.omg");
    assert_eq!(error.site.span.slice(source.as_bytes()), Some(b"predicate()".as_slice()));
    assert_eq!(error.message, "unsupported typed contract predicate record");
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
fn unsupported_domain_forms_do_not_publish_plain_classifications() {
    for (source, message) in [
        (
            "domain u8::Five = u8::Bound<5>; domain<const N:u8> u8::Bound<N>;",
            "domain alias requires indexed qualification lowering",
        ),
        ("domain<T> T::Tag;", "domain carrier requires a fixed scalar"),
        ("data Item { valid:bool; } domain Item::Tag;", "domain carrier requires a fixed scalar"),
        (
            "domain i32::Positive requires self > 0; domain i32::Positive::Small requires true;",
            "inherited domain requires its parent qualification",
        ),
    ] {
        let error = sources::lower(&[("domain.omg", source)]).unwrap_err();
        assert_eq!(error.site.file, "domain.omg");
        assert_eq!(error.message, message);
    }
}

#[test]
fn domain_access_refusal_identifies_the_authored_policy() {
    let source = "use omega\\language\\core;
                  use omega\\language\\core\\reflect;
                  use omega\\language\\core\\access;
                  domain i32::Locked access Rule;
                  Rule satisfies AccessPolicy {
                      machine plan<'t>(schema:TypeSchema<'t>)->AccessPlan<'t> reaches none crashes none terminates {
                          new_plan::<'t>(@schema, Issuance::Unrestricted{})
                      }
                  }";
    let error = sources::lower(&[("policy.omg", source)]).unwrap_err();
    assert_eq!(error.site.file, "policy.omg");
    assert_eq!(error.site.span.slice(source.as_bytes()), Some(b"access Rule".as_slice()));
    assert_eq!(error.message, "unsupported domain member: Access");
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
        assert_eq!(alias.members, [a.declaration.clone(), second.declaration.clone()]);
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
        assert_eq!(edge.scalar_qualification_changes, [ScalarQualificationChange { argument_index: 0, direction }]);
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
        assert_eq!(domains.iter().map(|id| id.path.as_str()).collect::<Vec<_>>(), names);
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
            assert_eq!(operation.kind, OperationKind::ExactIntegerCast { operand: n });
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
    let error = sources::lower(&[("cast.omg", &program)]).unwrap_err();
    assert_eq!(error.site.span.slice(program.as_bytes()), Some(b"a as -A +B".as_slice()));
    assert_eq!(error.message, "Psi has no specified edge for simultaneous domain removal and addition");
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
fn predicate_membership_cannot_be_added_with_only_a_tag_edge() {
    let text = "domain u8::Positive requires self > 0u8; machine f(x:u8)->u8 in Positive {x as +Positive}";
    let err = sources::lower(&[("domain.omg", text)]).unwrap_err();
    assert_eq!(err.site.span.slice(text.as_bytes()), Some("x as +Positive".as_bytes()));
    assert_eq!(err.message, "adding a predicate-bearing domain requires a retained establishment proof");
}
