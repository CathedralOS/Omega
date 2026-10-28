//! Declaration-only entry recovery. Runtime loans and backing never enter this record.
use super::value::{
    RecordError, RecordReader, RecordValue, RecordWriter, write_text, write_values,
};
use program_entry_plan::*;
use terminal_psi::TerminalIdentityTable;

#[cfg(test)]
mod tests {
    use super::super::value::RecordLimits;
    use super::*;

    #[test]
    fn entry_record_round_trip_preserves_declaration_without_synthetic_fields() {
        let mut identities = TerminalIdentityTable::default().into_builder();
        let callable = identities.intern("fixture::start");
        let source = SelectedProgramEntrySignature::from_checked_declaration(
            target::TargetProfile::host().program_entry_slot(),
            "fixture::start".to_owned(),
            "start".to_owned(),
            "start".to_owned(),
            callable,
            Vec::new(),
            identities.build(),
        )
        .expect("free hosted entry signature");
        let entry = crate::NativeProgramEntrySettlement::new(&source, None);
        let mut output = RecordWriter::new(RecordLimits::default());
        EntryRecord::write_borrowed(entry, &mut output).expect("entry encoding");
        let bytes = output.finish();
        let mut input =
            RecordReader::new(&bytes, RecordLimits::default()).expect("bounded entry bytes");
        let recovered = EntryRecord::read(&mut input).expect("entry decoding");
        input.finish().expect("complete entry consumption");
        assert_eq!(recovered.source, source);
        assert!(recovered.semantic.is_none());
        assert!(recovered.physical.is_none());
        assert!(recovered.storage.is_none());
        assert!(recovered.checked.is_none());
        assert!(!recovered.physical_adapter_requested);
        let mut output = RecordWriter::new(RecordLimits::default());
        EntryRecord::write_borrowed(recovered.borrowed(), &mut output)
            .expect("recovered entry encoding");
        assert_eq!(output.finish(), bytes);

        for length in 0..bytes.len() {
            let mut input = RecordReader::new(&bytes[..length], RecordLimits::default())
                .expect("bounded truncated input");
            assert!(
                EntryRecord::read(&mut input).is_err(),
                "accepted prefix {length}"
            );
        }
    }
}

pub(super) struct EntryRecord {
    pub source: SelectedProgramEntrySignature,
    pub semantic: Option<calling_conventions::BoundaryCallingApplication>,
    pub physical: Option<calling_conventions::BoundaryCallingApplication>,
    pub storage: Option<SelectedProgramStorageEntryPlan>,
    pub checked: Option<terminal_psi::CheckedProgramEntryTerminalReceipt>,
    pub physical_adapter_requested: bool,
}

impl EntryRecord {
    pub fn borrowed(&self) -> crate::NativeProgramEntrySettlement<'_> {
        crate::NativeProgramEntrySettlement {
            source: &self.source,
            semantic_calling_application: self.semantic.as_ref(),
            physical_calling_application: self.physical.as_ref(),
            storage_entry: self.storage.as_ref(),
            checked_entry: self.checked.as_ref(),
            physical_adapter: None,
        }
    }
    pub fn write_borrowed(
        entry: crate::NativeProgramEntrySettlement<'_>,
        output: &mut RecordWriter,
    ) -> Result<(), RecordError> {
        entry.source().write(output)?;
        write_optional_ref(entry.semantic_calling_application(), output)?;
        write_optional_ref(entry.physical_calling_application(), output)?;
        write_optional_ref(entry.storage_entry(), output)?;
        write_optional_ref(entry.checked_entry, output)?;
        entry.physical_adapter().is_some().write(output)
    }
    pub fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        let source = SelectedProgramEntrySignature::read(input)?;
        let semantic = RecordValue::read(input)?;
        let physical = RecordValue::read(input)?;
        let storage = RecordValue::read(input)?;
        Ok(Self {
            source,
            semantic,
            physical,
            storage,
            checked: RecordValue::read(input)?,
            physical_adapter_requested: RecordValue::read(input)?,
        })
    }
}

pub(super) fn write_optional_ref<T: RecordValue>(
    value: Option<&T>,
    output: &mut RecordWriter,
) -> Result<(), RecordError> {
    match value {
        None => 0u8.write(output),
        Some(value) => {
            1u8.write(output)?;
            value.write(output)
        }
    }
}

impl RecordValue for ProgramStorageEntryRootRole {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        match self {
            Self::Image => 0u8.write(output),
            Self::InitialStorage => 1u8.write(output),
        }
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        match u8::read(input)? {
            0 => Ok(Self::Image),
            1 => Ok(Self::InitialStorage),
            _ => Err(input.error("invalid program-storage root role")),
        }
    }
}

impl RecordValue for ProgramEntryExtentValueLayout {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.shape().write(output)?;
        for field in self.fields() {
            field.byte_offset().write(output)?;
            field.shape().write(output)?;
        }
        Ok(())
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        let shape = RecordValue::read(input)?;
        Self::from_checked_record(
            RecordValue::read(input)?,
            RecordValue::read(input)?,
            RecordValue::read(input)?,
            RecordValue::read(input)?,
            shape,
        )
        .map_err(|reason| input.error(reason))
    }
}

impl RecordValue for ProgramEntryVisibleParameterSignature {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.role().write(output)?;
        self.visible_parameter_index().write(output)?;
        self.normalized_type_identity().write(output)?;
        self.value_shape().write(output)?;
        self.extent_value_layout().write(output)?;
        self.is_const().write(output)?;
        self.is_mutable().write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        Ok(SelectedProgramEntrySignature::visible_parameter(
            RecordValue::read(input)?,
            RecordValue::read(input)?,
            RecordValue::read(input)?,
            RecordValue::read(input)?,
            RecordValue::read(input)?,
            RecordValue::read(input)?,
            RecordValue::read(input)?,
        ))
    }
}

impl RecordValue for SelectedProgramEntrySignature {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.target_slot().owner.write(output)?;
        write_text(self.declaration(), output)?;
        write_text(self.machine_name(), output)?;
        write_text(self.state_name(), output)?;
        self.normalized_callable_identity().write(output)?;
        write_values(self.visible_parameters(), output)?;
        match self.result() {
            ProgramEntryResultSignature::Unit => 0u8.write(output)?,
        }
        self.identities().write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        let slot = target::TargetProfile::read(input)?.program_entry_slot();
        let declaration = String::read(input)?;
        let machine = String::read(input)?;
        let state = String::read(input)?;
        let callable = RecordValue::read(input)?;
        let visible = RecordValue::read(input)?;
        if u8::read(input)? != 0 {
            return Err(input.error("invalid ProgramEntry result"));
        }
        Self::from_checked_declaration(
            slot,
            declaration,
            machine,
            state,
            callable,
            visible,
            TerminalIdentityTable::read(input)?,
        )
        .map_err(|reason| input.error(reason))
    }
}

impl RecordValue for ProgramEntryPhysicalContractPlan {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        let text = |identity| {
            self.identities()
                .text(identity)
                .ok_or_else(|| output.error("physical contract has an out-of-range identity"))
        };
        // Resolve before borrowing the output mutably.
        let requirement = text(self.requirement_identity())?.to_owned();
        let parameters = self
            .parameter_type_identities()
            .iter()
            .map(|identity| text(*identity).map(str::to_owned))
            .collect::<Result<Vec<_>, _>>()?;
        let result = text(self.result_type_identity())?.to_owned();
        self.target_slot().owner.write(output)?;
        requirement.write(output)?;
        self.target_package().write(output)?;
        self.target_package_source_digest()
            .as_bytes()
            .write(output)?;
        self.non_authoritative_target_package_source_report_fingerprint()
            .write(output)?;
        parameters.write(output)?;
        result.write(output)?;
        self.calling_plan_report_fingerprint().write(output)?;
        self.boundary_entry_plan().write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        let slot = target::TargetProfile::read(input)?.program_entry_slot();
        let requirement = String::read(input)?;
        let package = target::ProgramEntryPhysicalContractPackage::read(input)?;
        let digest = ProgramEntryPhysicalContractPackageSourceDigest::from_retained_digest(
            package,
            <[u8; 32]>::read(input)?,
        );
        Self::new(
            slot,
            requirement,
            package,
            digest,
            RecordValue::read(input)?,
            RecordValue::read(input)?,
            RecordValue::read(input)?,
            RecordValue::read(input)?,
            RecordValue::read(input)?,
        )
        .map_err(|reason| input.error(reason))
    }
}

impl RecordValue for SelectedProgramStorageEntryPlan {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.target_slot().owner.write(output)?;
        self.schema().write(output)?;
        write_text(
            self.identities()
                .text(self.requirement_identity())
                .ok_or_else(|| output.error("storage entry has an out-of-range requirement"))?,
            output,
        )?;
        write_optional_ref(self.physical_contract(), output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        let slot = target::TargetProfile::read(input)?.program_entry_slot();
        let plan = Self::from_target_slot(slot, RecordValue::read(input)?, String::read(input)?)
            .map_err(|reason| input.error(reason.to_string()))?;
        match Option::<ProgramEntryPhysicalContractPlan>::read(input)? {
            None => Ok(plan),
            Some(physical) => plan
                .with_physical_contract(physical)
                .map_err(|reason| input.error(reason.to_string())),
        }
    }
}
