use super::{POLICY_SOURCE, corpus_source, write_cross_package_program, write_program};
use crate::fixture_roster;
use access_plans::{AccessExposure, ExternalRead, FieldAccess, ObservationModel};
use build_time_evaluation::{compute_access_plan, compute_layout_plan, compute_placement_plan};
use calling_conventions::{MachineRegister, ValueLocation};
use checked_trees_to_lowered_psi::lower_machine;
use compiler::{CheckedCompileRequest, compile_to_checked};
use language_core::ReferenceAccess;
use layout::{DataShape, build_layout_plan};
use std::fs;
use target::NativeTarget;

#[test]
fn compiler_accessor_templates_are_inert_without_placed_views() {
    let main = write_program(
        "no-placed-view",
        r#"
data Main {}
machine Main::main(&mut self) {}
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("ordinary program should ignore accessor templates");
    assert!(
        checked
            .typed
            .machines()
            .iter()
            .all(|machine| !machine.name.as_str().starts_with("PlacedField::")),
        "compiler-only accessor templates must not enter the typed program"
    );
}

#[test]
fn corpus_inaccessible_seed_exposes_only_the_selected_field() {
    let source = fs::read_to_string(corpus_source(fixture_roster::ACCESS_PLAN_INACCESSIBLE_SEED))
        .expect("read inaccessible-seed corpus source");
    // Supply geometry for the fixture's two u32 fields through the ordinary
    // source evaluator; access evaluation must replay that validated layout.
    let main = write_program(
        "corpus-inaccessible-seed",
        &format!(
            "{source}\n{}",
            r#"
data PairLayout { entries: [FieldEntry; 64]; }
machine PairLayout::plan(&mut self, schema: Schema) -> Plan {
    let mut owned_entries: [FieldEntry; 64];
    owned_entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 },
    };
    owned_entries[1] = FieldEntry {
        key: schema.fields[1].key,
        placement: FieldPlan::At { offset: 4 },
    };
    Plan { entries: owned_entries, entry_count: 2,
           size_fixed: 8, size_is_dynamic: false, align: 4 }
}
"#
        ),
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("inaccessible-seed source should check");
    let layout = compute_layout_plan(&checked.typed, "PairLayout::plan", "Pair", None)
        .expect("pair geometry should validate");
    let access = compute_access_plan(&checked.typed, "PairAccess::plan", "Pair", &layout)
        .expect("corpus access policy should evaluate");
    assert_eq!(access.plan().entries().len(), 2);
    let [readable] = access.field_descriptors() else {
        panic!("only the readable field should produce an access descriptor")
    };
    assert_eq!(readable.field(), "readable");
    assert_eq!(readable.transfer_width_bits(), 32);
    assert_eq!(readable.observation(), ObservationModel::Stable);
    assert!(readable.permissions().read);
    assert!(!readable.permissions().write);
    assert_eq!(readable.exposure(), AccessExposure::Exported);
    let hidden = access
        .plan()
        .entries()
        .iter()
        .find(|entry| entry.field() == "hidden")
        .expect("the hidden field retains its inaccessible decision");
    assert!(matches!(hidden.access(), FieldAccess::Inaccessible));
}

#[test]
fn corpus_placed_policy_records_reach_checked_trees() {
    compile_to_checked(CheckedCompileRequest::new(
        &corpus_source(fixture_roster::PLACED_POLICY_CORE_RECORDS),
        None,
    ))
    .expect("ordinary policy records should check without granting placement authority");
}

#[test]
fn source_access_policy_evaluates_against_validated_layout() {
    let main = write_program("source-access", POLICY_SOURCE);
    let checked = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("source policy should compile");
    let layout = compute_layout_plan(&checked.typed, "UartLayout::plan", "Registers", None)
        .expect("layout should validate before access evaluation");
    let access = compute_access_plan(&checked.typed, "UartAccess::plan", "Registers", &layout)
        .expect("source access policy should evaluate and normalize");

    assert_ne!(access.identity().compatibility_fingerprint(), 0);
    assert_eq!(access.field_descriptors().len(), 4);
    assert_eq!(access.plan().entries().len(), 5);
    assert!(matches!(
        access
            .plan()
            .entries()
            .iter()
            .find(|entry| entry.field() == "reserved")
            .expect("reserved source decision")
            .access(),
        FieldAccess::Inaccessible
    ));

    let status = access
        .field_descriptors()
        .iter()
        .find(|field| field.field() == "status")
        .expect("status descriptor");
    assert_eq!(status.transfer_width_bits(), 32);
    assert_eq!(status.observation(), ObservationModel::External);
    assert!(status.permissions().read);
    assert!(!status.permissions().write);

    let transmit = access
        .field_descriptors()
        .iter()
        .find(|field| field.field() == "transmit")
        .expect("transmit descriptor");
    assert_eq!(transmit.transfer_width_bits(), 8);
    assert_eq!(transmit.observation(), ObservationModel::External);
    assert!(!transmit.permissions().read);
    assert!(transmit.permissions().write);

    let snapshot = access
        .field_descriptors()
        .iter()
        .find(|field| field.field() == "snapshot")
        .expect("snapshot descriptor");
    assert_eq!(snapshot.transfer_width_bits(), 16);
    assert_eq!(snapshot.observation(), ObservationModel::Stable);
    assert_eq!(snapshot.exposure(), AccessExposure::BindingPrivate);

    let counter = access
        .field_descriptors()
        .iter()
        .find(|field| field.field() == "counter")
        .expect("counter descriptor");
    assert_eq!(counter.transfer_width_bits(), 64);
    assert_eq!(counter.observation(), ObservationModel::Atomic);
    assert!(counter.permissions().atomic.load);
    assert!(counter.permissions().atomic.fetch_add);
    assert!(!counter.permissions().atomic.store);

    assert!(matches!(
        access
            .plan()
            .entries()
            .iter()
            .find(|entry| entry.field() == "status")
            .expect("status source decision")
            .access(),
        FieldAccess::External {
            read: ExternalRead::Read,
            ..
        }
    ));
}

#[test]
fn numbered_access_policy_rejoins_a_retained_layout_after_field_rename() {
    let legacy = write_program(
        "numbered-access-legacy-layout",
        r#"
use omega::language::core::layout;

data RetainedLayout { entries: [FieldEntry; 64]; }
machine RetainedLayout::plan(&mut self, schema: Schema) -> Plan {
    let mut owned_entries: [FieldEntry; 64];
    owned_entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 },
    };
    Plan { entries: owned_entries, entry_count: 1,
           size_fixed: 4, size_is_dynamic: false, align: 4 }
}

data Registers { #7 legacy_status: u32; }
data Main {}
machine Main::main(&mut self) {}
"#,
    );
    let legacy = compile_to_checked(CheckedCompileRequest::new(&legacy, None))
        .expect("legacy numbered schema should check");
    let retained = compute_layout_plan(&legacy.typed, "RetainedLayout::plan", "Registers", None)
        .expect("legacy numbered layout should validate");
    assert_eq!(retained.entries[0].field, "legacy_status");
    assert_eq!(retained.entries[0].member_identity, Some(7));

    let renamed = write_program(
        "numbered-access-renamed-schema",
        r#"
use omega::language::core::layout;

data Registers { #7 status: u32; }
data RegisterAccess {}
machine RegisterAccess::plan(schema: Schema, layout: Plan) -> AccessPlan
satisfies Access::plan
{
    let plan: AccessPlan = AccessPlan::inaccessible(&schema);
    transition layout.entry_count == 1 && layout.size_fixed == 4 {
        true -> (plan.with(
            schema.fields[0].key,
            FieldAccess::Stable {
                read: true,
                write: false,
                exposure: Exposure::Exported,
            },
        ))
        _ -> (plan)
    }
}

data Main {}
machine Main::main(&mut self) {}
"#,
    );
    let renamed = compile_to_checked(CheckedCompileRequest::new(&renamed, None))
        .expect("renamed numbered schema should check");
    let access = compute_access_plan(
        &renamed.typed,
        "RegisterAccess::plan",
        "Registers",
        &retained,
    )
    .expect("stable identity should rejoin the retained layout for access evaluation");
    let [descriptor] = access.field_descriptors() else {
        panic!("one accessible field should produce one descriptor")
    };
    assert_eq!(descriptor.field(), "status");
    assert_eq!(descriptor.container_byte_offset(), 0);
    assert_eq!(descriptor.transfer_width_bits(), 32);
    assert_eq!(descriptor.observation(), ObservationModel::Stable);

    let mut drifted = retained.clone();
    drifted.entries[0].member_identity = Some(8);
    let error = compute_access_plan(
        &renamed.typed,
        "RegisterAccess::plan",
        "Registers",
        &drifted,
    )
    .expect_err("retained layout identity drift must reject before sealing access");
    assert!(
        error.contains("stable identity #8") && error.contains("outside the reflected schema"),
        "unexpected diagnostic: {error}"
    );

    let mut positional = retained.clone();
    positional.entries[0].member_identity = None;
    let error = compute_access_plan(
        &renamed.typed,
        "RegisterAccess::plan",
        "Registers",
        &positional,
    )
    .expect_err("an unnumbered spelling cannot claim the numbered field identity");
    assert!(
        error.contains("positional field `legacy_status`")
            && error.contains("outside the reflected schema"),
        "unexpected diagnostic: {error}"
    );

    let mut aliased = retained.clone();
    let mut forged_alias = aliased.entries[0].clone();
    forged_alias.field = "forged_alias".into();
    aliased.entries.push(forged_alias);
    let error = compute_access_plan(
        &renamed.typed,
        "RegisterAccess::plan",
        "Registers",
        &aliased,
    )
    .expect_err("one stable identity cannot be replayed under two presentation names");
    assert!(
        error.contains("not a canonical field-identity set")
            && error.contains("identity names both `legacy_status` and `forged_alias`"),
        "unexpected diagnostic: {error}"
    );

    let mut ambiguous = retained;
    let mut forged_identity = ambiguous.entries[0].clone();
    forged_identity.member_identity = Some(8);
    ambiguous.entries.push(forged_identity);
    let error = compute_access_plan(
        &renamed.typed,
        "RegisterAccess::plan",
        "Registers",
        &ambiguous,
    )
    .expect_err("one presentation name cannot replay two stable identities");
    assert!(
        error.contains("not a canonical field-identity set")
            && error.contains(
                "identifies both stable member identity #7 and stable member identity #8"
            ),
        "unexpected diagnostic: {error}"
    );
}

#[test]
fn source_placement_policy_normalizes_layout_access_and_reach_together() {
    let main = write_program("source-placement", POLICY_SOURCE);
    let checked = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("source policy should compile");
    let placement = compute_placement_plan(&checked.typed, "UartPlacement::plan", "Registers")
        .expect("source placement policy should evaluate and normalize");

    assert_ne!(placement.identity().compatibility_fingerprint(), 0);
    assert_eq!(placement.layout().size, Some(24));
    assert_eq!(placement.access().field_descriptors().len(), 4);
    assert_eq!(placement.reach().services().len(), 1);
    assert_eq!(
        placement
            .reach()
            .services()
            .next()
            .expect("service reach")
            .normalized_identity(),
        19
    );
}

#[test]
fn placed_view_exposes_derived_source_accessors() {
    let source = POLICY_SOURCE.replace(
        "data Main {}",
        r#"
machine inspect(view: &mut Placed<UartPlacement, Registers>) {
    let status: u32 = view.status.read();
    view.transmit.write(1);
    let snapshot: u16 = view.snapshot.read();
    view.snapshot.write(snapshot);
}

data Main {}
"#,
    );
    let main = write_program("placed-view-accessors", &source);
    let checked = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("derived placed-view accessors should compile");
    let status_read = checked
        .typed
        .machines()
        .iter()
        .find(|machine| {
            machine.name.as_str() == "PlacedField<UartPlacement,Registers,status>::read"
        })
        .expect("status read accessor");
    let conformances = checked.typed.machine_trait_conformances(status_read);
    assert_eq!(conformances.len(), 1);
    assert_eq!(conformances[0].name.as_str(), "Readable");
    assert_eq!(
        conformances[0]
            .requirement
            .as_ref()
            .expect("single readable requirement")
            .as_str(),
        "read"
    );

    let inspect = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .expect("placed-view consumer");
    let entry = checked
        .typed
        .machine_states(inspect)
        .first()
        .expect("placed-view consumer entry");
    let inputs = checked
        .facts
        .placed_view_inputs
        .iter()
        .filter(|input| input.machine == inspect.symbol)
        .collect::<Vec<_>>();
    let [input] = inputs.as_slice() else {
        panic!("one direct checked placed-view input")
    };
    let view = checked
        .typed
        .placed_view_plans
        .iter()
        .find(|view| view.policy_name == "UartPlacement")
        .expect("source-derived placed-view plan");
    assert_eq!(input.state, entry.symbol);
    assert_eq!(input.position, 0);
    assert_eq!(input.reference_access, ReferenceAccess::Mutable);
    assert_eq!(input.view, view.data_symbol);
    assert_eq!(input.policy, view.policy_symbol);
    assert_eq!(input.policy_plan_machine, view.policy_plan_machine_symbol);
    assert_eq!(input.schema, view.schema_symbol);
    assert_eq!(input.placement, view.placement);
}

#[test]
fn subordinate_placed_view_input_retains_exact_checked_state_custody() {
    let (main, inputs) = write_cross_package_program(
        "placed-view-subordinate-input",
        r#"
data Inspector {}
machine Inspector::inspect(
    &mut self,
    view: &mut Placed<UartPlacement, Registers>
) {
    state inspect_again(view: &mut Placed<UartPlacement, Registers>) {}
}
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&main, None)
    })
    .expect("subordinate placed-view input should compile to checked custody");
    let inspect = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Inspector::inspect")
        .expect("placed-view consumer");
    let [entry, subordinate] = checked.typed.machine_states(inspect) else {
        panic!("entry and subordinate state")
    };
    let inputs = checked
        .facts
        .placed_view_inputs
        .iter()
        .filter(|input| input.machine == inspect.symbol)
        .collect::<Vec<_>>();
    let [entry_input, subordinate_input] = inputs.as_slice() else {
        panic!("entry and subordinate checked placed-view inputs")
    };
    assert_eq!(entry_input.state, entry.symbol);
    assert_eq!(entry_input.position, 1);
    assert_eq!(subordinate_input.state, subordinate.symbol);
    assert_eq!(subordinate_input.position, 0);
    assert_ne!(entry_input.parameter, subordinate_input.parameter);
    assert_eq!(entry_input.reference_access, ReferenceAccess::Mutable);
    assert_eq!(subordinate_input.reference_access, ReferenceAccess::Mutable);
    assert_eq!(entry_input.view, subordinate_input.view);
    assert_eq!(entry_input.policy, subordinate_input.policy);
    assert_eq!(
        entry_input.policy_plan_machine,
        subordinate_input.policy_plan_machine
    );
    assert_eq!(entry_input.schema, subordinate_input.schema);
    assert_eq!(entry_input.placement, subordinate_input.placement);
}

#[test]
fn direct_placed_view_input_crosses_terminal_with_exact_source_custody() {
    let (main, inputs) = write_cross_package_program(
        "placed-view-terminal-input",
        r#"
data Sink {}
machine Sink::fill(destination: &write i32) {
    destination = 2;
}

data Inspector {}
machine Inspector::inspect(
    &mut self,
    view: &mut Placed<UartPlacement, Registers>,
    destination: &mut i32
) {
    Sink::fill(&write destination);
}
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&main, None)
    })
    .expect("direct placed-view input should compile");
    let inspect = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Inspector::inspect")
        .expect("placed-view consumer");
    let input = checked
        .facts
        .placed_view_inputs
        .iter()
        .find(|input| input.machine == inspect.symbol)
        .expect("checked placed-view input");
    let lowered = lower_machine(&checked, "Inspector::inspect")
        .expect("direct placed-view input should cross the Terminal boundary");
    let [terminal_input] = lowered.semantic_module.placed_view_inputs.as_slice() else {
        panic!("one direct Terminal placed-view input")
    };
    assert_eq!(terminal_input.machine, lowered.semantic_module.entry);
    assert_eq!(terminal_input.position, input.position);
    assert_eq!(
        terminal_input.access,
        terminal_psi::StructuralAccess::MutableBorrow
    );
    assert_eq!(
        terminal_input.source_machine_identity,
        checked
            .typed
            .normalized_hermetic_symbol_identity(input.machine)
            .unwrap()
    );
    assert_eq!(
        terminal_input.source_state_identity,
        checked
            .typed
            .normalized_hermetic_symbol_identity(input.state)
            .unwrap()
    );
    assert_eq!(
        terminal_input.source_parameter_identity,
        checked
            .typed
            .normalized_hermetic_symbol_identity(input.parameter)
            .unwrap()
    );
    assert_eq!(
        terminal_input.placement_report_fingerprint,
        input.placement.identity().compatibility_fingerprint()
    );
    assert_eq!(
        terminal_input.placement_commitment,
        input.placement.content_interpretation().commitment()
    );
    let encoded = terminal_codec::encode_module(&lowered.semantic_module)
        .expect("placed-view Terminal module should encode");
    assert_eq!(
        terminal_codec::decode_module(&encoded),
        Ok(lowered.semantic_module)
    );
}

#[test]
fn direct_placed_view_input_reaches_exact_dual_target_entry_abi() {
    let (main, inputs) = write_cross_package_program(
        "placed-view-target-entry",
        r#"
data Inspector {}
machine Inspector::inspect(
    &mut self,
    view: &mut Placed<UartPlacement, Registers>
) {}
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&main, None)
    })
    .expect("direct placed-view input should compile");
    let inspect = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Inspector::inspect")
        .expect("placed-view consumer");
    let source_input = checked
        .facts
        .placed_view_inputs
        .iter()
        .find(|input| input.machine == inspect.symbol)
        .expect("checked direct placed-view input");
    let lowered =
        lower_machine(&checked, "Inspector::inspect").expect("lower placed-view consumer");
    let semantic = terminal_codec::encode_module(&lowered.semantic_module)
        .expect("encode placed-view Terminal module");
    let proof =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .expect("encode placed-view proof bundle");

    assert!(matches!(
        terminal_psi_to_abstract_operations::lower_artifact(terminal_psi_to_abstract_operations::ArtifactSections { semantic_bytes: &semantic, proof_bytes: &proof, obligation_ledger_bytes: None }, &proof_admission::AdmissionProfile::default()).and_then(|admitted| admitted.try_into_plan()),
        Err(terminal_psi_to_abstract_operations::ArtifactLoweringError::PlacedViewInputsRequireCustodyLowering)
    ));
    let abstract_plan = terminal_psi_to_abstract_operations::lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .map(|admitted| admitted.into_parts())
    .expect("retain exact placed-view custody");
    let [terminal_input] = abstract_plan.placed_view_inputs.as_slice() else {
        panic!("one retained placed-view input")
    };
    assert_eq!(
        terminal_input,
        &lowered.semantic_module.placed_view_inputs[0]
    );
    let selections = [
        abstract_operations_to_target_operations::SelectedPlacedViewInputPlan {
            terminal_input,
            placement_plan: &source_input.placement,
        },
    ];

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_plan = abstract_operations_to_target_operations::
            lower_to_target_operations_with_placed_view_inputs(
                &abstract_plan,
                target,
                &selections,
            )
            .expect("lower exact placed-view target entry ABI");
        let [placed] = target_plan.placed_view_inputs.as_slice() else {
            panic!("one target placed-view input")
        };
        assert_eq!(placed.terminal, *terminal_input);
        assert_eq!(placed.abi_parameter_ordinal, 0);
        assert_eq!(placed.referent_byte_size, 24);
        assert_eq!(placed.referent_alignment, 8);
        assert_eq!(placed.placement, target_plan.entry_call_plan.parameters[0]);
        assert_eq!(
            placed.placement.shape.byte_size as usize,
            target.pointer_size
        );
        assert_eq!(
            placed.placement.shape.alignment as usize,
            target.pointer_alignment
        );

        let mut corrupted = target_plan.clone();
        corrupted.placed_view_inputs[0]
            .terminal
            .placement_commitment[0] ^= 1;
        assert_eq!(
            abstract_operations_to_target_operations::
                validate_placed_view_input_translation(
                    &abstract_plan,
                    &selections,
                    target,
                    &corrupted,
                ),
            Err(abstract_operations_to_target_operations::
                PlacedViewInputTranslationError::CandidateInputRosterMismatch)
        );

        let mut corrupted = target_plan.clone();
        corrupted.entry_call_plan.parameters[0].shape.byte_size = 4;
        assert_eq!(
            abstract_operations_to_target_operations::
                validate_placed_view_input_translation(
                    &abstract_plan,
                    &selections,
                    target,
                    &corrupted,
                ),
            Err(abstract_operations_to_target_operations::
                PlacedViewInputTranslationError::CandidateEntryCallPlanMismatch)
        );

        let other_target = if target == NativeTarget::linux_x64() {
            NativeTarget::linux_arm64()
        } else {
            NativeTarget::linux_x64()
        };
        assert_eq!(
            abstract_operations_to_target_operations::
                validate_placed_view_input_translation(
                    &abstract_plan,
                    &selections,
                    other_target,
                    &target_plan,
                ),
            Err(abstract_operations_to_target_operations::
                PlacedViewInputTranslationError::CandidatePlanMismatch)
        );
    }

    let mut drifted = abstract_plan.clone();
    drifted.placed_view_inputs[0].placement_report_fingerprint ^= 1;
    let drifted_selections = [
        abstract_operations_to_target_operations::SelectedPlacedViewInputPlan {
            terminal_input: &drifted.placed_view_inputs[0],
            placement_plan: &source_input.placement,
        },
    ];
    assert!(matches!(
        abstract_operations_to_target_operations::
            lower_to_target_operations_with_placed_view_inputs(
                &drifted,
                NativeTarget::linux_x64(),
                &drifted_selections,
            ),
        Err(abstract_operations_to_target_operations::LoweringError::PlacedViewInput(
            abstract_operations_to_target_operations::PlacedViewInputTranslationError::PlacementPlanIdentityMismatch
        ))
    ));
}

#[test]
fn direct_placed_view_input_survives_codec_and_native_replay() {
    let (main, inputs) = write_cross_package_program(
        "placed-view-replay",
        r#"
data Inspector {}
machine Inspector::inspect(
    &mut self,
    view: &mut Placed<UartPlacement, Registers>
) {}
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&main, None)
    })
    .expect("direct placed-view input should compile");
    let lowered =
        lower_machine(&checked, "Inspector::inspect").expect("lower placed-view consumer");
    let semantic = terminal_codec::encode_module(&lowered.semantic_module)
        .expect("encode placed-view Terminal module");
    let proof =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .expect("encode placed-view proof bundle");
    let profile = proof_admission::AdmissionProfile::default();
    let trust_graph =
        terminal_codec::current_terminal_trust_graph().expect("current terminal trust graph");
    let obligation_ledger =
        terminal_codec::build_terminal_obligation_ledger(&lowered.semantic_module, &trust_graph)
            .and_then(|ledger| terminal_codec::encode_terminal_obligation_ledger(&ledger))
            .expect("canonical obligation ledger for the placed-view module");

    // Admission retains the roster; extracting an input that cannot carry it fails closed.
    assert!(matches!(
        terminal_psi_to_abstract_operations::lower_artifact(terminal_psi_to_abstract_operations::ArtifactSections { semantic_bytes: &semantic, proof_bytes: &proof, obligation_ledger_bytes: Some(&obligation_ledger) }, &profile).and_then(|admitted| admitted.try_into_plan()),
        Err(terminal_psi_to_abstract_operations::ArtifactLoweringError::PlacedViewInputsRequireCustodyLowering)
    ));
    assert!(matches!(
        terminal_psi_to_abstract_operations::lower_artifact_for_native_realization(terminal_psi_to_abstract_operations::ArtifactSections { semantic_bytes: &semantic, proof_bytes: &proof, obligation_ledger_bytes: None }, &profile).and_then(|admitted| admitted.try_into_native_input()),
        Err(terminal_psi_to_abstract_operations::ArtifactLoweringError::PlacedViewInputsRequireCustodyLowering)
    ));
    // A successfully replayed ledger does not bypass the custody gate either.
    assert!(matches!(
        terminal_psi_to_abstract_operations::lower_artifact_for_native_realization(terminal_psi_to_abstract_operations::ArtifactSections { semantic_bytes: &semantic, proof_bytes: &proof, obligation_ledger_bytes: Some(&obligation_ledger) }, &profile).and_then(|admitted| admitted.try_into_native_input()),
        Err(terminal_psi_to_abstract_operations::ArtifactLoweringError::PlacedViewInputsRequireCustodyLowering)
    ));
    assert!(matches!(
        terminal_psi_to_abstract_operations::lower_artifact_for_optimization(terminal_psi_to_abstract_operations::ArtifactSections { semantic_bytes: &semantic, proof_bytes: &proof, obligation_ledger_bytes: None }, &profile).and_then(|admitted| admitted.try_into_optimization_input()),
        Err(terminal_psi_to_abstract_operations::ArtifactLoweringError::PlacedViewInputsRequireCustodyLowering)
    ));
    assert!(matches!(
        terminal_psi_to_abstract_operations::lower_artifact_for_optimization(terminal_psi_to_abstract_operations::ArtifactSections { semantic_bytes: &semantic, proof_bytes: &proof, obligation_ledger_bytes: Some(&obligation_ledger) }, &profile).and_then(|admitted| admitted.try_into_optimization_input()),
        Err(terminal_psi_to_abstract_operations::ArtifactLoweringError::PlacedViewInputsRequireCustodyLowering)
    ));

    // The owning entrances replay and admit the exact verified roster.
    let codec_plan = terminal_psi_to_abstract_operations::lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &profile,
    )
    .map(|admitted| admitted.into_parts())
    .expect("retain exact placed-view custody");
    let replayed = terminal_psi_to_abstract_operations::lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: Some(&obligation_ledger),
        },
        &profile,
    )
    .map(|admitted| admitted.into_parts())
    .expect("placed-view input survives codec replay");
    assert_eq!(replayed, codec_plan);
    assert_eq!(
        replayed.placed_view_inputs,
        lowered.semantic_module.placed_view_inputs
    );

    // The ordinary establishment route supplies the roster's custody at
    // interpretation: each direct-entry row joins exactly one establishment
    // lending the qualified referent backing for the invocation's duration.
    // The referent carrier stays opaque — Psi never interprets its type,
    // layout, or address — so the provider's structural runtime value is the
    // whole supply, and the entry machine completes with the input bound.
    let establishments = [terminal_interpreter::TerminalPlacedViewEstablishment {
        input: lowered.semantic_module.placed_view_inputs[0].clone(),
        referent: terminal_interpreter::TerminalStructuralValue {
            opaque_identity: 0x5a17,
            structural_type: semantic_vocabulary::StructuralTypeId::new(1)
                .expect("nonzero structural type"),
            qualifications: Vec::new(),
            path: Vec::new(),
        },
    }];
    let mut execution = terminal_interpreter::TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &profile,
        &[],
        terminal_interpreter::TerminalStructuralInputs {
            placed_view_establishments: &establishments,
            ..Default::default()
        },
    )
    .expect("an exact establishment binds the declared placed-view input");
    let mut meter = terminal_fuel::TerminalFuelMeter::unbounded();
    assert_eq!(
        execution
            .resume(&mut meter, &mut terminal_interpreter::AcceptTerminalEffects)
            .expect("the established view runs the entry to completion"),
        terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit
        )
    );
    // Completing the invocation retires the loan; the custody gate still
    // holds when no establishment answers the declared row, and a supply
    // whose roster row is stale — or that the roster never declared — rejects
    // before the entry machine runs.
    assert!(matches!(
        terminal_interpreter::TerminalExecution::start_artifact(
            &semantic,
            &proof,
            &profile,
            &[],
            Default::default(),
        ),
        Err(
            terminal_interpreter::TerminalArtifactInterpretError::Execution(
                terminal_interpreter::TerminalInterpretError::PlacedViewInputsRequireCustody
            )
        )
    ));
    let mut stale_supply = establishments[0].clone();
    stale_supply.input.placement_commitment[0] ^= 1;
    assert!(matches!(
        terminal_interpreter::TerminalExecution::start_artifact(
            &semantic,
            &proof,
            &profile,
            &[],
            terminal_interpreter::TerminalStructuralInputs {
                placed_view_establishments: &[stale_supply],
                ..Default::default()
            },
        ),
        Err(terminal_interpreter::TerminalArtifactInterpretError::Execution(
            terminal_interpreter::TerminalInterpretError::PlacedViewInputEstablishmentUnexpected { .. }
        ))
    ));

    let native = terminal_psi_to_abstract_operations::lower_artifact_for_native_realization(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &profile,
    )
    .expect("placed-view input survives native admission");
    assert_eq!(native.plan(), &codec_plan.plan);
    assert_eq!(
        native.placed_view_inputs(),
        lowered.semantic_module.placed_view_inputs.as_slice()
    );
    // Native admission replays the offered exact ledger before retaining the
    // roster, the same as the ordinary and optimizer entrances.
    let native_replayed =
        terminal_psi_to_abstract_operations::lower_artifact_for_native_realization(
            terminal_psi_to_abstract_operations::ArtifactSections {
                semantic_bytes: &semantic,
                proof_bytes: &proof,
                obligation_ledger_bytes: Some(&obligation_ledger),
            },
            &profile,
        )
        .expect("placed-view input survives native replay of the exact ledger");
    assert_eq!(native_replayed.plan(), native.plan());
    assert_eq!(
        native_replayed.placed_view_inputs(),
        native.placed_view_inputs()
    );

    // The executable input boundary owns the ordinary provider establishment
    // route: each declared direct-entry roster row joins exactly one supply —
    // the provider's loan of the exact qualified backing — bound at admission
    // so the realized entry boundary can lend it for the invocation's
    // duration. The bound set is invocation-scoped evidence; custody of the
    // supply stays with the provider.
    let native_established = native_replayed
        .try_into_native_input_with_placed_view_establishments(&establishments)
        .expect("exact provider establishments bind the declared roster");
    assert_eq!(native_established.plan(), &codec_plan.plan);
    assert_eq!(
        native_established.placed_view_establishments(),
        establishments.as_slice()
    );
    assert_eq!(
        native_established.context().module().placed_view_inputs,
        lowered.semantic_module.placed_view_inputs
    );
    // Optimizer authority remains recoverable; the invocation-scoped
    // establishment binding does not transfer into it.
    assert_eq!(
        native_established
            .into_optimization_input()
            .context()
            .module()
            .placed_view_inputs,
        lowered.semantic_module.placed_view_inputs
    );

    // Rejection legs at the executable boundary. An unanswered row keeps
    // failing custody exactly like the establishment-less entrance; a supply
    // answering no declared row — stale layout, qualified backing, range,
    // rights, or occurrence all move the sealed row identity — or one row
    // answered twice rejects before access, as does a noncanonical referent
    // qualification list.
    let readmit_native = || {
        terminal_psi_to_abstract_operations::lower_artifact_for_native_realization(
            terminal_psi_to_abstract_operations::ArtifactSections {
                semantic_bytes: &semantic,
                proof_bytes: &proof,
                obligation_ledger_bytes: None,
            },
            &profile,
        )
        .expect("native re-admission for the establishment rejection legs")
    };
    assert!(matches!(
        readmit_native().try_into_native_input_with_placed_view_establishments(&[]),
        Err(terminal_psi_to_abstract_operations::ArtifactLoweringError::PlacedViewInputsRequireCustodyLowering)
    ));
    let mut stale_native_supply = establishments[0].clone();
    stale_native_supply.input.placement_commitment[0] ^= 1;
    assert!(matches!(
        readmit_native()
            .try_into_native_input_with_placed_view_establishments(&[stale_native_supply]),
        Err(terminal_psi_to_abstract_operations::ArtifactLoweringError::PlacedViewEstablishmentUnexpected { .. })
    ));
    assert!(matches!(
        readmit_native().try_into_native_input_with_placed_view_establishments(&[
            establishments[0].clone(),
            establishments[0].clone()
        ]),
        Err(terminal_psi_to_abstract_operations::ArtifactLoweringError::PlacedViewEstablishmentDuplicate { .. })
    ));
    let mut noncanonical_supply = establishments[0].clone();
    noncanonical_supply.referent.qualifications = vec![
        semantic_vocabulary::StructuralDomainId::new(2).expect("nonzero domain"),
        semantic_vocabulary::StructuralDomainId::new(1).expect("nonzero domain"),
    ];
    assert!(matches!(
        readmit_native()
            .try_into_native_input_with_placed_view_establishments(&[noncanonical_supply]),
        Err(terminal_psi_to_abstract_operations::ArtifactLoweringError::PlacedViewEstablishmentQualificationsNonCanonical)
    ));

    let native_input = native.into_optimization_artifact();
    assert_eq!(native_input.plan(), &codec_plan.plan);
    assert_eq!(
        native_input.placed_view_inputs(),
        lowered.semantic_module.placed_view_inputs.as_slice()
    );
    assert_eq!(
        native_input.context().module().placed_view_inputs,
        lowered.semantic_module.placed_view_inputs
    );

    // The owning optimization entrances retain the same roster beside the
    // verified optimizer input. The roster rejoins the optimizer handoff as a
    // private-construction carrier, so a stale or substituted roster is
    // unrepresentable there rather than merely unchecked.
    let optimized = terminal_psi_to_abstract_operations::lower_artifact_for_optimization(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &profile,
    )
    .expect("placed-view input survives optimizer admission");
    assert_eq!(optimized.plan(), &codec_plan.plan);
    assert_eq!(
        optimized.placed_view_inputs(),
        lowered.semantic_module.placed_view_inputs.as_slice()
    );
    assert_eq!(optimized, native_input);
    let optimized_replayed = terminal_psi_to_abstract_operations::lower_artifact_for_optimization(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: Some(&obligation_ledger),
        },
        &profile,
    )
    .expect("placed-view input survives optimizer replay");
    assert_eq!(optimized_replayed, optimized);

    // A stale roster row still decodes and validates on its own bytes, but its
    // module fingerprint differs, so the original ledger cannot replay it.
    let mut stale = lowered.semantic_module.clone();
    stale.placed_view_inputs[0].placement_commitment[0] ^= 1;
    let stale_semantic =
        terminal_codec::encode_module(&stale).expect("stale roster still encodes canonically");
    let stale_ledger = terminal_codec::build_terminal_obligation_ledger(&stale, &trust_graph)
        .and_then(|ledger| terminal_codec::encode_terminal_obligation_ledger(&ledger))
        .expect("obligation ledger for the substituted roster");
    assert!(matches!(
        terminal_psi_to_abstract_operations::lower_artifact(
            terminal_psi_to_abstract_operations::ArtifactSections {
                semantic_bytes: &semantic,
                proof_bytes: &proof,
                obligation_ledger_bytes: Some(&stale_ledger)
            },
            &profile
        )
        .map(|admitted| admitted.into_parts()),
        Err(terminal_psi_to_abstract_operations::ArtifactLoweringError::ObligationReplay(_))
    ));
    assert!(matches!(
        terminal_psi_to_abstract_operations::lower_artifact_for_optimization(
            terminal_psi_to_abstract_operations::ArtifactSections {
                semantic_bytes: &semantic,
                proof_bytes: &proof,
                obligation_ledger_bytes: Some(&stale_ledger)
            },
            &profile
        ),
        Err(terminal_psi_to_abstract_operations::ArtifactLoweringError::ObligationReplay(_))
    ));
    assert!(matches!(
        terminal_psi_to_abstract_operations::lower_artifact_for_native_realization(
            terminal_psi_to_abstract_operations::ArtifactSections {
                semantic_bytes: &semantic,
                proof_bytes: &proof,
                obligation_ledger_bytes: Some(&stale_ledger)
            },
            &profile
        ),
        Err(terminal_psi_to_abstract_operations::ArtifactLoweringError::ObligationReplay(_))
    ));
    let stale_replayed = terminal_psi_to_abstract_operations::lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &stale_semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: Some(&stale_ledger),
        },
        &profile,
    )
    .map(|admitted| admitted.into_parts());
    assert!(stale_replayed.is_err() || stale_replayed.unwrap() != codec_plan);
    // At the native and optimizer entrances the substituted module replays its
    // own ledger but is rejected when the proof section's sealed subject names
    // the original module instead.
    assert!(matches!(
        terminal_psi_to_abstract_operations::lower_artifact_for_native_realization(
            terminal_psi_to_abstract_operations::ArtifactSections {
                semantic_bytes: &stale_semantic,
                proof_bytes: &proof,
                obligation_ledger_bytes: Some(&stale_ledger)
            },
            &profile
        ),
        Err(terminal_psi_to_abstract_operations::ArtifactLoweringError::ProofDecode(_))
    ));
    assert!(matches!(
        terminal_psi_to_abstract_operations::lower_artifact_for_optimization(
            terminal_psi_to_abstract_operations::ArtifactSections {
                semantic_bytes: &stale_semantic,
                proof_bytes: &proof,
                obligation_ledger_bytes: Some(&stale_ledger)
            },
            &profile
        ),
        Err(terminal_psi_to_abstract_operations::ArtifactLoweringError::ProofDecode(_))
    ));

    // An invalid roster row cannot reach replay at all: canonical encode
    // rejects it before an artifact exists.
    let mut invalid = lowered.semantic_module.clone();
    invalid.placed_view_inputs[0].placement_commitment = [0; 32];
    assert!(terminal_codec::encode_module(&invalid).is_err());

    // The roster reaches the native-realization optimization stage unchanged:
    // the artifact-sections entrance is custody-owning, so the validated
    // optimized plan keeps the exact rows inside its replay context beside
    // the unchanged abstract plan.
    let empty_selections =
        optimization_core::OptimizationSelections::new([]).expect("empty selections");
    let stage_optimized = native_realization::optimize_artifact_sections(
        &semantic,
        &proof,
        &profile,
        native_realization::compiler_baseline_request_v1(&empty_selections),
    )
    .expect("placed-view input survives the native-realization optimization stage");
    assert_eq!(stage_optimized.plan(), &codec_plan.plan);
    assert_eq!(
        stage_optimized
            .verified_input()
            .context()
            .module()
            .placed_view_inputs,
        lowered.semantic_module.placed_view_inputs
    );

    // The same stage accepts the custody-owning optimizer input moved out of
    // the native-admitted artifact: the exact handoff the realization
    // pipeline performs. The fail-closed extraction above remains the route
    // for consumers without custody support.
    let stage_input = native_input.into_optimization_input_with_placed_view_inputs();
    assert_eq!(
        stage_input.context().module().placed_view_inputs,
        lowered.semantic_module.placed_view_inputs
    );
    let stage_optimized = native_realization::optimize_verified_abstract_input(
        stage_input,
        native_realization::compiler_baseline_request_v1(&empty_selections),
    )
    .expect("custody-owning optimizer input reaches the optimization stage");
    assert_eq!(stage_optimized.plan(), &codec_plan.plan);
    assert_eq!(
        stage_optimized
            .verified_input()
            .context()
            .module()
            .placed_view_inputs,
        lowered.semantic_module.placed_view_inputs
    );

    // End-to-end native placement rejoins the optimized plan and its retained
    // roster through the exact placement-plan carrier. The derived entry ABI
    // is the only authority the roster participates in; it still grants no
    // backing, range, access, or lifetime authority by itself.
    let inspect = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Inspector::inspect")
        .expect("placed-view consumer");
    let source_input = checked
        .facts
        .placed_view_inputs
        .iter()
        .find(|input| input.machine == inspect.symbol)
        .expect("checked direct placed-view input");

    // Substitution controls for the plan identity rejoin. Each variant edits
    // one sealed axis inside `UartPlacement` — the referent extent (range),
    // one field's access grant (access), or the boundary service reach
    // (backing) — and still validates, so a rejection below is a stale-plan
    // substitution rejection rather than a malformed-input rejection.
    fn uart_placement_variant(edit: impl FnOnce(&mut String)) -> String {
        let marker = "pub data UartPlacement";
        let split = POLICY_SOURCE
            .find(marker)
            .expect("UartPlacement policy section");
        let mut policy = POLICY_SOURCE.to_string();
        let mut tail = policy.split_off(split);
        edit(&mut tail);
        policy.push_str(&tail);
        policy
    }
    fn variant_placement(name: &str, policy: &str) -> access_plans::ValidatedPlacementPlan {
        let main = write_program(name, policy);
        let checked = compile_to_checked(CheckedCompileRequest::new(&main, None))
            .expect("variant placement policy should compile");
        compute_placement_plan(&checked.typed, "UartPlacement::plan", "Registers")
            .expect("variant placement policy should still validate")
    }
    let stale_range_placement = variant_placement(
        "placed-view-stale-range",
        &uart_placement_variant(|tail| {
            *tail = tail.replacen("size_fixed: 24", "size_fixed: 32", 1);
        }),
    );
    let stale_access_placement = variant_placement(
        "placed-view-stale-access",
        &uart_placement_variant(|tail| {
            *tail = tail.replacen("write: false", "write: true", 1);
        }),
    );
    let stale_backing_placement = variant_placement(
        "placed-view-stale-backing",
        &uart_placement_variant(|tail| {
            *tail = tail.replacen("self.services[0] = 19;", "self.services[0] = 23;", 1);
        }),
    );
    for stale_plan in [
        &stale_range_placement,
        &stale_access_placement,
        &stale_backing_placement,
    ] {
        assert_ne!(
            stale_plan.content_interpretation().commitment(),
            source_input.placement.content_interpretation().commitment()
        );
    }

    let canonical_artifact = || {
        terminal_codec::CanonicalTerminalArtifact::from_parts(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &terminal_codec::build_identity_optimization_execution_record(
                &lowered.semantic_module,
                &lowered.proof_bundle,
            )
            .expect("identity optimization record for the placed-view module"),
            None,
        )
        .expect("canonical artifact for the placed-view module")
    };

    // The host leg below links the emitted fragment with a C driver, so it
    // runs only where the harness can execute the selected target's bytes.
    let host_target = if cfg!(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )) {
        Some(NativeTarget::host())
    } else {
        eprintln!(
            "skip: native placed-view execution needs a Linux x86-64/aarch64 or macOS aarch64 host"
        );
        None
    };

    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let mut placed = codec_plan.clone();
        placed.plan = stage_optimized.plan().clone();
        placed.placed_view_inputs = stage_optimized
            .verified_input()
            .context()
            .module()
            .placed_view_inputs
            .clone();
        let selections = [
            abstract_operations_to_target_operations::SelectedPlacedViewInputPlan {
                terminal_input: &placed.placed_view_inputs[0],
                placement_plan: &source_input.placement,
            },
        ];
        let target_plan = abstract_operations_to_target_operations::
            lower_to_target_operations_with_placed_view_inputs(&placed, target, &selections)
            .expect("optimized placed-view input reaches the placed-entry ABI");
        let [placed_input] = target_plan.placed_view_inputs.as_slice() else {
            panic!("one target placed-view input")
        };
        assert_eq!(placed_input.terminal, placed.placed_view_inputs[0]);
        assert_eq!(placed_input.abi_parameter_ordinal, 0);
        assert_eq!(placed_input.referent_byte_size, 24);
        assert_eq!(placed_input.referent_alignment, 8);
        assert_eq!(
            placed_input.placement,
            target_plan.entry_call_plan.parameters[0]
        );
        assert_eq!(
            placed_input.placement.shape.byte_size as usize,
            target.pointer_size
        );
        assert_eq!(
            placed_input.placement.shape.alignment as usize,
            target.pointer_alignment
        );
        assert_eq!(
            abstract_operations_to_target_operations::validate_placed_view_input_translation(
                &placed,
                &selections,
                target,
                &target_plan,
            ),
            Ok(())
        );

        // A stale placement-plan identity still rejects the exact join.
        let mut stale_plan = placed.clone();
        stale_plan.placed_view_inputs[0].placement_report_fingerprint ^= 1;
        let stale_selections = [
            abstract_operations_to_target_operations::SelectedPlacedViewInputPlan {
                terminal_input: &stale_plan.placed_view_inputs[0],
                placement_plan: &source_input.placement,
            },
        ];
        assert!(matches!(
            abstract_operations_to_target_operations::
                lower_to_target_operations_with_placed_view_inputs(
                    &stale_plan,
                    target,
                    &stale_selections,
                ),
            Err(abstract_operations_to_target_operations::LoweringError::PlacedViewInput(
                abstract_operations_to_target_operations::
                    PlacedViewInputTranslationError::PlacementPlanIdentityMismatch
            ))
        ));

        // A substituted roster inside an emitted candidate rejects at
        // translation validation.
        let mut corrupted = target_plan.clone();
        corrupted.placed_view_inputs[0]
            .terminal
            .placement_commitment[0] ^= 1;
        assert_eq!(
            abstract_operations_to_target_operations::
                validate_placed_view_input_translation(
                    &placed,
                    &selections,
                    target,
                    &corrupted,
                ),
            Err(abstract_operations_to_target_operations::
                PlacedViewInputTranslationError::CandidateInputRosterMismatch)
        );

        // A stale validated plan offered at the selection boundary rejects on
        // each sealed axis: substituting the referent range, the access
        // grants, or the backing service reach changes the plan's
        // compatibility fingerprint and content commitment, so the rejoin
        // fails before ABI derivation — at lowering and again at independent
        // translation validation.
        for stale_plan in [
            &stale_range_placement,
            &stale_access_placement,
            &stale_backing_placement,
        ] {
            let stale_selections = [
                abstract_operations_to_target_operations::SelectedPlacedViewInputPlan {
                    terminal_input: &placed.placed_view_inputs[0],
                    placement_plan: stale_plan,
                },
            ];
            assert!(matches!(
                abstract_operations_to_target_operations::
                    lower_to_target_operations_with_placed_view_inputs(
                        &placed,
                        target,
                        &stale_selections,
                    ),
                Err(abstract_operations_to_target_operations::
                    LoweringError::PlacedViewInput(
                        abstract_operations_to_target_operations::
                            PlacedViewInputTranslationError::PlacementPlanIdentityMismatch
                    ))
            ));
            assert_eq!(
                abstract_operations_to_target_operations::
                    validate_placed_view_input_translation(
                        &placed,
                        &stale_selections,
                        target,
                        &target_plan,
                    ),
                Err(abstract_operations_to_target_operations::
                    PlacedViewInputTranslationError::PlacementPlanIdentityMismatch)
            );
        }

        // The complete optimized plan reaches the physical stage, and the
        // emitted object rejoins the canonical Terminal artifact with the
        // roster still visible inside the joined module.
        let emit_object = || {
            let physical = native_realization::
                stage_optimized_verified_physical_pipeline_with_provider_executions(
                    native_realization::optimize_artifact_sections(
                        &semantic,
                        &proof,
                        &profile,
                        native_realization::compiler_baseline_request_v1(&empty_selections),
                    )
                    .expect("placed-view module re-optimizes for the physical stage"),
                    target,
                    &[],
                )
                .expect("placed-view module reaches physical custody");
            let emitted = machine_emission::stage_optimized_function_fragment_emission(
                physical.into_function_fragment_emission_source(),
            )
            .expect("function-fragment emission");
            let applied = machine_emission::stage_function_fragment_frame_application(emitted)
                .expect("frame application");
            let text = machine_emission::stage_optimized_fixed_frame_text_section(applied)
                .expect("text section");
            object_file::stage_optimized_relocation_free_object_container(text)
                .expect("relocation-free object container")
        };
        let staged = object_file::stage_validated_optimized_object_artifact(
            canonical_artifact(),
            emit_object(),
        )
        .expect("emitted object rejoins the canonical Terminal artifact");
        assert_eq!(
            object_file::validate_optimized_object_artifact(&staged),
            Ok(staged.custody())
        );
        assert_eq!(
            terminal_codec::decode_module(staged.terminal().semantic_bytes())
                .expect("staged terminal decodes")
                .placed_view_inputs,
            lowered.semantic_module.placed_view_inputs
        );

        // A stale Terminal artifact substituted beside an identically emitted
        // object fails the semantic join rather than replaying stale custody.
        let mut stale_module = lowered.semantic_module.clone();
        stale_module.placed_view_inputs[0].placement_commitment[0] ^= 1;
        let stale_artifact = terminal_codec::CanonicalTerminalArtifact::from_parts(
            &stale_module,
            &lowered.proof_bundle,
            &terminal_codec::build_identity_optimization_execution_record(
                &stale_module,
                &lowered.proof_bundle,
            )
            .expect("identity optimization record for the stale module"),
            None,
        )
        .expect("stale canonical artifact encodes");
        assert!(matches!(
            object_file::stage_validated_optimized_object_artifact(stale_artifact, emit_object(),),
            Err(object_file::OptimizedObjectArtifactError::InvalidTerminalArtifact)
                | Err(object_file::OptimizedObjectArtifactError::SemanticMismatch)
                | Err(object_file::OptimizedObjectArtifactError::ProofMismatch)
        ));

        // The derived placement is the target's first pointer argument, so a
        // C caller using the platform convention supplies the same register
        // the plan-laid ABI derived. Checking the register here ties the host
        // leg to the derived placement instead of assuming the convention.
        let expected_register = match target.architecture {
            target::Architecture::X86_64 => MachineRegister::X86Rdi,
            target::Architecture::Aarch64 => MachineRegister::Aarch64X(0),
        };
        assert_eq!(
            placed_input.placement.locations,
            vec![ValueLocation::Register {
                register: expected_register,
                value_byte_offset: 0,
                byte_size: u16::try_from(target.pointer_size).expect("pointer width fits"),
            }]
        );

        // End-to-end native placement on the host: the driver owns a referent
        // of exactly the validated geometry and lends its address through the
        // derived ABI. The fragment returns normally, the referent bytes and
        // the guards around them survive both calls, and the address is never
        // retained: the physical address exists only for the call and grants
        // no ownership of the referent. Backing, range, access, and lifetime
        // authority all stayed with the driver.
        if Some(target) == host_target {
            execute_placed_entry_with_host_referent(emit_object(), placed_input);
        }
    }

    // The image-emitting realization boundary stays fail-closed. The host leg
    // above lends the referent from a C caller; an executable image's entry
    // shim has no such caller, and the realization input does not yet carry
    // the bound provider establishments the admission boundary now supplies,
    // so executable realization rejects a nonempty roster instead of silently
    // erasing the declared input.
    let realization_error = native_realization::prepare_native_realization_input(
        &canonical_artifact(),
        &profile,
        &optimization_core::PostTerminalOptimizationSelections::default(),
    )
    .expect_err("executable realization still rejects plan-laid input custody");
    assert!(
        realization_error.iter().any(|diagnostic| diagnostic
            .message
            .contains("PlacedViewInputsRequireCustodyLowering")),
        "executable realization rejection names the custody boundary: {realization_error:?}"
    );
}

/// Link the emitted placed-entry fragment against a host driver that lends a
/// referent of the validated geometry for the duration of each call.
///
/// The referent size and alignment come from the validated
/// `TargetPlacedViewInput` row rather than from the source fixture, so the
/// driver cannot agree with the fragment by accident: a substituted plan that
/// survived to this point would change the geometry the driver allocates.
fn execute_placed_entry_with_host_referent(
    container: object_file::StagedOptimizedRelocationFreeObjectContainer,
    placed_input: &target_operations::TargetPlacedViewInput,
) {
    let source = std::sync::Arc::new(container);
    let object = image_emission::build_function_fragment_object_artifact(source.clone())
        .expect("placed-entry fragment publishes as an object artifact");
    image_emission::validate_function_fragment_object_artifact(&source, &object)
        .expect("published placed-entry object replays its fragment source");
    let entry_offset = object.entry_function().text_offset;
    let size = placed_input.referent_byte_size;
    let alignment = placed_input.referent_alignment;
    let driver = format!(
        r#"#include <stdint.h>
#include <string.h>

extern void omega_entry(void *view);

int main(void) {{
    struct {{
        uint64_t before;
        _Alignas({alignment}) uint8_t referent[{size}];
        uint64_t after;
    }} image;
    uint8_t expected[{size}];
    const uint64_t guard = UINT64_C(0x5a5a5a5a5a5a5a5a);
    unsigned index;

    for (index = 0; index < {size}; ++index) {{
        expected[index] = (uint8_t)(0xc0 + index);
    }}
    image.before = guard;
    image.after = guard;
    memcpy(image.referent, expected, {size});
    if (((uintptr_t)image.referent) % {alignment}) return 4;
    omega_entry(image.referent);
    omega_entry(image.referent);
    if (memcmp(image.referent, expected, {size})) return 1;
    if (image.before != guard || image.after != guard) return 2;
    return 0;
}}
"#
    );
    crate::native_function::assert_c_text(object.text_bytes(), entry_offset, &driver);
}

#[test]
fn placed_view_input_custody_excludes_open_and_nonchecked_machines() {
    let source = POLICY_SOURCE.replace(
        "data Main {}",
        r#"
machine generic_inspect<Element>(view: &Placed<UartPlacement, Registers>) {}
machine lifetime_inspect<'view>(view: &'view Placed<UartPlacement, Registers>) {}
boundary machine boundary_inspect(view: &Placed<UartPlacement, Registers>);

data Main {}
"#,
    );
    let main = write_program("placed-view-input-fences", &source);
    let checked = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("open and non-checked placed-view declarations should remain fenced");
    for name in ["generic_inspect", "lifetime_inspect", "boundary_inspect"] {
        let machine = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .expect("fenced placed-view declaration");
        assert!(
            checked
                .facts
                .placed_view_inputs
                .iter()
                .all(|input| input.machine != machine.symbol),
            "{name} must not gain checked placed-view input custody"
        );
    }
}

#[test]
fn compiler_derived_placed_accessors_retain_runtime_addresses() {
    let source = POLICY_SOURCE.replace(
        "data Main {}",
        r#"
machine inspect(view: &mut Placed<UartPlacement, Registers>) {
    let status: u32 = view.status.read();
    view.transmit.write(1);
    let snapshot: u16 = view.snapshot.read();
    view.snapshot.write(snapshot);
}

data Main {}
"#,
    );
    let main = write_program("placed-accessor-runtime-layout", &source);
    let checked = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("derived placed accessors should compile");
    let [view] = checked.typed.placed_view_plans.as_slice() else {
        panic!("fixture should derive exactly one placed view")
    };
    assert_eq!(view.fields.len(), 4);
    assert!(
        view.fields
            .iter()
            .all(|field| field.field_name != "reserved")
    );

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let layouts = build_layout_plan(&checked, target, &[]).expect("placed layout should build");
        for field in &view.fields {
            let accessor = layouts
                .data_layouts
                .iter()
                .map(|(_, layout)| layout)
                .find(|layout| layout.name.as_str() == field.accessor_name)
                .unwrap_or_else(|| {
                    panic!(
                        "accessor `{}` is missing from runtime layouts {:?}",
                        field.accessor_name,
                        layouts
                            .data_layouts
                            .iter()
                            .map(|(_, layout)| layout.name.as_str())
                            .collect::<Vec<_>>()
                    )
                });
            assert_eq!(accessor.layout.size, target.pointer_size);
            assert_eq!(accessor.layout.alignment, target.pointer_alignment);
            assert!(
                matches!(&accessor.shape, DataShape::Record { fields } if fields.is_empty()),
                "the address carrier must remain opaque rather than expose source fields"
            );
        }

        let placed = layouts
            .data_layouts
            .iter()
            .map(|(_, layout)| layout)
            .find(|layout| layout.name.as_str() == view.data_name)
            .expect("derived Placed record should have a runtime layout");
        let DataShape::Record { fields } = &placed.shape else {
            panic!("derived Placed data should remain a record")
        };
        let fields = layouts.fields.span_or_empty(*fields);
        assert_eq!(fields.len(), view.fields.len());
        assert_eq!(
            placed.layout.size,
            target.pointer_size * view.fields.len(),
            "one exact address carrier is retained for each admitted field"
        );
        assert_eq!(placed.layout.alignment, target.pointer_alignment);
        assert!(fields.iter().all(|field| field.name.as_str() != "reserved"));
        for field in fields {
            let expected = view
                .fields
                .iter()
                .find(|expected| expected.field_name == field.name.as_str())
                .expect("every runtime field should come from the exact placed-view plan");
            assert_eq!(field.type_name.as_ref(), expected.accessor_name);
            assert_eq!(field.layout.size, target.pointer_size);
            assert_eq!(field.layout.alignment, target.pointer_alignment);
        }
    }
}
