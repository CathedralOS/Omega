//! Invocation-private modeled time for exactly admitted Darwin/Linux calls.
//!
//! Admission selects a complete locator or target/syscall and validates the
//! retained declaration and evaluated ABI. This is an explicit host embedding
//! choice, not proof of foreign behavior. Checked calibration, wait splitting
//! and retries stay in guest code. All selected slots share one supplied clock;
//! reads do not advance it and finite waits advance both elapsed and Unix time.
//! Signed and unsigned 32-bit clock IDs share native word interpretation without
//! changing either declaration or admitting other widths.
//! Native placement and authenticated loans select timespec storage, never
//! record names or host pointers. Read-only requests receive no writeback.
//!
//! No host suspension, CPU accounting, signals or descriptor readiness is
//! simulated. Consequently this model cannot establish native latency or
//! interruption equivalence. Unsupported operations reject before changing time.
//! Darwin nanosleep uses this same signal-free timespec wait. Its invalid-request
//! path depends on libc errno state, which this snapshot does not provision;
//! reject that path rather than borrowing Linux's raw error convention.

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use runtime_abi::NativeBorrowedStorageLayout;
use semantic_vocabulary::{IntegerValue, ScalarType};
use target_operations::NormalizedForeignCallBinding;
use terminal_interpreter::{
    TerminalBoundaryByteBuffer, TerminalBoundaryStorageBuffer, TerminalEffect,
    TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult, TerminalImportAdapter,
    TerminalImportSignature, TerminalScalarValue,
};
use terminal_psi::BoundaryMachineDeclaration;

use super::TerminalRuntimeError;

mod admission;
mod timespec;

/// An invocation's explicitly supplied elapsed and epoch origins. This licenses
/// private modeled time, not access to live host clocks or native equivalence.
#[derive(Clone, Copy)]
pub struct TerminalClockSnapshot {
    pub monotonic_nanoseconds: u64,
    pub unix_nanoseconds: u64,
}

#[derive(Clone, Copy)]
enum ClockNamespace {
    Darwin,
    Linux,
}

impl TerminalClockSnapshot {
    fn reading(
        &self,
        namespace: ClockNamespace,
        selector: u128,
    ) -> Result<u64, TerminalEffectRejection> {
        // Clock IDs belong to the selected platform, not the executing host.
        // Darwin SDK _time.h and Linux uapi/linux/time.h define different
        // monotonic IDs. Only realtime and the ordinary monotonic family are
        // admitted on Linux; CPU, alarm, dynamic and coarse clocks need models
        // beyond this nanosecond domain.
        match (namespace, selector) {
            (_, 0) => Ok(self.unix_nanoseconds),
            (ClockNamespace::Darwin, 4 | 5 | 6 | 8 | 9) | (ClockNamespace::Linux, 1 | 4 | 7) => {
                Ok(self.monotonic_nanoseconds)
            }
            _ => Err(TerminalEffectRejection::new(
                "clock selector requires an unavailable clock model",
            )),
        }
    }

    fn advanced(&self, nanoseconds: u64) -> Result<Self, TerminalEffectRejection> {
        Ok(Self {
            monotonic_nanoseconds: self
                .monotonic_nanoseconds
                .checked_add(nanoseconds)
                .ok_or_else(|| TerminalEffectRejection::new("monotonic clock overflow"))?,
            unix_nanoseconds: self
                .unix_nanoseconds
                .checked_add(nanoseconds)
                .ok_or_else(|| TerminalEffectRejection::new("Unix clock overflow"))?,
        })
    }

    /// Capture the host epoch once. Guest waits then advance only this private
    /// domain, never sleep the host.
    pub fn capture_host_epoch() -> Result<Self, TerminalRuntimeError> {
        let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| {
            TerminalRuntimeError::InvalidProvision("host clock precedes the Unix epoch")
        })?;
        let unix_nanoseconds = u64::try_from(elapsed.as_nanos()).map_err(|_| {
            TerminalRuntimeError::InvalidProvision(
                "host epoch exceeds the clock model's u64 nanoseconds",
            )
        })?;
        Ok(Self {
            monotonic_nanoseconds: 0,
            unix_nanoseconds,
        })
    }
}

#[derive(Clone, Copy)]
enum ClockImplementation {
    Read,
    ReadTimespec,
    Poll,
    Nanosleep,
}

enum ClockOperation {
    Read,
    ReadTimespec(NativeBorrowedStorageLayout),
    Poll,
    Nanosleep(NativeBorrowedStorageLayout),
}

pub(super) struct ClockImport {
    signature: TerminalImportSignature,
    namespace: ClockNamespace,
    operation: ClockOperation,
    clock: Arc<Mutex<TerminalClockSnapshot>>,
}

impl ClockImport {
    /// Absence means no exact implementation is offered. A matching offer with
    /// an incompatible declaration is an error, not another adapter's fallback.
    pub(super) fn admit(
        declaration: &BoundaryMachineDeclaration,
        binding: &NormalizedForeignCallBinding,
        storage_layouts: &[NativeBorrowedStorageLayout],
        clock: Option<Arc<Mutex<TerminalClockSnapshot>>>,
    ) -> Result<Option<Self>, TerminalRuntimeError> {
        let Some((namespace, implementation)) = admission::select(binding)? else {
            return Ok(None);
        };
        let signature = TerminalImportSignature::from(declaration);
        let operation = admission::validate(
            declaration,
            binding,
            &signature,
            implementation,
            namespace,
            storage_layouts,
        )?;
        let clock = clock.ok_or_else(|| {
            TerminalRuntimeError::missing(
                "runtime-clock",
                "no isolated clock snapshot was supplied",
            )
        })?;
        Ok(Some(Self {
            signature,
            namespace,
            operation,
            clock,
        }))
    }
}

/// Retain the authored logical carrier while interpreting its native 32-bit
/// clock ID. Negative signed IDs retain their bits: Linux uses negative IDs
/// for descriptor-backed clocks, which still need their unavailable custody
/// model. Neither numerical normalization nor ABI compatibility licenses one.
fn selector(
    arguments: &[TerminalScalarValue],
    signature: &TerminalImportSignature,
) -> Result<u128, TerminalEffectRejection> {
    let invalid =
        || TerminalEffectRejection::new("clock selector differs from its installed carrier");
    let [TerminalScalarValue::Integer { scalar_type, value }] = arguments else {
        return Err(invalid());
    };
    if signature.scalar_parameters.as_slice() != [ScalarType::Integer(*scalar_type)]
        || scalar_type.bits() != 32
        || scalar_type.is_address()
        || !scalar_type.admits(*value)
    {
        return Err(invalid());
    }
    let bits = match value {
        IntegerValue::Signed(value) => *value as u128,
        IntegerValue::Unsigned(value) => *value,
    };
    Ok(bits & u128::from(u32::MAX))
}

fn status_result(
    signature: &TerminalImportSignature,
    status: i128,
) -> Result<TerminalEffectResult, TerminalEffectRejection> {
    if signature.result.is_unit() {
        return Ok(TerminalEffectResult::Unit);
    }
    let Some(ScalarType::Integer(scalar_type)) = signature.result.scalar() else {
        return Err(TerminalEffectRejection::new(
            "clock status has no integer carrier",
        ));
    };
    let value = IntegerValue::Signed(status);
    if !scalar_type.admits(value) {
        return Err(TerminalEffectRejection::new(
            "clock status exceeds its declared carrier",
        ));
    }
    Ok(TerminalEffectResult::Scalar(TerminalScalarValue::Integer {
        scalar_type,
        value,
    }))
}

fn wait(
    clock: &Arc<Mutex<TerminalClockSnapshot>>,
    signature: &TerminalImportSignature,
    nanoseconds: u64,
) -> Result<TerminalEffectResult, TerminalEffectRejection> {
    let mut clock = clock
        .lock()
        .map_err(|_| TerminalEffectRejection::new("clock snapshot lock is poisoned"))?;
    let advanced = clock.advanced(nanoseconds)?;
    let result = status_result(signature, 0)?;
    // No partial advancement on overflow or failed result construction.
    *clock = advanced;
    Ok(result)
}

import_signature_via_clone!(ClockImport);

impl TerminalEffectHandler for ClockImport {
    fn storage_arguments(&self, _boundary: semantic_vocabulary::BoundaryMachineId) -> &[usize] {
        match self.operation {
            ClockOperation::ReadTimespec(_) | ClockOperation::Nanosleep(_) => &[0],
            _ => &[],
        }
    }

    fn handle_effect_with_buffers(
        &mut self,
        effect: &TerminalEffect,
        byte_buffers: &mut [TerminalBoundaryByteBuffer],
        storage_buffers: &mut [TerminalBoundaryStorageBuffer],
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        if !byte_buffers.is_empty() {
            return Err(TerminalEffectRejection::new(
                "clock imports do not accept byte-view arguments",
            ));
        }
        match &self.operation {
            ClockOperation::ReadTimespec(layout) => timespec::read(
                effect,
                storage_buffers,
                &self.signature,
                layout,
                &self.clock,
                self.namespace,
            ),
            ClockOperation::Nanosleep(layout) => timespec::sleep(
                effect,
                storage_buffers,
                &self.signature,
                layout,
                &self.clock,
                self.namespace,
            ),
            _ if storage_buffers.is_empty() => self.handle_effect_result(effect),
            _ => Err(TerminalEffectRejection::new(
                "scalar clock imports do not accept borrowed storage",
            )),
        }
    }

    fn handle_effect(&mut self, _effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new(
            "clock imports require result-aware dispatch",
        ))
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let invalid =
            || TerminalEffectRejection::new("clock operands differ from the installed contract");
        let TerminalEffect::BoundaryCall {
            arguments,
            structural_arguments,
            ..
        } = effect
        else {
            return Err(invalid());
        };
        if !structural_arguments.is_empty()
            || arguments.len() != self.signature.scalar_parameters.len()
            || !arguments.iter().zip(&self.signature.scalar_parameters).all(|(value, expected)| {
                value.scalar_type() == *expected && matches!(value,
                    TerminalScalarValue::Integer { scalar_type, value } if scalar_type.admits(*value))
            })
        {
            return Err(invalid());
        }
        match &self.operation {
            ClockOperation::Read => {
                let selector = selector(arguments, &self.signature)?;
                let reading = self
                    .clock
                    .lock()
                    .map_err(|_| TerminalEffectRejection::new("clock snapshot lock is poisoned"))?
                    .reading(self.namespace, selector)?;
                let Some(ScalarType::Integer(scalar_type)) = self.signature.result.scalar() else {
                    return Err(invalid());
                };
                Ok(TerminalEffectResult::Scalar(TerminalScalarValue::Integer {
                    scalar_type,
                    value: IntegerValue::Unsigned(u128::from(reading)),
                }))
            }
            ClockOperation::ReadTimespec(_) | ClockOperation::Nanosleep(_) => {
                Err(TerminalEffectRejection::new(
                    "timespec operations require authenticated borrowed storage",
                ))
            }
            ClockOperation::Poll => {
                let [
                    TerminalScalarValue::Integer {
                        value: IntegerValue::Unsigned(_),
                        ..
                    },
                    TerminalScalarValue::Integer {
                        value: IntegerValue::Unsigned(count),
                        ..
                    },
                    TerminalScalarValue::Integer {
                        value: IntegerValue::Signed(milliseconds),
                        ..
                    },
                ] = arguments.as_slice()
                else {
                    return Err(invalid());
                };
                // XNU poll_nocancel and Linux fs/select.c consume unsigned-int
                // nfds, even when the authored ABI passes u64. At zero count
                // neither implementation accesses fds: requiring null would
                // invent a native precondition. No pointer is dereferenced here.
                if *count & u128::from(u32::MAX) != 0 || *milliseconds < 0 {
                    return Err(TerminalEffectRejection::new(
                        "poll requires descriptor custody or an indefinite-wait model",
                    ));
                }
                let nanoseconds = u64::try_from(*milliseconds)
                    .ok()
                    .and_then(|milliseconds| milliseconds.checked_mul(1_000_000))
                    .ok_or_else(|| {
                        TerminalEffectRejection::new("poll duration exceeds modeled nanoseconds")
                    })?;
                wait(&self.clock, &self.signature, nanoseconds)
            }
        }
    }
}
