use super::{FOREIGN_OPAQUE_BUILD, FOREIGN_OPAQUE_SOURCE, POLICY, write_program, write_project};
use compiler::{CheckedCompileRequest, compile_to_checked};
use provider_planning::calling_policy_plans::BoundaryValueClass;
use std::fs;

#[test]
fn rejected_calling_relationship_is_a_compile_diagnostic() {
    let source = POLICY.replace("machine tick();", "machine tick() -> i64;");
    let main_path = write_program("relationship-rejected", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect_err("a rejected Calling<C> relationship must fail compilation");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        rendered.contains("calling policy rejected the boundary"),
        "unexpected diagnostics:\n{rendered}"
    );
    assert!(
        rendered.contains("return values are not supported"),
        "unexpected diagnostics:\n{rendered}"
    );
    let rejection = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("calling policy rejected the boundary")
        })
        .expect("policy rejection diagnostic");
    let span = rejection
        .source_span
        .expect("policy rejection should retain the Calling<C> source span")
        .span;
    assert_eq!(&source[span.start..span.end], "Calling");
}

#[test]
fn source_policy_receives_erased_stripped_nested_record_parameters_and_results() {
    let source = r#"
use omega::language::std::calling;

data Evidence { case Only; }
data Inner {
    byte: u8;
    proof [erased]: Evidence;
}
data Certified {
    head: u16;
    witness [erased]: Evidence;
    inner: Inner;
    tail: u32;
}

data ErasedRecordPolicy { }
ErasedRecordPolicyCallingPolicy: ErasedRecordPolicy satisfies CallingPolicy;

machine ErasedRecordPolicy::plan(
    signature: BoundarySignature
) -> BoundaryPlanResult
    satisfies CallingPolicy::plan
{
    transition signature.parameter_count == 1 && signature.has_result {
        true -> outer(signature, signature.parameters[0], signature.result)
        _ -> wrong()
    }

    state outer(signature: BoundarySignature, root: u64, result: u64) -> BoundaryPlanResult {
        transition signature.shapes[root].class {
            ValueClass::Record { first_field, field_count } -> outer_fields(signature, result, root, first_field, field_count)
            _ -> wrong()
        }
    }

    state outer_fields(
        signature: BoundarySignature,
        result: u64,
        root: u64,
        first: u64,
        count: u64
    ) -> BoundaryPlanResult {
        transition count == 3
            && signature.shapes[root].byte_size == 8
            && signature.shapes[root].alignment == 4
            && signature.fields[first].byte_offset == 0
            && signature.fields[first + 1].byte_offset == 2
            && signature.fields[first + 2].byte_offset == 4 {
            true -> inner(signature, result, signature.fields[first + 1].shape)
            _ -> wrong()
        }
    }

    state inner(signature: BoundarySignature, result: u64, root: u64) -> BoundaryPlanResult {
        transition signature.shapes[root].class {
            ValueClass::Record { first_field, field_count } -> inner_fields(signature, result, root, first_field, field_count)
            _ -> wrong()
        }
    }

    state inner_fields(
        signature: BoundarySignature,
        result: u64,
        root: u64,
        first: u64,
        count: u64
    ) -> BoundaryPlanResult {
        transition count == 1
            && signature.shapes[root].byte_size == 1
            && signature.shapes[root].alignment == 1
            && signature.fields[first].byte_offset == 0 {
            true -> result_record(signature, result)
            _ -> wrong()
        }
    }

    state result_record(signature: BoundarySignature, root: u64) -> BoundaryPlanResult {
        transition signature.shapes[root].class {
            ValueClass::Record { first_field, field_count } -> result_fields(signature, root, first_field, field_count)
            _ -> wrong()
        }
    }

    state result_fields(
        signature: BoundarySignature,
        root: u64,
        first: u64,
        count: u64
    ) -> BoundaryPlanResult {
        transition count == 3
            && signature.shapes[root].byte_size == 8
            && signature.shapes[root].alignment == 4
            && signature.fields[first].byte_offset == 0
            && signature.fields[first + 1].byte_offset == 2
            && signature.fields[first + 2].byte_offset == 4 {
            true -> observed()
            _ -> wrong()
        }
    }

    state observed() -> BoundaryPlanResult {
        BoundaryPlanResult::Rejected {
            reason: CallingPolicyRejection {
                reason: "erased-stripped nested records observed",
            },
        }
    }

    state wrong() -> BoundaryPlanResult {
        BoundaryPlanResult::Rejected {
            reason: CallingPolicyRejection {
                reason: "erased-stripped record mismatch",
            },
        }
    }
}

boundary trait Probe: Calling<ErasedRecordPolicy> {
    machine inspect(value: Certified) -> Certified;
}

data Main { }
machine Main::main(&mut self) { }
"#;
    let main_path = write_program("erased-boundary-record", source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect_err("the observing policy deliberately rejects after checking its input graph");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        rendered.contains("erased-stripped nested records observed"),
        "policy did not observe the erased-stripped recursive graph:\n{rendered}"
    );
    assert!(
        !rendered.contains("erased-stripped record mismatch"),
        "{rendered}"
    );
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary policy directory"));
}

#[test]
fn erased_case_data_remains_rejected_as_an_unclassified_sum_shape() {
    let source = POLICY.replace(
        "boundary trait Tick: Calling<NoResultPolicy> {\n    machine tick();\n}",
        "data Evidence { case Only; }\ndata Choice { case None; case Some(value: i32, proof [erased]: Evidence); }\n\nboundary trait Tick: Calling<NoResultPolicy> {\n    machine tick(value: Choice);\n}",
    );
    let main_path = write_program("erased-boundary-sum", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect_err("case-bearing data has no public calling-policy graph shape yet");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        rendered.contains("case data `Choice` is not yet classifiable as a boundary value"),
        "unexpected diagnostics:\n{rendered}"
    );
    assert!(
        !rendered.contains("erased-stripped ABI classification is not implemented yet"),
        "the retired blanket relevance fence should not mask the sum-shape diagnostic:\n{rendered}"
    );
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary policy directory"));
}

#[test]
fn policy_source_identity_is_absent_from_the_published_fingerprint() {
    let fingerprint = |name: &str| {
        let source = POLICY.replace("NoResultPolicy", name);
        let main_path = write_program(name, &source);
        let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
            .expect("policy program should compile");
        let tick = checked
            .typed
            .traits()
            .iter()
            .find(|definition| definition.name.as_str() == "Tick")
            .expect("Tick boundary trait");
        provider_planning::service_schema::from_typed(&checked.typed, tick)
            .expect("Tick service schema")
            .methods[0]
            .calling_plan_report_fingerprint
            .expect("evaluated calling identity")
    };

    assert_eq!(fingerprint("FirstPolicy"), fingerprint("RenamedPolicy"));
}

#[test]
fn calling_policy_evaluates_distinct_same_named_requirement_overloads() {
    let source = r#"
use omega::language::std::calling;

data OneParameterPolicy { }
OneParameterPolicyCallingPolicy: OneParameterPolicy satisfies CallingPolicy;

machine OneParameterPolicy::plan(signature: BoundarySignature) -> BoundaryPlanResult
    satisfies CallingPolicy::plan
{
    let mut output: BoundaryEntryPlan;
    output.call.convention = CallingConvention::MicrosoftX64;
    output.call.parameter_count = 1;
    output.call.parameters[0].shape.class = AbiValueClass::Integer;
    output.call.parameters[0].shape.byte_size = signature.shapes[0].byte_size;
    output.call.parameters[0].shape.alignment = signature.shapes[0].alignment;
    output.call.parameters[0].location_count = 1;
    output.call.parameters[0].locations[0] = ValueLocation::Register {
        register: MachineRegister::X86Rcx,
        value_byte_offset: 0,
        byte_size: signature.shapes[0].byte_size,
    };
    output.call.stack_alignment = 16;
    output.call.shadow_bytes = 32;
    output.call.entry_control = EntryControl::CallReturn;
    BoundaryPlanResult::Accepted { plan: output }
}

boundary trait OverloadedEntry: Calling<OneParameterPolicy> {
    machine enter(value: u64);
    machine enter(value: i64);
}

data Main { }
machine Main::main(&mut self) { }
"#;
    let main_path = write_program("same-named-requirement-overloads", source);
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("same-named exact requirement overloads should each evaluate their policy");
    let overloaded = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "OverloadedEntry")
        .expect("OverloadedEntry boundary trait");
    let schema = provider_planning::service_schema::from_typed(&checked.typed, overloaded)
        .expect("overloaded boundary service schema");
    let methods = schema
        .methods
        .iter()
        .filter(|method| method.name == "enter")
        .collect::<Vec<_>>();

    assert_eq!(methods.len(), 2);
    assert!(!methods[0].requirement_identity.is_empty());
    assert!(!methods[1].requirement_identity.is_empty());
    assert_ne!(
        methods[0].requirement_identity, methods[1].requirement_identity,
        "the readable method name is not an overload identity"
    );
    assert!(methods[0].calling_plan_report_fingerprint.is_some());
    assert!(methods[0].calling_plan_commitment.is_some());
    assert!(methods[1].calling_plan_report_fingerprint.is_some());
    assert!(methods[1].calling_plan_commitment.is_some());

    let _ = fs::remove_dir_all(main_path.parent().expect("temporary policy directory"));
}

#[test]
fn generic_boundary_conformance_selects_and_publishes_its_policy_instance() {
    let source = POLICY.replace(
        "boundary trait Tick: Calling<NoResultPolicy> {\n    machine tick();\n}",
        "boundary trait Tick<C>: Calling<C>\nwhere C satisfies CallingPolicy\n{\n    machine tick(&mut self);\n}\n\ndata TickProvider { count: i64; }\nTickProviderTick: TickProvider satisfies Tick<NoResultPolicy>;\nmachine TickProvider::tick(&mut self) satisfies Tick<NoResultPolicy>::tick {\n    self.count = 1;\n}",
    );
    let main_path = write_program("generic-boundary-policy", &source);
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("generic policy instance compiles");
    let tick = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Tick")
        .expect("Tick boundary trait");
    let conformance = checked
        .typed
        .conformances()
        .iter()
        .find(|conformance| {
            conformance
                .carrier_name()
                .is_some_and(|carrier| carrier.as_str() == "TickProvider")
        })
        .expect("TickProvider conformance");
    let arguments = checked
        .typed
        .type_reference_table
        .type_reference_handles(conformance.arguments);
    let schema =
        provider_planning::service_schema::from_typed_instance(&checked.typed, tick, arguments)
            .expect("generic Tick service schema");

    assert_eq!(schema.methods.len(), 1);
    assert!(schema.methods[0].calling_plan_report_fingerprint.is_some());
    assert!(schema.methods[0].calling_plan_commitment.is_some());
    assert_eq!(
        provider_planning::service_schema::from_typed(&checked.typed, tick)
            .expect("uninstantiated schema")
            .methods[0]
            .calling_plan_report_fingerprint,
        None,
        "a generic declaration is not itself a concrete ABI"
    );
}

#[test]
fn uninstantiated_generic_boundary_does_not_publish_an_abi() {
    let source = POLICY.replace(
        "boundary trait Tick: Calling<NoResultPolicy> {",
        "boundary trait Tick<C>: Calling<C>\nwhere C satisfies CallingPolicy\n{",
    );
    let main_path = write_program("uninstantiated-generic-boundary", &source);
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("generic declaration compiles");
    let tick = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Tick")
        .expect("Tick boundary trait");
    let schema = provider_planning::service_schema::from_typed(&checked.typed, tick)
        .expect("generic declaration schema");

    assert_eq!(schema.methods[0].calling_plan_report_fingerprint, None);
    assert_eq!(schema.methods[0].calling_plan_commitment, None);
}

#[test]
fn source_policy_receives_recursive_fixed_array_and_record_shapes() {
    let source = r#"
use omega::language::std::calling;

data Pair {
    left: f32;
    right: f32;
}

data RecursiveShapePolicy { }
RecursiveShapePolicyCallingPolicy: RecursiveShapePolicy satisfies CallingPolicy;

machine RecursiveShapePolicy::plan(
    signature: BoundarySignature
) -> BoundaryPlanResult
    satisfies CallingPolicy::plan
{
    transition signature.parameter_count == 2 {
        true -> bytes(signature, signature.parameters[0])
        _ -> wrong()
    }

    state bytes(signature: BoundarySignature, root: u64) -> BoundaryPlanResult {
        transition signature.shapes[root].class {
            ValueClass::FixedArray { element, length } -> bytes_array(signature, root, element, length)
            _ -> wrong()
        }
    }

    state bytes_array(signature: BoundarySignature, root: u64, element: u64, length: u64) -> BoundaryPlanResult {
        transition length == 16 && signature.shapes[root].byte_size == 16 && signature.shapes[root].alignment == 1 {
            true -> bytes_element(signature, element)
            _ -> wrong()
        }
    }

    state bytes_element(signature: BoundarySignature, element: u64) -> BoundaryPlanResult {
        transition signature.shapes[element].class {
            ValueClass::Integer -> pairs(signature, signature.parameters[1])
            _ -> wrong()
        }
    }

    state pairs(signature: BoundarySignature, root: u64) -> BoundaryPlanResult {
        transition signature.shapes[root].class {
            ValueClass::FixedArray { element, length } -> pair_array(signature, root, element, length)
            _ -> wrong()
        }
    }

    state pair_array(signature: BoundarySignature, root: u64, element: u64, length: u64) -> BoundaryPlanResult {
        transition length == 2 && signature.shapes[root].byte_size == 16 && signature.shapes[root].alignment == 4 {
            true -> pair_record(signature, element)
            _ -> wrong()
        }
    }

    state pair_record(signature: BoundarySignature, root: u64) -> BoundaryPlanResult {
        transition signature.shapes[root].class {
            ValueClass::Record { first_field, field_count } -> pair_fields(signature, first_field, field_count)
            _ -> wrong()
        }
    }

    state pair_fields(signature: BoundarySignature, first: u64, count: u64) -> BoundaryPlanResult {
        transition count == 2 && signature.fields[first].byte_offset == 0 && signature.fields[first + 1].byte_offset == 4 {
            true -> first_float(signature, signature.fields[first].shape, signature.fields[first + 1].shape)
            _ -> wrong()
        }
    }

    state first_float(signature: BoundarySignature, first: u64, second: u64) -> BoundaryPlanResult {
        transition signature.shapes[first].class {
            ValueClass::Float -> second_float(signature, second)
            _ -> wrong()
        }
    }

    state second_float(signature: BoundarySignature, second: u64) -> BoundaryPlanResult {
        transition signature.shapes[second].class {
            ValueClass::Float -> observed()
            _ -> wrong()
        }
    }

    state observed() -> BoundaryPlanResult {
        BoundaryPlanResult::Rejected {
            reason: CallingPolicyRejection {
                reason: "recursive fixed-array/record shape observed",
            },
        }
    }

    state wrong() -> BoundaryPlanResult {
        BoundaryPlanResult::Rejected {
            reason: CallingPolicyRejection {
                reason: "recursive shape mismatch",
            },
        }
    }
}

boundary trait Probe: Calling<RecursiveShapePolicy> {
    machine inspect(bytes: [u8; 16], pairs: [Pair; 2]);
}

data Main { }
machine Main::main(&mut self) { }
"#;
    let main_path = write_program("recursive-shape", source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect_err("the observing policy deliberately rejects after checking its input graph");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("recursive fixed-array/record shape observed"),
        "policy did not observe the expected recursive boundary graph:\n{rendered}"
    );
    assert!(!rendered.contains("recursive shape mismatch"), "{rendered}");
}

#[test]
fn source_policy_receives_stored_integer_physical_shape() {
    let source = r#"
use omega::language::std::calling;

data IntegerInterpretation { case Signed; case Unsigned; }
data FieldKind { case Scalar; case Text; case Nested; case Repeated; }
data SchemaField { key: u64; size: u64; align: u64; number: i64; kind: FieldKind; }
data Schema { fields: [SchemaField; 32]; field_count: u64; }
data FieldPlan {
    case At(offset: u64);
    case IntegerAt(offset: u64, stored_width: u64, interpretation: IntegerInterpretation);
}
data FieldEntry { key: u64; placement: FieldPlan; }
data Plan {
    entries: [FieldEntry; 64];
    entry_count: u64;
    size_fixed: u64;
    size_is_dynamic: bool;
    align: u64;
}

data SignedByte { entries: [FieldEntry; 64]; }
machine SignedByte::plan(&mut self, schema: Schema) -> Plan {
    let mut owned_entries: [FieldEntry; 64];
    owned_entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::IntegerAt {
            offset: 0,
            stored_width: 8,
            interpretation: IntegerInterpretation::Signed,
        },
    };
    Plan {
        entries: owned_entries,
        entry_count: 1,
        size_fixed: 1,
        size_is_dynamic: false,
        align: 1,
    }
}

data PortableByte { value: i64; }

data StoredWidthPolicy { }
StoredWidthPolicyCallingPolicy: StoredWidthPolicy satisfies CallingPolicy;
machine StoredWidthPolicy::plan(signature: BoundarySignature) -> BoundaryPlanResult
    satisfies CallingPolicy::plan
{
    transition signature.parameter_count == 1 {
        true -> record(signature, signature.parameters[0])
        _ -> wrong()
    }

    state record(signature: BoundarySignature, root: u64) -> BoundaryPlanResult {
        transition signature.shapes[root].class {
            ValueClass::Record { first_field, field_count } -> field(signature, root, first_field, field_count)
            _ -> wrong()
        }
    }

    state field(signature: BoundarySignature, root: u64, first: u64, count: u64) -> BoundaryPlanResult {
        transition count == 1
            && signature.shapes[root].byte_size == 1
            && signature.shapes[root].alignment == 1
            && signature.fields[first].byte_offset == 0 {
            true -> scalar(signature, signature.fields[first].shape)
            _ -> wrong()
        }
    }

    state scalar(signature: BoundarySignature, field: u64) -> BoundaryPlanResult {
        transition signature.shapes[field].class {
            ValueClass::Integer -> scalar_width(signature, field)
            _ -> wrong()
        }
    }

    state scalar_width(signature: BoundarySignature, field: u64) -> BoundaryPlanResult {
        transition signature.shapes[field].byte_size == 1
            && signature.shapes[field].alignment == 1 {
            true -> observed()
            _ -> wrong()
        }
    }

    state observed() -> BoundaryPlanResult {
        BoundaryPlanResult::Rejected {
            reason: CallingPolicyRejection {
                reason: "stored-integer physical shape observed",
            },
        }
    }

    state wrong() -> BoundaryPlanResult {
        BoundaryPlanResult::Rejected {
            reason: CallingPolicyRejection {
                reason: "stored-integer shape mismatch",
            },
        }
    }
}

boundary trait Probe: Calling<StoredWidthPolicy> {
    machine inspect(value: SignedByte<PortableByte>);
}

data Main { }
machine Main::main(&mut self) { }
"#;
    let main_path = write_program("stored-integer-shape", source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect_err("the observing policy deliberately rejects after checking the physical shape");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("stored-integer physical shape observed"),
        "policy did not observe the validated stored width:\n{rendered}"
    );
    assert!(
        !rendered.contains("stored-integer shape mismatch"),
        "{rendered}"
    );
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary policy directory"));
}

#[test]
fn borrowed_dynamic_trait_record_fields_retain_both_descriptor_words() {
    let source = r#"
trait Shape { machine code(&self) -> i32; }
data Descriptor<'item> { handler: &'item dyn Shape; }
data References<'item> {
    shared: &'item dyn Shape;
    unique: &'item mut dyn Shape;
    nested: &'item Descriptor<'item>;
    scalar: &'item u64;
    slice: &'item [u8];
    tail: u64;
}
data Main {}
machine Main::main(&mut self) {}
"#;
    let main_path = write_program("dynamic-trait-record-layout", source);
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("borrowed dynamic descriptors are ordinary stored references");
    for target in [
        target::NativeTarget::windows_x64(),
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let layouts =
            layout::build_layout_plan(&checked, target, checked.opaque_representation_selections())
                .expect("the target lays out both descriptor words");
        let record = layouts
            .data_layouts
            .iter()
            .map(|(_, record)| record)
            .find(|record| record.name.as_str() == "References")
            .unwrap();
        let layout::DataShape::Record { fields } = record.shape else {
            panic!("reference fields remain a record");
        };
        let fields = layouts.fields.span_or_empty(fields);
        for (name, words) in [
            ("shared", 2),
            ("unique", 2),
            ("nested", 1),
            ("scalar", 1),
            ("slice", 2),
            ("tail", 1),
        ] {
            let field = fields
                .iter()
                .find(|field| field.name.as_str() == name)
                .unwrap();
            assert_eq!(
                field.layout.size,
                words * target.pointer_size,
                "{name} on {target:?}"
            );
            assert_eq!(field.layout.alignment, target.pointer_alignment);
            assert!(field.offset + field.layout.size <= record.layout.size);
        }
        assert_eq!(record.layout.size, 9 * target.pointer_size);
        for (field_index, field) in fields.iter().enumerate() {
            for later in &fields[field_index + 1..] {
                assert!(
                    field.offset + field.layout.size <= later.offset
                        || later.offset + later.layout.size <= field.offset,
                    "descriptor and neighboring storage must not overlap"
                );
            }
        }
    }
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary policy directory"));
}

#[test]
fn borrowed_dynamic_trait_parameter_materializes_fat_descriptor_shape() {
    // A `&dyn Trait` boundary parameter is an unsized-referent reference: the
    // runtime carrier is the two-word `{instance, table}` existential
    // descriptor that ordinary dynamic-descriptor arguments expand to. The
    // normalized boundary signature must therefore publish a fat two-pointer
    // shape, not a thin pointer — a policy placing the parameter needs both
    // eightbytes to carry the table word.
    let source = r#"
use omega::language::std::calling;

trait Shape {
    machine code(&self) -> i32;
}

data FatDescriptorPolicy { }
FatDescriptorPolicyCallingPolicy: FatDescriptorPolicy satisfies CallingPolicy;

machine FatDescriptorPolicy::plan(
    signature: BoundarySignature
) -> BoundaryPlanResult
    satisfies CallingPolicy::plan
{
    transition signature.parameter_count == 1 {
        true -> bound(signature, signature.parameters[0])
        _ -> reject()
    }

    state bound(signature: BoundarySignature, root: u64) -> BoundaryPlanResult {
        transition root < 256 {
            true -> build(signature, root)
            _ -> reject()
        }
    }

    state build(signature: BoundarySignature, root: u64) -> BoundaryPlanResult {
        let mut output: BoundaryEntryPlan;
        output.call.convention = CallingConvention::MicrosoftX64;
        output.call.parameter_count = 1;
        output.call.parameters[0].shape.class = AbiValueClass::Integer;
        output.call.parameters[0].shape.byte_size = signature.shapes[root].byte_size;
        output.call.parameters[0].shape.alignment = signature.shapes[root].alignment;
        output.call.parameters[0].location_count = 2;
        output.call.parameters[0].locations[0] = ValueLocation::Register {
            register: MachineRegister::X86Rcx,
            value_byte_offset: 0,
            byte_size: 8,
        };
        output.call.parameters[0].locations[1] = ValueLocation::Register {
            register: MachineRegister::X86Rdx,
            value_byte_offset: 8,
            byte_size: 8,
        };
        output.call.stack_alignment = 16;
        output.call.shadow_bytes = 32;
        output.call.entry_control = EntryControl::CallReturn;
        output.state.initial_regime = MachineRegime::X86Long64;
        output.state.stack = EntryStack::ProviderSelected;
        output.state.preemption = Preemption::NotApplicable;
        BoundaryPlanResult::Accepted { plan: output }
    }

    state reject() -> BoundaryPlanResult {
        BoundaryPlanResult::Rejected {
            reason: CallingPolicyRejection { reason: "unsupported signature" },
        }
    }
}

boundary trait Inspect: Calling<FatDescriptorPolicy> {
    machine inspect(value: &dyn Shape);
}

data Main { }
machine Main::main(&mut self) { }
"#;
    let main_path = write_program("dynamic-trait-descriptor-shape", source);
    let checked = compile_to_checked(CheckedCompileRequest::new(
        &main_path,
        Some("windows_x86_64"),
    ))
    .expect("a borrowed dynamic-trait parameter must carry its descriptor width");
    let inspect_trait = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Inspect")
        .expect("exact boundary trait declaration");
    let inspect = checked
        .typed
        .trait_machine_signatures(inspect_trait)
        .iter()
        .find(|signature| signature.name.as_str() == "inspect")
        .expect("exact boundary requirement");
    let realization = checked
        .boundary_calling_plan_realizations()
        .iter()
        .find(|realization| realization.requirement_machine == inspect.symbol)
        .expect("retained calling-plan realization");
    let signature = realization.materialized_signature();
    let [root] = signature.parameters() else {
        panic!("one semantic parameter root")
    };
    let shape = signature.shapes()[usize::from(*root)];
    assert_eq!(
        shape.class(),
        BoundaryValueClass::Reference,
        "a borrowed dynamic-trait parameter is a reference descriptor"
    );
    let target = target::NativeTarget::windows_x64();
    let [parameter] = checked.typed.state_signature_parameters(inspect) else {
        panic!("one authored dynamic reference parameter");
    };
    let stored = layout::layout_type_reference(
        &checked,
        target,
        checked.opaque_representation_selections(),
        parameter.type_reference,
    )
    .expect("reference storage agrees with its calling-policy shape");
    assert_eq!(stored.size, usize::from(shape.byte_size()));
    assert_eq!(stored.alignment, usize::from(shape.alignment()));
    assert_eq!(
        shape.byte_size(),
        u16::try_from(target.pointer_size * 2).expect("descriptor width fits u16"),
        "the descriptor needs both pointer words"
    );
    assert_eq!(
        shape.alignment(),
        u16::try_from(target.pointer_alignment).expect("descriptor alignment fits u16")
    );
    let [placement] = realization.boundary_entry_plan.call.parameters.as_slice() else {
        panic!("one placed parameter")
    };
    assert_eq!(placement.shape.byte_size, shape.byte_size());
    assert_eq!(placement.shape.alignment, shape.alignment());
    let [
        calling_conventions::ValueLocation::Register {
            register: first_register,
            value_byte_offset: 0,
            byte_size: 8,
        },
        calling_conventions::ValueLocation::Register {
            register: second_register,
            value_byte_offset: 8,
            byte_size: 8,
        },
    ] = placement.locations.as_slice()
    else {
        panic!("the descriptor's two words must be placed in order")
    };
    assert_eq!(
        *first_register,
        calling_conventions::MachineRegister::X86Rcx
    );
    assert_eq!(
        *second_register,
        calling_conventions::MachineRegister::X86Rdx
    );
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary policy directory"));
}

#[test]
fn compatibility_boundary_materializes_the_selected_opaque_carrier() {
    // A foreign import leaf has no authored `Calling<P>` policy, so its row's
    // calling plan comes from the compatibility materialization. That path must
    // see the authoritative build's `OpaqueRepresentation` selection: the
    // opaque semantic type stays opaque in the source signature while the
    // retained entry plan places the exact selected carrier bytes.
    let main_path = write_project(
        "foreign-opaque-selected",
        FOREIGN_OPAQUE_SOURCE,
        FOREIGN_OPAQUE_BUILD,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(
        &main_path,
        Some("windows_x86_64"),
    ))
    .expect("the selected representation must close the compatibility boundary demand");
    let [selection] = checked.opaque_representation_selections() else {
        panic!("one exact opaque-representation selection")
    };
    let carrier = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "ForeignTokenCarrier")
        .expect("exact representation carrier");
    assert_eq!(selection.carrier(), carrier.symbol);
    let matching = checked
        .external_binding_rows()
        .iter()
        .filter(|row| row.method == "deliver")
        .collect::<Vec<_>>();
    let [row] = matching.as_slice() else {
        panic!("one foreign import binding row for `deliver`, found {matching:?}")
    };
    assert_eq!(row.trait_name, "ForeignChannel");
    assert!(matches!(
        row.binding,
        calling_conventions::ExternalBindingKind::Import { .. }
    ));
    let plan = row
        .boundary_entry_plan
        .as_ref()
        .expect("the compatibility row must retain its validated entry plan");
    let [parameter] = plan.call.parameters.as_slice() else {
        panic!("the opaque carrier crosses as one semantic parameter")
    };
    // `ForeignTokenCarrier` is `{low: u64, high: u64}`: the placement must
    // cover the complete selected carrier, not a truncated descriptor word.
    assert_eq!(parameter.shape.byte_size, 16);
    assert_eq!(parameter.shape.alignment, 8);
    assert!(
        !parameter.locations.is_empty(),
        "the carrier's bytes must land in declared ABI locations"
    );
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary policy directory"));
}

#[test]
fn compatibility_boundary_rejects_opaque_by_value_without_build_selection() {
    // Without the authoritative build selection the same foreign leaf still
    // fails closed: no carrier visibility exists, so no physical plan may be
    // fabricated for the opaque semantic parameter.
    let unselected = r#"
machine build(builder: &mut Build) {
    builder.application("foreign_channel");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
}
"#;
    let main_path = write_project(
        "foreign-opaque-unselected",
        FOREIGN_OPAQUE_SOURCE,
        unselected,
    );
    let rendered = compile_to_checked(CheckedCompileRequest::new(
        &main_path,
        Some("windows_x86_64"),
    ))
    .expect_err("an unselected opaque by-value foreign boundary must reject")
    .iter()
    .map(|diagnostic| diagnostic.message.as_str())
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        rendered.contains("crosses this boundary by value")
            && rendered.contains("selects no exact `OpaqueRepresentation<ForeignToken>`"),
        "unexpected diagnostics:\n{rendered}"
    );
}

const MIXED_RECORD_SOURCE: &str = r#"use omega::language::core::binding;
use omega::language::core::external_binding;

pub data Pair {
    first: u64;
    second: u64;
}

pub boundary trait Aggregate {
    machine combine(tag: u64, pair: Pair) -> u64;
    machine peek(tag: u64, pair: &Pair) -> u64;
}

linux_x86_64 machine combine_binding() -> ForeignBinding<15, 11, 7> {
    ForeignBinding::DllImport {
        import: DllImport::ElfVersioned {
            object: "libagg-probe.so",
            symbol: "agg_combine",
            version: "OMEGA_1",
        },
    }
}

linux_x86_64 machine peek_binding() -> ForeignBinding<15, 8, 7> {
    ForeignBinding::DllImport {
        import: DllImport::ElfVersioned {
            object: "libagg-probe.so",
            symbol: "agg_peek",
            version: "OMEGA_1",
        },
    }
}

machine combine_leaf(tag: u64, pair: Pair) -> u64
    satisfies Aggregate::combine via combine_binding();
machine peek_leaf(tag: u64, pair: &Pair) -> u64
    satisfies Aggregate::peek via peek_binding();

data Main { boundary: Binding<Aggregate>; }
machine Main::main(&mut self) reaches Aggregate {
    let pair: Pair = Pair { first: 2u64, second: 22u64 };
    let answer: u64 = self.boundary.combine(20u64, pair);
}
"#;

const MIXED_RECORD_BUILD: &str = r#"
machine build(builder: &mut Build) {
    builder.application("mixed_record");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
}
"#;

fn mixed_record_binding_plan<'a>(
    checked: &'a compiler::CheckedCompilation,
    method: &str,
) -> &'a calling_conventions::BoundaryEntryPlan {
    let matching = checked
        .external_binding_rows()
        .iter()
        .filter(|row| row.method == method)
        .collect::<Vec<_>>();
    let [row] = matching.as_slice() else {
        panic!("one foreign import binding row for `{method}`, found {matching:?}")
    };
    row.boundary_entry_plan
        .as_ref()
        .expect("the compatibility row must retain its validated entry plan")
}

#[test]
fn mixed_scalar_record_boundary_splits_the_by_value_record_across_registers() {
    // `combine(tag: u64, pair: Pair)` mixes a scalar and a by-value record on
    // one foreign signature: the scalar takes the first integer register while
    // the record classifies INTEGER/INTEGER and splits across the next two,
    // each half carrying its own `value_byte_offset`. The result stays a
    // single register word.
    let main_path = write_project(
        "mixed-record-value",
        MIXED_RECORD_SOURCE,
        MIXED_RECORD_BUILD,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, Some("linux_x86_64")))
        .expect("checked compile");
    let plan = mixed_record_binding_plan(&checked, "combine");
    assert_eq!(
        plan.call.policy,
        calling_conventions::CallingPolicy::SystemVAMD64
    );
    let [scalar, record] = plan.call.parameters.as_slice() else {
        panic!("tag plus the by-value record: two semantic parameters")
    };
    assert_eq!(scalar.shape.class, calling_conventions::ValueClass::Integer);
    assert_eq!(scalar.shape.byte_size, 8);
    assert_eq!(scalar.shape.alignment, 8);
    assert_eq!(
        scalar.locations.as_slice(),
        &[calling_conventions::ValueLocation::Register {
            register: calling_conventions::MachineRegister::X86Rdi,
            value_byte_offset: 0,
            byte_size: 8,
        }]
    );
    assert_eq!(record.shape.class, calling_conventions::ValueClass::Integer);
    assert_eq!(record.shape.byte_size, 16);
    assert_eq!(record.shape.alignment, 8);
    assert_eq!(
        record.locations.as_slice(),
        &[
            calling_conventions::ValueLocation::Register {
                register: calling_conventions::MachineRegister::X86Rsi,
                value_byte_offset: 0,
                byte_size: 8,
            },
            calling_conventions::ValueLocation::Register {
                register: calling_conventions::MachineRegister::X86Rdx,
                value_byte_offset: 8,
                byte_size: 8,
            },
        ]
    );
    let result = plan.call.result.as_ref().expect("u64 result");
    assert_eq!(result.shape.class, calling_conventions::ValueClass::Integer);
    assert_eq!(result.shape.byte_size, 8);
    assert_eq!(
        result.locations.as_slice(),
        &[calling_conventions::ValueLocation::Register {
            register: calling_conventions::MachineRegister::X86Rax,
            value_byte_offset: 0,
            byte_size: 8,
        }]
    );
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary policy directory"));
}

#[test]
fn mixed_scalar_record_boundary_crosses_the_borrowed_record_as_one_pointer_word() {
    // `peek(tag: u64, pair: &Pair)` takes the same record borrowed: the
    // reference crosses as a single pointer word in the second integer
    // register — it must not fan out into the record's two eightbytes.
    let main_path = write_project(
        "mixed-record-borrow",
        MIXED_RECORD_SOURCE,
        MIXED_RECORD_BUILD,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, Some("linux_x86_64")))
        .expect("checked compile");
    let plan = mixed_record_binding_plan(&checked, "peek");
    assert_eq!(
        plan.call.policy,
        calling_conventions::CallingPolicy::SystemVAMD64
    );
    let [scalar, borrow] = plan.call.parameters.as_slice() else {
        panic!("tag plus the borrowed record: two semantic parameters")
    };
    assert_eq!(scalar.shape.class, calling_conventions::ValueClass::Integer);
    assert_eq!(scalar.shape.byte_size, 8);
    assert_eq!(
        scalar.locations.as_slice(),
        &[calling_conventions::ValueLocation::Register {
            register: calling_conventions::MachineRegister::X86Rdi,
            value_byte_offset: 0,
            byte_size: 8,
        }]
    );
    assert_eq!(borrow.shape.class, calling_conventions::ValueClass::Integer);
    assert_eq!(
        borrow.shape.byte_size, 8,
        "the borrowed record crosses as one pointer word, not the record bytes"
    );
    assert_eq!(borrow.shape.alignment, 8);
    assert_eq!(
        borrow.locations.as_slice(),
        &[calling_conventions::ValueLocation::Register {
            register: calling_conventions::MachineRegister::X86Rsi,
            value_byte_offset: 0,
            byte_size: 8,
        }]
    );
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary policy directory"));
}
