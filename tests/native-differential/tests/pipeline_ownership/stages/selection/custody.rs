//! Optimized selection custody mutation coverage.
//!
//! `stage_optimized_instruction_selection` seals a
//! `StagedOptimizedSelectionCustodyReceipt` pinning the terminal-psi root,
//! target, entry machine, optimization bundle, abstract-plan projection,
//! pre-physical manifest, optimization unit, fuel schedule, register
//! environment, legalized and legalization-validator identities, the selected
//! plan identity, and the function count. The receipt never encodes, so every
//! field is representable in memory and none is canonical-encoding-closed.
//! Its independent checker is `validate_optimized_selection_custody`, which
//! replays legalization and selection from the retained parts and rebuilds
//! the expected receipt; a substitution in any field makes the rebuilt
//! receipt diverge from the retained claim, so the claim is rejected at every
//! honest join that compares them.

use crate::tests::{
    NativeTarget, OptimizedSelectionCustodyFieldForTest, staged_conditional,
    validate_optimized_selection_custody,
};

#[test]
fn optimized_selection_custody_rejects_every_one_field_substitution() {
    use OptimizedSelectionCustodyFieldForTest::*;
    let fields: [(&str, OptimizedSelectionCustodyFieldForTest); 13] = [
        ("psi", Psi),
        ("target", Target),
        ("entry", Entry),
        ("optimization", Optimization),
        ("projection", Projection),
        ("manifest", Manifest),
        ("optimization_unit", OptimizationUnit),
        ("fuel_schedule", FuelSchedule),
        ("register_environment", RegisterEnvironment),
        ("legalized", Legalized),
        ("legalization_validator", LegalizationValidator),
        ("selected", Selected),
        ("function_count", FunctionCount),
    ];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        // An authentic foreign selection on the opposite architecture is the
        // donor for nested-receipt fields and pins that the honest claim
        // differs across targets.
        let donor = staged_conditional(match target.architecture {
            target::Architecture::X86_64 => NativeTarget::linux_arm64(),
            _ => NativeTarget::linux_x64(),
        });
        assert_ne!(
            staged_conditional(target).custody(),
            donor.custody(),
            "{target:?}: the foreign target must produce a distinct custody receipt",
        );
        for (name, field) in fields {
            let mut substituted = staged_conditional(target);
            let honest = substituted.custody();
            substituted.corrupt_custody_for_test(field, &donor);
            assert_ne!(
                substituted.custody(),
                honest,
                "{target:?}: mutation `{name}` must change the retained custody receipt",
            );
            let rebuilt = validate_optimized_selection_custody(
                substituted.optimized_target(),
                substituted.register_environment(),
                substituted.legalized(),
                substituted.selected(),
            )
            .unwrap_or_else(|error| {
                panic!(
                    "{target:?}: honest replay must still succeed after custody mutation `{name}`: {error:?}"
                )
            });
            assert_ne!(
                rebuilt,
                substituted.custody(),
                "{target:?}: independent replay must reject substituted selection-custody field {name}",
            );
        }
    }
}
