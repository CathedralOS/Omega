//! Evaluated foreign binding values and retained evaluator usage.

use build_time_evaluation::EvaluationUsage;
use effects::provider_plan::EvaluatedBindingUsage;
pub(crate) use language_semantics::native_bindings::decode_binding_value;
pub(crate) type DecodedBindingValue = semantic_vocabulary::NativeBindingLocator;

pub(crate) fn retained_usage(usage: EvaluationUsage) -> Result<EvaluatedBindingUsage, String> {
    EvaluatedBindingUsage::from_evaluator(
        usage.schema().schema_version(),
        usage.schedule().marker(),
        usage.fuel_units(),
        usage.fuel_ceiling(),
        usage.build_log_bytes(),
        usage.filesystem_operation_attempts(),
        usage.peak_live_cells(),
        usage.peak_live_text_bytes(),
        usage.result_cells(),
        usage.result_text_bytes(),
    )
}
