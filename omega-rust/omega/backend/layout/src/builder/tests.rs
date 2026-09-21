//! Layout builder tests.

use super::build_layout_plan;
use crate::{DataShape, FieldLayout, TypeLayout};
use checked_trees::{CheckFacts, CheckedTrees};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use target::NativeTarget;
use tokens_to_syntax_trees::parse_syntax_trees;

fn checked(source: &str) -> CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    CheckedTrees::with_roots(typed, CheckFacts::default())
}

fn private_callback_layout(offset: u64, size: usize) -> typed_trees::typed_trees::PlanLaidLayout {
    typed_trees::typed_trees::PlanLaidLayout {
        data_name: "Spread<ForeignRecord>".to_owned(),
        data_symbol: symbols::SymbolHandle::from_arena_index(1),
        field_symbols: vec![symbols::SymbolHandle::from_arena_index(2)],
        schema_symbol: symbols::SymbolHandle::from_arena_index(3),
        schema_field_symbols: vec![symbols::SymbolHandle::from_arena_index(4)],
        policy_symbol: symbols::SymbolHandle::from_arena_index(5),
        policy_plan_machine_symbol: symbols::SymbolHandle::from_arena_index(6),
        validated_layout: layout_plans::LayoutPlanReport {
            schema_report_fingerprint: 0x51,
            entries: vec![layout_plans::LayoutFieldEntryReport {
                field: "payload".to_owned(),
                member_identity: None,
                placement: layout_plans::LayoutPlacementReport::At { offset: 0 },
            }],
            offsets: Some(vec![0]),
            size: Some(size as u64),
            align: 8,
        },
        private_callback_demands: vec![layout_plans::PrivateCallbackLayoutDemandReport {
            slot_application: typed_trees::typed_trees::ClosedConformanceApplication {
                declaration: symbols::SymbolHandle::from_arena_index(7),
                arguments: Box::new([]),
                lifetime_arguments: Vec::new(),
                type_arguments: Vec::new(),
                const_arguments: Vec::new(),
                machine_arguments: Vec::new(),
                subject_identity: Some("package::Spread".to_owned()),
                trait_definition: symbols::SymbolHandle::from_arena_index(8),
                trait_lifetime_arguments: Vec::new(),
                trait_arguments: vec!["package::WindowProcedure::call#exact".to_owned()],
                rows: Vec::new(),
                report_fingerprint: 0x91,
                commitment:
                    typed_trees::typed_trees::ClosedConformanceApplicationCommitment::from_digest(
                        [0x92; 32],
                    ),
            },
            slot_identity: "package::WndClassWindowProcedureSlot#exact".to_owned(),
            layout_subject_identity: "package::Spread".to_owned(),
            callback_requirement_identity: "package::WindowProcedure::call#exact".to_owned(),
            offset,
        }],
        offsets: vec![0],
        bit_fields: Vec::new(),
        integer_fields: Vec::new(),
        repeated_fields: Vec::new(),
        size,
        align: 8,
    }
}

fn private_callback_fields() -> [FieldLayout; 1] {
    [FieldLayout {
        symbol: symbols::SymbolHandle::from_arena_index(2),
        name: checked_trees::name::Identifier::from("payload"),
        offset: 0,
        layout: TypeLayout {
            size: 8,
            alignment: 8,
        },
        ..FieldLayout::default()
    }]
}

fn close_private_callback_demands(
    plan: &typed_trees::typed_trees::PlanLaidLayout,
    fields: &[FieldLayout],
    target: NativeTarget,
    canonical_layout_subject: &str,
) -> Result<Vec<crate::TargetClosedPrivateCallbackDemand>, diagnostics::Diagnostic> {
    let fingerprint = layout_plans::normalized_native_layout_plan_report_fingerprint(
        &layout_plans::NativeLayoutPlanReport {
            layout: plan.validated_layout.clone(),
            private_callback_demands: plan.private_callback_demands.clone(),
        },
    );
    let layout = calling_conventions::callback_layout_plan_id(
        fingerprint,
        target.pointer_size,
        target.pointer_alignment,
    );
    super::private_callback_closure::close_private_callback_demands(
        plan,
        fields,
        target,
        canonical_layout_subject,
        layout,
    )
}

#[test]
fn target_closes_private_callback_geometry_and_rejects_exact_mutations() {
    let target = NativeTarget::windows_x64();
    let fields = private_callback_fields();
    let valid = private_callback_layout(8, 16);
    let [closed] = close_private_callback_demands(&valid, &fields, target, "package::Spread")
        .expect("aligned private callback padding should close")
        .try_into()
        .expect("one private callback demand");
    assert_eq!(
        (closed.offset, closed.byte_size, closed.alignment),
        (8, 8, 8)
    );
    assert_eq!(
        closed.slot_application,
        valid.private_callback_demands[0].slot_application
    );
    assert_eq!(
        closed.requirement,
        calling_conventions::callback_requirement_id("package::WindowProcedure::call#exact")
    );

    let moved = close_private_callback_demands(
        &private_callback_layout(16, 24),
        &fields,
        target,
        "package::Spread",
    )
    .expect("another valid offset should close");
    assert_ne!(closed.layout, moved[0].layout);
    assert_ne!(closed.slot, moved[0].slot);
    assert_eq!(closed.slot_application, moved[0].slot_application);

    // Geometry/report coordinates do not replace exact retained selection.
    // This fixture changes only custody to pin the compatibility report's
    // deliberately unchanged inputs, not to claim application validation.
    let mut changed_application = valid.clone();
    changed_application.private_callback_demands[0]
        .slot_application
        .declaration = symbols::SymbolHandle::from_arena_index(9);
    changed_application.private_callback_demands[0]
        .slot_application
        .commitment =
        typed_trees::typed_trees::ClosedConformanceApplicationCommitment::from_digest([0x93; 32]);
    let changed =
        close_private_callback_demands(&changed_application, &fields, target, "package::Spread")
            .expect("retaining application custody does not re-evaluate its declaration");
    assert_eq!(closed.layout, changed[0].layout);
    assert_eq!(closed.slot, changed[0].slot);
    assert_eq!(closed.requirement, changed[0].requirement);
    assert_ne!(closed.slot_application, changed[0].slot_application);
    assert_eq!(
        changed[0].slot_application,
        changed_application.private_callback_demands[0].slot_application
    );

    let unaligned = close_private_callback_demands(
        &private_callback_layout(9, 24),
        &fields,
        target,
        "package::Spread",
    )
    .expect_err("unaligned callback slot must reject");
    assert!(unaligned.message.contains("is not aligned"));

    let outside = close_private_callback_demands(
        &private_callback_layout(16, 16),
        &fields,
        target,
        "package::Spread",
    )
    .expect_err("out-of-bounds callback slot must reject");
    assert!(outside.message.contains("lies outside its 16-byte layout"));

    let semantic_overlap = close_private_callback_demands(
        &private_callback_layout(0, 16),
        &fields,
        target,
        "package::Spread",
    )
    .expect_err("semantic/private overlap must reject");
    assert!(
        semantic_overlap
            .message
            .contains("overlaps semantic field storage")
    );

    let mut private_overlap = valid;
    let mut second = private_overlap.private_callback_demands[0].clone();
    second.slot_identity.push_str("::Second");
    private_overlap.private_callback_demands.push(second);
    let private_overlap =
        close_private_callback_demands(&private_overlap, &fields, target, "package::Spread")
            .expect_err("private/private overlap must reject");
    assert!(
        private_overlap.message.contains("private callback slots")
            && private_overlap.message.contains("overlap")
    );

    let subject_tamper = close_private_callback_demands(
        &private_callback_layout(8, 16),
        &fields,
        target,
        "package::OtherLayout",
    )
    .expect_err("layout-subject substitution must reject");
    assert!(subject_tamper.message.contains("changed layout subject"));

    let mut duplicate_slot = private_callback_layout(8, 24);
    let mut conflicting = duplicate_slot.private_callback_demands[0].clone();
    conflicting
        .callback_requirement_identity
        .push_str("::Other");
    conflicting.offset = 16;
    duplicate_slot.private_callback_demands.push(conflicting);
    let duplicate_slot =
        close_private_callback_demands(&duplicate_slot, &fields, target, "package::Spread")
            .expect_err("one canonical slot cannot close under conflicting requirements");
    assert!(
        duplicate_slot
            .message
            .contains("repeats private callback slot")
    );
}

#[test]
fn transparent_record_layout_excludes_erased_fields() {
    let source = r#"
        data Packed {
            head: u8;
            proof [erased]: u64;
            tail: u8;
        }
    "#;
    let checked = checked(source);

    let plan = build_layout_plan(&checked, NativeTarget::host(), &[]).expect("layout");
    let packed = plan
        .data_layouts
        .iter()
        .map(|(_, layout)| layout)
        .find(|layout| layout.name.as_str() == "Packed")
        .expect("Packed layout");
    assert_eq!(packed.layout.size, 2);
    assert_eq!(packed.layout.alignment, 1);
    let DataShape::Record { fields } = packed.shape else {
        panic!("Packed should have record layout");
    };
    let fields = plan.fields.span_or_empty(fields);
    assert_eq!(
        fields
            .iter()
            .map(|field| (field.name.as_str(), field.offset))
            .collect::<Vec<_>>(),
        [("head", 0), ("tail", 1)]
    );
}

#[test]
fn attached_machine_layout_excludes_erased_record_fields() {
    let checked = checked(
        r#"
        data Packed {
            head: u8;
            proof [erased]: u64;
            tail: u16;
        }
        machine Packed::read(&self) -> u8 { self.head }
        "#,
    );

    let plan = build_layout_plan(&checked, NativeTarget::host(), &[]).expect("layout");
    let data = plan
        .data_layouts
        .iter()
        .map(|(_, layout)| layout)
        .find(|layout| layout.name.as_str() == "Packed")
        .expect("Packed data layout");
    let machine = plan
        .machine_layouts
        .iter()
        .map(|(_, layout)| layout)
        .find(|layout| layout.name.as_str() == "Packed::read")
        .expect("Packed::read machine layout");

    assert_eq!(machine.layout, data.layout);
    assert_eq!(
        plan.fields
            .span_or_empty(machine.fields)
            .iter()
            .map(|field| (field.name.as_str(), field.offset))
            .collect::<Vec<_>>(),
        [("head", 0), ("tail", 2)]
    );
}

#[test]
fn pure_sum_layout_excludes_erased_payloads_without_changing_variants() {
    let checked = checked(
        r#"
        data Message {
            case Empty;
            case Data(value: u8, proof [erased]: u64);
            case ProofOnly(proof [erased]: u64);
        }
        "#,
    );
    let plan = build_layout_plan(&checked, NativeTarget::host(), &[]).expect("layout");
    let message = plan
        .data_layouts
        .iter()
        .map(|(_, layout)| layout)
        .find(|layout| layout.name.as_str() == "Message")
        .expect("Message layout");
    assert_eq!(message.layout.size, 8);
    assert_eq!(message.layout.alignment, 4);
    let DataShape::Enum {
        common_fields,
        variants,
    } = message.shape
    else {
        panic!("Message should have case layout");
    };
    assert!(plan.fields.span_or_empty(common_fields).is_empty());
    let variants = plan.variants.span_or_empty(variants);
    assert_eq!(
        variants
            .iter()
            .map(|variant| variant.name.as_str())
            .collect::<Vec<_>>(),
        ["Empty", "Data", "ProofOnly"]
    );
    assert!(plan.fields.span_or_empty(variants[0].fields).is_empty());
    assert_eq!(
        plan.fields
            .span_or_empty(variants[1].fields)
            .iter()
            .map(|field| (field.name.as_str(), field.offset))
            .collect::<Vec<_>>(),
        [("value", 4)]
    );
    assert!(plan.fields.span_or_empty(variants[2].fields).is_empty());
}

#[test]
fn mixed_case_layout_excludes_erased_common_and_payload_fields() {
    let checked = checked(
        r#"
        data Event {
            sequence: u8;
            common_proof [erased]: u64;
            case Ready(value: u16, payload_proof [erased]: u64);
            case Waiting(code: u8);
        }
        "#,
    );
    let plan = build_layout_plan(&checked, NativeTarget::host(), &[]).expect("layout");
    let event = plan
        .data_layouts
        .iter()
        .map(|(_, layout)| layout)
        .find(|layout| layout.name.as_str() == "Event")
        .expect("Event layout");
    assert_eq!(event.layout.size, 8);
    assert_eq!(event.layout.alignment, 4);
    let DataShape::Enum {
        common_fields,
        variants,
    } = event.shape
    else {
        panic!("Event should have mixed case layout");
    };
    assert_eq!(
        plan.fields
            .span_or_empty(common_fields)
            .iter()
            .map(|field| (field.name.as_str(), field.offset))
            .collect::<Vec<_>>(),
        [("sequence", 4)]
    );
    let variants = plan.variants.span_or_empty(variants);
    assert_eq!(
        variants
            .iter()
            .map(|variant| variant.name.as_str())
            .collect::<Vec<_>>(),
        ["Ready", "Waiting"]
    );
    assert_eq!(
        plan.fields
            .span_or_empty(variants[0].fields)
            .iter()
            .map(|field| (field.name.as_str(), field.offset))
            .collect::<Vec<_>>(),
        [("value", 6)]
    );
    assert_eq!(
        plan.fields
            .span_or_empty(variants[1].fields)
            .iter()
            .map(|field| (field.name.as_str(), field.offset))
            .collect::<Vec<_>>(),
        [("code", 6)]
    );
}

#[test]
fn full_width_generic_capacity_rejects_placement_instead_of_panicking() {
    // The witnessed defect: `Capacity` bound to `u64::MAX` gives `storage` a
    // full-width extent, so aligning the following `length` field wraps. The
    // layout must reject the placement through its diagnostic vocabulary.
    let checked = checked(
        r#"
        data TinyBytes<Length, const Capacity: u64> {
            storage: [u8; Capacity];
            length: Length;
        }
        data Main {
            bytes: TinyBytes<u64, 18446744073709551615>;
        }
        "#,
    );

    let error = build_layout_plan(&checked, NativeTarget::host(), &[])
        .expect_err("a full-width capacity cannot place the trailing length field");
    assert!(
        error.message.contains("overflows the addressable size")
            && error.message.contains("aligning field `length`"),
        "{}",
        error.message
    );
}

#[test]
fn full_width_case_payload_rejects_placement_instead_of_panicking() {
    // The payload overlay starts after the 4-byte tag, so a full-width array
    // payload ends past the addressable size.
    let checked = checked(
        r#"
        data Slab<const Capacity: u64> {
            case Empty;
            case Full(storage: [u8; Capacity]);
        }
        data Main {
            slab: Slab<18446744073709551615>;
        }
        "#,
    );

    let error = build_layout_plan(&checked, NativeTarget::host(), &[])
        .expect_err("a full-width payload cannot end within the addressable size");
    assert!(
        error.message.contains("overflows the addressable size")
            && error.message.contains("field `storage`"),
        "{}",
        error.message
    );
}

#[test]
fn literal_array_extent_that_wraps_the_record_end_is_rejected() {
    // `2^61 - 1` eight-byte elements occupy `usize::MAX - 7` bytes; the
    // trailing byte then ends at `usize::MAX - 6`, and rounding the record
    // extent up to its 8-byte alignment wraps.
    let checked = checked(
        r#"
        data Wide {
            words: [u64; 2305843009213693951];
            tail: u8;
        }
        "#,
    );

    let error = build_layout_plan(&checked, NativeTarget::host(), &[])
        .expect_err("a record extent past usize::MAX must be rejected");
    assert!(
        error.message.contains("overflows the addressable size")
            && error.message.contains("record extent"),
        "{}",
        error.message
    );
}

#[test]
fn quotient_over_runtime_carrier_lays_out_as_its_representative() {
    let checked = checked(
        r#"
        data Carrier {
            value: u64;
            tag: u8;
        }
        proposition equivalent(left: Carrier, right: Carrier);
        data Quotient = Carrier % equivalent;
        "#,
    );

    let plan = build_layout_plan(&checked, NativeTarget::host(), &[]).expect("layout");
    let carrier = plan
        .data_layouts
        .iter()
        .map(|(_, layout)| layout)
        .find(|layout| layout.name.as_str() == "Carrier")
        .expect("Carrier layout");
    let quotient = plan
        .data_layouts
        .iter()
        .map(|(_, layout)| layout)
        .find(|layout| layout.name.as_str() == "Quotient")
        .expect("a quotient over runtime data is laid out as its representative");
    assert_eq!(quotient.layout, carrier.layout);
    assert_eq!(quotient.shape, carrier.shape);
}

#[test]
fn record_holding_a_realized_quotient_stays_runtime_data() {
    let checked = checked(
        r#"
        data Carrier {
            value: u64;
        }
        proposition equivalent(left: Carrier, right: Carrier);
        data Quotient = Carrier % equivalent;
        data Wrapper {
            head: u8;
            instance: Quotient;
        }
        "#,
    );

    let plan = build_layout_plan(&checked, NativeTarget::host(), &[]).expect("layout");
    let wrapper = plan
        .data_layouts
        .iter()
        .map(|(_, layout)| layout)
        .find(|layout| layout.name.as_str() == "Wrapper")
        .expect("Wrapper layout");
    assert_eq!(wrapper.layout.size, 16);
    assert_eq!(wrapper.layout.alignment, 8);
    let DataShape::Record { fields } = wrapper.shape else {
        panic!("Wrapper should have record layout");
    };
    assert_eq!(
        plan.fields
            .span_or_empty(fields)
            .iter()
            .map(|field| (field.name.as_str(), field.offset))
            .collect::<Vec<_>>(),
        [("head", 0), ("instance", 8)]
    );
}

#[test]
fn quotient_over_proof_only_carrier_stays_proof_only() {
    let checked = checked(
        r#"
        data Infinite {
            next: Infinite;
        }
        proposition equivalent(left: Infinite, right: Infinite);
        data Quotient = Infinite % equivalent;
        "#,
    );

    let plan = build_layout_plan(&checked, NativeTarget::host(), &[]).expect("layout");
    assert!(
        plan.data_layouts
            .iter()
            .map(|(_, layout)| layout)
            .all(|layout| layout.name.as_str() != "Quotient"),
        "a quotient whose carrier has no layout stays proof-only"
    );

    let quotient_symbol = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Quotient")
        .expect("Quotient definition")
        .symbol;
    let classification = checked_trees::proof_only::classify(&checked.typed);
    assert!(matches!(
        classification.reason(quotient_symbol),
        Some(checked_trees::proof_only::ProofOnlyReason::Quotient { .. })
    ));
}

#[test]
fn quotient_of_quotient_realizes_through_the_root_carrier() {
    let checked = checked(
        r#"
        data Carrier {
            value: u64;
        }
        proposition equivalent(left: Carrier, right: Carrier);
        proposition coarser(left: Quotient, right: Quotient);
        data Quotient = Carrier % equivalent;
        data CoarserQuotient = Quotient % coarser;
        "#,
    );

    let plan = build_layout_plan(&checked, NativeTarget::host(), &[]).expect("layout");
    let carrier = plan
        .data_layouts
        .iter()
        .map(|(_, layout)| layout)
        .find(|layout| layout.name.as_str() == "Carrier")
        .expect("Carrier layout");
    for name in ["Quotient", "CoarserQuotient"] {
        let layout = plan
            .data_layouts
            .iter()
            .map(|(_, layout)| layout)
            .find(|layout| layout.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} should be laid out"));
        assert_eq!(layout.layout, carrier.layout, "{name}");
    }
}
