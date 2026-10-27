//! Declaration-only entry recovery. Runtime loans and backing never enter this record.
use super::records::record_fields;
use super::value::{
    RecordError, RecordReader, RecordValue, RecordWriter, write_text, write_values,
};
use program_entry_plan::*;
use terminal_psi::{TerminalIdentityIndex, TerminalIdentityTable};

pub(super) struct EntryRecord {
    pub source: SelectedProgramEntrySignature,
    pub semantic: Option<calling_conventions::BoundaryCallingApplication>,
    pub physical: Option<calling_conventions::BoundaryCallingApplication>,
    pub storage: Option<SelectedProgramStorageEntryPlan>,
    pub fused: Vec<ProgramEntryFusedServiceEstablishment>,
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
            fused_service_establishments: &self.fused,
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
        output.sequence(entry.fused_service_establishments().len())?;
        for row in entry.fused_service_establishments() {
            FusedRecord::from(row).write(output)?;
        }
        write_optional_ref(entry.checked_entry, output)?;
        entry.physical_adapter().is_some().write(output)
    }
    pub fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        let source = SelectedProgramEntrySignature::read(input)?;
        let semantic = RecordValue::read(input)?;
        let physical = RecordValue::read(input)?;
        let storage = RecordValue::read(input)?;
        let records = Vec::<FusedRecord>::read(input)?;
        let mut identities = source.identities().clone().into_builder();
        let mut fused = Vec::with_capacity(records.len());
        input.allocate(
            records
                .len()
                .saturating_mul(std::mem::size_of::<ProgramEntryFusedServiceEstablishment>()),
        )?;
        for record in records {
            fused.push(
                record
                    .recover(source.identities(), &mut identities)
                    .map_err(|reason| input.error(reason))?,
            );
        }
        if identities.build() != *source.identities() {
            return Err(input.error("Fused establishment changed its entry identity table"));
        }
        Ok(Self {
            source,
            semantic,
            physical,
            storage,
            fused,
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

#[derive(PartialEq, Eq)]
struct FusedRecord {
    source_signature: [u8; 32],
    target: target::TargetProfile,
    receiver: TerminalIdentityIndex,
    attachment: TerminalIdentityIndex,
    field: TerminalIdentityIndex,
    path: Vec<TerminalIdentityIndex>,
    carrier: TerminalIdentityIndex,
    carrier_base: TerminalIdentityIndex,
    requirement: TerminalIdentityIndex,
    schema: effects::provider_plan::ServiceSchemaDigest,
    provider: effects::provider_plan::ProviderPlanDigest,
    application: usize,
    receiving: Vec<ProgramEntryReceivingRequirement>,
}
record_fields!(FusedRecord {
    source_signature,
    target,
    receiver,
    attachment,
    field,
    path,
    carrier,
    carrier_base,
    requirement,
    schema,
    provider,
    application,
    receiving
});
record_fields!(ProgramEntryReceivingRequirement {
    source_requirement,
    requirement,
    candidate,
    provider_declaration,
    receiving_application,
    receiving_regions
});

impl From<&ProgramEntryFusedServiceEstablishment> for FusedRecord {
    fn from(row: &ProgramEntryFusedServiceEstablishment) -> Self {
        Self {
            source_signature: row.source_signature_identity().bytes(),
            target: row.target_slot().owner,
            receiver: row.receiver_type_identity(),
            attachment: row.attachment_type_identity(),
            field: row.field_identity(),
            path: row.field_path().to_vec(),
            carrier: row.carrier_type_identity(),
            carrier_base: row.carrier_base_identity(),
            requirement: row.requirement_identity(),
            schema: row.service_schema_digest(),
            provider: row.selected_provider_plan_digest(),
            application: row.receiver_application_index(),
            receiving: row.receiving_requirements().to_vec(),
        }
    }
}
impl FusedRecord {
    fn recover(
        self,
        table: &TerminalIdentityTable,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    ) -> Result<ProgramEntryFusedServiceEstablishment, String> {
        let text = |identity| {
            table
                .text(identity)
                .filter(|text| !text.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| {
                    "Fused establishment has an absent or out-of-range identity".to_owned()
                })
        };
        let recovered = ProgramEntryFusedServiceEstablishment::new(
            ProgramEntrySignatureIdentity::from_bytes(self.source_signature),
            self.target.program_entry_slot(),
            self.receiver,
            text(self.attachment)?,
            text(self.field)?,
            self.path
                .iter()
                .map(|identity| text(*identity))
                .collect::<Result<Vec<_>, _>>()?,
            text(self.carrier)?,
            text(self.carrier_base)?,
            text(self.requirement)?,
            self.schema,
            self.provider,
            self.application,
            self.receiving.clone(),
            identities,
        )
        .map_err(str::to_owned)?;
        if FusedRecord::from(&recovered) != self {
            return Err("Fused establishment changed its retained coordinates".to_owned());
        }
        Ok(recovered)
    }
}
