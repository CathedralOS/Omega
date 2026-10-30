//! Decode candidate metadata, then enter each vocabulary owner's validator.
use super::value::{RecordError, RecordReader, RecordValue, RecordWriter};

impl RecordValue for terminal_psi::TerminalIdentityIndex {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.0.write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        u32::read(input).map(Self)
    }
}

impl RecordValue for terminal_psi::TerminalIdentityTable {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        if self.invocation_scope().is_some() {
            return Err(
                output.error("invocation-local code identity cannot leave its retained owner")
            );
        }
        super::value::write_values(self.entries(), output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        let entries = Vec::<String>::read(input)?;
        if entries.first().is_none_or(|entry| !entry.is_empty()) {
            return Err(input.error("identity table has no absent entry"));
        }
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        for (ordinal, entry) in entries.iter().enumerate().skip(1) {
            if identities.intern(entry).0 as usize != ordinal {
                return Err(input.error("identity table repeats an entry or has an empty identity"));
            }
        }
        Ok(identities.build())
    }
}

impl RecordValue for terminal_psi::TerminalPsiIdentity {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        let Self {
            vocabulary_marker,
            program_fingerprint,
        } = self;
        vocabulary_marker.get().write(output)?;
        program_fingerprint.as_bytes().write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        Ok(Self {
            vocabulary_marker: terminal_psi::VocabularyMarker::new(u16::read(input)?)
                .ok_or_else(|| input.error("foreign Terminal vocabulary"))?,
            program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes(<[u8; 32]>::read(
                input,
            )?),
        })
    }
}

impl RecordValue for terminal_psi::CheckedProgramEntryTerminalReceipt {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.source_signature_identity().write(output)?;
        super::value::write_text(self.source_machine_name(), output)?;
        self.declaration().identity().write(output)?;
        self.terminal_psi_identity().write(output)?;
        self.terminal_entry().get().write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        let signature = <[u8; 32]>::read(input)?;
        let name = String::read(input)?;
        let declaration = terminal_psi::NominalDeclarationIdentity::new(RecordValue::read(input)?);
        let artifact = RecordValue::read(input)?;
        let entry = semantic_vocabulary::MachineId::new(u64::read(input)?)
            .ok_or_else(|| input.error("checked entry has an absent machine"))?;
        Ok(Self::new(signature, name, declaration, artifact, entry))
    }
}

impl RecordValue for optimization_core::OptimizationSelectionIdentity {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.bytes().write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        Ok(Self::from_bytes(<[u8; 32]>::read(input)?))
    }
}

impl RecordValue for optimization_core::PostTerminalOptimizationSelectionProjection {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.complete_selections().encode().write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        let complete = optimization_core::OptimizationSelections::decode(&Vec::<u8>::read(input)?)
            .map_err(|reason| input.error(format!("complete optimization selection: {reason}")))?;
        Ok(complete.project_post_terminal())
    }
}

impl RecordValue for calling_conventions::BoundaryCallingApplication {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        let Self {
            requirement_identity,
            target,
            report_fingerprint,
            commitment,
            plan,
            signature,
            opaque_applications,
            callback_context,
            callback_context_closed,
        } = self;
        requirement_identity.write(output)?;
        target.write(output)?;
        report_fingerprint.write(output)?;
        commitment.write(output)?;
        plan.plan().write(output)?;
        signature.write(output)?;
        opaque_applications.write(output)?;
        callback_context.write(output)?;
        callback_context_closed.write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        input.record(|input| {
            calling_conventions::BoundaryCallingApplicationCandidate {
                requirement_identity: RecordValue::read(input)?,
                target: RecordValue::read(input)?,
                report_fingerprint: RecordValue::read(input)?,
                commitment: RecordValue::read(input)?,
                plan: RecordValue::read(input)?,
                signature: RecordValue::read(input)?,
                opaque_applications: RecordValue::read(input)?,
                callback_context: RecordValue::read(input)?,
                callback_context_closed: RecordValue::read(input)?,
            }
            .validate()
            .map_err(|reason| input.error(reason))
        })
    }
}

impl RecordValue for target::AdmittedX86ScalarFmaProvider {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.requirement().profile().write(output)?;
        self.requirement().features().write(output)?;
        self.deployment().profile().write(output)?;
        self.deployment().features().write(output)?;
        for receipt in self.differential_receipts() {
            receipt.slot().write(output)?;
            receipt.operand_bits().write(output)?;
            receipt.fused_result_bits().write(output)?;
            receipt.multiply_then_add_result_bits().write(output)?;
        }
        Ok(())
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        let profile = target::TargetProfile::read(input)?;
        let features = <[target::X86TargetFeature; 2]>::read(input)?;
        let requirement = target::X86FeatureRequirement::scalar_fma(profile)
            .filter(|requirement| requirement.features() == &features)
            .ok_or_else(|| input.error("FMA requirement profile or feature set is invalid"))?;
        let deployment = target::X86DeploymentFeatures::scalar_fma(
            target::TargetProfile::read(input)?,
            &<[target::X86TargetFeature; 2]>::read(input)?,
        )
        .map_err(|reason| input.error(format!("FMA deployment: {reason:?}")))?;
        let mut read_receipt = || {
            target::X86ScalarFmaDifferentialReceipt::admit(
                RecordValue::read(input)?,
                RecordValue::read(input)?,
                RecordValue::read(input)?,
                RecordValue::read(input)?,
            )
            .map_err(|reason| input.error(format!("FMA differential receipt: {reason:?}")))
        };
        let receipts = [read_receipt()?, read_receipt()?];
        Self::admit(requirement, deployment, receipts)
            .map_err(|reason| input.error(format!("FMA provider: {reason:?}")))
    }
}
