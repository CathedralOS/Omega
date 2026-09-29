//! Allocation-recovery storage through stack provisioning, end to end.
//!
//! A source program whose entry keeps more integer values live than any
//! target has allocatable registers compiles through the common physical
//! pipeline. Register allocation fails direct assignment with
//! `NoCompatibleHome` and recovers through `assignment::runtime_spill`, whose
//! rewrite adds eight-byte `LocalStorageSlotId::Spill` homes. This test pins
//! that those homes are not a side table: they are members of the final
//! `FunctionTargetFrameLayout`, the fragment route publishes that frame's
//! `frame_size_bytes` as the function's stack record
//! (`function_fragments/production.rs`), `function_fragments/validation/stack.rs`
//! replays the record against the same frame, `derive_stack_demand` composes
//! it, the hosted receiver bridge sizes its private stack partition from that
//! demand, and `derive_installation_stack_demand` recomposes the identical
//! demand from the installation record.
//!
//! No join here is new. The existing demand/supply joins are reused under the
//! compiler-owned stack contract (`wiki/spec/resources/storage.md`); ABI
//! entry-spill fixtures do not reach the allocator's recovery route, so this
//! fixture is the allocation-recovery coverage for them.

use super::fragment_container::compile_attached_entry;
use super::hosted_exit_runtime::assert_exit;
use super::hosted_receiver::bind_receiver;
use image_emission::{
    build_installation_record, derive_installation_stack_demand, derive_stack_demand,
    emit_direct_executable_image, validate_direct_executable_image,
    validate_function_fragment_object_artifact,
};
use selected_instructions::LocalStorageSlotId;
use semantic_vocabulary::ProfileDecisionId;
use target::TargetProfile;

/// More simultaneously live 64-bit values than the 30 AArch64 or 15 x86-64
/// general registers can hold, so every target reaches runtime spill.
const LIVE_VALUES: usize = 40;

/// Every value is loaded and offset before the first is consumed, so all of
/// them are live across the whole summation chain. Wrapping arithmetic keeps
/// the program free of proof obligations the fixture does not care about.
fn pressure_source() -> String {
    let fields = (0..LIVE_VALUES)
        .map(|index| format!("a{index}: i64 in Wrapping; "))
        .collect::<String>();
    let values = (0..LIVE_VALUES)
        .map(|index| {
            format!(
                "        let v{index}: i64 in Wrapping = self.a{index} + {};\n",
                index + 1
            )
        })
        .collect::<String>();
    let sum = (0..LIVE_VALUES)
        .map(|index| format!("v{index}"))
        .collect::<Vec<_>>()
        .join(" + ");
    format!(
        "    data Main {{ {fields}out: i64 in Wrapping; }}\n    machine Main::launch(&mut self) {{\n{values}        self.out = {sum};\n    }}\n"
    )
}

#[test]
fn runtime_spilled_entry_frame_reaches_stack_demand_and_receiver_supply() {
    let source = pressure_source();
    for profile in [
        TargetProfile::MacosArm64,
        TargetProfile::LinuxArm64,
        TargetProfile::LinuxX64,
    ] {
        let compiled = compile_attached_entry(&source, "Main::launch", profile);
        let entry = compiled.artifact.entry();
        let frames = compiled
            .container
            .source()
            .source()
            .source()
            .source()
            .frame_layout();
        let frame = frames
            .functions
            .iter()
            .find(|frame| frame.machine == entry)
            .expect("the entry has one final frame");

        // Recovery storage is part of the final frame. Each slot is counted
        // once: distinct slots occupy disjoint extents inside the frame, so a
        // reused slot cannot be charged twice and no slot lies outside it.
        let mut spill_extents = frame
            .local_storage_slots
            .iter()
            .filter(|slot| matches!(slot.id, LocalStorageSlotId::Spill { .. }))
            .map(|slot| {
                (
                    slot.frame_offset_bytes,
                    slot.frame_offset_bytes + u64::from(slot.size_bytes),
                )
            })
            .collect::<Vec<_>>();
        assert!(
            !spill_extents.is_empty(),
            "{profile:?} register pressure must reach runtime spill storage in the final frame",
        );
        spill_extents.sort_unstable();
        assert!(
            spill_extents.windows(2).all(|pair| pair[0].1 <= pair[1].0),
            "{profile:?} spill slots occupy disjoint frame extents: {spill_extents:?}",
        );
        let spill_end = spill_extents.last().map_or(0, |extent| extent.1);
        assert!(
            spill_end <= frame.frame_size_bytes,
            "{profile:?} spill storage lies inside the {}-byte frame",
            frame.frame_size_bytes,
        );

        // The published stack record is that final frame, including any
        // red-zone-resident bytes the prologue never commits.
        let function = compiled
            .artifact
            .functions()
            .iter()
            .find(|function| function.machine == entry)
            .expect("published entry function");
        let stack = function
            .unit_stack
            .expect("a Unit entry publishes its frame record");
        assert_eq!(u64::from(stack.frame_bytes), frame.frame_size_bytes);
        assert!(u64::from(stack.local_peak_bytes) >= frame.frame_size_bytes);

        let demand = derive_stack_demand(&compiled.artifact, entry).expect("entry stack demand");
        assert!(demand.contributing_machines().contains(&entry));
        assert!(
            demand.ceiling_bytes() >= spill_end,
            "{profile:?} demand {} covers the spilled frame extent {spill_end}",
            demand.ceiling_bytes(),
        );

        // A frame record that forgets recovery storage is stale against the
        // retained final frame and cannot publish.
        let mut forgotten = compiled.artifact.clone();
        let changed = forgotten
            .functions_mut_for_test()
            .iter_mut()
            .find(|function| function.machine == entry)
            .and_then(|function| function.unit_stack.as_mut())
            .expect("entry frame record");
        changed.frame_bytes -= 16;
        changed.local_peak_bytes -= 16;
        assert!(
            validate_function_fragment_object_artifact(&compiled.container, &forgotten).is_err(),
            "{profile:?} a frame record without its spill bytes rejects",
        );

        // Supply: the receiver bridge binds exactly the re-derived demand and
        // its private stack partition is sized from it.
        let mut artifact = compiled.artifact;
        bind_receiver(
            &mut artifact,
            &compiled.signature,
            &compiled.checked_entry,
            profile,
            false,
        );
        let binding = artifact
            .hosted_receiver_binding()
            .expect("the bound receiver is retained");
        assert_eq!(binding.stack_demand(), &demand);
        let image = emit_direct_executable_image(&artifact, 3).expect("receiver image emits");
        validate_direct_executable_image(&artifact, &image)
            .expect("the emitted bridge replays its partitions");

        // Insufficient supply rejects before any bytes execute: a binding whose
        // ceiling no longer covers the spilled frame neither emits nor
        // validates against the image built from the true demand.
        let mut starved = artifact.clone();
        *starved
            .hosted_receiver_stack_ceiling_mut_for_test()
            .expect("bound ceiling") = frame.frame_size_bytes - 16;
        assert!(
            emit_direct_executable_image(&starved, 3).is_err(),
            "{profile:?} a ceiling below the spilled frame cannot emit",
        );
        assert!(
            validate_direct_executable_image(&starved, &image).is_err(),
            "{profile:?} a ceiling below the spilled frame cannot replay",
        );

        // The installation record recomposes the same closure from its
        // retained function records.
        let record = build_installation_record(&image, ProfileDecisionId::new(1).expect("profile"))
            .expect("installation record");
        assert_eq!(
            derive_installation_stack_demand(&record, &image, entry)
                .expect("installed stack demand"),
            demand,
        );

        if profile == TargetProfile::LinuxX64
            && cfg!(all(target_os = "linux", target_arch = "x86_64"))
        {
            // The spilled entry runs on the bridge's private stack and the
            // bridge exits with status zero.
            assert_exit(&image.output().bytes, 0);
        }
    }
}
