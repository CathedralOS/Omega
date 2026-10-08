//! Invocation-private modeled time for explicitly admitted Darwin imports.
//!
//! Only exact libSystem locators and compatible evaluated native ABIs receive
//! these trusted adapters. This is a host embedding choice, not a proof of
//! foreign behavior. Checked calibration, wait splitting and retries stay in
//! guest code. One supplied snapshot backs every selected clock and poll slot;
//! reads do not advance it and finite descriptorless waits advance all clocks.
//! Scalar reads and borrowed timespec writes observe that same snapshot. Native
//! byte placement and authenticated loans jointly select the out-parameter;
//! neither record names nor host pointers enter execution.
//! There are no simulated host suspensions, CPU usage or descriptor readiness.
//! Consequently this model cannot establish native latency or arbitrary clock
//! behavior. Unsupported operations reject before modifying the snapshot.

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use calling_conventions::{CallSignature, ValueShape, validate_boundary_entry_plan};
use runtime_abi::NativeBorrowedStorageLayout;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
use target_operations::NormalizedForeignCallBinding;
use terminal_interpreter::{
    TerminalBoundaryByteBuffer, TerminalBoundaryScalarBuffer, TerminalEffect,
    TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult, TerminalImportAdapter,
    TerminalImportSignature, TerminalScalarValue,
};
use terminal_psi::{
    BoundaryCompletion, BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind,
};

use super::TerminalRuntimeError;

mod timespec;

/// Initial nanosecond readings for an isolated clock domain. Each invocation
/// owns its copy. Supplying a snapshot explicitly licenses modeled time, not
/// native timing equivalence or access to the host's live clocks.
#[derive(Clone, Copy)]
pub struct TerminalClockSnapshot {
    pub monotonic_nanoseconds: u64,
    pub unix_nanoseconds: u64,
}

impl TerminalClockSnapshot {
    fn reading(&self, selector: u128) -> Result<u64, TerminalEffectRejection> {
        // Darwin SDK _time.h: realtime 0, monotonic 6, monotonic raw
        // 4/5, uptime raw 8/9. Without suspension these simulated
        // monotonic families share elapsed time, not CPU accounting.
        match selector {
            0 => Ok(self.unix_nanoseconds),
            4 | 5 | 6 | 8 | 9 => Ok(self.monotonic_nanoseconds),
            _ => Err(TerminalEffectRejection::new(
                "clock selector requires an unavailable clock model",
            )),
        }
    }

    /// Capture the host epoch once, with a fresh zero monotonic origin. Guest
    /// waits subsequently advance only this private domain, never sleep the host.
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
}

impl ClockImplementation {
    fn locator(self) -> Result<target::NormalizedForeignLocator, TerminalRuntimeError> {
        target::normalize_foreign_locator(
            target::ForeignLocatorCandidate::MachODylibSymbol {
                install_name: b"/usr/lib/libSystem.B.dylib".to_vec(),
                symbol: match self {
                    Self::Read => b"_clock_gettime_nsec_np".to_vec(),
                    Self::ReadTimespec => b"_clock_gettime".to_vec(),
                    Self::Poll => b"_poll".to_vec(),
                },
            },
            target::TargetProfile::MacosArm64,
        )
        .map_err(|_| {
            TerminalRuntimeError::InvalidProvision("clock implementation has an invalid locator")
        })
    }
}

enum ClockOperation {
    Read,
    ReadTimespec(NativeBorrowedStorageLayout),
    Poll,
}

pub(super) struct ClockImport {
    signature: TerminalImportSignature,
    operation: ClockOperation,
    clock: Arc<Mutex<TerminalClockSnapshot>>,
}

impl ClockImport {
    /// None means this complete locator is not offered. A matching locator with
    /// an incompatible declaration is an error, never another adapter's fallback.
    pub(super) fn admit(
        declaration: &BoundaryMachineDeclaration,
        binding: &NormalizedForeignCallBinding,
        storage_layouts: &[NativeBorrowedStorageLayout],
        clock: Option<Arc<Mutex<TerminalClockSnapshot>>>,
    ) -> Result<Option<Self>, TerminalRuntimeError> {
        let Some((locator, _)) = binding.import() else {
            return Ok(None);
        };
        let mut operation = None;
        for offered in [
            ClockImplementation::Read,
            ClockImplementation::ReadTimespec,
            ClockImplementation::Poll,
        ] {
            if offered.locator()? == *locator {
                operation = Some(offered);
                break;
            }
        }
        let Some(operation) = operation else {
            return Ok(None);
        };
        let invalid = || {
            TerminalRuntimeError::InvalidProvision(
                "clock adapter differs from its exact admitted declaration/ABI",
            )
        };
        let signed = IntegerType::new(IntegerSign::Signed, 32).map_err(|_| invalid())?;
        let unsigned = IntegerType::new(IntegerSign::Unsigned, 32).map_err(|_| invalid())?;
        let wide = IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?;
        let signature = TerminalImportSignature::from(declaration);
        let scalar = ScalarType::Integer;
        let (operation, shape_valid, parameter_order) = match operation {
            ClockImplementation::Read => (
                ClockOperation::Read,
                signature.scalar_parameters == [scalar(unsigned)]
                    && signature.result == BoundaryMachineResult::Scalar(scalar(wide))
                    && signature.structural_parameters.is_empty(),
                vec![BoundaryParameterKind::Scalar],
            ),
            ClockImplementation::ReadTimespec => (
                ClockOperation::ReadTimespec(timespec::admit(&signature, storage_layouts)?),
                signature.scalar_parameters == [scalar(unsigned)]
                    && signature.result == BoundaryMachineResult::Scalar(scalar(signed)),
                vec![
                    BoundaryParameterKind::Scalar,
                    BoundaryParameterKind::Structural,
                ],
            ),
            ClockImplementation::Poll => (
                ClockOperation::Poll,
                matches!(signature.scalar_parameters.as_slice(), [pointer, count, timeout]
                if *pointer == scalar(wide) && (*count == scalar(unsigned) || *count == scalar(wide)) && *timeout == scalar(signed))
                    && (signature.result.is_unit()
                        || signature.result == BoundaryMachineResult::Scalar(scalar(signed)))
                    && signature.structural_parameters.is_empty(),
                vec![BoundaryParameterKind::Scalar; 3],
            ),
        };
        let mut native_signature = CallSignature {
            parameters: Vec::new(),
            result: None,
        };
        for parameter in &signature.scalar_parameters {
            let ScalarType::Integer(integer) = parameter else {
                return Err(invalid());
            };
            let bytes = integer.bits() / 8;
            native_signature
                .parameters
                .push(ValueShape::integer(u32::from(bytes), bytes));
        }
        if matches!(operation, ClockOperation::ReadTimespec(_)) {
            // This exact C ABI appends a thin timespec pointer after clockid_t.
            // The semantic loan remains separately authenticated by the core.
            native_signature.parameters.push(ValueShape::integer(8, 8));
        }
        native_signature.result = match signature.result.scalar() {
            Some(ScalarType::Integer(integer)) => {
                let bytes = integer.bits() / 8;
                Some(ValueShape::integer(u32::from(bytes), bytes))
            }
            _ => None,
        };
        if !shape_valid
            || signature.completion != BoundaryCompletion::NormalReturn
            || declaration.parameter_order != parameter_order
            || !binding.has_valid_entry_mechanism()
            || binding.requirement_identity() != declaration.identity
            || binding.provider_plan_report_identity() == 0
            || !binding
                .boundary_entry_plan
                .call
                .callback_materializations
                .is_empty()
            || validate_boundary_entry_plan(binding.boundary_entry_plan.clone(), &native_signature)
                .is_err()
            || !declaration.requires.is_empty()
            || !declaration.scalar_requires.is_empty()
            || !declaration.scalar_ensures.is_empty()
            || !declaration.content_guarantees.is_empty()
            || !declaration.program_local_root_introductions.is_empty()
            || !declaration.crash_routes.is_empty()
        {
            return Err(invalid());
        }
        let clock = clock.ok_or_else(|| {
            TerminalRuntimeError::missing(
                "runtime-clock",
                "no isolated clock snapshot was supplied",
            )
        })?;
        Ok(Some(Self {
            signature,
            operation,
            clock,
        }))
    }

    fn scalar_result(
        &self,
        value: IntegerValue,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let Some(ScalarType::Integer(scalar_type)) = self.signature.result.scalar() else {
            return Err(TerminalEffectRejection::new(
                "clock result has no integer carrier",
            ));
        };
        Ok(TerminalEffectResult::Scalar(TerminalScalarValue::Integer {
            scalar_type,
            value,
        }))
    }
}

impl TerminalImportAdapter for ClockImport {
    fn implemented_signature(&self) -> TerminalImportSignature {
        self.signature.clone()
    }
}

impl TerminalEffectHandler for ClockImport {
    fn scalar_storage_arguments(
        &self,
        _boundary: semantic_vocabulary::BoundaryMachineId,
    ) -> &[usize] {
        match self.operation {
            ClockOperation::ReadTimespec(_) => &[0],
            _ => &[],
        }
    }

    fn handle_effect_with_buffers(
        &mut self,
        effect: &TerminalEffect,
        byte_buffers: &mut [TerminalBoundaryByteBuffer],
        scalar_buffers: &mut [TerminalBoundaryScalarBuffer],
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        if !byte_buffers.is_empty() {
            return Err(TerminalEffectRejection::new(
                "clock imports do not accept byte-view arguments",
            ));
        }
        match &self.operation {
            ClockOperation::ReadTimespec(layout) => {
                timespec::read(effect, scalar_buffers, &self.signature, layout, &self.clock)
            }
            _ if scalar_buffers.is_empty() => self.handle_effect_result(effect),
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
        if !structural_arguments.is_empty() || arguments.len() != self.signature.scalar_parameters.len()
            || !arguments.iter().zip(&self.signature.scalar_parameters).all(|(value, expected)| {
                value.scalar_type() == *expected && matches!(value, TerminalScalarValue::Integer { scalar_type, value } if scalar_type.admits(*value))
            }) { return Err(invalid()); }
        let mut clock = self
            .clock
            .lock()
            .map_err(|_| TerminalEffectRejection::new("clock snapshot lock is poisoned"))?;
        match &self.operation {
            ClockOperation::Read => {
                let [
                    TerminalScalarValue::Integer {
                        value: IntegerValue::Unsigned(selector),
                        ..
                    },
                ] = arguments.as_slice()
                else {
                    return Err(invalid());
                };
                let reading = clock.reading(*selector)?;
                self.scalar_result(IntegerValue::Unsigned(u128::from(reading)))
            }
            ClockOperation::ReadTimespec(_) => Err(TerminalEffectRejection::new(
                "timespec reads require authenticated borrowed storage dispatch",
            )),
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
                // XNU poll_nocancel does not read fds when nfds is zero;
                // zero-byte copyout likewise ignores the address. Requiring
                // a null carrier here would invent a native precondition.
                // No pointer is dereferenced or translated by this runtime.
                // Darwin's actual nfds_t is unsigned int (sys/poll.h), even
                // for the authored u64 count ABI. The native callee consumes
                // its low 32 bits; preserve that behavior without narrowing
                // the demanded declaration or its evaluated call plan.
                let descriptor_count = *count & u128::from(u32::MAX);
                if descriptor_count != 0 || *milliseconds < 0 {
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
                let monotonic_nanoseconds = clock
                    .monotonic_nanoseconds
                    .checked_add(nanoseconds)
                    .ok_or_else(|| TerminalEffectRejection::new("monotonic clock overflow"))?;
                let unix_nanoseconds = clock
                    .unix_nanoseconds
                    .checked_add(nanoseconds)
                    .ok_or_else(|| TerminalEffectRejection::new("Unix clock overflow"))?;
                let result = if self.signature.result.is_unit() {
                    TerminalEffectResult::Unit
                } else {
                    self.scalar_result(IntegerValue::Signed(0))?
                };
                // Commit both readings only after all checks, including result
                // construction. A rejected wait cannot advance either clock.
                *clock = TerminalClockSnapshot {
                    monotonic_nanoseconds,
                    unix_nanoseconds,
                };
                Ok(result)
            }
        }
    }
}
