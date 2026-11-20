//! A 16-byte interrupt gate stores three pieces of a private address beside four public fields.
//! The native layout must retain the exact three fragments the policy placed.

#[path = "../../../../tests/support/fixture_repository.rs"]
mod fixture_repository;

use diagnostics::Diagnostic;
use layout_plans::LayoutPlacementReport;
use std::path::Path;

fn gate_placements(main: &Path) -> Result<Vec<LayoutPlacementReport>, Vec<Diagnostic>> {
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            let product = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(main, Some("linux_x86_64")),
            )
            .result
            .map_err(|failure| failure.into_diagnostics())?;
            let module = product.terminal_publication().accepted().unwrap().module();
            let layouts = &module.layouts.native_layouts;
            let mut slots = layouts
                .records()
                .filter_map(|record| layouts.for_target(record, "linux_x86_64").unwrap())
                .flat_map(|realization| &realization.report.private_callback_demands);
            let slot = slots
                .next()
                .expect("the selected gate has a private callback");
            assert!(
                slots.next().is_none(),
                "one callback supplies all three pieces"
            );
            assert!(slot.slot_application.has_complete_identity());
            let overload = slot.slot_application.requirement_identity.as_bytes();
            assert!(
                slot.slot_application
                    .application_encoding
                    .windows(overload.len())
                    .any(|part| part == overload),
                "the source commitment must bind the exact callback overload"
            );
            Ok(slot.placements.clone())
        })
        .unwrap()
        .join()
        .unwrap()
    })
}

#[test]
fn interrupt_gate_retains_the_executed_fragments_of_its_own_callback() {
    let root = fixture_repository::root();
    let fixture = root.join("source-rust/subsystems/compiler/tests/fixtures/layouts/pass/private_callback_gate");
    let placements = gate_placements(&fixture.join("main.omg")).unwrap();
    let bits = |container, width, source_lsb| LayoutPlacementReport::Bits {
        container,
        container_width: width,
        destination_lsb: 0,
        source_lsb,
        width,
    };
    assert_eq!(
        placements,
        [bits(0, 16, 0), bits(6, 16, 16), bits(8, 32, 32)]
    );

    let source = std::fs::read_to_string(fixture.join("main.omg")).unwrap();
    let scratch = root
        .join("build")
        .join(format!("private-callback-gate-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    std::fs::copy(fixture.join("build.omg"), scratch.join("build.omg")).unwrap();
    let main = scratch.join("main.omg");
    let (_, body) = source.split_once("let low: LayoutPlan = ").unwrap();
    let (low, _) = body.split_once(';').unwrap();
    let partial = low.replace("source_lsb: 0, width: 16", "source_lsb: 0, width: 1");
    assert_ne!(low, partial);
    let branch =
        format!("match schema.field_count == 4u64 {{ true -> {low}, false -> {partial} }}");
    std::fs::write(&main, source.replace(low, &branch)).unwrap();
    assert_eq!(
        gate_placements(&main).expect("the untaken branch cannot place an extra address bit"),
        placements
    );
    let incomplete = source.replace("source_lsb: 32, width: 32", "source_lsb: 32, width: 31");
    assert_ne!(source, incomplete);
    std::fs::write(&main, incomplete).unwrap();
    let errors =
        gate_placements(&main).expect_err("one missing address bit must refuse the layout");
    assert!(
        errors.iter().any(|d| d
            .message
            .contains("do not cover the complete target pointer")),
        "{errors:?}"
    );
    for (original, substitute, expected_error) in [
        (
            "public GateSlot00 satisfies PrivateCallbackSlot<InterruptGate,",
            "public data OtherGate { selector: u16; }\npublic GateSlot00 satisfies PrivateCallbackSlot<OtherGate,",
            "another semantic type",
        ),
        (
            "public GateSlot00 satisfies PrivateCallbackSlot<InterruptGate, GateLayout00, FatalVector00::enter>",
            "public machine local() {}\npublic GateSlot00 satisfies PrivateCallbackSlot<InterruptGate, GateLayout00, local>",
            "private callback slot requirement has no boundary trait owner",
        ),
    ] {
        assert_eq!(source.matches(original).count(), 1);
        std::fs::write(&main, source.replace(original, substitute)).unwrap();
        let errors = gate_placements(&main).expect_err("a substituted slot must refuse");
        assert!(
            errors.iter().any(|d| d.message.contains(expected_error)),
            "{errors:?}"
        );
    }
    let ordinary = "\npublic machine ordinary(plan: LayoutPlan, field: FieldPlan) -> LayoutPlan {\n    layout_plan_place_private_field::<GateSlot00>(plan, field)\n}\n";
    let (_, policy) = source.split_once("public GateLayout00 satisfies ").unwrap();
    let (policy, _) = policy.split_once("\nmachine main()").unwrap();
    let other_owner = format!("\npublic OtherGatePolicy satisfies {policy}\n");
    for (declaration, expected_error) in [
        (ordinary, "outside its owning fixed layout policy"),
        (
            other_owner.as_str(),
            "belongs to another fixed layout policy",
        ),
    ] {
        std::fs::write(&main, format!("{source}{declaration}")).unwrap();
        let errors = gate_placements(&main).expect_err("only a slot's owning policy can place it");
        assert!(
            errors.iter().any(|d| d.message.contains(expected_error)),
            "{errors:?}"
        );
    }
    std::fs::remove_dir_all(scratch).unwrap();
}
