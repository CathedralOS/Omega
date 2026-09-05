use std::collections::BTreeMap;

use crate::tests::*;

mod effect;

use effect::assert_exact_effect;

use super::{
    super::recursive_reload_value_homes::{Bundle, original_bundle, reload_bundle},
    fixture::{EXACT_USAGE, build, exact_budget, lower},
};

#[test]
fn both_recursive_paths_project_exact_abstract_accesses_on_both_targets() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        for (original, constructor) in [
            (false, reload_bundle as fn(NativeTarget) -> Bundle),
            (true, original_bundle as fn(NativeTarget) -> Bundle),
        ] {
            let source = build(constructor, target);
            let first = lower(&source, exact_budget()).unwrap();
            let second = lower(&source, exact_budget()).unwrap();
            assert_eq!(first, second);
            assert_eq!(first.receipt().function_count(), 1);
            assert_eq!(first.receipt().write_count(), 3);
            assert_eq!(first.receipt().read_count(), 3);
            assert_eq!(first.receipt().max_spill_area_bytes(), 16);
            assert_eq!(first.receipt().usage(), EXACT_USAGE);
            assert_eq!(
                first.receipt().identity(),
                selected_instructions_to_register_homes::abstract_spill_memory_effect_plan_identity(
                    first.plan()
                ),
            );
            assert_eq!(
                first.receipt().homed_spill_pseudo_instructions(),
                source.homed.receipt().identity(),
            );

            let function = &first.plan().functions[0];
            let pseudos = &source.homed.plan().functions[0];
            assert_eq!(function.machine, pseudos.machine);
            assert_eq!(function.spill_area_bytes, 16);
            assert_eq!(function.effects.len(), pseudos.instructions.len());
            let mut access_counts = BTreeMap::new();
            for (effect, pseudo) in function.effects.iter().zip(&pseudos.instructions) {
                assert_exact_effect(effect, pseudo, pseudos);
                let (storage, write) = match effect {
                    selected_instructions_to_register_homes::AbstractSpillMemoryEffect::Write {
                        storage,
                        ..
                    } => (*storage, true),
                    selected_instructions_to_register_homes::AbstractSpillMemoryEffect::Read {
                        storage,
                        ..
                    } => (*storage, false),
                };
                let counts = access_counts.entry(storage).or_insert((0, 0));
                if write { counts.0 += 1 } else { counts.1 += 1 }
            }
            assert_eq!(access_counts.len(), 3);
            assert!(access_counts.values().all(|counts| *counts == (1, 1)));
            assert!(
                function.effects.iter().any(|effect| matches!(
                    effect,
                    selected_instructions_to_register_homes::AbstractSpillMemoryEffect::Write {
                        source: selected_instructions_to_register_homes::SpillPseudoStoredValue::Original(
                            selected_instructions::VirtualRegisterId(5)
                        ),
                        ..
                    }
                )) == original
            );
        }
    }
}
