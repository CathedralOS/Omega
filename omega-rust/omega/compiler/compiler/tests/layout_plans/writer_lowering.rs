use super::{lower_writer_on_both_linux_isas, write_program};
use build_time_evaluation::compute_layout_plan;
use compiler::{CheckedCompileRequest, compile_to_checked};
use layout::build_layout_plan;
use layout_plans::{
    ByteOrder, ConsumptionInstant, DataSymbolId, EntryStubId, LayoutPlacementReport,
    MaterializationAction, MaterializationContext, RelocationTarget, SymbolicFieldInnerLayout,
    SymbolicFieldPathSegment, SymbolicFieldValue, derive_symbolic_materialization,
    derive_symbolic_materialization_with_inner_layouts,
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
    let report = compute_layout_plan(&checked.typed, "DispatchLayout::plan", "DispatchTable")
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
    let report = compute_layout_plan(&checked.typed, "DispatchLayout::plan", "DispatchTable")
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
    let inner_report = compute_layout_plan(&checked.typed, "SlotLayout::plan", "DispatchSlot")
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
    let report = compute_layout_plan(&checked.typed, "DispatchLayout::plan", "DispatchTable")
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
    let inner_report = compute_layout_plan(&checked.typed, "SlotLayout::plan", "DispatchSlot")
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
    let report = compute_layout_plan(&checked.typed, "DispatchLayout::plan", "DispatchTable")
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
    let slot_report = compute_layout_plan(&checked.typed, "SlotLayout::plan", "DispatchSlot")
        .expect("the slot record's own policy supplies its interior geometry");
    let inner_report = compute_layout_plan(&checked.typed, "InnerLayout::plan", "DispatchInner")
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
