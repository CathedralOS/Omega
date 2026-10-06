//! Post-call IEEE policy execution. The boundary already produced the raw
//! result; this owner neither calls it again nor recomputes its arithmetic.
//! Shared float semantics decide policy, but unchanged payloads retain their
//! original bits because proof meaning deliberately forgets NaN payloads.

use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::{OperationFlow, TerminalExecution};
use crate::results::{TerminalCrash, TerminalCrashSite, TerminalExecutionStatus};
use crate::values::TerminalScalarValue;
use numerics::float_semantics::{FloatFormat, FloatMeaning, FloatSemantics};
use semantic_vocabulary::{IeeeFloatFormat, IeeeFloatValue, ScalarType, ValueId};
use terminal_psi::{CrashCause, FloatResultPolicy, FloatSaturationRule, Operation, OperationKind};

fn meaning(value: IeeeFloatValue) -> FloatMeaning {
    match value {
        IeeeFloatValue::Binary32(bits) => FloatMeaning::from_f32(f32::from_bits(bits)),
        IeeeFloatValue::Binary64(bits) => FloatMeaning::from_f64(f64::from_bits(bits)),
    }
}

impl TerminalExecution {
    fn float_policy_operand(
        &self,
        value: ValueId,
        format: IeeeFloatFormat,
    ) -> Result<IeeeFloatValue, TerminalInterpretError> {
        match self.values.get(&value).copied() {
            Some(TerminalScalarValue::IeeeFloat(payload)) if payload.format() == format => {
                Ok(payload)
            }
            Some(_) => Err(malformed()),
            None => Err(TerminalInterpretError::VerifiedValueMissing(value)),
        }
    }

    pub(crate) fn execute_float_result_policy(
        &mut self,
        operation: &Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::FloatResultPolicy { policy } = operation.kind else {
            return Err(malformed());
        };
        let Some(result) = operation.result.scalar_ref() else {
            return Err(malformed());
        };
        let ScalarType::IeeeFloat(format) = result.scalar_type else {
            return Err(malformed());
        };
        let raw_result = self.float_policy_operand(policy.raw_result(), format)?;
        let raw_meaning = meaning(raw_result);
        let adapted = match policy {
            FloatResultPolicy::Trapping { .. } => {
                if FloatSemantics::apply_trapping_policy(raw_meaning).is_err() {
                    let crash = TerminalCrash {
                        site: TerminalCrashSite::Operation {
                            machine: self.current_machine,
                            block: self.current,
                            operation: operation.id,
                        },
                        cause: CrashCause::Trap,
                        site_guard: Vec::new(),
                        frontier_lower_bound: self.live_claims.keys().copied().collect(),
                    };
                    self.crash = Some(crash.clone());
                    return Ok(OperationFlow::Yield(TerminalExecutionStatus::Crashed(
                        crash,
                    )));
                }
                raw_result
            }
            FloatResultPolicy::Saturating {
                left, right, rule, ..
            } => {
                let left = meaning(self.float_policy_operand(left, format)?);
                let right = meaning(self.float_policy_operand(right, format)?);
                let semantic_format = match format {
                    IeeeFloatFormat::Binary32 => FloatFormat::BINARY32,
                    IeeeFloatFormat::Binary64 => FloatFormat::BINARY64,
                };
                let was_infinite = raw_meaning.is_infinite();
                let adapted = match rule {
                    FloatSaturationRule::FiniteOperandOverflow => {
                        FloatSemantics::apply_saturating_policy(
                            semantic_format,
                            &[&left, &right],
                            raw_meaning,
                        )
                    }
                    FloatSaturationRule::FiniteDivisionOverflow => {
                        FloatSemantics::apply_saturating_divide_policy(
                            semantic_format,
                            &left,
                            &right,
                            raw_meaning,
                        )
                    }
                };
                // Only a clamp creates new bits. Re-encoding unchanged NaNs
                // through their payloadless meaning would alter runtime data.
                if was_infinite && adapted.is_finite() {
                    match format {
                        IeeeFloatFormat::Binary32 => {
                            IeeeFloatValue::Binary32(adapted.to_f32().to_bits())
                        }
                        IeeeFloatFormat::Binary64 => {
                            IeeeFloatValue::Binary64(adapted.to_f64().to_bits())
                        }
                    }
                } else {
                    raw_result
                }
            }
        };
        self.values
            .insert(result.id, TerminalScalarValue::IeeeFloat(adapted));
        Ok(OperationFlow::Advance)
    }
}
