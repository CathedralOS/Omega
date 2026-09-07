//! Runs unchanged validated code, then an explicitly non-artifact ABI stress copy.

use super::physical;
use crate::tests::*;

use super::super::super::native_execution as memory;
mod stress;

pub(super) fn run() {
    for count in 0..=4 {
        for choices in [
            Vec::new(),
            vec![Optimization::CopyPropagation],
            vec![Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1],
        ] {
            let selections = OptimizationSelections::new(choices).unwrap();
            let physical = physical(count, &selections);
            let emitted = stage_optimized_function_fragment_emission(
                physical.into_function_fragment_emission_source(),
            )
            .unwrap();
            let framed = stage_function_fragment_frame_application(emitted).unwrap();
            let text = stage_optimized_fixed_frame_text_section(framed).unwrap();
            let object = stage_optimized_relocation_free_object_container(text).unwrap();
            validate_optimized_relocation_free_object_container(&object).unwrap();
            let text = object.source().text_section();
            assert_eq!(text.resolved_internal_machine_calls.len(), 3);
            let callee = text
                .functions
                .iter()
                .find(|function| function.machine.get() == SCALAR_CALL_UNIT_CALLEE_BASE + 1)
                .unwrap();
            let entry = usize::try_from(text.semantic_entry_offset).unwrap();
            let callee_offset = usize::try_from(callee.section_offset).unwrap();

            // These bytes are exactly the independently validated output.
            let native = memory::Code::new(&text.bytes);
            native.call_unit(entry);
            for arguments in [[7, 9, 13, 17], [u64::MAX, 0, 31, 47]] {
                assert_eq!(
                    native.call_scalar(callee_offset, arguments),
                    if count == 0 { 19 } else { arguments[count - 1] },
                    "arity {count}, {selections:?}",
                );
            }

            // This separate copy has test-only call interposition. It is never
            // submitted as an artifact or accompanied by fabricated receipts.
            let mut trace = [[u64::MAX; 4]; 3];
            let mut stressed = text.bytes.clone();
            stress::interpose(
                &mut stressed,
                &text.resolved_internal_machine_calls,
                &mut trace,
            );
            let wrapper = stress::preservation_wrapper(&mut stressed, entry);
            let native = memory::Code::new(&stressed);
            assert_eq!(
                native.call_scalar(wrapper, [0; 4]),
                0,
                "caller must preserve all Microsoft nonvolatile integer registers"
            );
            let first = [7, 9, 7, 9];
            let second = [9, 7, 9, 7];
            let result = |arguments: [u64; 4]| if count == 0 { 19 } else { arguments[count - 1] };
            let third = [result(first), result(second), result(first), result(second)];
            for (observed, expected) in trace.iter().zip([first, second, third]) {
                assert_eq!(
                    &observed[..count],
                    &expected[..count],
                    "arity {count}, {selections:?}: argument transport and prior call results"
                );
            }
        }
    }
}
