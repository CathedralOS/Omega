use super::{
    PILOT, package_inputs_for_source, package_inputs_with_standard_library, write_program,
};
use crate::fixture_roster;
use build_time_evaluation::compute_layout_plan;
use compiler::{CheckedCompileRequest, compile_to_checked};
use layout::{DataShape, build_layout_plan};
use std::fs;
use std::path::Path;
use target::NativeTarget;

/// L4: plan-laid VALUE TYPES. The run canary's `gdt: Spread16<Gdtish>` field
/// resolves to a synthesized record whose NATIVE placement is the validated
/// plan's -- asserted here directly against the layout plan the backend
/// consumes, because placement is deliberately unobservable from inside the
/// language (the run canary proves no-miscompile; this proves the override
/// actually fired: native packing would give offsets [0,4,8,16], size 24).
#[test]
fn plan_laid_value_types_are_placed_by_their_plan() {
    let canary = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate should live under omega-rust/omega/compiler/compiler")
        .join("tests/omega/pass")
        .join(fixture_roster::RUNTIME_PLAN_LAID_VALUE_FIELD_EXIT)
        .join("main.omg");
    let mut checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs_with_standard_library(&canary)),
        ..CheckedCompileRequest::new(&canary, None)
    })
    .expect("plan-laid canary should compile")
    .into_program();

    // The pipeline recorded the validated plan on the typed trees.
    assert_eq!(checked.typed.plan_laid_layouts.len(), 1);
    let recorded = &checked.typed.plan_laid_layouts[0];
    assert_eq!(recorded.data_name, "Spread16<Gdtish>");
    let plan_laid_data_symbol = recorded.data_symbol;
    let schema_symbol = recorded.schema_symbol;
    let schema_field_symbols = recorded.schema_field_symbols.clone();
    let policy_symbol = recorded.policy_symbol;
    let policy_plan_machine_symbol = recorded.policy_plan_machine_symbol;
    let plan_laid_data = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == plan_laid_data_symbol)
        .expect("exact synthesized plan-laid data");
    assert_eq!(plan_laid_data.name.as_str(), recorded.data_name);
    assert_eq!(recorded.offsets, vec![0, 16, 32, 48]);
    assert_eq!(recorded.size, 64);
    assert_eq!(recorded.align, 16);

    // And the backend layout plan bakes those offsets into the synthesized
    // record's FieldLayouts.
    let target = NativeTarget::from_omega_target_name(None).expect("host target");
    let layouts = build_layout_plan(&checked, target, &[]).expect("layout plan should build");
    let data_layout = layouts
        .data_layouts
        .iter()
        .map(|(_, layout)| layout)
        .find(|layout| layout.name.as_str() == "Spread16<Gdtish>")
        .expect("the synthesized plan-laid record should be laid out");
    assert_eq!(data_layout.layout.size, 64);
    assert_eq!(data_layout.layout.alignment, 16);
    let DataShape::Record { fields } = &data_layout.shape else {
        panic!("plan-laid data should be a record");
    };
    let offsets: Vec<usize> = layouts
        .fields
        .span_or_empty(*fields)
        .iter()
        .map(|field| field.offset)
        .collect();
    assert_eq!(offsets, vec![0, 16, 32, 48]);

    checked.typed.plan_laid_layouts[0].data_name = "diagnostic-only-layout-name".to_owned();
    validation::validate_program(&checked.typed)
        .expect("presentation-name drift must not change retained plan identity");
    let layouts = build_layout_plan(&checked, target, &[])
        .expect("presentation-name drift must not redirect a plan-laid layout");
    let data_layout = layouts
        .data_layouts
        .iter()
        .map(|(_, layout)| layout)
        .find(|layout| layout.symbol == plan_laid_data_symbol)
        .expect("exact synthesized plan-laid record should remain selected by symbol");
    assert_eq!(data_layout.layout.size, 64);

    let (main_symbol, main_field_symbols) = {
        let main = checked
            .typed
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Main")
            .expect("fixture Main data");
        let symbols = checked
            .typed
            .data_members(main)
            .iter()
            .filter_map(|member| match member {
                typed_trees::data::DataMember::Field(field) => Some(field.symbol),
                typed_trees::data::DataMember::Variant(_) => None,
            })
            .collect::<Vec<_>>();
        (main.symbol, symbols)
    };

    checked.typed.plan_laid_layouts[0].schema_symbol = main_symbol;
    let diagnostics = validation::validate_program(&checked.typed)
        .expect_err("substituted source schema identity must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("changed its exact source schema field identity inventory")
    }));
    checked.typed.plan_laid_layouts[0].schema_symbol = schema_symbol;

    checked.typed.plan_laid_layouts[0].schema_symbol = main_symbol;
    checked.typed.plan_laid_layouts[0].schema_field_symbols = main_field_symbols;
    let diagnostics = validation::validate_program(&checked.typed)
        .expect_err("coordinated source schema substitution must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("changed its exact schema-to-synthesized field correspondence")
    }));
    checked.typed.plan_laid_layouts[0].schema_symbol = schema_symbol;
    checked.typed.plan_laid_layouts[0].schema_field_symbols = schema_field_symbols;

    let schema_report_fingerprint = checked.typed.plan_laid_layouts[0]
        .validated_layout
        .schema_report_fingerprint;
    checked.typed.plan_laid_layouts[0]
        .validated_layout
        .schema_report_fingerprint ^= 1;
    let diagnostics = validation::validate_program(&checked.typed)
        .expect_err("substituted normalized schema identity must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("changed its exact target-neutral layout report identity")
    }));
    checked.typed.plan_laid_layouts[0]
        .validated_layout
        .schema_report_fingerprint = schema_report_fingerprint;

    let report_first_offset = checked.typed.plan_laid_layouts[0]
        .validated_layout
        .offsets
        .as_ref()
        .expect("whole-field layout retains its derived offsets")[0];
    checked.typed.plan_laid_layouts[0]
        .validated_layout
        .offsets
        .as_mut()
        .expect("whole-field layout retains its derived offsets")[0] = report_first_offset + 1;
    let diagnostics = validation::validate_program(&checked.typed)
        .expect_err("drifted target-neutral offsets projection must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("changed its exact target-neutral layout report identity")
    }));
    checked.typed.plan_laid_layouts[0]
        .validated_layout
        .offsets
        .as_mut()
        .expect("whole-field layout retains its derived offsets")[0] = report_first_offset;

    let first_offset = checked.typed.plan_laid_layouts[0].offsets[0];
    checked.typed.plan_laid_layouts[0].offsets[0] = first_offset + 1;
    let diagnostics = validation::validate_program(&checked.typed)
        .expect_err("drifted flattened geometry must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("changed its exact validated geometry projection")
    }));
    checked.typed.plan_laid_layouts[0].offsets[0] = first_offset;

    checked.typed.plan_laid_layouts[0].policy_symbol = main_symbol;
    let diagnostics = validation::validate_program(&checked.typed)
        .expect_err("substituted nominal policy identity must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("changed its exact nominal policy binding")
    }));
    checked.typed.plan_laid_layouts[0].policy_symbol = policy_symbol;

    checked.typed.plan_laid_layouts[0].policy_plan_machine_symbol = main_symbol;
    let diagnostics = validation::validate_program(&checked.typed)
        .expect_err("substituted policy plan machine must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("no longer names its exact policy plan machine")
    }));
    checked.typed.plan_laid_layouts[0].policy_plan_machine_symbol = policy_plan_machine_symbol;

    checked.typed.plan_laid_layouts[0].data_symbol = main_symbol;
    let diagnostics = validation::validate_program(&checked.typed)
        .expect_err("substituted plan-laid data identity must fail validation");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("changed its exact synthesized field identity inventory")
    }));
    let diagnostic = build_layout_plan(&checked, target, &[])
        .expect_err("substituted plan-laid data identity must fail closed");
    assert!(
        diagnostic
            .message
            .contains("plan-laid data `Main` changed its exact field identity inventory")
    );
}

#[test]
fn plan_laid_private_callback_slot_retains_exact_target_neutral_demand() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate should live under omega-rust/omega/compiler/compiler");
    let core_layout = fs::read_to_string(repository.join("source/library/core/layout.omg"))
        .expect("read normative core layout source");
    assert_eq!(
        core_layout
            .match_indices("pub trait PrivateCallbackSlot<machine Requirement>")
            .count(),
        1,
        "core layout must publish exactly one normative PrivateCallbackSlot trait"
    );
    let canary = repository
        .join("tests/omega/pass")
        .join(fixture_roster::PRIVATE_CALLBACK_SLOT_DEMAND_COMPILE)
        .join("main.omg");
    let mut checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs_for_source(&canary, 0x51)),
        ..CheckedCompileRequest::new(&canary, None)
    })
    .expect("private callback-slot layout canary should compile")
    .into_program();

    let layout_index = checked
        .typed
        .plan_laid_layouts
        .iter()
        .position(|layout| layout.data_name == "Spread<ForeignRecord>")
        .expect("the plan-laid layout should be retained");
    let recorded = &checked.typed.plan_laid_layouts[layout_index];
    assert_eq!(recorded.offsets, vec![0]);
    assert_eq!(recorded.validated_layout.size, Some(16));
    assert_eq!(recorded.validated_layout.align, 8);

    let [demand] = recorded.private_callback_demands.as_slice() else {
        panic!("the exact private callback demand should be retained");
    };
    assert!(demand.slot_identity.contains("WndClassWindowProcedureSlot"));
    assert!(demand.layout_subject_identity.ends_with("::Spread"));
    assert!(
        demand
            .callback_requirement_identity
            .contains("WindowProcedure")
    );
    assert!(demand.callback_requirement_identity.contains("call"));
    assert_eq!(demand.offset, 8);

    let native = layout_plans::NativeLayoutPlanReport {
        layout: recorded.validated_layout.clone(),
        private_callback_demands: recorded.private_callback_demands.clone(),
    };
    let first = layout_plans::normalized_native_layout_plan_report_fingerprint(&native);
    let mut moved = native;
    moved.private_callback_demands[0].offset = 9;
    assert_ne!(
        first,
        layout_plans::normalized_native_layout_plan_report_fingerprint(&moved),
        "private placement must participate in native layout identity"
    );

    let target = NativeTarget::windows_x64();
    let closed = build_layout_plan(&checked, target, &[])
        .expect("the selected target should close the private callback slot");
    let [closed_demand] = closed.private_callback_demands.as_slice() else {
        panic!("the exact target-closed private callback demand should be published");
    };
    assert_eq!(closed_demand.data_symbol, recorded.data_symbol);
    assert_eq!(closed_demand.slot_application, demand.slot_application);
    assert_eq!(closed_demand.offset, 8);
    assert_eq!(closed_demand.byte_size, 8);
    assert_eq!(closed_demand.alignment, 8);
    assert_eq!(
        closed_demand.layout,
        calling_conventions::callback_layout_plan_id(first, 8, 8),
        "one-slot callback destinations retain their established layout identity"
    );
    assert_eq!(closed_demand.slot_identity.as_ref(), demand.slot_identity);
    assert_eq!(
        closed_demand.requirement,
        calling_conventions::callback_requirement_id(&demand.callback_requirement_identity)
    );
    let native_demand =
        closed_demand.native_demand(calling_conventions::NativeParameterId::new(1).unwrap());
    assert_eq!(native_demand.requirement, closed_demand.requirement);
    assert!(matches!(
        native_demand.destination,
        calling_conventions::NativePlace::Field {
            field_path,
            ..
        } if field_path == [closed_demand.slot]
    ));

    let first_layout_identity = closed_demand.layout;
    {
        let mutated = &mut checked.typed.plan_laid_layouts[layout_index];
        mutated.private_callback_demands[0].offset = 16;
        mutated.size = 24;
        mutated.validated_layout.size = Some(24);
    }
    let moved = build_layout_plan(&checked, target, &[])
        .expect("a distinct valid target layout should close independently");
    assert_ne!(
        first_layout_identity, moved.private_callback_demands[0].layout,
        "valid private-offset mutation must change target-closed layout identity"
    );
}

#[test]
fn target_closure_proves_one_inline_named_field_then_one_private_callback_slot() {
    let canary = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .unwrap()
        .join("tests/omega/pass")
        .join(fixture_roster::PRIVATE_CALLBACK_SLOT_DEMAND_COMPILE)
        .join("main.omg");
    let mut checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs_for_source(&canary, 0x52)),
        ..CheckedCompileRequest::new(&canary, None)
    })
    .expect("private callback-slot layout canary should compile")
    .into_program();
    let child = checked
        .typed
        .plan_laid_layouts
        .iter()
        .find(|layout| layout.data_name == "Spread<ForeignRecord>")
        .cloned()
        .unwrap();
    let main = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Main")
        .unwrap();
    let main_symbol = main.symbol;
    let main_field = checked
        .data_members(main)
        .iter()
        .find_map(|member| match member {
            checked_trees::data::DataMember::Field(field) => Some(field.symbol),
            _ => None,
        })
        .unwrap();
    let mut root = child.clone();
    root.data_name = "Main".to_owned();
    root.data_symbol = main_symbol;
    root.field_symbols = vec![main_field];
    root.schema_field_symbols = vec![main_field];
    root.private_callback_demands.clear();
    root.offsets = vec![0];
    root.bit_fields.clear();
    root.integer_fields.clear();
    root.repeated_fields.clear();
    root.size = 16;
    root.align = 8;
    root.validated_layout.entries.truncate(1);
    root.validated_layout.offsets = Some(vec![0]);
    root.validated_layout.size = Some(16);
    root.validated_layout.align = 8;
    checked.typed.plan_laid_layouts.push(root);

    let closed = build_layout_plan(&checked, NativeTarget::windows_x64(), &[]).unwrap();
    assert_eq!(
        closed.plan_laid_layout_identities.len(),
        2,
        "root and child identities: {:?}",
        closed.plan_laid_layout_identities
    );
    let [path] = closed.two_hop_private_callback_paths.as_slice() else {
        panic!("one exact root-field-child-slot path should close")
    };
    assert_eq!(path.root_layout.data_symbol, main_symbol);
    assert_eq!(
        path.root_layout.data_identity.as_ref(),
        checked
            .typed
            .normalized_hermetic_symbol_identity(main_symbol)
            .unwrap(),
    );
    assert_eq!(path.field_symbol, main_field);
    assert_eq!(
        path.field_identity.as_ref(),
        checked
            .typed
            .normalized_hermetic_symbol_identity(main_field)
            .unwrap(),
    );
    assert_eq!(path.child_layout.data_symbol, child.data_symbol);
    assert_eq!(path.field_relative_offset, 0);
    assert_eq!(path.field_extent, 16);
    assert_eq!(path.field_alignment, 8);
    assert_eq!(path.child_layout.physical.size, 16);
    assert_eq!(path.child_layout.physical.alignment, 8);
    assert_eq!(
        path.terminal_demand.slot_application,
        child.private_callback_demands[0].slot_application
    );
    assert_eq!(
        path.terminal_demand.slot_identity.as_ref(),
        child.private_callback_demands[0].slot_identity
    );
    assert_eq!(
        path.terminal_demand.callback_requirement_identity.as_ref(),
        child.private_callback_demands[0].callback_requirement_identity
    );
    assert_eq!(path.terminal_demand.offset, 8);
    assert_eq!(path.terminal_demand.byte_size, 8);
    assert_eq!(path.terminal_demand.alignment, 8);
    assert_eq!(path.composed_offset, 8);
    assert_ne!(path.root_layout.layout, path.child_layout.layout);
    let demand = path.native_demand(calling_conventions::NativeParameterId::new(1).unwrap());
    assert!(matches!(
        demand.destination,
        calling_conventions::NativePlace::Field { layout, field_path, .. }
            if layout == path.root_layout.layout
                && field_path == [path.field_slot, path.terminal_demand.slot]
    ));
}

#[test]
fn target_closed_private_callback_slot_rejects_geometry_mutations() {
    let canary = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate should live under omega-rust/omega/compiler/compiler")
        .join("tests/omega/pass")
        .join(fixture_roster::PRIVATE_CALLBACK_SLOT_DEMAND_COMPILE)
        .join("main.omg");
    let mut checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs_for_source(&canary, 0x56)),
        ..CheckedCompileRequest::new(&canary, None)
    })
    .expect("private callback-slot layout canary should compile")
    .into_program();
    let layout_index = checked
        .typed
        .plan_laid_layouts
        .iter()
        .position(|layout| layout.data_name == "Spread<ForeignRecord>")
        .expect("the plan-laid layout should be retained");
    let original = checked.typed.plan_laid_layouts[layout_index].clone();
    let target = NativeTarget::windows_x64();

    checked.typed.plan_laid_layouts[layout_index].private_callback_demands[0].offset = 9;
    let error =
        build_layout_plan(&checked, target, &[]).expect_err("unaligned private slot must reject");
    assert!(error.message.contains("is not aligned"), "{error:?}");

    checked.typed.plan_laid_layouts[layout_index] = original.clone();
    checked.typed.plan_laid_layouts[layout_index].private_callback_demands[0].offset = 16;
    let error = build_layout_plan(&checked, target, &[])
        .expect_err("out-of-bounds private slot must reject");
    assert!(
        error.message.contains("lies outside its 16-byte layout"),
        "{error:?}"
    );

    checked.typed.plan_laid_layouts[layout_index] = original.clone();
    checked.typed.plan_laid_layouts[layout_index].private_callback_demands[0].offset = 0;
    let error =
        build_layout_plan(&checked, target, &[]).expect_err("semantic/private overlap must reject");
    assert!(
        error.message.contains("overlaps semantic field storage"),
        "{error:?}"
    );

    checked.typed.plan_laid_layouts[layout_index] = original.clone();
    let mut overlapping =
        checked.typed.plan_laid_layouts[layout_index].private_callback_demands[0].clone();
    overlapping.slot_identity.push_str("::SecondSlot");
    checked.typed.plan_laid_layouts[layout_index]
        .private_callback_demands
        .push(overlapping);
    let error =
        build_layout_plan(&checked, target, &[]).expect_err("private/private overlap must reject");
    assert!(
        error.message.contains("private callback slots") && error.message.contains("overlap"),
        "{error:?}"
    );

    checked.typed.plan_laid_layouts[layout_index] = original.clone();
    checked.typed.plan_laid_layouts[layout_index].private_callback_demands[0]
        .layout_subject_identity
        .push_str("::Substituted");
    let error = build_layout_plan(&checked, target, &[])
        .expect_err("retained layout-subject substitution must reject");
    assert!(
        error.message.contains("changed layout subject"),
        "{error:?}"
    );

    checked.typed.plan_laid_layouts[layout_index] = original;
    let mut conflicting =
        checked.typed.plan_laid_layouts[layout_index].private_callback_demands[0].clone();
    conflicting
        .callback_requirement_identity
        .push_str("::Other");
    conflicting.offset = 16;
    checked.typed.plan_laid_layouts[layout_index]
        .private_callback_demands
        .push(conflicting);
    checked.typed.plan_laid_layouts[layout_index].size = 24;
    checked.typed.plan_laid_layouts[layout_index]
        .validated_layout
        .size = Some(24);
    let error = build_layout_plan(&checked, target, &[])
        .expect_err("one canonical slot cannot close under conflicting requirements");
    assert!(
        error.message.contains("repeats private callback slot"),
        "{error:?}"
    );
}

#[test]
fn private_callback_slot_rejects_a_different_layout_subject() {
    let canary = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate should live under omega-rust/omega/compiler/compiler")
        .join("tests/omega/fail")
        .join(fixture_roster::PRIVATE_CALLBACK_SLOT_WRONG_LAYOUT)
        .join("main.omg");
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs_for_source(&canary, 0x52)),
        ..CheckedCompileRequest::new(&canary, None)
    })
    .expect_err("a private slot for another layout must reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("but the active layout producer is attached to"),
        "unexpected diagnostics:\n{combined}"
    );
}

#[test]
fn private_callback_slot_rejects_duplicate_placement() {
    let canary = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate should live under omega-rust/omega/compiler/compiler")
        .join("tests/omega/fail")
        .join(fixture_roster::PRIVATE_CALLBACK_SLOT_DUPLICATE)
        .join("main.omg");
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs_for_source(&canary, 0x53)),
        ..CheckedCompileRequest::new(&canary, None)
    })
    .expect_err("placing one exact private slot twice must reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("places private callback slot") && combined.contains("more than once"),
        "unexpected diagnostics:\n{combined}"
    );
}

#[test]
fn authored_place_private_lookalike_cannot_mint_a_receipt() {
    let canary = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate should live under omega-rust/omega/compiler/compiler")
        .join("tests/omega/fail")
        .join(fixture_roster::PRIVATE_CALLBACK_SLOT_AUTHORED_LOOKALIKE)
        .join("main.omg");
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs_for_source(&canary, 0x54)),
        ..CheckedCompileRequest::new(&canary, None)
    })
    .expect_err("an authored Plan::place_private lookalike must reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("sealed to core `Plan::place_private`")
            && combined.contains("authored lookalike"),
        "unexpected diagnostics:\n{combined}"
    );
}

#[test]
fn untaken_private_callback_slot_branch_emits_no_receipt() {
    let canary = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate should live under omega-rust/omega/compiler/compiler")
        .join("tests/omega/pass")
        .join(fixture_roster::PRIVATE_CALLBACK_SLOT_UNTAKEN_COMPILE)
        .join("main.omg");
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs_for_source(&canary, 0x55)),
        ..CheckedCompileRequest::new(&canary, None)
    })
    .expect("the policy with an untaken private-placement branch should compile");
    let recorded = checked
        .typed
        .plan_laid_layouts
        .iter()
        .find(|layout| layout.data_name == "Spread<ForeignRecord>")
        .expect("the plan-laid layout should be retained");
    assert_eq!(recorded.offsets, vec![0]);
    assert_eq!(recorded.validated_layout.size, Some(8));
    assert!(recorded.private_callback_demands.is_empty());
}

#[test]
fn plan_laid_compact_bits_retain_validated_fragment_geometry() {
    let canary = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate should live under omega-rust/omega/compiler/compiler")
        .join("tests/omega/pass")
        .join(fixture_roster::RUNTIME_PLAN_LAID_COMPACT_BITS_EXIT)
        .join("main.omg");
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs_with_standard_library(&canary)),
        ..CheckedCompileRequest::new(&canary, None)
    })
    .expect("compact-bit canary should compile");

    assert_eq!(checked.typed.plan_laid_layouts.len(), 1);
    let recorded = &checked.typed.plan_laid_layouts[0];
    assert_eq!(recorded.data_name, "CompactBits<PackedFlags>");
    assert_eq!(recorded.offsets, vec![0, 0, 0]);
    assert_eq!(recorded.size, 1);
    assert_eq!(recorded.align, 1);
    assert_eq!(recorded.bit_fields.len(), 3);
    assert_eq!(recorded.bit_fields[0].fragments.len(), 1);
    assert_eq!(recorded.bit_fields[1].fragments.len(), 1);
    assert_eq!(recorded.bit_fields[2].fragments.len(), 2);
    assert_eq!(recorded.bit_fields[2].fragments[0].source_lsb, 0);
    assert_eq!(recorded.bit_fields[2].fragments[1].source_lsb, 2);

    let target = NativeTarget::from_omega_target_name(None).expect("host target");
    let layouts = build_layout_plan(&checked, target, &[]).expect("layout plan should build");
    let data_layout = layouts
        .data_layouts
        .iter()
        .map(|(_, layout)| layout)
        .find(|layout| layout.name.as_str() == "CompactBits<PackedFlags>")
        .expect("the synthesized compact-bit record should be laid out");
    let DataShape::Record { fields } = &data_layout.shape else {
        panic!("compact-bit data should be a record");
    };
    let fields = layouts.fields.span_or_empty(*fields);
    assert_eq!(fields.len(), 3);
    assert_eq!(
        fields.iter().map(|field| field.offset).collect::<Vec<_>>(),
        vec![0, 0, 0]
    );
    assert_eq!(
        layouts
            .bit_field(fields[2].symbol)
            .expect("split field should retain bit geometry")
            .fragments
            .len(),
        2
    );
}

#[test]
fn c_layout_policy_plans_a_uefi_ish_schema() {
    let main_path = write_program("clayout-pilot", PILOT);
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("pilot should compile");

    let report = compute_layout_plan(&checked.typed, "CLayout::plan", "GdtEntryish")
        .expect("the C layout plan should evaluate and validate");

    // u16 @ 0, u32 @ 4 (padded past 2), u8 @ 8, u64 @ 16 (padded past 9);
    // size 24 (rounded to align 8).
    assert_eq!(report.offsets, Some(vec![0, 4, 8, 16]));
    assert_eq!(report.size, Some(24));
    assert_eq!(report.align, 8);
}

#[test]
fn fixed_primitive_arrays_are_reflected_as_one_repeated_at_field() {
    let main_path = write_program(
        "fixed-array-at",
        r#"
use omega::language::core::layout;

data ArrayLayout { }
machine ArrayLayout::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 8 },
    };
    Plan { entries: entries, entry_count: 1,
           size_fixed: 16, size_is_dynamic: false, align: 2 }
}
data Samples { values: [u16; 3]; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("fixed array should reflect");
    let report = compute_layout_plan(&checked.typed, "ArrayLayout::plan", "Samples")
        .expect("one At placement should admit the complete fixed-array extent");
    assert_eq!(report.offsets, Some(vec![8]));
    assert_eq!(report.size, Some(16));
    assert_eq!(report.align, 2);
}

#[test]
fn nested_fixed_primitive_arrays_remain_one_repeated_at_field() {
    let main_path = write_program(
        "nested-fixed-array-at",
        r#"
use omega::language::core::layout;

data ArrayLayout { }
machine ArrayLayout::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 8 },
    };
    Plan { entries: entries, entry_count: 1,
           size_fixed: 16, size_is_dynamic: false, align: 2 }
}

data Samples { values: [[u16; 2]; 2]; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("nested fixed array should reflect");
    let report = compute_layout_plan(&checked.typed, "ArrayLayout::plan", "Samples")
        .expect("one At placement should admit the complete nested-array extent");
    assert_eq!(report.offsets, Some(vec![8]));
    assert_eq!(report.size, Some(16));
    assert_eq!(report.align, 2);
}
