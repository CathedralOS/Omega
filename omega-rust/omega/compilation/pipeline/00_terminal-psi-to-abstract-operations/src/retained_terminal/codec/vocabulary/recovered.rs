//! Values recovered through their ordinary vocabulary constructors.

use super::super::value::{RecordError, RecordReader, RecordValue, RecordWriter};

macro_rules! nominal_identity {
    ($($value:path),* $(,)?) => {$(
        impl RecordValue for $value {
            fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
                self.get().write(output)
            }
            fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
                Self::new(u64::read(input)?)
                    .ok_or_else(|| input.error(concat!("invalid ", stringify!($value), " identity")))
            }
        }
    )*};
}

nominal_identity!(
    calling_conventions::NativeParameterId,
    calling_conventions::StaticMachineBinderId,
    calling_conventions::CallbackRequirementId,
    calling_conventions::LayoutPlanId,
    calling_conventions::LayoutSlotId,
    semantic_vocabulary::MachineId,
    semantic_vocabulary::BoundaryMachineId,
);

impl RecordValue for semantic_vocabulary::PackageKeyIdentity {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.digest().write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        Self::from_digest(RecordValue::read(input)?)
            .ok_or_else(|| input.error("package identity is zero"))
    }
}

macro_rules! digest_value {
    ($($value:path),* $(,)?) => {$(
        impl RecordValue for $value {
            fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
                self.as_bytes().write(output)
            }
            fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
                Ok(Self::from_digest(RecordValue::read(input)?))
            }
        }
    )*};
}

digest_value!(
    effects::provider_plan::ProviderPlanDigest,
    effects::provider_plan::ServiceSchemaDigest,
    effects::provider_plan::BoundaryCallingPlanCommitment,
);

macro_rules! evaluated_digest_value {
    ($($value:path),* $(,)?) => {$(
        impl RecordValue for $value {
            fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
                self.as_bytes().write(output)
            }
            fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
                Self::from_bytes(RecordValue::read(input)?)
                    .map_err(|reason| input.error(reason))
            }
        }
    )*};
}

evaluated_digest_value!(
    effects::provider_plan::EvaluatedBindingProducerClosureDigest,
    effects::provider_plan::EvaluatedBindingEvaluationDigest,
    effects::provider_plan::EvaluatedBindingMaterializationDigest,
);

impl RecordValue for effects::provider_plan::ProviderIdentityTable {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        output.sequence(self.texts().count())?;
        for text in self.texts() {
            text.to_owned().write(output)?;
        }
        Ok(())
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        let texts = Vec::<String>::read(input)?;
        if texts.len() >= u32::MAX as usize {
            return Err(input.error("provider identity table exceeds its index width"));
        }
        // Rebuild every entry in original mint order, including unreferenced
        // rows. Interning a subset would rebind the retained numeric indices.
        input.allocate(texts.len().saturating_mul(8 * std::mem::size_of::<usize>()))?;
        let mut table = Self::default();
        for text in texts {
            if table.id_at(&text).is_some() {
                return Err(input.error("provider identity table repeats an entry"));
            }
            input.allocate(text.len().saturating_mul(2))?;
            table.intern(&text);
        }
        Ok(table)
    }
}

impl RecordValue for effects::provider_plan::ProviderPlanDigestMemo {
    fn write(&self, _output: &mut RecordWriter) -> Result<(), RecordError> {
        // This memo is expressly excluded from value equality. Its existing
        // Clone implementation also clears it instead of copying commitments.
        Ok(())
    }
    fn read(_input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        Ok(Self::default())
    }
}

impl RecordValue for calling_conventions::RegisterSet {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        output.sequence(self.as_slice().len())?;
        for register in self.as_slice() {
            register.write(output)?;
        }
        Ok(())
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        let registers = Vec::<calling_conventions::MachineRegister>::read(input)?;
        let set = Self::new(registers.iter().copied());
        if set.as_slice() != registers {
            return Err(input.error("register set is not canonical"));
        }
        Ok(set)
    }
}

impl RecordValue for calling_conventions::MachineStateSet {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.bits().write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        use calling_conventions::MachineState;
        let bits = u16::read(input)?;
        let states = [
            MachineState::GeneralRegisters,
            MachineState::VectorRegisters,
            MachineState::Flags,
            MachineState::InstructionPointer,
            MachineState::StackPointer,
            MachineState::SegmentState,
            MachineState::ControlState,
            MachineState::DebugState,
            MachineState::ExtendedState,
        ];
        let set = Self::new(
            states
                .into_iter()
                .filter(|state| bits & (1 << *state as u8) != 0),
        );
        if set.bits() != bits {
            return Err(input.error("machine state set contains unknown bits"));
        }
        Ok(set)
    }
}

impl RecordValue for calling_conventions::BoundaryCallbackLayout {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.formal_ordinal.write(output)?;
        self.native_ordinal.write(output)?;
        self.destination.write(output)?;
        self.root.write(output)?;
        let fields = self.fields().map_err(|reason| output.error(reason))?;
        output.sequence(fields.len())?;
        for field in fields {
            field.write(output)?;
        }
        self.terminal.write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        input.record(|input| {
            let formal_ordinal = RecordValue::read(input)?;
            let native_ordinal = RecordValue::read(input)?;
            let destination = RecordValue::read(input)?;
            let root = RecordValue::read(input)?;
            let fields = Vec::<calling_conventions::CallbackInlineFieldGeometry>::read(input)?;
            let terminal = RecordValue::read(input)?;
            // `new` owns a fresh field arena and clones its field payloads.
            input.allocate(
                fields
                    .len()
                    .saturating_add(1)
                    .saturating_mul(std::mem::size_of::<
                        calling_conventions::CallbackInlineFieldGeometry,
                    >()),
            )?;
            for field in &fields {
                input.allocate(
                    field
                        .identity
                        .len()
                        .saturating_add(field.child.identity.len())
                        .saturating_add(field.child.policy.as_ref().map_or(0, String::len)),
                )?;
            }
            Ok(Self::new(
                formal_ordinal,
                native_ordinal,
                destination,
                root,
                &fields,
                terminal,
            ))
        })
    }
}

impl RecordValue for effects::TerminalAuthorityDisposition {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        output.sequence(self.classes().len())?;
        for class in self.classes() {
            class.write(output)?;
        }
        Ok(())
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        Self::try_from_canonical_classes(RecordValue::read(input)?)
            .map_err(|_| input.error("authority classes are not strictly ordered"))
    }
}

impl RecordValue for effects::ServiceTerminalAuthorityPermission {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.service_schema().write(output)?;
        self.requirement_identity().write(output)?;
        self.permitted().write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        input.record(|input| {
            Ok(Self::new(
                RecordValue::read(input)?,
                RecordValue::read(input)?,
                RecordValue::read(input)?,
            ))
        })
    }
}

impl RecordValue for target::ApplicationIdentifier {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.as_str().to_owned().write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        Self::new(String::read(input)?.as_bytes()).map_err(|reason| input.error(reason))
    }
}

impl RecordValue for target::NormalizedForeignLocator {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.target().write(output)?;
        self.locator().write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        input.record(|input| {
            let target = RecordValue::read(input)?;
            let locator = RecordValue::read(input)?;
            target::normalize_foreign_locator(locator, target)
                .map_err(|reason| input.error(reason.to_string()))
        })
    }
}

impl RecordValue for target::NormalizedElfInterpreterPlan {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.target().write(output)?;
        output.sequence(self.interpreter_path().len())?;
        output.extend(self.interpreter_path())
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        input.record(|input| {
            let target = RecordValue::read(input)?;
            let path = RecordValue::read(input)?;
            target::normalize_elf_interpreter_plan(path, target)
                .map_err(|reason| input.error(reason.to_string()))
        })
    }
}

impl RecordValue for effects::provider_plan::EvaluatedBindingUsage {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.usage_schema_version().write(output)?;
        self.step_schedule_marker().write(output)?;
        self.fuel_units().write(output)?;
        self.fuel_ceiling().write(output)?;
        self.build_log_bytes().write(output)?;
        self.filesystem_operation_attempts().write(output)?;
        self.peak_live_cells().write(output)?;
        self.peak_live_text_bytes().write(output)?;
        self.result_cells().write(output)?;
        self.result_text_bytes().write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        input.record(|input| {
            Self::from_evaluator(
                RecordValue::read(input)?,
                RecordValue::read(input)?,
                RecordValue::read(input)?,
                RecordValue::read(input)?,
                RecordValue::read(input)?,
                RecordValue::read(input)?,
                RecordValue::read(input)?,
                RecordValue::read(input)?,
                RecordValue::read(input)?,
                RecordValue::read(input)?,
            )
            .map_err(|reason| input.error(reason))
        })
    }
}

impl RecordValue for effects::provider_plan::EvaluatedForeignImport {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.locator().write(output)?;
        self.receipt().write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        input.record(|input| {
            Self::from_retained_evidence(RecordValue::read(input)?, RecordValue::read(input)?)
                .map_err(|reason| input.error(reason))
        })
    }
}

impl RecordValue for effects::provider_plan::EvaluatedForeignSyscall {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.target().write(output)?;
        u64::try_from(self.number())
            .map_err(|_| output.error("evaluated syscall number is negative"))?
            .write(output)?;
        self.receipt().write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        input.record(|input| {
            Self::from_retained_evidence(
                RecordValue::read(input)?,
                RecordValue::read(input)?,
                RecordValue::read(input)?,
            )
            .map_err(|reason| input.error(reason))
        })
    }
}

super::record_fields!(source::Span { start, end });

impl RecordValue for source::SourceId {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.0.write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        usize::read(input).map(Self)
    }
}

impl RecordValue for std::path::PathBuf {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        if let Some(text) = self.to_str() {
            0u8.write(output)?;
            return text.to_owned().write(output);
        }
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            1u8.write(output)?;
            let bytes = self.as_os_str().as_bytes();
            output.sequence(bytes.len())?;
            output.extend(bytes)
        }
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            2u8.write(output)?;
            let units: Vec<_> = self.as_os_str().encode_wide().collect();
            units.write(output)
        }
        #[cfg(not(any(unix, windows)))]
        Err(output.error("host path encoding is unsupported"))
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        input.record(|input| match u8::read(input)? {
            0 => String::read(input).map(Self::from),
            1 => {
                let bytes = Vec::<u8>::read(input)?;
                #[cfg(unix)]
                {
                    use std::os::unix::ffi::OsStringExt;
                    Ok(Self::from(std::ffi::OsString::from_vec(bytes)))
                }
                #[cfg(not(unix))]
                {
                    String::from_utf8(bytes).map(Self::from).map_err(|_| {
                        input.error("Unix diagnostic path is not representable on this host")
                    })
                }
            }
            2 => {
                let units = Vec::<u16>::read(input)?;
                input.allocate(units.len().saturating_mul(3))?;
                #[cfg(windows)]
                {
                    use std::os::windows::ffi::OsStringExt;
                    Ok(Self::from(std::ffi::OsString::from_wide(&units)))
                }
                #[cfg(not(windows))]
                {
                    String::from_utf16(&units).map(Self::from).map_err(|_| {
                        input.error("Windows diagnostic path is not representable on this host")
                    })
                }
            }
            _ => Err(input.error("unknown diagnostic path encoding")),
        })
    }
}

impl RecordValue for effects::provider_plan::ProviderIdentityId {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.index().write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        Ok(Self::from_index(u32::read(input)?))
    }
}

impl RecordValue for diagnostics::ExceededProvision {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        let Self {
            resource,
            limit,
            requested,
            coordinate,
        } = self;
        super::super::value::write_text(resource, output)?;
        limit.write(output)?;
        requested.write(output)?;
        coordinate.write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        input.record(|input| {
            let resource = String::read(input)?;
            let limit = u64::read(input)?;
            let requested = u64::read(input)?;
            let coordinate = u64::read(input)?;
            let form = diagnostics::CoverageForm::from_resource(&resource).ok_or_else(|| {
                input.error(format!("unknown retained provision resource `{resource}`"))
            })?;
            let recovered = form.exceeded(coordinate);
            if recovered.limit != limit || recovered.requested != requested {
                return Err(
                    input.error("retained coverage provision has inconsistent limit or demand")
                );
            }
            Ok(recovered)
        })
    }
}
