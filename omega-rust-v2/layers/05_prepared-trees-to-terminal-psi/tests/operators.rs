#[path = "support/source.rs"]
mod source;
use omega_lowering::{BlockTerminator, Parameter, ReturnEdge, TypeShape};

#[test]
fn an_indexed_operand_keeps_its_domain_when_the_extent_is_computed() {
    use omega_lowering::{ArithmeticPolicy, CallArgument, IntegerPrimitive, OperationKind, OperationResult};

    let program = "domain u8::Policy;
        machine + add(x:u8 in Policy,y:u8)->u8 in Policy { ((x as -Policy) +| y) as +Policy }
        machine width()->u64 {4u64}
        machine literal(a:[u8 in Policy;4],i:u64)->u8 in Policy requires i < a.len {a[i]+1}
        machine computed(a:[u8 in Policy;width()],i:u64)->u8 in Policy requires i < a.len {a[i]+1}
        machine pack(x:u8 in Policy)->[u8 in Policy;1] {[x]}";
    let package = source::lower(&[("policy.omg", program)]).unwrap();
    let machine = |name| {
        package
            .machines
            .iter()
            .find(|m| m.declaration.path == name)
            .unwrap()
    };
    let add = machine("add");
    assert!(
        add.body
            .operations
            .iter()
            .any(|op| matches!(&op.kind, OperationKind::Integer { operation, .. }
        if operation.primitive == IntegerPrimitive::Add && operation.policy == ArithmeticPolicy::Saturating))
    );
    assert_eq!(machine("literal").parameter_types, machine("computed").parameter_types);
    let pack = machine("pack");
    let [Parameter::Scalar(input)] = pack.body.parameters[..] else {
        panic!("expected qualified element input")
    };
    assert!(matches!(&pack.body.operations[..], [op] if matches!(&op.kind,
        OperationKind::EstablishScalarArray{elements} if elements == &[input])));
    for name in ["literal", "computed"] {
        let caller = machine(name);
        use omega_lowering::PredicateStep::{Apply, FixedArrayLength, Formal};
        assert_eq!(
            caller.contract.requires[0].predicate,
            [
                Formal(1),
                Formal(0),
                FixedArrayLength,
                Apply(omega_lowering::PredicateOperator::LessThan)
            ]
        );
        let Some(TypeShape::FixedArray { element, length: 4 }) = package.types.get(caller.parameter_types[0]) else {
            panic!("expected four elements")
        };
        assert_eq!(Some(*element), add.result_type);
        let Some(TypeShape::QualifiedScalar { domains, .. }) = package.types.get(*element) else {
            panic!("missing element qualification")
        };
        assert_eq!(
            package.types.domain_set(*domains).unwrap(),
            std::slice::from_ref(&package.domains[0].declaration)
        );
        let body = &caller.body;
        let [Parameter::Structural(array), Parameter::Scalar(index)] = body.parameters[..] else {
            panic!("array and selector")
        };
        let [read, one, call] = body.operations.as_slice() else {
            panic!("expected the read, constant and selected addition")
        };
        let OperationKind::PrimitiveScalarRead { source, path } = &read.kind else {
            panic!("expected array read")
        };
        let [segment] = path.as_slice() else { panic!("expected a single index") };
        assert_eq!((*source, segment.selector), (array, index));
        assert_eq!(body.diagnostics.bounds.len(), 1);
        assert_eq!(
            body.diagnostics.bounds[segment.obligation.get() - 1]
                .span
                .slice(program.as_bytes()),
            Some(&b"i"[..])
        );
        let (OperationResult::Scalar(value), OperationResult::Scalar(rhs)) = (read.result, one.result) else {
            panic!("expected scalar operands")
        };
        assert_eq!(body.values[value.index()].ty, *element);
        assert!(
            matches!(&call.kind, OperationKind::Call{callee: omega_lowering::CallTarget::Machine(callee),arguments,..}
            if *callee==add.id && arguments == &[CallArgument::Scalar(value),CallArgument::Scalar(rhs)])
        );
    }
}

#[test]
fn array_reads_cannot_acquire_a_domain_or_bypass_its_predicate() {
    use omega_lowering::{
        OperationKind, OperationResult, PredicateOperator::GreaterThan, PredicateStep::{Apply, Integer, Result, SelfValue}, ScalarType,
    };

    let source = "domain u8::Mark; machine f(a:[u8;4],i:u64)->u8 in Mark{a[i]}";
    let package = source::resolve(&[("mark.omg", source)]);
    let error = omega_preparation::prepare_source_package(&package).unwrap_err();
    assert!(error.message.contains("expected u8 in app\\u8::Mark, found u8"), "{error:?}");
    let guarded = "domain u8::Mark requires self > 0u8; machine f(a:[u8 in Mark;4],i:u64)->u8 in Mark{a[i]}";
    let files = [("guarded.omg", guarded)];
    let resolved = source::resolve(&files);
    let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
    let trees = prepared.typed();
    let sig = trees.signatures.iter().find(|s| s.machine.path == "f").unwrap();
    assert_eq!(sig.parameters[0].1, omega_typed::ValueType::Array(Box::new(sig.result.clone()), 4));
    let [mark] = sig.result.domains() else { panic!("expected one prepared membership") };
    let access = trees.collection_accesses.iter().find(|a| a.site.file == "guarded.omg").unwrap();
    assert_eq!((&access.element, &access.result), (&sig.result, &sig.result));
    let package = source::lower_prepared(&prepared, &files).unwrap();
    let [domain] = package.domains.as_slice() else { panic!("expected Mark") };
    let [f] = package.machines.as_slice() else { panic!("expected f") };
    let Some(TypeShape::FixedArray { element, length: 4 }) = package.types.get(f.parameter_types[0]) else { panic!("expected four elements") };
    let Some(TypeShape::QualifiedScalar { domains, carrier }) = package.types.get(*element) else { panic!("expected qualified element") };
    assert_eq!(*carrier, ScalarType::Integer { signed: false, width: 8 });
    assert_eq!(mark.declaration.path, "u8::Mark");
    assert_eq!(domain.declaration.path, mark.declaration.path);
    let [theory] = domain.requires.as_slice() else { panic!("one domain requirement") };
    let [guarantee] = f.contract.ensures.as_slice() else { panic!("one scalar result guarantee") };
    spec_corpus::observe_handoff!(psi "psi-output/array-element-predicate", assert_eq!(
        (&theory.predicate[..], &guarantee.predicate[..]),
        (&[SelfValue, Integer(0.into()), Apply(GreaterThan)][..], &[Result, Integer(0.into()), Apply(GreaterThan)][..])
    ));
    let body = &f.body;
    let [Parameter::Structural(array), Parameter::Scalar(index)] = body.parameters[..] else { panic!("expected array and index") };
    let [read] = body.operations.as_slice() else { panic!("expected only the indexed read") };
    let OperationKind::PrimitiveScalarRead { source, path } = &read.kind else { panic!("expected scalar read") };
    let [step] = path.as_slice() else { panic!("expected one selector") };
    let OperationResult::Scalar(value) = read.result else { panic!("expected scalar leaf") };
    let [bound] = body.diagnostics.bounds.as_slice() else { panic!("expected index obligation") };
    assert_eq!(step.obligation.get(), 1);
    assert_eq!(bound.span.get_start_offset()..bound.span.get_end_offset(), source::span_within(guarded, "a[i]", "i"));
    let BlockTerminator::Return { result: ReturnEdge::Scalar(returned), .. } = body.blocks[0].terminator else { panic!("expected scalar return") };
    spec_corpus::observe_handoff!(psi "psi-output/guarded-array-read", assert_eq!(
        (*source, step.selector, body.values[value.index()].ty, package.types.domain_set(*domains), f.result_type, returned),
        (array, index, *element, Some(std::slice::from_ref(&domain.declaration)), Some(*element), value)
    ));

    // Remove only the predicate: the type still carries Mark, but no result proof is due.
    let tag = guarded.replace(" requires self > 0u8", "");
    let twin = source::lower(&[("tag.omg", &tag)]).unwrap();
    assert_eq!(twin.types, package.types);
    assert_eq!(twin.domains[0].declaration, domain.declaration);
    assert!(twin.domains[0].requires.is_empty());
    assert!(twin.machines[0].contract.ensures.is_empty());
}

#[test]
fn unused_operators_retain_the_home_and_the_exact_forwarded_operand() {
    let defs = ("domains.omg", "public domain u8::Km; public domain u8::Metres;");
    for (name, home) in [("combine", "Km"), ("Km::combine", "Km"), ("combine", "Metres")] {
        let other = if home == "Km" { "Metres" } else { "Km" };
        let declarations = format!(
            "public machine + {name}(x:u8 in {home}, y:u8 in {home})->u8 in {home} {{y}} machine + other(x:u8 in {other},y:u8 in {other})->u8 in {other} {{x}} machine plain(x:u8)->u8 {{x}}"
        );
        let ops = ("operators.omg", declarations.as_str());
        let package = source::lower(&[defs, ops]).unwrap();
        assert_eq!(package, source::lower(&[ops, defs]).unwrap());
        assert_eq!(package.machines.len(), 3);
        assert_eq!(
            package
                .machines
                .iter()
                .filter(|machine| machine.operator.is_some())
                .count(),
            2
        );
        for machine in &package.machines {
            if machine.declaration.path == "plain" {
                assert!(machine.operator.is_none());
                continue;
            }
            let first = machine.declaration.path != "other";
            let domain_name = format!("u8::{}", if first { home } else { other });
            let domain = package
                .domains
                .iter()
                .find(|d| d.declaration.path == domain_name)
                .unwrap();
            let binding = machine.operator.as_ref().unwrap();
            spec_corpus::observe_handoff!(
                "lowered-operator-home/consumer",
                assert_eq!(binding.home.as_ref(), Some(&domain.declaration)),
            );
            assert_eq!(binding.token, "+");
            let owner = if first { name } else { "other" };
            assert_eq!(binding.site.file, "operators.omg");
            assert_eq!(binding.site.span.get_start_offset(), declarations.find(&format!("+ {owner}(")).unwrap());
            assert_eq!(binding.public, first);
            assert_eq!(binding.site.span.slice(declarations.as_bytes()), Some(b"+".as_slice()));
            let result = machine.scalar_result.unwrap();
            assert_eq!(machine.parameter_types, [result.ty, result.ty]);
            let Some(TypeShape::QualifiedScalar { domains, .. }) = package.types.get(result.ty) else {
                panic!("missing qualified scalar")
            };
            assert_eq!(package.types.domain_set(*domains), Some(std::slice::from_ref(&domain.declaration)));
            let Parameter::Scalar(value) = machine.body.parameters[usize::from(first)] else {
                panic!("expected scalar parameter")
            };
            assert!(matches!(machine.body.blocks[0].terminator, BlockTerminator::Return {result:ReturnEdge::Scalar(v),..} if v==value));
        }
    }
}

#[test]
fn a_token_call_selects_the_operator_whose_home_owns_its_operands() {
    use omega_lowering::{CallArgument, OperationKind};

    // Swapping the homes swaps which declaration each token call reaches.
    for (km, metres) in [("pick", "other"), ("other", "pick")] {
        let source = format!(
            "domain u8::Km; domain u8::Metres;
             machine + {metres}(left:u8 in Metres,right:u8 in Metres)->u8 in Metres {{left}}
             machine + {km}(left:u8 in Km,right:u8 in Km)->u8 in Km {{right}}
             machine km_sum(a:u8 in Km, b:u8 in Km)->u8 in Km {{a + b}}
             machine metres_sum(a:u8 in Metres, b:u8 in Metres)->u8 in Metres {{b + a}}"
        );
        let package = source::lower(&[("homes.omg", &source)]).unwrap();
        let machine = |name: &str| {
            package
                .machines
                .iter()
                .find(|m| m.declaration.path == name)
                .unwrap()
        };
        for (caller, selected, home, swapped) in [("km_sum", km, "u8::Km", false), ("metres_sum", metres, "u8::Metres", true)] {
            let operator = machine(selected);
            assert_eq!(
                operator
                    .operator
                    .as_ref()
                    .and_then(|binding| binding.home.as_ref())
                    .map(|owner| owner.path.as_str()),
                Some(home)
            );
            let body = &machine(caller).body;
            let [Parameter::Scalar(a), Parameter::Scalar(b)] = body.parameters[..] else {
                panic!("expected two qualified operands")
            };
            let ordered = if swapped { [b, a] } else { [a, b] };
            let [call] = body.operations.as_slice() else {
                panic!("expected one selected operator call")
            };
            spec_corpus::observe_handoff!(
                "operator-ownership/consumer",
                assert_eq!(
                    call.kind,
                    OperationKind::Call {
                        callee: omega_lowering::CallTarget::Machine(operator.id),
                        arguments: ordered.map(CallArgument::Scalar).to_vec(),
                        requirements: vec![],
                    }
                )
            );
        }
    }
}

#[test]
fn the_explicit_cast_establishes_result_membership_on_the_selected_operand() {
    let program = "domain u8::Km; machine + add(x:u8 in Km,y:u8)->u8 in Km { y as +Km }";
    let package = source::lower(&[("operator.omg", program)]).unwrap();
    let body = &package.machines[0].body;
    let Parameter::Scalar(y) = body.parameters[1] else {
        panic!("expected second parameter")
    };
    let BlockTerminator::Jump(edge) = &body.blocks[0].terminator else {
        panic!("expected the explicit cast edge")
    };
    assert_eq!(edge.arguments, [y]);
    assert_eq!(
        edge.scalar_qualification_changes,
        [omega_lowering::ScalarQualificationChange {
            argument_index: 0,
            direction: omega_lowering::QualificationDirection::Add,
        }]
    );
    assert_eq!(
        body.diagnostics.edges[edge.id.get() - 1]
            .span
            .slice(program.as_bytes()),
        Some(b"y as +Km".as_slice())
    );
}

#[test]
fn a_root_operator_keeps_its_token_without_a_domain_home() {
    let package = source::lower(&[("root.omg", "machine + pick(x:[u8; 2],y:[u8; 2])->[u8; 2] {x}")]).unwrap();
    let binding = package.machines[0].operator.as_ref().unwrap();
    assert_eq!(binding.token, "+");
    assert!(binding.home.is_none());
}

#[test]
fn an_explicit_home_survives_even_when_neither_operand_names_it() {
    let package = source::lower(&[("plain.omg", "domain u8::Km; machine + Km::add(x:u8,y:u8)->u8 {x}")]).unwrap();
    let machine = &package.machines[0];
    let operator = machine.operator.as_ref().unwrap();
    assert_eq!(operator.home.as_ref(), Some(&package.domains[0].declaration));
    assert_eq!(machine.parameter_types, [package.domains[0].carrier; 2]);
}

#[test]
fn token_and_named_calls_forward_the_same_selected_machine_and_ordered_qualified_values() {
    use omega_lowering::{CallArgument, OperationKind, OperationResult};

    let source = "domain u8::Km; domain u8::Metres; machine + other(left:u8 in Metres,right:u8 in Metres)->u8 in Metres {left}
        machine + pick(left:u8 in Km, right:u8 in Km)->u8 in Km {right}
        machine by_token(a:u8 in Km, b:u8 in Km)->u8 in Km {b + a}
        machine by_name(a:u8 in Km, b:u8 in Km)->u8 in Km {pick(b, a)}";
    let package = source::lower(&[("distance.omg", source)]).unwrap();
    let pick = package
        .machines
        .iter()
        .find(|m| m.declaration.path == "pick")
        .unwrap();
    for name in ["by_token", "by_name"] {
        let body = &package
            .machines
            .iter()
            .find(|m| m.declaration.path == name)
            .unwrap()
            .body;
        let [Parameter::Scalar(a), Parameter::Scalar(b)] = body.parameters[..] else {
            panic!("expected qualified parameters")
        };
        let [operation] = body.operations.as_slice() else {
            panic!("expected a single call")
        };
        assert_eq!(
            operation.kind,
            OperationKind::Call {
                callee: omega_lowering::CallTarget::Machine(pick.id),
                arguments: vec![CallArgument::Scalar(b), CallArgument::Scalar(a)],
                requirements: vec![],
            }
        );
        let OperationResult::Scalar(output) = operation.result else {
            panic!("expected a qualified result")
        };
        assert_eq!(Some(body.values[output.index()].ty), pick.result_type);
        assert!(matches!(body.blocks[0].terminator, BlockTerminator::Return {result: ReturnEdge::Scalar(source), ..} if source == output));
    }
}

#[test]
fn terminal_operator_calls_keep_the_selected_edge_and_each_operand_occurrence() {
    use omega_lowering::{CallArgument, CallRequirement, CallSite};

    for (token, parameters, expression, arguments) in [
        ("-", "v:u8 in Km", "-v", vec![0]),
        ("[]", "v:u8 in Km,i:u8", "v[j]", vec![0, 2]),
        ("[..]", "v:u8 in Km,i:u8,j:u8", "v[j..i]", vec![0, 2, 1]),
    ] {
        let source = format!(
            "domain u8::Km; machine {token} select({parameters})->u8 in Km requires true {{v}}
            machine caller(v:u8 in Km, i:u8, j:u8)->u8 in Km {{transition {{_ -> ({expression}),}}}}"
        );
        let package = source::lower(&[("operator.omg", &source)]).unwrap();
        let [caller, selected] = package.machines.as_slice() else {
            panic!("expected caller and callee")
        };
        assert_eq!(selected.declaration.path, "select");
        assert!(caller.body.operations.is_empty());
        let edges = caller
            .body
            .blocks
            .iter()
            .filter_map(|block| match &block.terminator {
                BlockTerminator::TerminalCall {
                    id,
                    callee: omega_lowering::CallTarget::Machine(callee),
                    arguments,
                    requirements,
                    ..
                } => Some((id, callee, arguments, requirements)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [(edge, callee, actual, requirements)] = edges.as_slice() else {
            panic!("expected one terminal call for {expression}")
        };
        assert_eq!(**callee, selected.id);
        let expected: Vec<_> = arguments
            .into_iter()
            .map(|index| {
                let Parameter::Scalar(value) = caller.body.parameters[index] else {
                    panic!("expected scalar parameter")
                };
                CallArgument::Scalar(value)
            })
            .collect();
        assert_eq!(**actual, expected);
        spec_corpus::observe_handoff!(
            psi "psi-output/operator-edge",
            assert_eq!(
                **requirements,
                [CallRequirement {
                    site: CallSite::Edge(**edge),
                    clause: 0
                }]
            ),
        );
    }
}

#[test]
fn a_qualified_call_argument_keeps_the_local_and_join_value() {
    use omega_lowering::{CallArgument, OperationKind};

    let source = "domain u8::Km; machine echo(a:u8 in Km)->u8 in Km {a}
        machine caller(flag:bool, a:u8 in Km, b:u8 in Km)->u8 in Km {
            let chosen = match flag {true -> a, false -> b,};
            echo(chosen)
        }";
    let package = source::lower(&[("local.omg", source)]).unwrap();
    let [caller, echo] = package.machines.as_slice() else {
        panic!("expected caller and callee")
    };
    let join = caller
        .body
        .blocks
        .iter()
        .find(|block| !block.scalar_parameters.is_empty())
        .unwrap();
    let [joined] = join.scalar_parameters[..] else {
        panic!("expected one joined value")
    };
    assert_eq!(Some(caller.body.values[joined.index()].ty), echo.result_type);
    let [call] = &caller.body.operations[join.operations.clone()] else {
        panic!("expected the call at the join")
    };
    assert_eq!(
        call.kind,
        OperationKind::Call {
            callee: omega_lowering::CallTarget::Machine(echo.id),
            arguments: vec![CallArgument::Scalar(joined)],
            requirements: vec![]
        }
    );
}

#[test]
fn nested_operator_visibility_uses_the_resolved_export() {
    let operator = "public machine + pick(a:[u8;2],b:[u8;2])->[u8;2] {a}";
    let nested = format!("public machine outer(x:[u8;2])->[u8;2] {{ x {operator} }}");
    for (program, path, public) in [
        (operator, "pick", true),
        (nested.as_str(), "outer::pick", false),
    ] {
        let package = source::lower(&[("visibility.omg", program)]).unwrap();
        let machine = package
            .machines
            .iter()
            .find(|machine| machine.declaration.path == path)
            .unwrap();
        assert_eq!(machine.contract.public, public, "{path}");
        assert_eq!(machine.operator.as_ref().unwrap().public, public, "{path}");
    }
}
