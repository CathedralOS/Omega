use super::{lower_writer_on_both_linux_isas, write_program};
use build_time_evaluation::compute_layout_plan;
use compiler::{CheckedCompileRequest, compile_to_checked};
use layout::build_layout_plan;
use layout_plans::{
    ByteOrder, ConsumptionInstant, ConventionalRecursiveRecordSumPathsLayoutReport, DataSymbolId,
    EntryStubId, LayoutPlacementReport, MaterializationAction, MaterializationContext,
    RelocationTarget, SymbolicFieldInnerLayout, SymbolicFieldPathSegment, SymbolicFieldValue,
    derive_symbolic_materialization, derive_symbolic_materialization_with_inner_layouts,
};
use target::NativeTarget;

#[test]
fn indexed_symbolic_materialization_preserves_the_exact_element_path() {
    // One nested field/index case, end to end: `handlers[2]` is a field/index
    // path into a repeated field. The symbolic value preserves the exact index
    // until materialization assigns element 2's `At` offset; the post-handoff
    // writer then realizes that offset without changing which element is
    // accessed. The bound `index < element count` is checked during derivation,
    // before any destination byte offset is chosen.
    let main_path = write_program(
        "indexed-symbolic-field",
        r#"
use omega::language::core::layout;

data DispatchLayout { }
machine DispatchLayout::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 },
    };
    entries[1] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 8 },
    };
    entries[2] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 16 },
    };
    entries[3] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 24 },
    };
    Plan { entries: entries, entry_count: 4,
           size_fixed: 32, size_is_dynamic: false, align: 8 }
}
data DispatchTable { handlers: [u64; 4]; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("indexed dispatch table should check");
    let report = compute_layout_plan(
        &checked.typed,
        "DispatchLayout::plan",
        "DispatchTable",
        None,
    )
    .expect("one element At per fixed-array element should validate");
    assert_eq!(
        report
            .entries
            .iter()
            .map(|entry| match entry.placement {
                LayoutPlacementReport::At { offset } => offset,
                _ => panic!("a repeated field retains only element At entries"),
            })
            .collect::<Vec<_>>(),
        vec![0, 8, 16, 24]
    );
    assert!(
        report.entries.iter().all(|entry| entry.field == "handlers"),
        "the repeated field retains one name across element placements"
    );

    let target = RelocationTarget::Entry(
        EntryStubId::from_normalized_identity(0x55aa).expect("normalized entry identity"),
    );
    let symbolic = SymbolicFieldValue::new_indexed("handlers", 2, 64, target)
        .expect("indexed symbolic field/index path");
    let materialization = derive_symbolic_materialization(
        &report,
        &[symbolic],
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: Some(64),
            placement: layout_plans::PlacementConstraints::unconstrained(
                layout_plans::PlacementPhase::PostHandoff,
            ),
        },
        |_| None,
    )
    .expect("the indexed field/index path derives one element write");
    assert_eq!(materialization.actions.len(), 1);
    let MaterializationAction::RuntimeWriter(write) = &materialization.actions[0] else {
        panic!("an unresolved indexed symbolic derives a post-handoff writer");
    };
    assert_eq!(write.container_byte_offset, 16);

    let writer = materialization
        .derive_post_handoff_writer()
        .expect("the element write derives a writer");
    let mut bytes = [0xa5_u8; 32];
    writer
        .execute(
            &mut bytes,
            layout_plans::PlacementSite {
                base_address: 0,
                phase: layout_plans::PlacementPhase::PostHandoff,
                machine_regime: None,
                installation_scope: None,
            },
            |resolved| {
                assert_eq!(resolved, target);
                Some(0x1122_3344_5566_7788)
            },
        )
        .expect("the indexed writer resolves the exact element");
    assert_eq!(&bytes[16..24], &0x1122_3344_5566_7788_u64.to_le_bytes());
    assert!(
        bytes[..16]
            .iter()
            .chain(&bytes[24..])
            .all(|byte| *byte == 0xa5),
        "index materialization writes only the addressed element"
    );
    lower_writer_on_both_linux_isas(&writer, 0xa5, &bytes, |resolved| {
        assert_eq!(resolved, target);
        0x1122_3344_5566_7788
    });
}

#[test]
fn nested_symbolic_materialization_preserves_the_exact_member_path() {
    // One nested record path, end to end: `slot.entry` is a field/field path
    // into the record stored in `slot`. The flat outer plan places `slot` as
    // one whole `At` extent; the record's member offsets are interior geometry
    // the outer plan deliberately does not carry, so a
    // `SymbolicFieldInnerLayout` carrier binds a second validated plan to the
    // `slot` field. The symbolic value preserves the exact `slot.entry` path
    // until derivation composes the outer offset with the member's inner
    // offset; the write then realizes byte 8 of the 24-byte outer object.
    let main_path = write_program(
        "nested-symbolic-field",
        r#"
use omega::language::core::layout;

data DispatchLayout { }
machine DispatchLayout::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 },
    };
    entries[1] = FieldEntry {
        key: schema.fields[1].key,
        placement: FieldPlan::At { offset: 8 },
    };
    Plan { entries: entries, entry_count: 2,
           size_fixed: 24, size_is_dynamic: false, align: 8 }
}

data SlotLayout { }
machine SlotLayout::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 },
    };
    entries[1] = FieldEntry {
        key: schema.fields[1].key,
        placement: FieldPlan::At { offset: 8 },
    };
    Plan { entries: entries, entry_count: 2,
           size_fixed: 16, size_is_dynamic: false, align: 8 }
}

data DispatchSlot { entry: u64; flags: u64; }
data DispatchTable { header: u64; slot: DispatchSlot; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("nested dispatch table should check");
    let report = compute_layout_plan(
        &checked.typed,
        "DispatchLayout::plan",
        "DispatchTable",
        None,
    )
    .expect("a nested record field placed as one At extent should validate");
    assert_eq!(
        report
            .entries
            .iter()
            .map(|entry| (entry.field.as_str(), entry.placement))
            .collect::<Vec<_>>(),
        vec![
            ("header", LayoutPlacementReport::At { offset: 0 }),
            ("slot", LayoutPlacementReport::At { offset: 8 }),
        ]
    );
    let inner_report =
        compute_layout_plan(&checked.typed, "SlotLayout::plan", "DispatchSlot", None)
            .expect("the record's own policy supplies its interior geometry");
    let inner = SymbolicFieldInnerLayout::new("slot", inner_report);

    let target = RelocationTarget::Entry(
        EntryStubId::from_normalized_identity(0x55aa).expect("normalized entry identity"),
    );
    let symbolic = SymbolicFieldValue::new("slot", 64, target)
        .expect("nested symbolic field path")
        .with_inner_segment(SymbolicFieldPathSegment::new("entry"));
    let materialization = derive_symbolic_materialization_with_inner_layouts(
        &report,
        std::slice::from_ref(&inner),
        std::slice::from_ref(&symbolic),
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: Some(64),
            placement: layout_plans::PlacementConstraints::unconstrained(
                layout_plans::PlacementPhase::PostHandoff,
            ),
        },
        |_| None,
    )
    .expect("the nested member path composes outer and interior offsets");
    assert_eq!(materialization.actions.len(), 1);
    let MaterializationAction::RuntimeWriter(write) = &materialization.actions[0] else {
        panic!("an unresolved nested symbolic derives a post-handoff writer");
    };
    assert_eq!(write.field, "slot.entry");
    assert_eq!(write.container_byte_offset, 8);

    let writer = materialization
        .derive_post_handoff_writer()
        .expect("the member write derives a writer");
    let mut bytes = [0xa5_u8; 24];
    writer
        .execute(
            &mut bytes,
            layout_plans::PlacementSite {
                base_address: 0,
                phase: layout_plans::PlacementPhase::PostHandoff,
                machine_regime: None,
                installation_scope: None,
            },
            |resolved| {
                assert_eq!(resolved, target);
                Some(0x1122_3344_5566_7788)
            },
        )
        .expect("the nested writer resolves the exact member");
    assert_eq!(&bytes[8..16], &0x1122_3344_5566_7788_u64.to_le_bytes());
    assert!(
        bytes[..8]
            .iter()
            .chain(&bytes[16..])
            .all(|byte| *byte == 0xa5),
        "nested materialization writes only the addressed member"
    );
    lower_writer_on_both_linux_isas(&writer, 0xa5, &bytes, |resolved| {
        assert_eq!(resolved, target);
        0x1122_3344_5566_7788
    });
}

#[test]
fn nested_indexed_symbolic_materialization_realizes_on_both_linux_isas() {
    // One nested field/index path, end to end: `slots[1].flags` selects the
    // `flags` member of element 1 of the repeated `slots` record field, and
    // `slots[0].entries[1]` carries a second index hop inside the same nested
    // element. The flat outer plan places `slots` as one `At` extent per
    // element; the `SymbolicFieldInnerLayout` carrier supplies the record's
    // compiler-derived interior plan so each exact path stays symbolic until
    // derivation composes the element offset with the interior offset. Both
    // writes then realize through one post-handoff writer, which physical
    // lowering emits for each Linux ISA without changing the semantic slots.
    let main_path = write_program(
        "nested-indexed-symbolic-field",
        r#"
use omega::language::core::layout;

data DispatchLayout { }
machine DispatchLayout::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 },
    };
    entries[1] = FieldEntry {
        key: schema.fields[1].key,
        placement: FieldPlan::At { offset: 8 },
    };
    entries[2] = FieldEntry {
        key: schema.fields[1].key,
        placement: FieldPlan::At { offset: 32 },
    };
    Plan { entries: entries, entry_count: 3,
           size_fixed: 56, size_is_dynamic: false, align: 8 }
}

data SlotLayout { }
machine SlotLayout::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 },
    };
    entries[1] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 8 },
    };
    entries[2] = FieldEntry {
        key: schema.fields[1].key,
        placement: FieldPlan::At { offset: 16 },
    };
    Plan { entries: entries, entry_count: 3,
           size_fixed: 24, size_is_dynamic: false, align: 8 }
}

data DispatchSlot { entries: [u64; 2]; flags: u64; }
data DispatchTable { header: u64; slots: [DispatchSlot; 2]; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("nested indexed dispatch table should check");
    let report = compute_layout_plan(
        &checked.typed,
        "DispatchLayout::plan",
        "DispatchTable",
        None,
    )
    .expect("a repeated record field retains one element At per element");
    assert_eq!(
        report
            .entries
            .iter()
            .map(|entry| (entry.field.as_str(), entry.placement))
            .collect::<Vec<_>>(),
        vec![
            ("header", LayoutPlacementReport::At { offset: 0 }),
            ("slots", LayoutPlacementReport::At { offset: 8 }),
            ("slots", LayoutPlacementReport::At { offset: 32 }),
        ]
    );
    let inner_report =
        compute_layout_plan(&checked.typed, "SlotLayout::plan", "DispatchSlot", None)
            .expect("the record's own policy supplies its interior geometry");
    assert_eq!(
        inner_report
            .entries
            .iter()
            .map(|entry| (entry.field.as_str(), entry.placement))
            .collect::<Vec<_>>(),
        vec![
            ("entries", LayoutPlacementReport::At { offset: 0 }),
            ("entries", LayoutPlacementReport::At { offset: 8 }),
            ("flags", LayoutPlacementReport::At { offset: 16 }),
        ]
    );
    let inner = SymbolicFieldInnerLayout::new("slots", inner_report);

    let entry_target = RelocationTarget::Entry(
        EntryStubId::from_normalized_identity(0x55aa).expect("normalized entry identity"),
    );
    let data_target = RelocationTarget::Data(
        DataSymbolId::from_normalized_identity(0x5a5a).expect("normalized data identity"),
    );
    let symbolic = [
        SymbolicFieldValue::new_indexed("slots", 1, 64, entry_target)
            .expect("indexed outer record element")
            .with_inner_segment(SymbolicFieldPathSegment::new("flags")),
        SymbolicFieldValue::new_indexed("slots", 0, 64, data_target)
            .expect("indexed outer record element")
            .with_inner_segment(SymbolicFieldPathSegment::new_indexed("entries", 1)),
    ];
    let materialization = derive_symbolic_materialization_with_inner_layouts(
        &report,
        std::slice::from_ref(&inner),
        &symbolic,
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: Some(64),
            placement: layout_plans::PlacementConstraints::unconstrained(
                layout_plans::PlacementPhase::PostHandoff,
            ),
        },
        |_| None,
    )
    .expect("the nested field/index paths compose outer and interior offsets");
    let writes = materialization
        .actions
        .iter()
        .map(|action| match action {
            MaterializationAction::RuntimeWriter(write) => {
                (write.field.as_str(), write.container_byte_offset)
            }
            other => panic!("unresolved nested index paths derive writers, found {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        writes,
        [("slots[1].flags", 48), ("slots[0].entries[1]", 16)]
    );

    let writer = materialization
        .derive_post_handoff_writer()
        .expect("the nested index writes derive a writer");
    let mut bytes = [0xa5_u8; 56];
    writer
        .execute(
            &mut bytes,
            layout_plans::PlacementSite {
                base_address: 0,
                phase: layout_plans::PlacementPhase::PostHandoff,
                machine_regime: None,
                installation_scope: None,
            },
            |resolved| {
                if resolved == entry_target {
                    Some(0x1122_3344_5566_7788)
                } else {
                    assert_eq!(resolved, data_target);
                    Some(0x99aa_bbcc_ddee_ff00)
                }
            },
        )
        .expect("the nested index writer resolves each exact slot");
    assert_eq!(&bytes[48..56], &0x1122_3344_5566_7788_u64.to_le_bytes());
    assert_eq!(&bytes[16..24], &0x99aa_bbcc_ddee_ff00_u64.to_le_bytes());
    assert!(
        bytes[..16]
            .iter()
            .chain(&bytes[24..48])
            .all(|byte| *byte == 0xa5),
        "nested index materialization writes only the addressed member slots"
    );
    lower_writer_on_both_linux_isas(&writer, 0xa5, &bytes, |resolved| {
        if resolved == entry_target {
            0x1122_3344_5566_7788
        } else {
            assert_eq!(resolved, data_target);
            0x99aa_bbcc_ddee_ff00
        }
    });
}

#[test]
fn deeply_nested_symbolic_materialization_preserves_the_exact_path() {
    // One three-segment record path, end to end: `slot.inner.entry` crosses
    // two record boundaries. The flat outer plan places `slot` as one whole
    // `At` extent; `slot`'s carrier retains the record's own validated plan
    // and binds `inner`'s interior under it, so record depth is data in the
    // carrier tree rather than another derivation. Derivation composes each
    // crossed boundary's `At` offset; the writes realize bytes 8, 16, and 24
    // of the 40-byte outer object and lower to each Linux ISA without
    // changing which semantic slots they address.
    let main_path = write_program(
        "deeply-nested-symbolic-field",
        r#"
use omega::language::core::layout;

data DispatchLayout { }
machine DispatchLayout::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 },
    };
    entries[1] = FieldEntry {
        key: schema.fields[1].key,
        placement: FieldPlan::At { offset: 8 },
    };
    Plan { entries: entries, entry_count: 2,
           size_fixed: 40, size_is_dynamic: false, align: 8 }
}

data SlotLayout { }
machine SlotLayout::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 },
    };
    entries[1] = FieldEntry {
        key: schema.fields[1].key,
        placement: FieldPlan::At { offset: 16 },
    };
    entries[2] = FieldEntry {
        key: schema.fields[2].key,
        placement: FieldPlan::At { offset: 24 },
    };
    Plan { entries: entries, entry_count: 3,
           size_fixed: 32, size_is_dynamic: false, align: 8 }
}

data InnerLayout { }
machine InnerLayout::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 },
    };
    entries[1] = FieldEntry {
        key: schema.fields[1].key,
        placement: FieldPlan::At { offset: 8 },
    };
    Plan { entries: entries, entry_count: 2,
           size_fixed: 16, size_is_dynamic: false, align: 8 }
}

data DispatchInner { entry: u64; args: u64; }
data DispatchSlot { inner: DispatchInner; flags: u64; pad: u64; }
data DispatchTable { header: u64; slot: DispatchSlot; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("deeply nested dispatch table should check");
    let report = compute_layout_plan(
        &checked.typed,
        "DispatchLayout::plan",
        "DispatchTable",
        None,
    )
    .expect("a nested record field placed as one At extent should validate");
    assert_eq!(
        report
            .entries
            .iter()
            .map(|entry| (entry.field.as_str(), entry.placement))
            .collect::<Vec<_>>(),
        vec![
            ("header", LayoutPlacementReport::At { offset: 0 }),
            ("slot", LayoutPlacementReport::At { offset: 8 }),
        ]
    );
    let slot_report = compute_layout_plan(&checked.typed, "SlotLayout::plan", "DispatchSlot", None)
        .expect("the slot record's own policy supplies its interior geometry");
    let inner_report =
        compute_layout_plan(&checked.typed, "InnerLayout::plan", "DispatchInner", None)
            .expect("the inner record's own policy supplies its interior geometry");
    let inner = SymbolicFieldInnerLayout::new("slot", slot_report)
        .with_inner_layout(SymbolicFieldInnerLayout::new("inner", inner_report));

    let entry_target = RelocationTarget::Entry(
        EntryStubId::from_normalized_identity(0x55aa).expect("normalized entry identity"),
    );
    let data_target = RelocationTarget::Data(
        DataSymbolId::from_normalized_identity(0x5a5a).expect("normalized data identity"),
    );
    let symbolic = [
        SymbolicFieldValue::new("slot", 64, entry_target)
            .expect("outer record field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new("inner")
                    .with_inner_segment(SymbolicFieldPathSegment::new("entry")),
            ),
        SymbolicFieldValue::new("slot", 64, data_target)
            .expect("outer record field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new("inner")
                    .with_inner_segment(SymbolicFieldPathSegment::new("args")),
            ),
        SymbolicFieldValue::new("slot", 64, data_target)
            .expect("outer record field")
            .with_inner_segment(SymbolicFieldPathSegment::new("flags")),
    ];
    let materialization = derive_symbolic_materialization_with_inner_layouts(
        &report,
        std::slice::from_ref(&inner),
        &symbolic,
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: Some(64),
            placement: layout_plans::PlacementConstraints::unconstrained(
                layout_plans::PlacementPhase::PostHandoff,
            ),
        },
        |_| None,
    )
    .expect("the deeper member paths compose every crossed boundary offset");
    let writes = materialization
        .actions
        .iter()
        .map(|action| match action {
            MaterializationAction::RuntimeWriter(write) => {
                (write.field.as_str(), write.container_byte_offset)
            }
            other => panic!("unresolved deeper paths derive writers, found {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        writes,
        [
            ("slot.inner.entry", 8),
            ("slot.inner.args", 16),
            ("slot.flags", 24),
        ]
    );

    let writer = materialization
        .derive_post_handoff_writer()
        .expect("the deeper member writes derive a writer");
    let mut bytes = [0xa5_u8; 40];
    writer
        .execute(
            &mut bytes,
            layout_plans::PlacementSite {
                base_address: 0,
                phase: layout_plans::PlacementPhase::PostHandoff,
                machine_regime: None,
                installation_scope: None,
            },
            |resolved| {
                if resolved == entry_target {
                    Some(0x1122_3344_5566_7788)
                } else {
                    assert_eq!(resolved, data_target);
                    Some(0x99aa_bbcc_ddee_ff00)
                }
            },
        )
        .expect("the deeper writer resolves each exact slot");
    assert_eq!(&bytes[8..16], &0x1122_3344_5566_7788_u64.to_le_bytes());
    assert_eq!(&bytes[16..24], &0x99aa_bbcc_ddee_ff00_u64.to_le_bytes());
    assert_eq!(&bytes[24..32], &0x99aa_bbcc_ddee_ff00_u64.to_le_bytes());
    assert!(
        bytes[..8]
            .iter()
            .chain(&bytes[32..])
            .all(|byte| *byte == 0xa5),
        "deeper materialization writes only the addressed member slots"
    );
    lower_writer_on_both_linux_isas(&writer, 0xa5, &bytes, |resolved| {
        if resolved == entry_target {
            0x1122_3344_5566_7788
        } else {
            assert_eq!(resolved, data_target);
            0x99aa_bbcc_ddee_ff00
        }
    });
}

#[test]
fn direct_sum_symbolic_materialization_realizes_on_both_linux_isas() {
    // Direct-sum coexistence, end to end: `choice.Run.callback` is a symbolic
    // field path whose last two hops spell the selected case and that case's
    // payload field inside the conventional tag-prefixed overlay. The exact
    // case and member stay symbolic until materialization assigns the payload
    // slot's byte offset; the sum's tag and the inactive cases' bytes remain
    // staged content the writer never emits, and scalar fields on the same
    // record coexist through the same writer.
    let main_path = write_program(
        "direct-sum-symbolic-field",
        r#"
data Choice [copy] {
    case Empty;
    case Run(callback: u64, clock: u64);
    case Pair(left: u64);
}
data Dispatch [copy] {
    header: u64;
    choice: Choice;
    tail: u64;
}
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("a direct sum record should check");
    let plan = build_layout_plan(&checked, NativeTarget::linux_x64(), &[])
        .expect("the direct sum record should lay out");
    let arm_plan = build_layout_plan(&checked, NativeTarget::linux_arm64(), &[])
        .expect("the direct sum record should lay out for linux_arm64");
    let owner = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Dispatch")
        .expect("the dispatch record");
    let (report, sum_fields) = layout::project_conventional_record_with_sum_materialization_layout(
        &checked,
        &plan,
        owner.symbol,
    )
    .expect("a direct sum field should project its conventional interior");
    let (arm_report, arm_sum_fields) =
        layout::project_conventional_record_with_sum_materialization_layout(
            &checked,
            &arm_plan,
            owner.symbol,
        )
        .expect("the same projection closes on linux_arm64");
    assert_eq!(
        report, arm_report,
        "both Linux ISAs retain the same outer report geometry"
    );
    assert_eq!(
        sum_fields, arm_sum_fields,
        "both Linux ISAs retain the same conventional sum interior"
    );
    assert_eq!(
        report
            .entries
            .iter()
            .map(|entry| (entry.field.as_str(), entry.placement))
            .collect::<Vec<_>>(),
        vec![
            ("header", LayoutPlacementReport::At { offset: 0 }),
            ("choice", LayoutPlacementReport::At { offset: 8 }),
            ("tail", LayoutPlacementReport::At { offset: 32 }),
        ]
    );
    let sum_row = sum_fields
        .iter()
        .find(|row| row.field == "choice")
        .expect("the direct sum field row");
    assert_eq!(sum_row.layout.size, 24);
    let run = sum_row
        .layout
        .cases
        .iter()
        .find(|case| case.case == "Run")
        .expect("the Run case");
    assert_eq!(
        run.payload_fields
            .iter()
            .map(|field| (field.field.as_str(), field.offset, field.size))
            .collect::<Vec<_>>(),
        vec![("callback", 8, 8), ("clock", 16, 8)]
    );
    let carrier = match sum_row.member_identity {
        Some(identity) => SymbolicFieldInnerLayout::new_sum_numbered(
            sum_row.field.clone(),
            identity,
            sum_row.layout.clone(),
        ),
        None => SymbolicFieldInnerLayout::new_sum(sum_row.field.clone(), sum_row.layout.clone()),
    };

    let entry_target = RelocationTarget::Entry(
        EntryStubId::from_normalized_identity(0x55aa).expect("normalized entry identity"),
    );
    let header_target = RelocationTarget::Data(
        DataSymbolId::from_normalized_identity(0x5a5a).expect("normalized data identity"),
    );
    let clock_target = RelocationTarget::Data(
        DataSymbolId::from_normalized_identity(0xbeef).expect("normalized data identity"),
    );
    let symbolic = [
        SymbolicFieldValue::new("header", 64, header_target).expect("scalar field"),
        SymbolicFieldValue::new("choice", 64, entry_target)
            .expect("direct sum field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new("Run")
                    .with_inner_segment(SymbolicFieldPathSegment::new("callback")),
            ),
        SymbolicFieldValue::new("choice", 64, clock_target)
            .expect("direct sum field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new("Run")
                    .with_inner_segment(SymbolicFieldPathSegment::new("clock")),
            ),
    ];
    let materialization = derive_symbolic_materialization_with_inner_layouts(
        &report,
        std::slice::from_ref(&carrier),
        &symbolic,
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: Some(64),
            placement: layout_plans::PlacementConstraints::unconstrained(
                layout_plans::PlacementPhase::PostHandoff,
            ),
        },
        |_| None,
    )
    .expect("case payload paths derive writes inside the sum interior");
    let writes = materialization
        .actions
        .iter()
        .map(|action| match action {
            MaterializationAction::RuntimeWriter(write) => {
                (write.field.as_str(), write.container_byte_offset)
            }
            other => panic!("unresolved case payload paths derive writers, found {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        writes,
        [
            ("header", 0),
            ("choice.Run.callback", 16),
            ("choice.Run.clock", 24),
        ]
    );

    let writer = materialization
        .derive_post_handoff_writer()
        .expect("the case payload writes derive a writer");
    let mut expected = vec![0xa5_u8; 40];
    expected[0..8].copy_from_slice(&0x99aa_bbcc_ddee_ff00_u64.to_le_bytes());
    expected[16..24].copy_from_slice(&0x1122_3344_5566_7788_u64.to_le_bytes());
    expected[24..32].copy_from_slice(&0xdead_beef_cafe_f00d_u64.to_le_bytes());
    lower_writer_on_both_linux_isas(&writer, 0xa5, &expected, |resolved| {
        if resolved == entry_target {
            0x1122_3344_5566_7788
        } else if resolved == clock_target {
            0xdead_beef_cafe_f00d
        } else {
            assert_eq!(resolved, header_target);
            0x99aa_bbcc_ddee_ff00
        }
    });
}

#[test]
fn nested_sum_array_symbolic_materialization_realizes_on_both_linux_isas() {
    // Nested sum arrays, end to end: `choices[i].Run.clock` keeps the exact
    // element index, selected case, and payload field symbolic until
    // materialization composes `field At + index * element stride + payload
    // offset`. The conventional projection retains the whole array extent as
    // one `At` placement; the carrier carries the element overlay, count, and
    // stride as data, and the same lowered writer executes on the host ISA.
    let main_path = write_program(
        "sum-array-symbolic-field",
        r#"
data Choice [copy] {
    case Empty;
    case Run(callback: u64, clock: u64);
}
data SumTable [copy] {
    header: u64;
    choices: [Choice; 2];
}
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("a sum array record should check");
    let plan = build_layout_plan(&checked, NativeTarget::linux_x64(), &[])
        .expect("the sum array record should lay out");
    let arm_plan = build_layout_plan(&checked, NativeTarget::linux_arm64(), &[])
        .expect("the sum array record should lay out for linux_arm64");
    let owner = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "SumTable")
        .expect("the sum table record");
    let (report, array_fields) =
        layout::project_conventional_record_with_sum_arrays_materialization_layout(
            &checked,
            &plan,
            owner.symbol,
        )
        .expect("a sum array field should project its conventional interior");
    let (arm_report, arm_array_fields) =
        layout::project_conventional_record_with_sum_arrays_materialization_layout(
            &checked,
            &arm_plan,
            owner.symbol,
        )
        .expect("the same projection closes on linux_arm64");
    assert_eq!(
        report, arm_report,
        "both Linux ISAs retain the same outer report geometry"
    );
    assert_eq!(
        array_fields, arm_array_fields,
        "both Linux ISAs retain the same repeated sum interior"
    );
    assert_eq!(
        report
            .entries
            .iter()
            .map(|entry| (entry.field.as_str(), entry.placement))
            .collect::<Vec<_>>(),
        vec![
            ("header", LayoutPlacementReport::At { offset: 0 }),
            ("choices", LayoutPlacementReport::At { offset: 8 }),
        ]
    );
    let array_row = array_fields
        .iter()
        .find(|row| row.field == "choices")
        .expect("the sum array field row");
    assert_eq!(array_row.element_count, 2);
    assert_eq!(array_row.element_stride, 24);
    assert_eq!(array_row.element_layout.size, 24);
    let carrier = match array_row.member_identity {
        Some(identity) => SymbolicFieldInnerLayout::new_sum_array_numbered(
            array_row.field.clone(),
            identity,
            array_row.element_layout.clone(),
            array_row.element_count,
            array_row.element_stride,
        ),
        None => SymbolicFieldInnerLayout::new_sum_array(
            array_row.field.clone(),
            array_row.element_layout.clone(),
            array_row.element_count,
            array_row.element_stride,
        ),
    };

    let entry_target = RelocationTarget::Entry(
        EntryStubId::from_normalized_identity(0x55aa).expect("normalized entry identity"),
    );
    let data_target = RelocationTarget::Data(
        DataSymbolId::from_normalized_identity(0x5a5a).expect("normalized data identity"),
    );
    let symbolic = [
        SymbolicFieldValue::new_indexed("choices", 0, 64, data_target)
            .expect("repeated sum field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new("Run")
                    .with_inner_segment(SymbolicFieldPathSegment::new("callback")),
            ),
        SymbolicFieldValue::new_indexed("choices", 1, 64, entry_target)
            .expect("repeated sum field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new("Run")
                    .with_inner_segment(SymbolicFieldPathSegment::new("clock")),
            ),
    ];
    let materialization = derive_symbolic_materialization_with_inner_layouts(
        &report,
        std::slice::from_ref(&carrier),
        &symbolic,
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: Some(64),
            placement: layout_plans::PlacementConstraints::unconstrained(
                layout_plans::PlacementPhase::PostHandoff,
            ),
        },
        |_| None,
    )
    .expect("element case payload paths derive writes inside each sum element");
    let writes = materialization
        .actions
        .iter()
        .map(|action| match action {
            MaterializationAction::RuntimeWriter(write) => {
                (write.field.as_str(), write.container_byte_offset)
            }
            other => panic!("unresolved element payload paths derive writers, found {other:?}"),
        })
        .collect::<Vec<_>>();
    // Element 0's `callback` lands at 8 + 0 * 24 + 8 = 16; element 1's
    // `clock` lands at 8 + 1 * 24 + 16 = 48.
    assert_eq!(
        writes,
        [
            ("choices[0].Run.callback", 16),
            ("choices[1].Run.clock", 48),
        ]
    );

    let writer = materialization
        .derive_post_handoff_writer()
        .expect("the element payload writes derive a writer");
    let mut expected = vec![0xa5_u8; 56];
    expected[16..24].copy_from_slice(&0x99aa_bbcc_ddee_ff00_u64.to_le_bytes());
    expected[48..56].copy_from_slice(&0x1122_3344_5566_7788_u64.to_le_bytes());
    lower_writer_on_both_linux_isas(&writer, 0xa5, &expected, |resolved| {
        if resolved == entry_target {
            0x1122_3344_5566_7788
        } else {
            assert_eq!(resolved, data_target);
            0x99aa_bbcc_ddee_ff00
        }
    });
}

#[test]
fn mixed_sum_array_symbolic_materialization_realizes_on_both_linux_isas() {
    // Mixed common-field/case elements, end to end: `events[i].sequence`
    // spells the element's common field beside the tag while
    // `events[i].Ready.value` spells the selected case's payload inside the
    // shared overlay. Both paths stay symbolic until materialization composes
    // `field At + index * element stride + member offset`; the same lowered
    // writer then executes on the host ISA.
    let main_path = write_program(
        "mixed-sum-array-symbolic-field",
        r#"
data Event [copy] {
    sequence: u8;
    case Ready(value: u64);
    case Waiting;
}
data Log [copy] {
    header: u64;
    events: [Event; 2];
}
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("a mixed sum array record should check");
    let plan = build_layout_plan(&checked, NativeTarget::linux_x64(), &[])
        .expect("the mixed sum array record should lay out");
    let arm_plan = build_layout_plan(&checked, NativeTarget::linux_arm64(), &[])
        .expect("the mixed sum array record should lay out for linux_arm64");
    let owner = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Log")
        .expect("the log record");
    let (report, array_fields) =
        layout::project_conventional_record_with_sum_arrays_materialization_layout(
            &checked,
            &plan,
            owner.symbol,
        )
        .expect("a mixed sum array field should project its conventional interior");
    let (arm_report, arm_array_fields) =
        layout::project_conventional_record_with_sum_arrays_materialization_layout(
            &checked,
            &arm_plan,
            owner.symbol,
        )
        .expect("the same projection closes on linux_arm64");
    assert_eq!(
        report, arm_report,
        "both Linux ISAs retain the same outer report geometry"
    );
    assert_eq!(
        array_fields, arm_array_fields,
        "both Linux ISAs retain the same repeated mixed interior"
    );
    let array_row = array_fields
        .iter()
        .find(|row| row.field == "events")
        .expect("the mixed sum array field row");
    assert_eq!(array_row.element_count, 2);
    assert_eq!(array_row.element_stride, 16);
    assert_eq!(array_row.element_layout.size, 16);
    let sequence = array_row
        .element_layout
        .common_fields
        .iter()
        .find(|field| field.field == "sequence")
        .expect("the element retains its common field row");
    assert_eq!((sequence.offset, sequence.size), (4, 1));
    let ready = array_row
        .element_layout
        .cases
        .iter()
        .find(|case| case.case == "Ready")
        .expect("the element retains its Ready case");
    assert_eq!(
        ready
            .payload_fields
            .iter()
            .map(|field| (field.field.as_str(), field.offset, field.size))
            .collect::<Vec<_>>(),
        vec![("value", 8, 8)]
    );
    let carrier = match array_row.member_identity {
        Some(identity) => SymbolicFieldInnerLayout::new_sum_array_numbered(
            array_row.field.clone(),
            identity,
            array_row.element_layout.clone(),
            array_row.element_count,
            array_row.element_stride,
        ),
        None => SymbolicFieldInnerLayout::new_sum_array(
            array_row.field.clone(),
            array_row.element_layout.clone(),
            array_row.element_count,
            array_row.element_stride,
        ),
    };

    let entry_target = RelocationTarget::Entry(
        EntryStubId::from_normalized_identity(0x55aa).expect("normalized entry identity"),
    );
    let data_target = RelocationTarget::Data(
        DataSymbolId::from_normalized_identity(0x5a5a).expect("normalized data identity"),
    );
    let symbolic = [
        SymbolicFieldValue::new_indexed("events", 1, 8, data_target)
            .expect("repeated mixed field")
            .with_inner_segment(SymbolicFieldPathSegment::new("sequence")),
        SymbolicFieldValue::new_indexed("events", 0, 64, entry_target)
            .expect("repeated mixed field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new("Ready")
                    .with_inner_segment(SymbolicFieldPathSegment::new("value")),
            ),
    ];
    let materialization = derive_symbolic_materialization_with_inner_layouts(
        &report,
        std::slice::from_ref(&carrier),
        &symbolic,
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: Some(64),
            placement: layout_plans::PlacementConstraints::unconstrained(
                layout_plans::PlacementPhase::PostHandoff,
            ),
        },
        |_| None,
    )
    .expect("mixed element member paths derive writes inside each element");
    let writes = materialization
        .actions
        .iter()
        .map(|action| match action {
            MaterializationAction::RuntimeWriter(write) => {
                (write.field.as_str(), write.container_byte_offset)
            }
            other => panic!("unresolved element member paths derive writers, found {other:?}"),
        })
        .collect::<Vec<_>>();
    // `events` spans 8..40 at a 16-byte stride: element 1's `sequence` lands
    // at 8 + 16 + 4 = 28 and element 0's `Ready.value` at 8 + 0 + 8 = 16.
    assert_eq!(
        writes,
        [("events[1].sequence", 28), ("events[0].Ready.value", 16),]
    );

    let writer = materialization
        .derive_post_handoff_writer()
        .expect("the mixed element writes derive a writer");
    let mut expected = vec![0xa5_u8; 40];
    expected[28] = 0x7f;
    expected[16..24].copy_from_slice(&0x1122_3344_5566_7788_u64.to_le_bytes());
    lower_writer_on_both_linux_isas(&writer, 0xa5, &expected, |resolved| {
        if resolved == entry_target {
            0x1122_3344_5566_7788
        } else {
            assert_eq!(resolved, data_target);
            0x7f
        }
    });
}

#[test]
fn recursive_sum_symbolic_materialization_realizes_on_both_linux_isas() {
    // Recursive shapes under the general rule, end to end:
    // `middle.inner.choice.Run.callback` crosses two record boundaries before
    // spelling the selected case and its payload field, while `middle`'s own
    // direct sum `route` coexists with that deeper path on the same record
    // level. The recursive projection retains every boundary's exact interior
    // as report data; the `SymbolicFieldInnerLayout` fold turns that report
    // into the carriers derivation binds, and the same bounded hop traversal
    // composes every offset — no depth-specific case enters either side.
    // Numbered members join the whole path on stable identity.
    let main_path = write_program(
        "recursive-sum-symbolic-field",
        r#"
data Choice [copy] {
    case #1 Empty;
    case #2 Run(#3 callback: u64, #4 clock: u64);
}
data Inner [copy] {
    #1 choice: Choice;
    #2 pad: u64;
}
data Middle [copy] {
    #1 inner: Inner;
    #2 tag: u64;
    #3 route: Choice;
}
data Outer [copy] {
    #1 header: u64;
    #2 middle: Middle;
}
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("a recursive record/sum record should check");
    let plan = build_layout_plan(&checked, NativeTarget::linux_x64(), &[])
        .expect("the recursive record should lay out");
    let arm_plan = build_layout_plan(&checked, NativeTarget::linux_arm64(), &[])
        .expect("the recursive record should lay out for linux_arm64");
    let owner = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Outer")
        .expect("the outer record");
    let paths =
        layout::project_conventional_record_with_recursive_nested_sums_materialization_layout(
            &checked,
            &plan,
            owner.symbol,
        )
        .expect("a record reaching sums through nested records should project");
    let arm_paths =
        layout::project_conventional_record_with_recursive_nested_sums_materialization_layout(
            &checked,
            &arm_plan,
            owner.symbol,
        )
        .expect("the same recursive projection closes on linux_arm64");
    assert_eq!(
        paths, arm_paths,
        "both Linux ISAs retain the same recursive path geometry"
    );
    assert_eq!(
        paths
            .outer_layout()
            .entries
            .iter()
            .map(|entry| (entry.field.as_str(), entry.placement))
            .collect::<Vec<_>>(),
        vec![
            ("header", LayoutPlacementReport::At { offset: 0 }),
            ("middle", LayoutPlacementReport::At { offset: 8 }),
        ]
    );
    let ConventionalRecursiveRecordSumPathsLayoutReport::Branch(root) = &paths else {
        panic!("the outer record projects as a recursive branch");
    };
    assert_eq!(root.paths.len(), 1);
    assert_eq!(root.paths[0].outer_field, "middle");
    assert_eq!(root.paths[0].outer_member_identity, Some(2));
    let ConventionalRecursiveRecordSumPathsLayoutReport::Branch(middle) = &root.paths[0].inner
    else {
        panic!("the middle record projects as a recursive branch");
    };
    assert_eq!(middle.paths.len(), 1);
    assert_eq!(middle.paths[0].outer_field, "inner");
    assert_eq!(middle.paths[0].outer_member_identity, Some(1));
    // `middle` retains its own direct sum `route` beside the deeper `inner`
    // record path — the branch level carries both child kinds.
    assert_eq!(middle.child_sum_layouts.len(), 1);
    assert_eq!(middle.child_sum_layouts[0].field, "route");
    assert_eq!(middle.child_sum_layouts[0].member_identity, Some(3));
    let ConventionalRecursiveRecordSumPathsLayoutReport::Leaf {
        child_sum_layouts, ..
    } = &middle.paths[0].inner
    else {
        panic!("the innermost record is the direct-sum leaf");
    };
    assert_eq!(child_sum_layouts.len(), 1);
    assert_eq!(child_sum_layouts[0].field, "choice");
    assert_eq!(child_sum_layouts[0].member_identity, Some(1));
    let carriers = SymbolicFieldInnerLayout::from_recursive_sum_paths(&paths)
        .expect("the recursive report folds into inner layout carriers");

    let header_target = RelocationTarget::Data(
        DataSymbolId::from_normalized_identity(0x5a5a).expect("normalized data identity"),
    );
    let entry_target = RelocationTarget::Entry(
        EntryStubId::from_normalized_identity(0x55aa).expect("normalized entry identity"),
    );
    let clock_target = RelocationTarget::Data(
        DataSymbolId::from_normalized_identity(0xbeef).expect("normalized data identity"),
    );
    let pad_target = RelocationTarget::Data(
        DataSymbolId::from_normalized_identity(0x0bad).expect("normalized data identity"),
    );
    let route_target = RelocationTarget::Data(
        DataSymbolId::from_normalized_identity(0x7ea7).expect("normalized data identity"),
    );
    let symbolic = [
        SymbolicFieldValue::new_numbered("header", 1, 64, header_target)
            .expect("numbered scalar field"),
        SymbolicFieldValue::new_numbered("middle", 2, 64, entry_target)
            .expect("numbered outer record field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new_numbered("inner", 1).with_inner_segment(
                    SymbolicFieldPathSegment::new_numbered("choice", 1).with_inner_segment(
                        SymbolicFieldPathSegment::new_numbered("Run", 2).with_inner_segment(
                            SymbolicFieldPathSegment::new_numbered("callback", 3),
                        ),
                    ),
                ),
            ),
        SymbolicFieldValue::new_numbered("middle", 2, 64, clock_target)
            .expect("numbered outer record field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new_numbered("inner", 1).with_inner_segment(
                    SymbolicFieldPathSegment::new_numbered("choice", 1).with_inner_segment(
                        SymbolicFieldPathSegment::new_numbered("Run", 2)
                            .with_inner_segment(SymbolicFieldPathSegment::new_numbered("clock", 4)),
                    ),
                ),
            ),
        SymbolicFieldValue::new_numbered("middle", 2, 64, pad_target)
            .expect("numbered outer record field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new_numbered("inner", 1)
                    .with_inner_segment(SymbolicFieldPathSegment::new_numbered("pad", 2)),
            ),
        SymbolicFieldValue::new_numbered("middle", 2, 64, route_target)
            .expect("numbered outer record field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new_numbered("route", 3).with_inner_segment(
                    SymbolicFieldPathSegment::new_numbered("Run", 2)
                        .with_inner_segment(SymbolicFieldPathSegment::new_numbered("callback", 3)),
                ),
            ),
    ];
    let materialization = derive_symbolic_materialization_with_inner_layouts(
        paths.outer_layout(),
        &carriers,
        &symbolic,
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: Some(64),
            placement: layout_plans::PlacementConstraints::unconstrained(
                layout_plans::PlacementPhase::PostHandoff,
            ),
        },
        |_| None,
    )
    .expect("recursive record/sum paths compose every crossed boundary");
    let writes = materialization
        .actions
        .iter()
        .map(|action| match action {
            MaterializationAction::RuntimeWriter(write) => {
                (write.field.as_str(), write.container_byte_offset)
            }
            other => panic!("unresolved recursive paths derive writers, found {other:?}"),
        })
        .collect::<Vec<_>>();
    // `middle` spans 8..72; inside it `inner` sits at 0 holding `choice` at 0
    // and `pad` at 24, so `Run.callback` lands at 16, `Run.clock` at 24, and
    // the `inner.pad` member leaf at 32 — while the level's own `route` sum
    // at 40 puts `Run.callback` at 56.
    assert_eq!(
        writes,
        [
            ("header", 0),
            ("middle.inner.choice.Run.callback", 16),
            ("middle.inner.choice.Run.clock", 24),
            ("middle.inner.pad", 32),
            ("middle.route.Run.callback", 56),
        ]
    );

    let writer = materialization
        .derive_post_handoff_writer()
        .expect("the recursive boundary writes derive a writer");
    let mut expected = vec![0xa5_u8; 72];
    expected[0..8].copy_from_slice(&0x99aa_bbcc_ddee_ff00_u64.to_le_bytes());
    expected[16..24].copy_from_slice(&0x1122_3344_5566_7788_u64.to_le_bytes());
    expected[24..32].copy_from_slice(&0xdead_beef_cafe_f00d_u64.to_le_bytes());
    expected[32..40].copy_from_slice(&0x55aa_bb00_00dd_ee11_u64.to_le_bytes());
    expected[56..64].copy_from_slice(&0x0bad_f00d_c001_d00d_u64.to_le_bytes());
    lower_writer_on_both_linux_isas(&writer, 0xa5, &expected, |resolved| {
        if resolved == entry_target {
            0x1122_3344_5566_7788
        } else if resolved == clock_target {
            0xdead_beef_cafe_f00d
        } else if resolved == pad_target {
            0x55aa_bb00_00dd_ee11
        } else if resolved == route_target {
            0x0bad_f00d_c001_d00d
        } else {
            assert_eq!(resolved, header_target);
            0x99aa_bbcc_ddee_ff00
        }
    });
}

#[test]
fn recursive_sum_array_symbolic_materialization_realizes_on_both_linux_isas() {
    // Nested sum arrays under the general recursive rule, end to end: every
    // recursive record level may carry its own direct sums, its own direct
    // fixed sum arrays, and deeper record paths at once. `middle`'s level
    // holds `route` and `batches` beside the `inner` path, and `inner`'s leaf
    // level holds `choice` beside `choices`, so
    // `middle.inner.choices[i].Run.<payload>` and `middle.batches[i].Run.
    // <payload>` compose `field At + index * element stride + payload offset`
    // through folded carriers — the same hop vocabulary the standalone
    // sum-array rung spells at the top level.
    let main_path = write_program(
        "recursive-sum-array-symbolic-field",
        r#"
data Choice [copy] {
    case #1 Empty;
    case #2 Run(#3 callback: u64, #4 clock: u64);
}
data Inner [copy] {
    #1 choice: Choice;
    #2 choices: [Choice; 2];
    #3 pad: u64;
}
data Middle [copy] {
    #1 inner: Inner;
    #2 tag: u64;
    #3 route: Choice;
    #4 batches: [Choice; 2];
}
data Outer [copy] {
    #1 header: u64;
    #2 middle: Middle;
}
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("a recursive record/sum-array record should check");
    let plan = build_layout_plan(&checked, NativeTarget::linux_x64(), &[])
        .expect("the recursive record should lay out");
    let arm_plan = build_layout_plan(&checked, NativeTarget::linux_arm64(), &[])
        .expect("the recursive record should lay out for linux_arm64");
    let owner = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Outer")
        .expect("the outer record");
    let paths =
        layout::project_conventional_record_with_recursive_nested_sums_materialization_layout(
            &checked,
            &plan,
            owner.symbol,
        )
        .expect("a record reaching sums and sum arrays through records should project");
    let arm_paths =
        layout::project_conventional_record_with_recursive_nested_sums_materialization_layout(
            &checked,
            &arm_plan,
            owner.symbol,
        )
        .expect("the same recursive projection closes on linux_arm64");
    assert_eq!(
        paths, arm_paths,
        "both Linux ISAs retain the same recursive path geometry"
    );
    assert_eq!(
        paths
            .outer_layout()
            .entries
            .iter()
            .map(|entry| (entry.field.as_str(), entry.placement))
            .collect::<Vec<_>>(),
        vec![
            ("header", LayoutPlacementReport::At { offset: 0 }),
            ("middle", LayoutPlacementReport::At { offset: 8 }),
        ]
    );
    let ConventionalRecursiveRecordSumPathsLayoutReport::Branch(root) = &paths else {
        panic!("the outer record projects as a recursive branch");
    };
    assert_eq!(root.paths.len(), 1);
    assert_eq!(root.paths[0].outer_field, "middle");
    assert_eq!(root.paths[0].outer_member_identity, Some(2));
    let ConventionalRecursiveRecordSumPathsLayoutReport::Branch(middle) = &root.paths[0].inner
    else {
        panic!("the middle record projects as a recursive branch");
    };
    assert_eq!(middle.paths.len(), 1);
    assert_eq!(middle.paths[0].outer_field, "inner");
    assert_eq!(middle.paths[0].outer_member_identity, Some(1));
    // The `middle` branch level co-locates all three child kinds: its own
    // direct sum `route`, its own sum array `batches`, and the deeper `inner`
    // record path.
    assert_eq!(
        middle
            .child_sum_layouts
            .iter()
            .map(|row| (row.field.as_str(), row.member_identity))
            .collect::<Vec<_>>(),
        vec![("route", Some(3))]
    );
    assert_eq!(
        middle
            .child_sum_array_layouts
            .iter()
            .map(|row| {
                (
                    row.field.as_str(),
                    row.member_identity,
                    row.element_count,
                    row.element_stride,
                )
            })
            .collect::<Vec<_>>(),
        vec![("batches", Some(4), 2, 24)]
    );
    let ConventionalRecursiveRecordSumPathsLayoutReport::Leaf {
        child_sum_layouts,
        child_sum_array_layouts,
        ..
    } = &middle.paths[0].inner
    else {
        panic!("the innermost record is a leaf level");
    };
    assert_eq!(
        child_sum_layouts
            .iter()
            .map(|row| (row.field.as_str(), row.member_identity))
            .collect::<Vec<_>>(),
        vec![("choice", Some(1))]
    );
    assert_eq!(
        child_sum_array_layouts
            .iter()
            .map(|row| {
                (
                    row.field.as_str(),
                    row.member_identity,
                    row.element_count,
                    row.element_stride,
                )
            })
            .collect::<Vec<_>>(),
        vec![("choices", Some(2), 2, 24)]
    );
    let carriers = SymbolicFieldInnerLayout::from_recursive_sum_paths(&paths)
        .expect("the recursive report folds sum and sum-array carriers at every level");

    let header_target = RelocationTarget::Data(
        DataSymbolId::from_normalized_identity(0x5a5a).expect("normalized data identity"),
    );
    let deep_target = RelocationTarget::Entry(
        EntryStubId::from_normalized_identity(0x55aa).expect("normalized entry identity"),
    );
    let element_zero_target = RelocationTarget::Data(
        DataSymbolId::from_normalized_identity(0xbeef).expect("normalized data identity"),
    );
    let element_one_target = RelocationTarget::Data(
        DataSymbolId::from_normalized_identity(0x0bad).expect("normalized data identity"),
    );
    let pad_target = RelocationTarget::Data(
        DataSymbolId::from_normalized_identity(0x7ea7).expect("normalized data identity"),
    );
    let route_target = RelocationTarget::Data(
        DataSymbolId::from_normalized_identity(0xc0de).expect("normalized data identity"),
    );
    let batch_zero_target = RelocationTarget::Data(
        DataSymbolId::from_normalized_identity(0xb17c).expect("normalized data identity"),
    );
    let batch_one_target = RelocationTarget::Entry(
        EntryStubId::from_normalized_identity(0x77ee).expect("normalized entry identity"),
    );
    let symbolic = [
        SymbolicFieldValue::new_numbered("header", 1, 64, header_target)
            .expect("numbered scalar field"),
        SymbolicFieldValue::new_numbered("middle", 2, 64, deep_target)
            .expect("numbered outer record field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new_numbered("inner", 1).with_inner_segment(
                    SymbolicFieldPathSegment::new_numbered("choice", 1).with_inner_segment(
                        SymbolicFieldPathSegment::new_numbered("Run", 2).with_inner_segment(
                            SymbolicFieldPathSegment::new_numbered("callback", 3),
                        ),
                    ),
                ),
            ),
        SymbolicFieldValue::new_numbered("middle", 2, 64, element_zero_target)
            .expect("numbered outer record field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new_numbered("inner", 1).with_inner_segment(
                    SymbolicFieldPathSegment::new_indexed_numbered("choices", 2, 0)
                        .with_inner_segment(
                            SymbolicFieldPathSegment::new_numbered("Run", 2).with_inner_segment(
                                SymbolicFieldPathSegment::new_numbered("callback", 3),
                            ),
                        ),
                ),
            ),
        SymbolicFieldValue::new_numbered("middle", 2, 64, element_one_target)
            .expect("numbered outer record field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new_numbered("inner", 1).with_inner_segment(
                    SymbolicFieldPathSegment::new_indexed_numbered("choices", 2, 1)
                        .with_inner_segment(
                            SymbolicFieldPathSegment::new_numbered("Run", 2).with_inner_segment(
                                SymbolicFieldPathSegment::new_numbered("clock", 4),
                            ),
                        ),
                ),
            ),
        SymbolicFieldValue::new_numbered("middle", 2, 64, pad_target)
            .expect("numbered outer record field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new_numbered("inner", 1)
                    .with_inner_segment(SymbolicFieldPathSegment::new_numbered("pad", 3)),
            ),
        SymbolicFieldValue::new_numbered("middle", 2, 64, route_target)
            .expect("numbered outer record field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new_numbered("route", 3).with_inner_segment(
                    SymbolicFieldPathSegment::new_numbered("Run", 2)
                        .with_inner_segment(SymbolicFieldPathSegment::new_numbered("callback", 3)),
                ),
            ),
        SymbolicFieldValue::new_numbered("middle", 2, 64, batch_zero_target)
            .expect("numbered outer record field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new_indexed_numbered("batches", 4, 0).with_inner_segment(
                    SymbolicFieldPathSegment::new_numbered("Run", 2)
                        .with_inner_segment(SymbolicFieldPathSegment::new_numbered("clock", 4)),
                ),
            ),
        SymbolicFieldValue::new_numbered("middle", 2, 64, batch_one_target)
            .expect("numbered outer record field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new_indexed_numbered("batches", 4, 1).with_inner_segment(
                    SymbolicFieldPathSegment::new_numbered("Run", 2)
                        .with_inner_segment(SymbolicFieldPathSegment::new_numbered("callback", 3)),
                ),
            ),
    ];
    let materialization = derive_symbolic_materialization_with_inner_layouts(
        paths.outer_layout(),
        &carriers,
        &symbolic,
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: Some(64),
            placement: layout_plans::PlacementConstraints::unconstrained(
                layout_plans::PlacementPhase::PostHandoff,
            ),
        },
        |_| None,
    )
    .expect("recursive record/sum-array paths compose every crossed boundary");
    let writes = materialization
        .actions
        .iter()
        .map(|action| match action {
            MaterializationAction::RuntimeWriter(write) => {
                (write.field.as_str(), write.container_byte_offset)
            }
            other => panic!("unresolved recursive paths derive writers, found {other:?}"),
        })
        .collect::<Vec<_>>();
    // `middle` spans 8..168; inside it `inner` sits at 0 holding `choice` at
    // 0, `choices` at 24 (24-byte stride), and `pad` at 72 — while the
    // level's own `route` sum sits at 88 and `batches` repeats at 112. Every
    // indexed hop composes `field At + index * stride + payload offset`.
    assert_eq!(
        writes,
        [
            ("header", 0),
            ("middle.inner.choice.Run.callback", 16),
            ("middle.inner.choices[0].Run.callback", 40),
            ("middle.inner.choices[1].Run.clock", 72),
            ("middle.inner.pad", 80),
            ("middle.route.Run.callback", 104),
            ("middle.batches[0].Run.clock", 136),
            ("middle.batches[1].Run.callback", 152),
        ]
    );

    let writer = materialization
        .derive_post_handoff_writer()
        .expect("the recursive boundary writes derive a writer");
    let mut expected = vec![0xa5_u8; 168];
    expected[0..8].copy_from_slice(&0x99aa_bbcc_ddee_ff00_u64.to_le_bytes());
    expected[16..24].copy_from_slice(&0x1122_3344_5566_7788_u64.to_le_bytes());
    expected[40..48].copy_from_slice(&0xdead_beef_cafe_f00d_u64.to_le_bytes());
    expected[72..80].copy_from_slice(&0x55aa_bb00_00dd_ee11_u64.to_le_bytes());
    expected[80..88].copy_from_slice(&0x0bad_f00d_c001_d00d_u64.to_le_bytes());
    expected[104..112].copy_from_slice(&0xcafe_babe_face_feed_u64.to_le_bytes());
    expected[136..144].copy_from_slice(&0x1234_5678_9abc_def0_u64.to_le_bytes());
    expected[152..160].copy_from_slice(&0xf00d_d00f_beef_cafe_u64.to_le_bytes());
    lower_writer_on_both_linux_isas(&writer, 0xa5, &expected, |resolved| {
        if resolved == deep_target {
            0x1122_3344_5566_7788
        } else if resolved == element_zero_target {
            0xdead_beef_cafe_f00d
        } else if resolved == element_one_target {
            0x55aa_bb00_00dd_ee11
        } else if resolved == pad_target {
            0x0bad_f00d_c001_d00d
        } else if resolved == route_target {
            0xcafe_babe_face_feed
        } else if resolved == batch_zero_target {
            0x1234_5678_9abc_def0
        } else if resolved == batch_one_target {
            0xf00d_d00f_beef_cafe
        } else {
            assert_eq!(resolved, header_target);
            0x99aa_bbcc_ddee_ff00
        }
    });
}

#[test]
fn record_array_symbolic_materialization_realizes_on_both_linux_isas() {
    // Record arrays under the general recursive rule, end to end:
    // `members[i].choice.Run.<payload>` composes one literal element hop with
    // the record boundary inside each element, while the same leaf level also
    // carries a direct sum `route` beside the record array. The recursive
    // projection retains the `members` row once — element count, constant
    // stride, and the element record's own leaf report — and the
    // `SymbolicFieldInnerLayout::RecordArray` carrier folds that row into the
    // same hop vocabulary every other repeated interior spells, so the exact
    // index stays symbolic until derivation assigns
    // `field At + index * stride + interior offset`.
    let main_path = write_program(
        "record-array-symbolic-field",
        r#"
data Choice [copy] {
    case #1 Empty;
    case #2 Run(#3 callback: u64, #4 clock: u64);
}
data Neighbor [copy] {
    #1 choice: Choice;
    #2 pad: u64;
}
data OnlyMembers [copy] {
    #1 header: u64;
    #2 members: [Neighbor; 2];
}
data Outer [copy] {
    #1 header: u64;
    #2 members: [Neighbor; 2];
    #3 route: Choice;
}
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("a recursive record/record-array record should check");
    let plan = build_layout_plan(&checked, NativeTarget::linux_x64(), &[])
        .expect("the record-array record should lay out");
    let arm_plan = build_layout_plan(&checked, NativeTarget::linux_arm64(), &[])
        .expect("the record-array record should lay out for linux_arm64");
    // The record array is the recursive owner's shape alone: a record holding
    // only `[Neighbor; 2]` beside a scalar rejects from both non-recursive
    // owners, each naming the row it refuses to lift.
    let only_members = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "OnlyMembers")
        .expect("the scalar-and-record-array record");
    let direct_error = layout::project_conventional_record_with_sum_materialization_layout(
        &checked,
        &plan,
        only_members.symbol,
    )
    .expect_err("the direct-sum owner must refuse a direct record array");
    assert!(
        direct_error
            .message
            .contains("does not lift the direct record array `members`"),
        "{direct_error:?}"
    );
    let array_error = layout::project_conventional_record_with_sum_arrays_materialization_layout(
        &checked,
        &plan,
        only_members.symbol,
    )
    .expect_err("the sum-array owner must refuse a direct record array");
    assert!(
        array_error
            .message
            .contains("does not lift the direct record array `members`"),
        "{array_error:?}"
    );
    let owner = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Outer")
        .expect("the outer record");
    let paths =
        layout::project_conventional_record_with_recursive_nested_sums_materialization_layout(
            &checked,
            &plan,
            owner.symbol,
        )
        .expect("a record reaching sums through a record array should project");
    let arm_paths =
        layout::project_conventional_record_with_recursive_nested_sums_materialization_layout(
            &checked,
            &arm_plan,
            owner.symbol,
        )
        .expect("the same recursive projection closes on linux_arm64");
    assert_eq!(
        paths, arm_paths,
        "both Linux ISAs retain the same recursive path geometry"
    );
    assert_eq!(
        paths
            .outer_layout()
            .entries
            .iter()
            .map(|entry| (entry.field.as_str(), entry.placement))
            .collect::<Vec<_>>(),
        vec![
            ("header", LayoutPlacementReport::At { offset: 0 }),
            ("members", LayoutPlacementReport::At { offset: 8 }),
            ("route", LayoutPlacementReport::At { offset: 72 }),
        ]
    );
    let ConventionalRecursiveRecordSumPathsLayoutReport::Leaf {
        child_sum_layouts,
        child_sum_array_layouts,
        child_record_array_layouts,
        ..
    } = &paths
    else {
        panic!("the outer record is a leaf level: no deeper record paths");
    };
    // The leaf level co-locates its own direct sum `route` beside the record
    // array `members` — one `At` extent per repeated field, one compact row
    // carrying the element record's complete leaf report for every index.
    assert_eq!(
        child_sum_layouts
            .iter()
            .map(|row| (row.field.as_str(), row.member_identity))
            .collect::<Vec<_>>(),
        vec![("route", Some(3))]
    );
    assert!(child_sum_array_layouts.is_empty());
    assert_eq!(
        child_record_array_layouts
            .iter()
            .map(|row| {
                (
                    row.field.as_str(),
                    row.member_identity,
                    row.element_count,
                    row.element_stride,
                )
            })
            .collect::<Vec<_>>(),
        vec![("members", Some(2), 2, 32)]
    );
    let ConventionalRecursiveRecordSumPathsLayoutReport::Leaf {
        child_sum_layouts: element_sums,
        ..
    } = &child_record_array_layouts[0].inner
    else {
        panic!("the record-array element record is a leaf level");
    };
    assert_eq!(
        element_sums
            .iter()
            .map(|row| (row.field.as_str(), row.member_identity))
            .collect::<Vec<_>>(),
        vec![("choice", Some(1))]
    );
    let carriers = SymbolicFieldInnerLayout::from_recursive_sum_paths(&paths)
        .expect("the recursive report folds the record-array carrier");

    let header_target = RelocationTarget::Data(
        DataSymbolId::from_normalized_identity(0x5a5a).expect("normalized data identity"),
    );
    let zero_clock_target = RelocationTarget::Data(
        DataSymbolId::from_normalized_identity(0xbeef).expect("normalized data identity"),
    );
    let pad_zero_target = RelocationTarget::Data(
        DataSymbolId::from_normalized_identity(0x0bad).expect("normalized data identity"),
    );
    let member_one_target = RelocationTarget::Entry(
        EntryStubId::from_normalized_identity(0x55aa).expect("normalized entry identity"),
    );
    let pad_one_target = RelocationTarget::Data(
        DataSymbolId::from_normalized_identity(0x7ea7).expect("normalized data identity"),
    );
    let route_target = RelocationTarget::Entry(
        EntryStubId::from_normalized_identity(0x77ee).expect("normalized entry identity"),
    );
    let symbolic = [
        SymbolicFieldValue::new_numbered("header", 1, 64, header_target)
            .expect("numbered scalar field"),
        SymbolicFieldValue::new_indexed_numbered("members", 2, 0, 64, zero_clock_target)
            .expect("indexed record-array field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new_numbered("choice", 1).with_inner_segment(
                    SymbolicFieldPathSegment::new_numbered("Run", 2)
                        .with_inner_segment(SymbolicFieldPathSegment::new_numbered("clock", 4)),
                ),
            ),
        SymbolicFieldValue::new_indexed_numbered("members", 2, 0, 64, pad_zero_target)
            .expect("indexed record-array field")
            .with_inner_segment(SymbolicFieldPathSegment::new_numbered("pad", 2)),
        SymbolicFieldValue::new_indexed_numbered("members", 2, 1, 64, member_one_target)
            .expect("indexed record-array field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new_numbered("choice", 1).with_inner_segment(
                    SymbolicFieldPathSegment::new_numbered("Run", 2)
                        .with_inner_segment(SymbolicFieldPathSegment::new_numbered("callback", 3)),
                ),
            ),
        SymbolicFieldValue::new_indexed_numbered("members", 2, 1, 64, pad_one_target)
            .expect("indexed record-array field")
            .with_inner_segment(SymbolicFieldPathSegment::new_numbered("pad", 2)),
        SymbolicFieldValue::new_numbered("route", 3, 64, route_target)
            .expect("numbered direct sum field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new_numbered("Run", 2)
                    .with_inner_segment(SymbolicFieldPathSegment::new_numbered("callback", 3)),
            ),
    ];
    let materialization = derive_symbolic_materialization_with_inner_layouts(
        paths.outer_layout(),
        &carriers,
        &symbolic,
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: Some(64),
            placement: layout_plans::PlacementConstraints::unconstrained(
                layout_plans::PlacementPhase::PostHandoff,
            ),
        },
        |_| None,
    )
    .expect("record-array paths compose the element hop and record boundary");
    let writes = materialization
        .actions
        .iter()
        .map(|action| match action {
            MaterializationAction::RuntimeWriter(write) => {
                (write.field.as_str(), write.container_byte_offset)
            }
            other => panic!("unresolved record-array paths derive writers, found {other:?}"),
        })
        .collect::<Vec<_>>();
    // `members` spans 8..72 at a 32-byte stride; inside each element `choice`
    // sits at 0 and `pad` at 24, while the level's own `route` sum sits at
    // 72. `members[1].choice.Run.callback` composes `8 + 1*32 + 0 + 8 = 48`;
    // `members[i].pad` composes `8 + i*32 + 24`.
    assert_eq!(
        writes,
        [
            ("header", 0),
            ("members[0].choice.Run.clock", 24),
            ("members[0].pad", 32),
            ("members[1].choice.Run.callback", 48),
            ("members[1].pad", 64),
            ("route.Run.callback", 80),
        ]
    );

    // The path bound stays exact until assignment: an unindexed hop into the
    // repeated record names no element, and an index outside the retained
    // count rejects against the row's own bounds — both before any byte
    // offset is chosen.
    let unindexed = derive_symbolic_materialization_with_inner_layouts(
        paths.outer_layout(),
        &carriers,
        &[
            SymbolicFieldValue::new_numbered("members", 2, 64, member_one_target)
                .expect("numbered record-array field")
                .with_inner_segment(
                    SymbolicFieldPathSegment::new_numbered("choice", 1).with_inner_segment(
                        SymbolicFieldPathSegment::new_numbered("Run", 2).with_inner_segment(
                            SymbolicFieldPathSegment::new_numbered("callback", 3),
                        ),
                    ),
                ),
        ],
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: Some(64),
            placement: layout_plans::PlacementConstraints::unconstrained(
                layout_plans::PlacementPhase::PostHandoff,
            ),
        },
        |_| None,
    )
    .expect_err("a record-array hop without an element index must reject");
    assert!(
        unindexed
            .0
            .contains("requires an element index into the repeated record field `members`"),
        "{unindexed:?}"
    );
    let out_of_range = derive_symbolic_materialization_with_inner_layouts(
        paths.outer_layout(),
        &carriers,
        &[
            SymbolicFieldValue::new_indexed_numbered("members", 2, 2, 64, member_one_target)
                .expect("indexed record-array field")
                .with_inner_segment(SymbolicFieldPathSegment::new_numbered("pad", 2)),
        ],
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: Some(64),
            placement: layout_plans::PlacementConstraints::unconstrained(
                layout_plans::PlacementPhase::PostHandoff,
            ),
        },
        |_| None,
    )
    .expect_err("a record-array index past the element count must reject");
    assert!(
        out_of_range
            .0
            .contains("element index 2 is outside its 2 element placements"),
        "{out_of_range:?}"
    );

    let writer = materialization
        .derive_post_handoff_writer()
        .expect("the record-array boundary writes derive a writer");
    let mut expected = vec![0xa5_u8; 96];
    expected[0..8].copy_from_slice(&0x99aa_bbcc_ddee_ff00_u64.to_le_bytes());
    expected[24..32].copy_from_slice(&0xdead_beef_cafe_f00d_u64.to_le_bytes());
    expected[32..40].copy_from_slice(&0x55aa_bb00_00dd_ee11_u64.to_le_bytes());
    expected[48..56].copy_from_slice(&0x1122_3344_5566_7788_u64.to_le_bytes());
    expected[64..72].copy_from_slice(&0x0bad_f00d_c001_d00d_u64.to_le_bytes());
    expected[80..88].copy_from_slice(&0xcafe_babe_face_feed_u64.to_le_bytes());
    lower_writer_on_both_linux_isas(&writer, 0xa5, &expected, |resolved| {
        if resolved == member_one_target {
            0x1122_3344_5566_7788
        } else if resolved == zero_clock_target {
            0xdead_beef_cafe_f00d
        } else if resolved == pad_zero_target {
            0x55aa_bb00_00dd_ee11
        } else if resolved == pad_one_target {
            0x0bad_f00d_c001_d00d
        } else if resolved == route_target {
            0xcafe_babe_face_feed
        } else {
            assert_eq!(resolved, header_target);
            0x99aa_bbcc_ddee_ff00
        }
    });
}

#[test]
fn nested_array_symbolic_materialization_realizes_on_both_linux_isas() {
    // Nested literal arrays under the general recursive rule, end to end:
    // `matrix: [[Choice; 2]; 2]` occupies exactly the extent `[Choice; 4]`
    // would, so the recursive projection retains one packed row — element
    // count 4, the innermost element's stride — and the symbolic path spells
    // the flat leaf index `matrix[k]` where `k = outer * 2 + inner`. The
    // same flattening carries `rows: [[Neighbor; 1]; 2]`'s record element:
    // count 2 at Neighbor's stride, the element's own leaf report retained
    // once for every packed index.
    let main_path = write_program(
        "nested-array-symbolic-field",
        r#"
data Choice [copy] {
    case #1 Empty;
    case #2 Run(#3 callback: u64, #4 clock: u64);
}
data Neighbor [copy] {
    #1 choice: Choice;
    #2 pad: u64;
}
data OnlyMatrix [copy] {
    #1 header: u64;
    #2 matrix: [[Choice; 2]; 2];
}
data Outer [copy] {
    #1 header: u64;
    #2 matrix: [[Choice; 2]; 2];
    #3 rows: [[Neighbor; 1]; 2];
    #4 route: Choice;
}
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("a nested-array record should check");
    let plan = build_layout_plan(&checked, NativeTarget::linux_x64(), &[])
        .expect("the nested-array record should lay out");
    let arm_plan = build_layout_plan(&checked, NativeTarget::linux_arm64(), &[])
        .expect("the nested-array record should lay out for linux_arm64");
    // The packed row is the recursive owner's shape alone: both standalone
    // owners keep their single literal element hop and refuse a field whose
    // element resolves only after a second array level.
    let only_matrix = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "OnlyMatrix")
        .expect("the scalar-and-nested-array record");
    let direct_error = layout::project_conventional_record_with_sum_materialization_layout(
        &checked,
        &plan,
        only_matrix.symbol,
    )
    .expect_err("the direct-sum owner must refuse a nested literal array");
    assert!(
        direct_error
            .message
            .contains("reaches a sum through an array deeper than one literal element hop"),
        "{direct_error:?}"
    );
    let array_error = layout::project_conventional_record_with_sum_arrays_materialization_layout(
        &checked,
        &plan,
        only_matrix.symbol,
    )
    .expect_err("the sum-array owner must refuse a nested literal array");
    assert!(
        array_error
            .message
            .contains("reaches a sum through an array deeper than one literal element hop"),
        "{array_error:?}"
    );
    let owner = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Outer")
        .expect("the outer record");
    let paths =
        layout::project_conventional_record_with_recursive_nested_sums_materialization_layout(
            &checked,
            &plan,
            owner.symbol,
        )
        .expect("a record reaching sums through nested literal arrays should project");
    let arm_paths =
        layout::project_conventional_record_with_recursive_nested_sums_materialization_layout(
            &checked,
            &arm_plan,
            owner.symbol,
        )
        .expect("the same recursive projection closes on linux_arm64");
    assert_eq!(
        paths, arm_paths,
        "both Linux ISAs retain the same recursive path geometry"
    );
    assert_eq!(
        paths
            .outer_layout()
            .entries
            .iter()
            .map(|entry| (entry.field.as_str(), entry.placement))
            .collect::<Vec<_>>(),
        vec![
            ("header", LayoutPlacementReport::At { offset: 0 }),
            ("matrix", LayoutPlacementReport::At { offset: 8 }),
            ("rows", LayoutPlacementReport::At { offset: 104 }),
            ("route", LayoutPlacementReport::At { offset: 168 }),
        ]
    );
    let ConventionalRecursiveRecordSumPathsLayoutReport::Leaf {
        child_sum_layouts,
        child_sum_array_layouts,
        child_record_array_layouts,
        ..
    } = &paths
    else {
        panic!("the outer record is a leaf level: no deeper record paths");
    };
    // `matrix` packs `2 * 2 = 4` elements at the innermost Choice stride (24)
    // spanning 8..104; `rows` packs `2 * 1 = 2` Neighbor elements at stride
    // 32 spanning 104..168; the level's own direct sum `route` sits at 168.
    assert_eq!(
        child_sum_layouts
            .iter()
            .map(|row| (row.field.as_str(), row.member_identity))
            .collect::<Vec<_>>(),
        vec![("route", Some(4))]
    );
    assert_eq!(
        child_sum_array_layouts
            .iter()
            .map(|row| {
                (
                    row.field.as_str(),
                    row.member_identity,
                    row.element_count,
                    row.element_stride,
                )
            })
            .collect::<Vec<_>>(),
        vec![("matrix", Some(2), 4, 24)]
    );
    assert_eq!(
        child_record_array_layouts
            .iter()
            .map(|row| {
                (
                    row.field.as_str(),
                    row.member_identity,
                    row.element_count,
                    row.element_stride,
                )
            })
            .collect::<Vec<_>>(),
        vec![("rows", Some(3), 2, 32)]
    );
    let ConventionalRecursiveRecordSumPathsLayoutReport::Leaf {
        child_sum_layouts: element_sums,
        ..
    } = &child_record_array_layouts[0].inner
    else {
        panic!("the record-array element record is a leaf level");
    };
    assert_eq!(
        element_sums
            .iter()
            .map(|row| (row.field.as_str(), row.member_identity))
            .collect::<Vec<_>>(),
        vec![("choice", Some(1))]
    );
    let carriers = SymbolicFieldInnerLayout::from_recursive_sum_paths(&paths)
        .expect("the recursive report folds the packed nested-array carriers");

    let header_target = RelocationTarget::Data(
        DataSymbolId::from_normalized_identity(0x5a5a).expect("normalized data identity"),
    );
    let matrix_one_target = RelocationTarget::Data(
        DataSymbolId::from_normalized_identity(0xbeef).expect("normalized data identity"),
    );
    let matrix_three_target = RelocationTarget::Data(
        DataSymbolId::from_normalized_identity(0x0bad).expect("normalized data identity"),
    );
    let row_zero_target = RelocationTarget::Data(
        DataSymbolId::from_normalized_identity(0x7ea7).expect("normalized data identity"),
    );
    let row_one_target = RelocationTarget::Entry(
        EntryStubId::from_normalized_identity(0x55aa).expect("normalized entry identity"),
    );
    let route_target = RelocationTarget::Entry(
        EntryStubId::from_normalized_identity(0x77ee).expect("normalized entry identity"),
    );
    let symbolic = [
        SymbolicFieldValue::new_numbered("header", 1, 64, header_target)
            .expect("numbered scalar field"),
        // `matrix[1]` is source element `[0][1]`; `matrix[3]` is `[1][1]`.
        SymbolicFieldValue::new_indexed_numbered("matrix", 2, 1, 64, matrix_one_target)
            .expect("indexed packed sum-array field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new_numbered("Run", 2)
                    .with_inner_segment(SymbolicFieldPathSegment::new_numbered("clock", 4)),
            ),
        SymbolicFieldValue::new_indexed_numbered("matrix", 2, 3, 64, matrix_three_target)
            .expect("indexed packed sum-array field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new_numbered("Run", 2)
                    .with_inner_segment(SymbolicFieldPathSegment::new_numbered("callback", 3)),
            ),
        SymbolicFieldValue::new_indexed_numbered("rows", 3, 0, 64, row_zero_target)
            .expect("indexed packed record-array field")
            .with_inner_segment(SymbolicFieldPathSegment::new_numbered("pad", 2)),
        SymbolicFieldValue::new_indexed_numbered("rows", 3, 1, 64, row_one_target)
            .expect("indexed packed record-array field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new_numbered("choice", 1).with_inner_segment(
                    SymbolicFieldPathSegment::new_numbered("Run", 2)
                        .with_inner_segment(SymbolicFieldPathSegment::new_numbered("callback", 3)),
                ),
            ),
        SymbolicFieldValue::new_numbered("route", 4, 64, route_target)
            .expect("numbered direct sum field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new_numbered("Run", 2)
                    .with_inner_segment(SymbolicFieldPathSegment::new_numbered("callback", 3)),
            ),
    ];
    let materialization = derive_symbolic_materialization_with_inner_layouts(
        paths.outer_layout(),
        &carriers,
        &symbolic,
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: Some(64),
            placement: layout_plans::PlacementConstraints::unconstrained(
                layout_plans::PlacementPhase::PostHandoff,
            ),
        },
        |_| None,
    )
    .expect("nested literal arrays fold into the packed hop vocabulary");
    let writes = materialization
        .actions
        .iter()
        .map(|action| match action {
            MaterializationAction::RuntimeWriter(write) => {
                (write.field.as_str(), write.container_byte_offset)
            }
            other => panic!("unresolved nested-array paths derive writers, found {other:?}"),
        })
        .collect::<Vec<_>>();
    // `matrix` spans 8..104 at a 24-byte stride, so `matrix[1].Run.clock`
    // composes `8 + 1*24 + 16 = 48` and `matrix[3].Run.callback` composes
    // `8 + 3*24 + 8 = 88`; `rows` spans 104..168 at a 32-byte stride with
    // `choice` at 0 and `pad` at 24 inside each element.
    assert_eq!(
        writes,
        [
            ("header", 0),
            ("matrix[1].Run.clock", 48),
            ("matrix[3].Run.callback", 88),
            ("rows[0].pad", 128),
            ("rows[1].choice.Run.callback", 144),
            ("route.Run.callback", 176),
        ]
    );

    // The flat packed index keeps the row's exact bound: index 4 names a
    // fifth packed element the `[[Choice; 2]; 2]` field does not carry.
    let out_of_range = derive_symbolic_materialization_with_inner_layouts(
        paths.outer_layout(),
        &carriers,
        &[
            SymbolicFieldValue::new_indexed_numbered("matrix", 2, 4, 64, matrix_three_target)
                .expect("indexed packed sum-array field")
                .with_inner_segment(
                    SymbolicFieldPathSegment::new_numbered("Run", 2)
                        .with_inner_segment(SymbolicFieldPathSegment::new_numbered("callback", 3)),
                ),
        ],
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: Some(64),
            placement: layout_plans::PlacementConstraints::unconstrained(
                layout_plans::PlacementPhase::PostHandoff,
            ),
        },
        |_| None,
    )
    .expect_err("a packed index past the hop product must reject");
    assert!(
        out_of_range
            .0
            .contains("element index 4 is outside its 4 element placements"),
        "{out_of_range:?}"
    );

    let writer = materialization
        .derive_post_handoff_writer()
        .expect("the nested-array boundary writes derive a writer");
    let mut expected = vec![0xa5_u8; 192];
    expected[0..8].copy_from_slice(&0x99aa_bbcc_ddee_ff00_u64.to_le_bytes());
    expected[48..56].copy_from_slice(&0xdead_beef_cafe_f00d_u64.to_le_bytes());
    expected[88..96].copy_from_slice(&0x0bad_f00d_c001_d00d_u64.to_le_bytes());
    expected[128..136].copy_from_slice(&0x7ea7_a55a_5aa5_a5a5_u64.to_le_bytes());
    expected[144..152].copy_from_slice(&0x1122_3344_5566_7788_u64.to_le_bytes());
    expected[176..184].copy_from_slice(&0xcafe_babe_face_feed_u64.to_le_bytes());
    lower_writer_on_both_linux_isas(&writer, 0xa5, &expected, |resolved| {
        if resolved == row_one_target {
            0x1122_3344_5566_7788
        } else if resolved == matrix_one_target {
            0xdead_beef_cafe_f00d
        } else if resolved == matrix_three_target {
            0x0bad_f00d_c001_d00d
        } else if resolved == row_zero_target {
            0x7ea7_a55a_5aa5_a5a5
        } else if resolved == route_target {
            0xcafe_babe_face_feed
        } else {
            assert_eq!(resolved, header_target);
            0x99aa_bbcc_ddee_ff00
        }
    });
}
