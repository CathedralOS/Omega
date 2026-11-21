use omega_preparation::{
    ConstantMaterializationError, LayoutExtent, LayoutGeometry, PreparedValue, SelectedLayout, TargetIdentity, TargetSemantics, prepare_source_package,
    prepare_source_package_for_target,
};
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::{StaticValue, ValueType, resolve_source_package};

fn package(source: &str) -> omega_typer::SourcePackage<'_> {
    let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
    let syntax = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
    resolve_source_package(&[StageSource {
        package: "app",
        source: ParsedSource {
            logical_path: "main.omg",
            text: source,
            tokens: &tokens,
            tree: &syntax,
        },
    }])
    .unwrap()
}

#[test]
fn lowering_can_distinguish_two_applications_of_the_same_field_recipe() {
    let package = package(
        "data Flag<const V:bool>{}
        machine negate(x:bool)->bool{!x}
        data Wrapper<const B:bool>{field:Flag<negate(B)>;}
        machine one(x:Wrapper<true>)->Flag<false>{x.field}
        machine two(x:Wrapper<false>)->Flag<true>{x.field}",
    );
    let prepared = prepare_source_package(&package).unwrap();
    let answers = prepared.specialized_indices();
    assert_eq!(answers.len(), 2);
    assert_eq!(answers[0].request, answers[1].request);
    for arg in [false, true] {
        let selected = answers
            .iter()
            .find(|answer| {
                matches!(answer.arguments.as_slice(),
                    [ValueType::StaticValue { value: StaticValue::Bool(b), .. }] if *b == arg
                )
            })
            .expect("the frozen stage retains each closed owner argument");
        spec_corpus::assert_handoff_eq!(format!("field-recipe/{arg}"), selected.value, StaticValue::Bool(!arg));
    }
}

fn target_with_layout(
    subject: ValueType,
    bytes: u64,
    plan: u8,
) -> TargetSemantics {
    let target = TargetIdentity {
        profile: "test-target".into(),
        semantics_version: 1,
        realization: vec![1].into(),
    };
    let layout = SelectedLayout {
        target: target.clone(),
        subject,
        schema: vec![1].into(),
        plan: vec![plan].into(),
        geometry: LayoutGeometry {
            extent: LayoutExtent::Fixed(bytes),
            alignment: Some(1),
        },
    };
    TargetSemantics::new(target, [layout], []).unwrap()
}

#[test]
fn materialization_pairs_the_frozen_value_with_its_exact_selected_layout() {
    let source = package("const VALUE:u8=7u8;");
    let prepared = prepare_source_package(&source).unwrap();
    let decl = &prepared.typed().constants[0].declaration;
    let byte = ValueType::Integer { signed: false, width: Some(8) };
    let target = target_with_layout(byte.clone(), 1, 2);
    let input = prepared.constant_for_materialization(decl, &target).unwrap();
    assert_eq!(&input.definition.declaration, decl);
    assert_eq!(input.value, &PreparedValue::Integer(7.into()));
    assert_eq!(input.layout.subject, byte);
    spec_corpus::assert_handoff_eq!("materialization/layout-plan", input.layout.plan.as_ref(), &[2]);
    let other = target_with_layout(ValueType::Bool, 1, 2);
    assert!(matches!(prepared.constant_for_materialization(decl, &other), Err(ConstantMaterializationError::MissingLayout(def)) if &def.declaration == decl));
}

#[test]
fn materializing_a_folded_size_requires_the_original_layout_selection() {
    let source = package("use omega\\language\\core; const SIZE:u64=core\\size_of::<u64>();");
    let word = ValueType::Integer {
        signed: false,
        width: Some(64),
    };
    let first = target_with_layout(word.clone(), 8, 1);
    let prepared = prepare_source_package_for_target(&source, &[], &first).unwrap();
    let decl = &prepared.typed().constants[0].declaration;
    assert_eq!(
        prepared
            .constant_for_materialization(decl, &first)
            .unwrap()
            .value,
        &PreparedValue::Integer(8.into())
    );
    let identical = target_with_layout(word.clone(), 8, 1);
    assert!(prepared.constant_for_materialization(decl, &identical).is_ok());
    let different_plan = target_with_layout(word, 8, 2);
    assert!(matches!(
        prepared.constant_for_materialization(decl, &different_plan),
        Err(ConstantMaterializationError::ChangedTargetDependency(_))
    ));
}

#[test]
fn selected_generic_bodies_keep_their_concrete_carriers_across_nested_calls() {
    let source = package(
        "machine identity<T>(x:T)->T{x}
        machine relay<T>(x:T)->T {let saved:T=x; identity::<T>(saved)}
        const BYTE:u8=relay::<u8>(11u8);
        const FLAG:bool=relay::<bool>(true);",
    );
    let prepared = prepare_source_package(&source).unwrap();
    for constant in &prepared.typed().constants {
        let expected = match constant.declaration.path.as_str() {
            "BYTE" => PreparedValue::Integer(11.into()),
            "FLAG" => PreparedValue::Bool(true),
            _ => unreachable!(),
        };
        assert_eq!(prepared.constant(&constant.declaration), Some(&expected));
    }
}

#[test]
fn generic_returns_discharge_possible_loans_only_for_complete_owned_results() {
    use omega_preparation::PreparedFloat;
    let machines = "machine identity<T>(x:T)->T{x}
        machine relay<T>(x:T)->T {let saved:T=x; identity::<T>(saved)}";
    let source = format!(
        "{machines}
        data Sample [copy] {{reading:f32; enabled:bool;}}
        const FLOAT:f64=relay::<f64>(0.5f64);
        const ARRAY:[u8;2]=relay::<[u8;2]>([3u8,7u8]);
        const RECORD:Sample=relay::<Sample>(Sample{{enabled:true,reading:1.5f32}});"
    );
    let source = package(&source);
    let prepared = prepare_source_package(&source).unwrap();
    for name in ["identity", "relay"] {
        assert!(prepared.typed().obligations.iter().any(|obligation| matches!(
            &obligation.kind, omega_typer::ObligationKind::ReturnBorrow { machine } if machine.path == name
        )));
    }
    let constant = |name| {
        let declaration = &prepared
            .typed()
            .constants
            .iter()
            .find(|constant| constant.declaration.path == name)
            .unwrap()
            .declaration;
        prepared.constant(declaration).unwrap()
    };
    assert_eq!(constant("FLOAT").float(), Some(PreparedFloat::Binary64(0x3fe0_0000_0000_0000)));
    assert_eq!(
        constant("ARRAY"),
        &PreparedValue::Array(
            [
                PreparedValue::Integer(3.into()),
                PreparedValue::Integer(7.into())
            ]
            .into()
        )
    );
    let PreparedValue::Record { owner, fields } = constant("RECORD") else {
        panic!("expected a Sample record")
    };
    assert_eq!(owner.path, "Sample");
    spec_corpus::observe_handoff!("declared-field-order/consumer", assert_eq!(fields[0].float(), Some(PreparedFloat::Binary32(0x3fc0_0000))));
    assert_eq!(fields[1], PreparedValue::Bool(true));

    let borrowed = format!("{machines}\nconst BORROWED:@[u8]=relay::<@[u8]>(\"data\");");
    assert!(prepare_source_package(&package(&borrowed)).is_err());
    let empty_reference_array = format!("{machines}\nconst BORROWED:[@[u8];0]=relay::<[@[u8];0]>([]);");
    let error = prepare_source_package(&package(&empty_reference_array))
        .unwrap_err()
        .message;
    assert!(error.contains("complete array element type is outside the prepared value fragment"), "{error}");
}

#[test]
fn generic_indexed_integer_operations_keep_their_selected_policy_and_width() {
    let source = "machine sum<const N:u64>(x:u<N>,y:u<N>)->u<N>{x+%y}
        const WRAPPED:u13=sum::<13>(8191u13,1u13);";
    let package_under_test = package(source);
    let prepared = prepare_source_package(&package_under_test).unwrap();
    spec_corpus::observe_handoff!(
        "selected-operator-policy-and-width/consumer",
        assert_eq!(prepared.integer_constant(&prepared.typed().constants[0].declaration), Some(&0.into()))
    );
    assert!(
        prepared
            .typed()
            .operations
            .iter()
            .any(|operation| operation.token == "+%" && matches!(operation.result, ValueType::IndexedInteger { .. }))
    );

    let exact = source.replace("+%", "+");
    let error = prepare_source_package(&package(&exact)).unwrap_err().message;
    assert!(error.contains("exact addition exceeds its selected integer carrier"), "{error}");
}

#[test]
fn a_frozen_alias_retains_the_origin_of_its_target_observation() {
    use omega_preparation::PreparedValueOrigin;
    let source = package(
        "use omega\\language\\core; const SIZE:u64=core\\size_of::<u64>(); const COPY:u64=SIZE;
                          machine consume(x:[u8;COPY]){}",
    );
    let word = ValueType::Integer {
        signed: false,
        width: Some(64),
    };
    let target = target_with_layout(word, 8, 1);
    let prepared = prepare_source_package_for_target(&source, &[], &target).unwrap();
    let find = |name| {
        &prepared
            .typed()
            .constants
            .iter()
            .find(|c| c.declaration.path == name)
            .unwrap()
            .declaration
    };
    let size = find("SIZE");
    let alias = find("COPY");
    let read = prepared
        .dependencies()
        .inputs(&PreparedValueOrigin::Constant(alias.clone()))
        .unwrap();
    assert_eq!(read.constants.as_slice(), std::slice::from_ref(size));
    let observations = prepared
        .dependencies()
        .inputs(&PreparedValueOrigin::Constant(size.clone()))
        .unwrap();
    assert_eq!(observations.target_observations, prepared.target_observations());
    let (request, _) = &prepared.typed().evaluated_indices[0];
    let index = PreparedValueOrigin::StaticIndex(omega_typer::StaticValueDemand::Closed(request.clone()));
    assert_eq!(
        prepared
            .dependencies()
            .inputs(&index)
            .unwrap()
            .constants
            .as_slice(),
        std::slice::from_ref(alias)
    );
}

#[test]
fn generic_bodies_read_the_closed_const_and_layout_arguments() {
    let source = package(
        "use omega\\language\\core;
        machine value<const N:u64>()->u64{N}
        machine bytes<T>()->u64{core\\size_of::<T>()}
        const N:u64=value::<12>(); const WIDTH:u64=bytes::<u64>();",
    );
    let word = ValueType::Integer {
        signed: false,
        width: Some(64),
    };
    let target = target_with_layout(word, 8, 1);
    let prepared = prepare_source_package_for_target(&source, &[], &target).unwrap();
    for constant in &prepared.typed().constants {
        let expected = match constant.declaration.path.as_str() {
            "N" => 12,
            "WIDTH" => 8,
            _ => unreachable!(),
        };
        assert_eq!(prepared.constant(&constant.declaration), Some(&PreparedValue::Integer(expected.into())));
    }
}
#[test]
fn typing_resumes_only_from_answers_for_the_same_round() {
    use omega_preparation::{CheckedPreparationEvidence, PreparationAdvanceError, PreparationProgress, PreparationRound, PreparationStep};
    let source = package("machine width()->u64{3u64} machine f(x:[u8;width()]){} const N:u8=7u8;");
    let mut round = PreparationRound::begin(&source, &[], None).unwrap();
    let other = PreparationRound::begin(&source, &[], None).unwrap();
    let first = round.evaluate(CheckedPreparationEvidence::default()).unwrap();
    assert!(matches!(other.advance(first), Err(PreparationAdvanceError::DifferentRound)));
    loop {
        let answers = round.evaluate(CheckedPreparationEvidence::default()).unwrap();
        match round.advance(answers).unwrap() {
            PreparationStep::Evaluate(next) => round = *next,
            PreparationStep::Finished(PreparationProgress::Complete(prepared)) => {
                assert_eq!(prepared, prepare_source_package(&source).unwrap());
                assert_eq!(prepared.typed().evaluated_indices[0].1, StaticValue::Integer(3.into()));
                break;
            }
            PreparationStep::Finished(PreparationProgress::OpenTarget(_)) => panic!("a literal extent has no target dependency"),
        }
    }
}
#[test]
fn fixed_array_reads_use_the_selected_bounds_and_can_project_an_element_field() {
    let setup = "data Packet [copy] { byte:u8; }
        const ITEMS:[Packet;2]=[Packet{byte:4u8}, Packet{byte:9u8}];
        machine pick(items:[Packet;2], i:u64)->u8{items[i].byte}";
    let accepted = format!("{setup} const N:u8=pick(ITEMS,1u64); const LENGTH:u64=ITEMS.len;");
    let source = package(&accepted);
    let prepared = prepare_source_package(&source).unwrap();
    for (name, expected) in [("N", 9), ("LENGTH", 2)] {
        let declaration = &prepared
            .typed()
            .constants
            .iter()
            .find(|c| c.declaration.path == name)
            .unwrap()
            .declaration;
        assert_eq!(prepared.integer_constant(declaration), Some(&expected.into()));
    }
    // L4 executes `items[i]` only through the selected core read whose
    // precondition obligation L3 attached to the index node. Twin: the
    // selector alone changes the frozen element or reaches that precondition.
    let read = |index: u64| -> Result<num_bigint::BigInt, String> {
        let program = format!("{setup} const N:u8=pick(ITEMS,{index}u64);");
        let source = package(&program);
        let prepared = prepare_source_package(&source).map_err(|error| error.message)?;
        let declaration = &prepared
            .typed()
            .constants
            .iter()
            .find(|c| c.declaration.path == "N")
            .unwrap()
            .declaration;
        let value = prepared.integer_constant(declaration).cloned().unwrap();
        Ok(value)
    };
    let reads = [0, 1, 2].map(read);
    spec_corpus::observe_handoff!(
        "selected-element-read-precondition/consumer",
        assert!(
            matches!(&reads, [Ok(first), Ok(second), Err(refusal)]
                if *first == num_bigint::BigInt::from(4) && *second == num_bigint::BigInt::from(9) && refusal.contains("selected array read requires index < items.len")),
            "{reads:?}"
        )
    );
}

#[test]
fn intrinsic_array_length_checks_its_result_before_freezing_the_whole_array() {
    use omega_preparation::{CheckedPreparationEvidence, PreparationAnswers, PreparationReason, RefusalCause, prepare_typing_pass_with_cause};
    use omega_typed::TypedTrees;
    use omega_typer::{TypingEnvironment, TypingProgress, type_source_package_for_preparation};

    for length in [2, 256] {
        let code = format!("const BUF:[u8;{length}]=\"{}\"; const NS:[u64;1]=[BUF.len];", "x".repeat(length));
        let source = package(&code);
        for narrow in [false, true] {
            let TypingProgress::Complete(trees) = type_source_package_for_preparation(&source, &TypingEnvironment::default()).unwrap() else {
                panic!("expected complete typing");
            };
            let resolved = trees.resolved().clone();
            let mut facts = trees.into_facts();
            let ns = facts.constants.iter_mut().find(|c| c.declaration.path == "NS").unwrap();
            let answer = ns.declaration.clone();
            if narrow {
                let byte = ValueType::Integer { signed: false, width: Some(8) };
                ns.initializer.carrier = ValueType::Array(Box::new(byte.clone()), 1);
                let length_op = facts.operations.iter_mut().find(|op| op.token == ".len").unwrap();
                length_op.result = byte.clone();
                for expression in &mut facts.expressions {
                    if expression.node == length_op.node {
                        expression.ty = byte.clone();
                    } else if expression.node == ns.initializer.node {
                        expression.ty = ns.initializer.carrier.clone();
                    }
                }
            }
            let progress = TypingProgress::Complete(TypedTrees::new(resolved, facts));
            let pass = prepare_typing_pass_with_cause(&progress, &[], None, CheckedPreparationEvidence::default());
            if narrow {
                let Err(error) = pass else { panic!("expected refusal at BUF.len") };
                let start = code.find("BUF.len").unwrap();
                spec_corpus::assert_preparation_cause!(
                    error,
                    RefusalCause::Preparation(PreparationReason::UnsupportedBody),
                    "app",
                    "main.omg",
                    (start, start + "BUF.len".len()),
                    "static evaluation cannot admit this invocation: the selected body is outside the supported static evaluation fragment",
                );
                continue;
            }
            let PreparationAnswers::Frozen { constants, .. } = pass.unwrap().answers else { panic!("expected frozen constants") };
            assert_eq!(constants[&answer], PreparedValue::Array([PreparedValue::Integer(length.into())].into()));
        }
    }
}

#[test]
fn owned_tuple_snapshot_preserves_each_component_and_its_type() {
    let source = package("const PAIR=(true,7u8);");
    let prepared = prepare_source_package(&source).unwrap();
    assert_eq!(
        prepared.constant(&prepared.typed().constants[0].declaration),
        Some(&PreparedValue::Tuple([PreparedValue::Bool(true), PreparedValue::Integer(7.into())].into()))
    );
}

#[test]
fn omitting_an_inferred_tuple_field_freezes_each_components_zero() {
    let source = package(
        "data Boxed<T [copy]> [copy] { hint:T; payload:T; } machine boxed<T [copy]>(hint:T)->Boxed<T> {Boxed::<T>{hint:hint,..}} const VALUE=boxed((7u8,true));"
    );
    let prepared = prepare_source_package(&source).unwrap();
    let constant = &prepared.typed().constants[0].declaration;
    let Some(PreparedValue::Record { fields, .. }) = prepared.constant(constant) else { panic!("expected a frozen Boxed record") };
    assert_eq!(fields.as_ref(), &[
        PreparedValue::Tuple([PreparedValue::Integer(7.into()), PreparedValue::Bool(true)].into()),
        PreparedValue::Tuple([PreparedValue::Integer(0.into()), PreparedValue::Bool(false)].into()),
    ]);
}

/// The same evaluation request can have two different issuance operands. Neither
/// the request nor the imported constructor's postconditions are its result.
#[test]
fn issuance_is_an_operand_of_the_library_call_not_a_fact_of_policy_selection() {
    const ACCESS_PLAN: &str = include_str!("fixtures/access_policy/selected.omg");
    let only = ACCESS_PLAN.replace("Unrestricted{}", "Only{..}");
    let empty = ACCESS_PLAN.replace("Unrestricted{}", "Only{routes: []}");
    for (source, case) in [
        (ACCESS_PLAN, "Issuance::Unrestricted"),
        (only.as_str(), "Issuance::Only"),
        (empty.as_str(), "Issuance::Only"),
    ] {
        let package = package(source);
        let prepared = prepare_source_package(&package).unwrap();
        let typed = prepared.typed();
        let [selection] = typed.access_policies.as_slice() else {
            panic!("expected one access selection")
        };
        assert_eq!(std::str::from_utf8(selection.site.span.slice(source.as_bytes()).unwrap()).unwrap(), "access Rule");
        assert_eq!(
            (selection.domain.path.as_str(), selection.policy.path.as_str(), selection.plan_machine.path.as_str()),
            ("i32::Locked", "Rule", "Rule::plan")
        );
        assert!(selection.arguments.is_empty());
        let core = omega_toolchain::core_interface();
        let row = typed
            .conformance_rows
            .iter()
            .find(|row| row.conformance.as_ref() == Some(&selection.policy) && &row.requirement.declaration == core.access_plan_requirement())
            .unwrap();
        assert_eq!(row.implementation, selection.plan_machine);
        let calls: Vec<_> = typed.calls.iter().filter(|call| call.caller.as_ref() == Some(&selection.plan_machine)).collect();
        let [call] = calls.as_slice() else {
            panic!("expected one plan constructor call")
        };
        assert_eq!(&call.machine, core.id("access\\new_plan"));
        let [schema, issuance] = call.operands.as_slice() else {
            panic!("expected schema and issuance arguments")
        };
        assert_eq!(schema.site.file, "main.omg");
        assert_eq!(schema.site.span.slice(source.as_bytes()), Some(&b"@schema"[..]));
        let tree = typed.trees().nth(issuance.node.file).unwrap();
        let construction = tree.construction(issuance.node.node).unwrap();
        assert_eq!(&construction.owner, core.id("Issuance"));
        assert_eq!(construction.case.as_ref(), Some(core.id(case)));
        // Only the explicit spelling writes `routes`. Its `[]` is the declared
        // owned `[CallableInfo<'t>]` field at the constructed issuance's own
        // `'t`, with no elements; it is not a view and not a `[T; 0]` value.
        let written: Vec<_> = construction
            .initializers
            .iter()
            .map(|field| field.expression)
            .collect();
        if source == empty {
            use omega_typed::ValueType::{Applied, Slice};
            let [routes] = written.as_slice() else {
                panic!("expected the written routes field, found {written:?}")
            };
            assert_eq!(tree.resolved().file.span(*routes).slice(source.as_bytes()), Some(&b"[]"[..]));
            assert!(tree.resolved().file.nodes[*routes].children.is_empty());
            let Applied(_, lifetime) = &construction.carrier else {
                panic!("Issuance is applied to its lifetime")
            };
            let published: Vec<_> = tree
                .semantics_at(*routes)
                .filter_map(|record| match record {
                    omega_typed::NodeSemantics::Expression(expression) => Some(expression.ty.clone()),
                    _ => None,
                })
                .collect();
            let element = Applied(Box::new(core.id(r"reflect\CallableInfo").clone()), lifetime.clone());
            assert_eq!(published, [Slice(Box::new(element))]);
        } else {
            assert!(written.is_empty(), "{case} writes no field: {written:?}");
        }

        let library = typed.library.unwrap();
        let signature = library.signatures.iter().find(|s| s.machine == call.machine).unwrap();
        let declaration = library.trees().nth(signature.node.file).unwrap();
        // An ordinary checked core body, not a closed-catalog entry. Its guarantees are its
        // own rows in the library forest; this package imports none of them.
        let body = declaration
            .resolved()
            .file
            .child(signature.node.node, omega_parser::NodeKind::Body);
        let owed = library
            .obligations
            .iter()
            .filter(|o| matches!(&o.kind, omega_typed::ObligationKind::ProveGuarantee { machine, .. } if machine == &call.machine))
            .count();
        spec_corpus::observe_handoff!(
            "access-plan-constructor-supply/consumer",
            assert!(body.is_some() && !core.supplies(&call.machine) && owed == 4),
        );
        assert!(typed.obligations.iter().all(|o| o.site.package == "app"));
    }
}

#[test]
fn pending_access_plans_bind_each_selection_to_its_own_domain_and_carrier() {
    let source = include_str!("fixtures/access_policy/selected.omg").replace(
        "domain i32::Locked access Rule;",
        "domain i32::Locked access Rule; domain u8::Small access Rule; domain i32::Free;",
    );
    let package = package(&source);
    let prepared = prepare_source_package(&package).unwrap();
    let domains = &prepared.typed().domain_carriers;
    let domain = |path: &str| domains.keys().find(|id| id.package == "app" && id.path == path).unwrap();
    let locked = prepared.pending_access_plan(domain("i32::Locked")).unwrap();
    let small = prepared.pending_access_plan(domain("u8::Small")).unwrap();
    assert!(prepared.pending_access_plan(domain("i32::Free")).is_none());
    assert_eq!(locked.selection.plan_machine, small.selection.plan_machine);
    assert_eq!(locked.selection.domain, *domain("i32::Locked"));
    assert_eq!(small.selection.domain, *domain("u8::Small"));
    assert_eq!(locked.carrier, &domains[domain("i32::Locked")]);
    assert_eq!(small.carrier, &domains[domain("u8::Small")]);
    assert_ne!(locked.carrier, small.carrier);
    assert_eq!(locked.selection.site.span.get_start_offset(), source.find("access Rule").unwrap());
    assert_eq!(small.selection.site.span.get_start_offset(), source.rfind("access Rule").unwrap());
}

#[test]
fn policy_schema_argument_describes_the_scalar_and_the_requesting_domain() {
    use omega_preparation::PolicySchemaInput;
    let base = include_str!("fixtures/access_policy/selected.omg");
    let source = base.replace(
        "domain i32::Locked access Rule;",
        "domain i32::Locked access Rule; domain u32::Small access Rule; domain i32::Other access Rule; data R { a: u8; } domain R::Record access Rule;",
    );
    let mut package = package(&source);
    {
        let prepared = prepare_source_package(&package).unwrap();
        for selection in &prepared.typed().access_policies {
            let demand = prepared.pending_access_plan(&selection.domain).unwrap();
            if selection.domain.path == "R::Record" {
                assert_eq!(*demand.schema, PolicySchemaInput::UnsupportedSubject);
            } else {
                assert_eq!(*demand.schema, PolicySchemaInput::NeedsPackageOwner);
            }
        }
    }
    assert!(package.bind_package_key("unrelated", [0x42; 32]).is_err());
    package.bind_package_key("app", [0x42; 32]).unwrap();
    package.bind_package_key("app", [0x42; 32]).unwrap();
    assert!(package.bind_package_key("app", [0x24; 32]).is_err());
    let prepared = prepare_source_package(&package).unwrap();

    fn word(
        image: &[u8],
        slot: usize,
    ) -> u64 {
        u64::from_le_bytes(image[slot * 8..(slot + 1) * 8].try_into().unwrap())
    }
    fn span(
        image: &[u8],
        slot: usize,
    ) -> &[u8] {
        let start = word(image, slot) as usize;
        &image[start..start + word(image, slot + 1) as usize]
    }
    let mut revisions = std::collections::HashSet::new();
    for selection in &prepared.typed().access_policies {
        let demand = prepared.pending_access_plan(&selection.domain).unwrap();
        if selection.domain.path == "R::Record" {
            assert_eq!(*demand.schema, PolicySchemaInput::UnsupportedSubject);
            continue;
        }
        let PolicySchemaInput::Ready {
            argument: PreparedValue::Record { owner, fields },
            subject_revision,
        } = demand.schema
        else {
            panic!("expected the schema argument for {}", selection.domain.path)
        };
        assert_eq!(owner, omega_toolchain::core_interface().id(r"reflect\TypeSchema"));
        let [PreparedValue::SharedBytes(image)] = fields.as_ref() else {
            panic!("TypeSchema.storage is a shared byte view")
        };
        let signed = selection.domain.path != "u32::Small";
        let (atom, app) = if signed {
            ("i32", r"named(name(compiler-type\(atom\(i32\)\)))")
        } else {
            ("u32", r"named(name(compiler-type\(atom\(u32\)\)))")
        };
        assert_eq!(
            subject_revision.as_slice(),
            [b"omega.reflect.primitive.v2\0".as_slice(), app.as_bytes()].concat()
        );
        assert_eq!([word(image, 0), word(image, 1), word(image, 2), word(image, 4)], [2, 2, 1, 1]);
        let expected_revision = match selection.domain.path.as_str() {
            "i32::Locked" => 0x2a42e8767c21f387,
            "i32::Other" => 0xa9fdc503b71d7116,
            "u32::Small" => 0xdecbcd4ffd284297,
            other => panic!("unexpected fixture domain {other}"),
        };
        assert_eq!(word(image, 3), expected_revision);
        assert!(revisions.insert(word(image, 3)), "separate requesters have different revisions");
        let requester = format!("package:{}::declaration(path({}),owner({app}))", "42".repeat(32), selection.domain.path);
        assert_eq!(span(image, 5), requester.as_bytes());
        assert_eq!(span(image, 8), &[0; 34 * 8]);
        let root = span(image, 10);
        assert_eq!(
            [
                word(root, 0),
                word(root, 1),
                word(root, 2),
                word(root, 3),
                word(root, 6),
                word(root, 7)
            ],
            [1, 1, 32, u64::from(signed), 4, 1]
        );
        assert_eq!(span(root, 8), format!("compiler-type(atom({atom}))").as_bytes());
        assert_eq!(span(root, 10), atom.as_bytes());
        assert_eq!(span(root, 16), app.as_bytes());
        assert_eq!(span(root, 20), b"Copy");
        for slot in [18, 22, 24, 26, 30, 32] {
            assert!(span(root, slot).is_empty());
        }
    }
    assert_eq!(revisions.len(), 3);
}

#[test]
fn compiler_owned_schema_roots_keep_address_and_ieee_formats_distinct_from_integers() {
    use omega_preparation::PolicySchemaInput;
    for (carrier, atom, kind, width, signed) in [
        ("bool", "bool", 0, 0, 0),
        ("u13", "u13", 1, 13, 0),
        ("i1", "i1", 1, 1, 1),
        ("addr", "address", 2, 0, 0),
        ("f32", "f32", 3, 0, 0),
        ("f64", "f64", 4, 0, 0),
    ] {
        let source = include_str!("fixtures/access_policy/selected.omg").replace("i32::Locked", &format!("{carrier}::Locked"));
        let mut package = package(&source);
        package.bind_package_key("app", [0x34; 32]).unwrap();
        let prepared = prepare_source_package(&package).unwrap();
        let [selection] = prepared.typed().access_policies.as_slice() else {
            panic!("one policy")
        };
        let PolicySchemaInput::Ready {
            argument: PreparedValue::Record { fields, .. },
            subject_revision,
        } = prepared.pending_access_plan(&selection.domain).unwrap().schema
        else {
            panic!("missing schema for {carrier}")
        };
        let [PreparedValue::SharedBytes(storage)] = &fields[..] else {
            panic!("expected byte view")
        };
        let word = |bytes: &[u8], n: usize| u64::from_le_bytes(bytes[8 * n..8 * n + 8].try_into().unwrap());
        let node = &storage[word(storage, 10) as usize..];
        assert_eq!([word(node, 1), word(node, 2), word(node, 3)], [kind, width, signed]);
        let name = word(node, 10) as usize;
        assert_eq!(&node[name..name + word(node, 11) as usize], carrier.as_bytes());
        assert_eq!(
            subject_revision,
            format!("omega.reflect.primitive.v2\0named(name(compiler-type\\(atom\\({atom}\\)\\)))").as_bytes()
        );
    }
}

#[test]
fn a_computed_policy_argument_is_evaluated_before_selecting_the_access_plan() {
    use omega_typed::ObligationKind;
    use omega_typer::{TypingProgress, type_source_package_for_preparation};

    let base = include_str!("fixtures/access_policy/selected.omg")
        .replace("access Rule;", "access Rule<choose()>;")
        .replace("Rule satisfies", "Rule<const Open:bool> satisfies")
        .replace(
            "Issuance::Unrestricted{}",
            "match Open { true -> Issuance::Unrestricted{}, false -> Issuance::Only{..}, }",
        );
    for open in [false, true] {
        let source = format!("{base}\nmachine choose()->bool {{ {open} }}");
        let package = package(&source);
        let TypingProgress::Pending(initial) = type_source_package_for_preparation(&package, &Default::default()).unwrap() else {
            panic!("the computed argument requires preparation")
        };
        let [demand] = initial.demands() else { panic!("one policy argument demand") };
        assert_eq!(demand.request().expression.span.slice(source.as_bytes()), Some(b"choose()".as_slice()));
        assert!(initial.trees().access_policies.is_empty());

        let prepared = prepare_source_package(&package).unwrap();
        let typed = prepared.typed();
        assert_eq!(typed.evaluated_indices.as_slice(), [(demand.request().clone(), StaticValue::Bool(open))]);
        let [selection] = typed.access_policies.as_slice() else {
            panic!("one resumed policy selection")
        };
        spec_corpus::observe_handoff!("computed-policy-argument/consumer", assert_eq!(
            selection.arguments,
            [ValueType::StaticValue {
                carrier: Box::new(ValueType::Bool),
                value: StaticValue::Bool(open)
            }]
        ));
        let debts: Vec<_> = typed
            .obligations
            .iter()
            .filter_map(|o| match &o.kind {
                ObligationKind::EvaluateAccessPlan { domain, machine } => Some((domain, machine, &o.site)),
                _ => None,
            })
            .collect();
        assert_eq!(debts, [(&selection.domain, &selection.plan_machine, &selection.site)]);
        let plan = prepared.pending_access_plan(&selection.domain).unwrap();
        assert_eq!(plan.schema, &omega_preparation::PolicySchemaInput::NeedsPackageOwner);
    }
    let source = format!("{base}\nboundary trait Sensor {{}} machine choose()->bool reaches Sensor {{ true }}");
    let error = prepare_source_package(&package(&source)).unwrap_err();
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"choose()".as_slice()));
    assert_eq!(error.message, "fact or static argument requires an effect-free computation");
}
