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
        assert_eq!(selected.value, StaticValue::Bool(!arg));
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
    assert_eq!(input.layout.plan.as_ref(), &[2]);
    let other = target_with_layout(ValueType::Bool, 1, 2);
    assert!(matches!(prepared.constant_for_materialization(decl, &other), Err(ConstantMaterializationError::MissingLayout(def)) if &def.declaration == decl));
}

#[test]
fn materializing_a_folded_size_requires_the_original_layout_selection() {
    let source = package("use omega::language::core; const SIZE:u64=core::size_of::<u64>();");
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
    assert_eq!(fields[0].float(), Some(PreparedFloat::Binary32(0x3fc0_0000)));
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
    assert_eq!(prepared.integer_constant(&prepared.typed().constants[0].declaration), Some(&0.into()));
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
        "use omega::language::core; const SIZE:u64=core::size_of::<u64>(); const COPY:u64=SIZE;
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
        "use omega::language::core;
        machine value<const N:u64>()->u64{N}
        machine bytes<T>()->u64{core::size_of::<T>()}
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
    let refused = format!("{setup} const N:u8=pick(ITEMS,2u64);");
    let error = prepare_source_package(&package(&refused)).unwrap_err();
    assert!(error.message.contains("index < items.len"), "{}", error.message);
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
