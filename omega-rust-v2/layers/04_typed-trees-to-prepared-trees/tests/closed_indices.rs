use num_bigint::BigInt;
use omega_preparation::{PreparedTrees, prepare_source_package};
use omega_resolver::{ParsedSource, StageSource, Symbol};
use omega_typer::{
    CallUse, NodeSemantics, SourcePackage, StaticIntegerDemand, TypeError, TypingEnvironment, TypingProgress, ValueType, resolve_source_package,
    type_source_package_for_preparation,
};

const WIDTH: &str = "machine width(n:u8)->u8 reaches none crashes none terminates {n +% 1}";

fn resolve(source: &str) -> SourcePackage<'_> {
    let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
    resolve_source_package(&[StageSource {
        package: "app",
        source: ParsedSource {
            logical_path: "main.omg",
            text: source,
            tokens: &tokens,
            tree: &tree,
        },
    }])
    .unwrap()
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
        assert_eq!(value, &BigInt::from(expected));
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
        let [StaticIntegerDemand::Closed(request)] = pending.demands() else {
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
