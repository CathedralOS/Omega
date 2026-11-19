use num_bigint::BigInt;
use omega_preparation::{PreparedTrees, prepare_source_package};
use omega_resolver::{ParsedSource, StageSource, Symbol};
use omega_typer::{
    CallUse, NodeSemantics, SourcePackage, StaticValueDemand, TypeError, TypingEnvironment, TypingProgress, ValueType, resolve_source_package,
    type_source_package_for_preparation,
};

const WIDTH: &str = "machine width(n:u8)->u8 reaches none crashes none terminates {n +% 1}";

fn resolve(source: &str) -> SourcePackage<'_> {
    resolve_files(&[("main.omg", source)])
}

fn resolve_files<'text>(files: &[(&'text str, &'text str)]) -> SourcePackage<'text> {
    let syntax: Vec<_> = files
        .iter()
        .map(|(_, source)| {
            let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
            let tree = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
            (tokens, tree)
        })
        .collect();
    let sources: Vec<_> = files
        .iter()
        .zip(&syntax)
        .map(|(&(logical_path, text), (tokens, tree))| StageSource {
            package: "app",
            source: ParsedSource {
                logical_path,
                text,
                tokens,
                tree,
            },
        })
        .collect();
    resolve_source_package(&sources).unwrap()
}

fn prepare(source: &str) -> Result<PreparedTrees<'_>, TypeError> {
    prepare_source_package(&resolve(source))
}

/// Emit just the closed FixedArray shape from the Psi structural type table.
/// The caller supplies fixture type IDs. The only program input is L4's product.
fn lower_array_parameter_shapes(
    prepared: &PreparedTrees<'_>,
    element_types: &[(ValueType, u64)],
) -> Vec<Vec<u8>> {
    let signature = prepared
        .typed()
        .signatures
        .iter()
        .find(|sig| sig.machine.path == "consume")
        .unwrap();
    signature
        .parameters
        .iter()
        .map(|(_, ty)| {
            let ValueType::Array(element, length) = ty else {
                panic!("expected a closed fixed array, got {ty:?}")
            };
            let (_, element_id) = element_types
                .iter()
                .find(|(ty, _)| ty == element.as_ref())
                .expect("fixture element declaration");
            let mut shape = vec![2]; // FixedArray
            shape.extend_from_slice(&element_id.to_le_bytes());
            shape.push(0); // Concrete extent
            shape.extend_from_slice(&length.to_le_bytes());
            shape
        })
        .collect()
}

#[test]
fn lowering_consumes_the_prepared_array_element_and_extent() {
    let source = "machine width(n:u64)->u64 reaches none crashes none terminates {n + 1}
                  const SIZE:u64=width(7);
                  machine consume(direct:[u8; width(7)], named:[u8; SIZE], literal:[u8; 8], wide:[u16; 8]){}";
    let element_types = [
        (ValueType::Integer { signed: false, width: Some(8) }, 1),
        (
            ValueType::Integer {
                signed: false,
                width: Some(16),
            },
            2,
        ),
    ];
    // FixedArray tag, element type id, concrete extent tag, u64 element count.
    let bytes_of_8 = [2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0];
    let bytes_of_9 = [2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 9, 0, 0, 0, 0, 0, 0, 0];
    let words_of_8 = [2, 2, 0, 0, 0, 0, 0, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0];
    for (program, expected) in [
        (source.to_owned(), bytes_of_8),
        (source.replace("n + 1", "n + 2"), bytes_of_9),
    ] {
        let prepared = prepare(&program).unwrap();
        assert_eq!(
            lower_array_parameter_shapes(&prepared, &element_types),
            [expected, expected, bytes_of_8, words_of_8]
        );
    }
    let overflow = source.replace("n + 1", "n - 8");
    let error = prepare(&overflow).unwrap_err();
    assert_eq!(
        error.message,
        "static evaluation cannot admit this invocation: exact subtraction exceeds its selected integer carrier"
    );
    assert_eq!(error.span.slice(overflow.as_bytes()), Some(b"width(7)".as_slice()));
}

#[test]
fn guarded_division_closes_array_returns_and_preserves_identity_errors() {
    let source = "machine divide(n:u64,d:u64)->u64 requires d != 0 reaches none crashes none terminates {n / d}
                  const SIZE:u64=divide(12,3);
                  machine carry(x:[u8; SIZE])->[u8; 4]{x}";
    prepare(source).unwrap();
    prepare(&source.replace("x:[u8; SIZE])->[u8; 4]", "x:[u8; 4])->[u8; SIZE]")).unwrap();
    for (divisor, message, site) in [
        ("2", "expected [u8; 4], found [u8; 6]", "x"),
        (
            "0",
            "static evaluation cannot admit this invocation: the selected integer precondition is false for the supplied arguments",
            "divide(12,0)",
        ),
    ] {
        let source = source.replace("12,3", &format!("12,{divisor}"));
        let error = prepare(&source).unwrap_err();
        assert_eq!(error.message, message);
        assert_eq!(error.span.slice(source.as_bytes()), Some(site.as_bytes()));
    }
}

fn buffer_with_index(
    literal: &ValueType,
    expected: u8,
) -> ValueType {
    let ValueType::Applied(declaration, _) = literal else {
        panic!("expected an applied Buffer, found {literal:?}");
    };
    assert_eq!(declaration.package, "app");
    assert_eq!(declaration.path, "Buffer");
    ValueType::Applied(
        declaration.clone(),
        vec![ValueType::StaticInteger {
            carrier: Box::new(ValueType::Integer { signed: false, width: Some(8) }),
            value: expected.into(),
        }],
    )
}

#[test]
fn direct_and_literal_indices_share_a_closed_identity() {
    let source = format!(
        "data Buffer<const N:u8>{{}}
         {WIDTH}
         machine literal(x:Buffer<8>){{}}
         machine computed(x:Buffer<width(7)>){{}}"
    );
    for (source, expected) in [(source.clone(), 8), (source.replace("+% 1", "+% 2"), 9)] {
        let prepared = prepare(&source).unwrap();
        let typed = prepared.typed();
        let parameter = |name: &str| {
            &typed
                .signatures
                .iter()
                .find(|signature| signature.machine.path == name)
                .unwrap()
                .parameters[0]
                .1
        };
        let literal = parameter("literal");
        assert_eq!(literal, &buffer_with_index(literal, 8));
        assert_eq!(parameter("computed"), &buffer_with_index(literal, expected));
        let receipts = &typed.evaluated_indices;
        let [(request, value)] = receipts.as_slice() else {
            panic!("expected one direct evaluation, found {receipts:?}");
        };
        assert_eq!(request.expression.span.slice(source.as_bytes()), Some(b"width(7)".as_slice()));
        assert_eq!(request.carrier, ValueType::Integer { signed: false, width: Some(8) });
        assert_eq!(value, &omega_preparation::PreparedValue::Integer(BigInt::from(expected)));
    }
}

#[test]
fn specialized_and_literal_indices_share_a_closed_identity() {
    let source = format!(
        "data Buffer<const N:u8>{{}}
         {WIDTH}
         machine literal(x:Buffer<8>){{}}
         machine make<const N:u8>()->Buffer<width(N)>{{Buffer::<width(N)>{{}}}}
         machine use_it(){{_ = make::<7>();}}"
    );
    for (source, expected) in [(source.clone(), 8), (source.replace("+% 1", "+% 2"), 9)] {
        let prepared = prepare(&source).unwrap();
        let typed = prepared.typed();
        let literal = &typed
            .signatures
            .iter()
            .find(|signature| signature.machine.path == "literal")
            .unwrap()
            .parameters[0]
            .1;
        let result = &typed
            .calls
            .iter()
            .find(|call| call.machine.path == "make")
            .unwrap()
            .result;
        assert_eq!(literal, &buffer_with_index(literal, 8));
        assert_eq!(result, &buffer_with_index(literal, expected));
    }
}

#[test]
fn a_changed_computation_refuses_a_literal_body_with_the_old_index() {
    let source = format!(
        "data Buffer<const N:u8>{{}}
         {WIDTH}
         machine make()->Buffer<width(7)>{{Buffer::<8>{{}}}}"
    );
    prepare(&source).unwrap();
    let changed = source.replace("+% 1", "+% 2");
    let error = prepare(&changed).expect_err("the body constructs the old Buffer specialization");
    let diagnostic = &error.message;
    assert!(diagnostic.contains("Buffer<9>") && diagnostic.contains("Buffer<8>"), "{diagnostic}");
}

#[test]
fn a_failed_precondition_cannot_supply_a_closed_index() {
    let source = "
        data Buffer<const N:u8>{}
        machine width(n:u8)->u8 requires n < 8 reaches none crashes none terminates {n +% 1}
        machine computed(x:Buffer<width(8)>){}
    ";
    let error = prepare(source).expect_err("width(8) violates the source precondition");
    let diagnostic = &error.message;
    assert_eq!(
        diagnostic,
        "static evaluation cannot admit this invocation: the selected integer precondition is false for the supplied arguments"
    );
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"width(8)".as_slice()));
}

#[test]
fn a_representable_result_of_the_wrong_carrier_cannot_supply_an_index() {
    let source = "
        data Buffer<const N:u8>{}
        machine wide(n:u16)->u16 reaches none crashes none terminates {n +% 1}
        machine computed(x:Buffer<wide(7)>){}
    ";
    let error = prepare(source).expect_err("a u16 result must not become a u8 index");
    let diagnostic = &error.message;
    assert_eq!(diagnostic, "expected u8, found u16");
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"wide(7)".as_slice()));
}

#[test]
fn declared_service_reach_cannot_supply_a_closed_index() {
    let source = "
        data Buffer<const N:u8>{}
        boundary trait Sensor {}
        machine width(n:u8)->u8 reaches Sensor crashes none terminates {n +% 1}
        machine computed(x:Buffer<width(7)>){}
    ";
    let error = prepare(source).unwrap_err();
    assert_eq!(error.message, "fact or static argument requires an effect-free computation");
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"width(7)".as_slice()));
}

#[test]
fn preparation_executes_only_the_overload_selected_by_typing() {
    let source = format!(
        "data Buffer<const N:u8>{{}}
         {WIDTH}
         machine width(n:u16)->u16 reaches none crashes none terminates {{n +% 13}}
         machine computed(x:Buffer<width(7u8)>){{}}"
    );
    for (source, expected) in [
        (source.clone(), 8),
        (source.replace("+% 13", "+% 19"), 8),
        (source.replace("+% 1}", "+% 2}"), 9),
    ] {
        let package = resolve(&source);
        let width = |operand: &str| {
            package
                .declarations
                .iter()
                .find(|declaration| declaration.path == "width" && declaration.operands == [operand])
                .unwrap()
        };
        let byte_width = width("u8");
        let wide_width = width("u16");
        let occurrence = package
            .uses()
            .find(|name| name.span.slice(source.as_bytes()) == Some(b"width".as_slice()) && matches!(&name.target, Symbol::Candidates(_)))
            .expect("L2 retains the visible overload set at the call head");
        let Symbol::Candidates(candidates) = &occurrence.target else { unreachable!() };
        assert_eq!(candidates.len(), 2);
        assert!(candidates.contains(byte_width));
        assert!(candidates.contains(wide_width));

        let progress = type_source_package_for_preparation(&package, &TypingEnvironment::default()).unwrap();
        let TypingProgress::Pending(pending) = progress else {
            panic!("the selected call must be prepared")
        };
        let [StaticValueDemand::Closed(request)] = pending.demands() else {
            panic!("expected one closed demand, got {:?}", pending.demands());
        };
        assert_eq!(request.expression.span.slice(source.as_bytes()), Some(b"width(7u8)".as_slice()));
        let request_tree = pending.trees().trees().nth(request.node.file).unwrap();
        let selected: Vec<_> = request_tree
            .semantics_at(request.node.node)
            .filter_map(|record| match record {
                NodeSemantics::Call(call) => Some(call),
                _ => None,
            })
            .collect();
        let [selected] = selected.as_slice() else {
            panic!("expected one typed call at the demand")
        };
        assert_eq!(&selected.machine, byte_width);
        assert_eq!(selected.use_site, CallUse::StaticArgument);
        assert_eq!(selected.result, request.carrier);

        let prepared = prepare_source_package(&package).unwrap();
        let result = &prepared
            .typed()
            .signatures
            .iter()
            .find(|signature| signature.machine.path == "computed")
            .unwrap()
            .parameters[0]
            .1;
        assert_eq!(result, &buffer_with_index(result, expected));
    }
}

#[test]
fn public_machine_needs_a_declared_termination_promise() {
    let source = format!("data Buffer<const N:u8>{{}} public {WIDTH} machine computed(x:Buffer<width(7)>){{}}");
    let prepared = prepare(&source).unwrap();
    let result = &prepared
        .typed()
        .signatures
        .iter()
        .find(|signature| signature.machine.path == "computed")
        .unwrap()
        .parameters[0]
        .1;
    assert_eq!(result, &buffer_with_index(result, 8));

    let no_promise = source.replace(" terminates", "");
    let error = prepare(&no_promise).unwrap_err();
    assert_eq!(
        error.message,
        "static evaluation cannot admit this invocation: the selected public machine does not declare termination"
    );
    assert_eq!(error.span.slice(no_promise.as_bytes()), Some(b"width(7)".as_slice()));
}

#[test]
fn resumed_typing_retains_build_selected_boundary_directions() {
    use omega_preparation::prepare_source_package_with_boundary_slots;
    use omega_typer::{BoundarySlot, InvocationTarget, RequirementSlot, SlotDirection};
    let source = "trait Host{machine enter();} machine direct(){Host::enter()}
                  machine width(n:u64)->u64{n+1}
                  machine carry(x:[u8; width(3)])->[u8;4]{x}";
    let package = resolve(source);
    let selected = package
        .declarations
        .iter()
        .find(|id| id.path == "Host::enter")
        .unwrap()
        .clone();
    for direction in [
        SlotDirection::EnvironmentToProgram,
        SlotDirection::ProgramToProvider,
    ] {
        let slots = [BoundarySlot {
            requirement: RequirementSlot {
                declaration: selected.clone(),
                trait_arguments: vec![],
            },
            arguments: vec![],
            direction,
        }];
        let prepared = prepare_source_package_with_boundary_slots(&package, &slots).unwrap();
        let stage = prepared.typed();
        let [(site, slot)] = stage.boundary_calls.as_slice() else {
            panic!("expected the applied boundary call")
        };
        assert_eq!(slot, &slots[0]);
        assert_eq!(site.span.slice(source.as_bytes()), Some(b"Host::enter()".as_slice()));
        let effects = stage
            .machine_effects
            .iter()
            .find(|effects| effects.machine.path == "direct")
            .unwrap();
        assert_eq!(
            effects
                .invokes
                .contains(&InvocationTarget::Declaration(slot.requirement.clone())),
            direction == SlotDirection::EnvironmentToProgram
        );
        let carry = stage
            .signatures
            .iter()
            .find(|s| s.machine.path == "carry")
            .unwrap();
        assert_eq!(
            carry.parameters[0].1,
            ValueType::Array(Box::new(ValueType::Integer { signed: false, width: Some(8) }), 4)
        );
    }
}

#[test]
fn preparation_refuses_an_applied_runtime_boundary_at_its_use() {
    use omega_preparation::prepare_source_package_with_boundary_slots;
    use omega_typer::{BoundarySlot, RequirementSlot, SlotDirection};
    let source = "trait Host{machine width(n:u64)->u64;} machine carry(x:[u8; Host::width(4)])->[u8;4]{x}";
    let package = resolve(source);
    let selected = package
        .declarations
        .iter()
        .find(|id| id.path == "Host::width")
        .unwrap()
        .clone();
    for direction in [
        SlotDirection::EnvironmentToProgram,
        SlotDirection::ProgramToProvider,
    ] {
        let slots = [BoundarySlot {
            requirement: RequirementSlot {
                declaration: selected.clone(),
                trait_arguments: vec![],
            },
            arguments: vec![],
            direction,
        }];
        let error = prepare_source_package_with_boundary_slots(&package, &slots).unwrap_err();
        assert_eq!(
            error.message,
            "static evaluation cannot admit this invocation: the selected runtime boundary slot cannot execute during preparation"
        );
        assert_eq!(error.span.slice(source.as_bytes()), Some(b"Host::width(4)".as_slice()));
    }
}

#[test]
fn boolean_calls_names_and_literals_have_one_closed_type_identity() {
    use omega_preparation::PreparedValue;

    let source = "machine enabled()->bool{true}
                  const ENABLED:bool=enabled();
                  data Flag<const B:bool>{}
                  machine sink(x:Flag<true>){}
                  machine carry(x:Flag<enabled()>)->Flag<ENABLED>{sink(x); x}";
    let prepared = prepare(source).unwrap();
    let typed = prepared.typed();
    let signature = |name| {
        typed
            .signatures
            .iter()
            .find(|s| s.machine.path == name)
            .unwrap()
    };
    let computed = &signature("carry").parameters[0].1;
    assert_eq!(computed, &signature("sink").parameters[0].1);
    assert_eq!(computed, &signature("carry").result);
    assert_eq!(prepared.constant(&typed.constants[0].declaration), Some(&PreparedValue::Bool(true)));
    assert!(
        typed
            .evaluated_indices
            .iter()
            .any(|(request, value)| request.expression.span.slice(source.as_bytes()) == Some(b"enabled()".as_slice()) && value == &PreparedValue::Bool(true))
    );
    let incompatible = source.replace("->bool{true}", "->bool{false}");
    let error = prepare(&incompatible).unwrap_err();
    assert_eq!(error.file, "main.omg");
    assert!(error.message.contains("sink"), "{}", error.message);
    assert_eq!(error.span.slice(incompatible.as_bytes()), Some(b"sink".as_slice()));
}

#[test]
fn a_computed_boolean_operand_selects_the_exact_overload_in_both_file_orders() {
    const TRUE: &str = "data Flag<const B:bool>{}
        machine enabled()->bool{true}
        machine sink(x:Flag<true>)->u8{1}
        machine sink(x:Flag<false>)->u8{2}";
    const USE: &str = "machine carry(x:Flag<enabled()>)->u8{sink(x)}";
    for enabled in [true, false] {
        let producers = TRUE.replace("bool{true}", &format!("bool{{{enabled}}}"));
        for files in [
            [("producers.omg", producers.as_str()), ("use.omg", USE)],
            [("use.omg", USE), ("producers.omg", producers.as_str())],
        ] {
            let package = resolve_files(&files);
            let TypingProgress::Pending(pending) = type_source_package_for_preparation(&package, &TypingEnvironment::default()).unwrap() else {
                panic!("typing needs the computed Boolean");
            };
            let [StaticValueDemand::Closed(request)] = pending.demands() else {
                panic!("expected just the operand index");
            };
            assert_eq!(request.expression.file, "use.omg");
            assert_eq!(request.expression.span.slice(USE.as_bytes()), Some(b"enabled()".as_slice()));
            assert!(
                pending
                    .trees()
                    .calls
                    .iter()
                    .all(|call| call.machine.path != "sink")
            );

            let prepared = prepare_source_package(&package).unwrap();
            let typed = prepared.typed();
            let chosen = typed
                .calls
                .iter()
                .find(|call| call.machine.path == "sink")
                .unwrap();
            let expected = typed
                .signatures
                .iter()
                .find(|signature| {
                    signature.machine.path == "sink"
                        && signature
                            .site
                            .span
                            .slice(producers.as_bytes())
                            .is_some_and(|source| String::from_utf8_lossy(source).contains(&format!("Flag<{enabled}>")))
                })
                .unwrap();
            assert_eq!(chosen.machine, expected.machine);
        }
    }
}

#[test]
fn a_lower_level_demand_finishes_an_overload_in_a_static_producer() {
    let program = "data Flag<const B:bool> [copy] {}
        machine bit()->bool{true}
        machine pick(x:Flag<true>)->bool{true}
        machine pick(x:Flag<false>)->bool{false}
        machine enabled()->bool{pick(Flag::<bit()>{})}
        machine relay()->bool{enabled()}
        machine carry(x:Flag<relay()>)->Flag<true>{x}";
    let source = resolve(program);
    let TypingProgress::Pending(pending) = type_source_package_for_preparation(&source, &TypingEnvironment::default()).unwrap() else {
        panic!("the producer needs its operand index");
    };
    let [StaticValueDemand::Closed(request)] = pending.demands() else {
        panic!("only the inner request is ready");
    };
    assert_eq!(request.expression.span.slice(program.as_bytes()), Some(b"bit()".as_slice()));
    prepare_source_package(&source).unwrap();
}

#[test]
fn unresolved_overload_still_reports_a_failed_static_helper_at_its_call() {
    let source = "data Flag<const B:bool>{}
        machine guard(b:bool)->bool requires b {b}
        machine enabled()->bool{guard(false)}
        machine sink(x:Flag<true>){}
        machine sink(x:Flag<false>){}
        machine carry(x:Flag<enabled()>){sink(x);}";
    let error = prepare(source).unwrap_err();
    assert_eq!(
        error.message,
        "static evaluation cannot admit this invocation: the selected Boolean precondition is false for the supplied arguments\ncall chain: enabled -> guard"
    );
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"guard(false)".as_slice()));
}

#[test]
fn incompatible_known_operands_rule_out_candidates_before_preparation() {
    let source = "data Flag<const B:bool>{}
        machine enabled()->bool requires false {true}
        machine sink(x:Flag<true>, y:u8){}
        machine sink(x:Flag<false>, y:u16){}
        machine carry(x:Flag<enabled()>){sink(x,false);}";
    let error = prepare(source).unwrap_err();
    assert!(error.message.starts_with("no matching overload for sink("), "{}", error.message);
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"sink".as_slice()));
}

#[test]
fn preparation_cannot_execute_an_overload_while_a_competitor_could_match() {
    let source = "data Flag<const B:bool> [copy] {}
        machine enabled()->bool{false}
        machine choose(x:Flag<false>)->bool requires false {false}
        machine choose(x:Flag<enabled()>)->bool {true}
        machine dependent()->bool{choose(Flag::<false>{})}
        machine carry(x:Flag<dependent()>){ }";
    let error = prepare(source).unwrap_err();
    assert!(error.message.contains("duplicate") && error.message.contains("choose"), "{}", error.message);
    let distinct = source.replace("enabled()->bool{false}", "enabled()->bool{true}");
    let error = prepare(&distinct).unwrap_err();
    assert!(error.message.contains("precondition is false"), "{}", error.message);
    assert_eq!(error.span.slice(distinct.as_bytes()), Some(b"choose(Flag::<false>{})".as_slice()));
}

#[test]
fn an_inferred_constant_and_its_alias_wait_for_the_selected_overload() {
    let source = "data Flag<const B:bool> [copy] {}
        machine enabled()->bool{true}
        machine value(x:Flag<true>)->bool{true}
        machine value(x:Flag<false>)->bool{false}
        const ALIAS=SELECTED;
        const SELECTED=value(Flag::<enabled()>{});
        machine carry(x:Flag<ALIAS>)->Flag<true>{x}";
    let resolved = resolve(source);
    let TypingProgress::Pending(pending) = type_source_package_for_preparation(&resolved, &TypingEnvironment::default()).unwrap() else {
        panic!("the inferred initializer needs its operand index");
    };
    let [StaticValueDemand::Closed(request)] = pending.demands() else {
        panic!("the initializer must wait for enabled()");
    };
    assert_eq!(request.expression.span.slice(source.as_bytes()), Some(b"enabled()".as_slice()));
    let prepared = prepare_source_package(&resolved).unwrap();
    for constant in &prepared.typed().constants {
        assert_eq!(prepared.constant(&constant.declaration), Some(&omega_preparation::PreparedValue::Bool(true)));
    }
}

#[test]
fn admission_checks_a_static_producers_transitive_skipped_operand() {
    let source = "data Flag<const B:bool> [copy] {}
        boundary trait Sensor {}
        machine enabled()->bool{true}
        machine read(x:Flag<true>)->bool reaches Sensor {true}
        machine read(x:Flag<false>)->bool {false}
        machine fetch()->bool{true || read(Flag::<enabled()>{})}
        machine relay()->bool{fetch()}
        machine carry(x:Flag<relay()>)->Flag<true>{x}";
    let error = prepare(source).unwrap_err();
    assert_eq!(error.message, "fact or static argument requires an effect-free computation");
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"relay()".as_slice()));
    let pure = source.replace("enabled()->bool{true}", "enabled()->bool{false}");
    prepare(&pure).unwrap();
}

#[test]
fn a_computed_result_domain_selects_the_exact_overload() {
    let source = "domain<const K:bool> u8::Label<K>;
        machine enabled()->bool{false}
        machine pick(x:u8)->u8 in Label<true>{x as +Label<true>}
        machine pick(x:u8)->u8 in Label<false>{x as +Label<false>}
        machine carry(x:u8)->u8 in Label<enabled()>{pick(x)}";
    let prepared = prepare(source).unwrap();
    let typed = prepared.typed();
    let chosen = typed
        .calls
        .iter()
        .find(|call| call.machine.path == "pick")
        .unwrap();
    let expected = typed
        .signatures
        .iter()
        .find(|signature| {
            signature.machine.path == "pick"
                && signature
                    .site
                    .span
                    .slice(source.as_bytes())
                    .is_some_and(|text| {
                        text.windows(b"Label<false>".len())
                            .any(|part| part == b"Label<false>")
                    })
        })
        .unwrap();
    assert_eq!(chosen.machine, expected.machine);
}

#[test]
fn a_static_producer_cannot_supply_an_index_needed_to_type_its_own_body() {
    let source = "data Flag<const B:bool> [copy] {}
        machine enabled()->bool{choose(Flag::<enabled()>{})}
        machine choose(x:Flag<true>)->bool{true}
        machine choose(x:Flag<false>)->bool{false}";
    let error = prepare(source).unwrap_err();
    assert_eq!(error.message, "static evaluation is blocked by a typing dependency cycle");
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"enabled()".as_slice()));
}

#[test]
fn an_unrelated_nested_declaration_does_not_block_its_container() {
    const PREFIX: &str = "data Flag<const B:bool> [copy]{}
        machine inspect(x:Outer)->bool{true}
        machine enabled()->bool{inspect(Outer{})}";
    for declarations in [
        "data Outer [copy]{data Nested {item:Flag<enabled()>;}}",
        "data Outer [copy]{} data Nested {item:Flag<enabled()>;}",
    ] {
        let source = format!("{PREFIX}\n{declarations}");
        let prepared = prepare(&source).unwrap();
        let result = prepared
            .typed()
            .evaluated_indices
            .iter()
            .find(|(request, _)| request.expression.span.slice(source.as_bytes()) == Some(b"enabled()".as_slice()))
            .unwrap();
        assert_eq!(result.1, omega_preparation::PreparedValue::Bool(true));
    }
}

#[test]
fn a_pending_callback_body_does_not_supply_a_provisional_pause_summary() {
    let source = "data Flag<const B:bool> [copy]{}
        machine enabled()->bool{true}
        machine take(x:Flag<true>){}
        machine apply<const F:machine()->bool suspends>()->bool suspends {
            take(Flag::<enabled()>{});
            suspend F()
        }
        machine local()->bool{true}
        machine verified() requires apply::<local>() {}";
    prepare(source).unwrap();
    prepare(&source.replace("Flag::<enabled()>", "Flag::<true>")).unwrap();
}

#[test]
fn a_short_circuited_obligation_still_needs_a_checked_body() {
    let source = "data Flag<const B:bool> [copy] {}
        machine accepts(x:Flag<enabled()>)->bool{true}
        machine enabled()->bool{true || accepts(Flag::<true>{})}";
    let error = prepare(source).unwrap_err();
    assert_eq!(error.message, "static evaluation is blocked by a typing dependency cycle");
    let acyclic = format!(
        "machine prerequisite()->bool{{true}}\n{}",
        source.replace("x:Flag<enabled()>", "x:Flag<prerequisite()>")
    );
    prepare(&acyclic).unwrap();
}

#[test]
fn an_inline_conformance_waits_for_its_implementations_effects() {
    let source = "data Flag<const B:bool>[copy]{}
        machine enabled()->bool{true}
        machine take(x:Flag<true>){}
        trait Log{machine put() blocks;}
        trait Local=Log{machine * blocks false reaches none terminates;}
        Quiet satisfies Log{machine put(){take(Flag::<enabled()>{});}}
        machine use_it<const E:Local>(){E::put()}
        machine f(){use_it::<Quiet>()}";
    prepare(source).unwrap();
    prepare(&source.replace("Flag::<enabled()>", "Flag::<true>")).unwrap();
    let wrong = source.replace("enabled()->bool{true}", "enabled()->bool{false}");
    let error = prepare(&wrong).unwrap_err();
    assert!(error.message.starts_with("no matching overload for take("), "{}", error.message);
    assert_eq!(error.span.slice(wrong.as_bytes()), Some(b"take".as_slice()));
}

#[test]
fn an_inactive_sum_case_waits_for_its_inline_fields_cleanup() {
    let source = "data Flag<const B:bool>[copy]{}
        data Outer [copy]{child:data Nested [copy] dropped by dispose {}; }
        data Sum [copy]{case Empty; case Other(v:Outer);}
        machine seed()->bool{true}
        machine take(x:Flag<true>){}
        machine dispose(x:Outer::Nested){take(Flag::<seed()>{});}
        machine read(x:Sum)->bool{true}
        machine answer()->bool{read(Sum::Empty)}
        machine observe(x:Flag<answer()>){ }";
    for program in [
        source.to_owned(),
        source.replace("Flag::<seed()>", "Flag::<true>"),
    ] {
        let error = prepare(&program).unwrap_err();
        assert!(
            error
                .message
                .contains("the complete constant type has a cleanup obligation"),
            "{}",
            error.message
        );
        assert_eq!(error.span.slice(program.as_bytes()), Some(b"read(Sum::Empty)".as_slice()));
    }
    prepare(&source.replace(" dropped by dispose", "")).unwrap();
}

#[test]
fn a_bare_sum_case_waits_for_its_owners_cleanup() {
    let source = "data Flag<const B:bool>[copy]{}
        data Sum [copy] dropped by dispose {case Empty;}
        machine seed()->bool{true}
        machine take(x:Flag<true>){}
        machine dispose(x:Sum){take(Flag::<seed()>{});}
        machine answer()->bool{let x=Sum::Empty; true}
        machine witness(x:Flag<answer()>){ }";
    for program in [
        source.to_owned(),
        source.replace("Flag::<seed()>", "Flag::<true>"),
    ] {
        let error = prepare(&program).unwrap_err();
        assert!(
            error
                .message
                .contains("the complete constant type has a cleanup obligation"),
            "{}",
            error.message
        );
        assert_eq!(error.span.slice(program.as_bytes()), Some(b"answer()".as_slice()));
    }
    prepare(&source.replace(" dropped by dispose", "")).unwrap();
}

#[test]
fn a_record_index_compares_values_in_declared_field_order() {
    let source = "data Key [copy]{count:u8; active:bool;}
                  const NAMED:Key=Key{active:true,count:7};
                  data Row<const K:Key>{}
                  machine carry(x:Row<NAMED>)->Row<Key{count:7,active:true}>{x}";
    let prepared = prepare(source).unwrap();
    let carry = prepared
        .typed()
        .signatures
        .iter()
        .find(|s| s.machine.path == "carry")
        .unwrap();
    assert_eq!(carry.parameters[0].1, carry.result);
    let incompatible = source.replace("count:7,active:true", "count:7,active:false");
    let error = prepare(&incompatible).unwrap_err();
    assert_eq!(error.span.slice(incompatible.as_bytes()), Some(b"x".as_slice()));
    assert!(error.message.contains("false") && error.message.contains("true"), "{}", error.message);
}
