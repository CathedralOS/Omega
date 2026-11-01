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

#[cfg(test)]
mod exact_abi_tests {
    use super::*;
    use calling_conventions::{
        BoundaryCallingApplication, BoundarySignatureGeometry, CallSignature, CallingPolicy,
    };
    use semantic_vocabulary::{CallingAbiIdentity, CallingOperandKind, CallingStaticOperand};

    fn application(actual: &str) -> BoundaryCallingApplication {
        let abi = CallingAbiIdentity {
            requirement_identity: "Service::call".into(),
            declaring_application: "Service<u8>".into(),
            operands: vec![CallingStaticOperand {
                ordinal: 2,
                kind: CallingOperandKind::Type,
                identity: actual.into(),
            }],
        };
        let mut application = BoundaryCallingApplication {
            requirement_identity: abi.requirement_identity.clone(),
            abi,
            target: target::NativeTarget::from_omega_target_name(Some("windows_x86_64")).unwrap(),
            report_fingerprint: 73,
            commitment: [0; 32],
            plan: calling_conventions::evaluate_ordinary_boundary_entry_plan(
                CallingPolicy::MicrosoftX64,
                &CallSignature {
                    parameters: vec![],
                    result: None,
                },
            )
            .unwrap(),
            signature: BoundarySignatureGeometry {
                shapes: vec![],
                fields: vec![],
                parameters: vec![],
                result: None,
                native_parameters: Default::default(),
                native_parameter_rows: Default::default(),
                callback_layouts: Default::default(),
                callback_layout_rows: Default::default(),
                callback_bindings: Default::default(),
                callback_binding_rows: Default::default(),
            },
            opaque_applications: vec![],
            callback_context: Default::default(),
            callback_context_closed: false,
        };
        application.commitment = application.recomputed_commitment().unwrap();
        application
    }

    #[test]
    fn native_recovery_preserves_same_shape_phantom_applications() {
        let applications = vec![application("u32"), application("u64")];
        assert_eq!(applications[0].signature, applications[1].signature);
        assert_ne!(applications[0].commitment, applications[1].commitment);
        let mut output = RecordWriter::new(Default::default());
        applications.write(&mut output).unwrap();
        let bytes = output.finish();
        let mut input = RecordReader::new(&bytes, Default::default()).unwrap();
        let recovered = Vec::<BoundaryCallingApplication>::read(&mut input).unwrap();
        input.finish().unwrap();
        assert_eq!(applications, recovered);
        use effects::provider_plan::{
            BoundaryCallingPlanCommitment, ProviderIdentityTable, ServiceCallingApplication,
            ServiceCallingPlan, ServiceMethod, ServiceSchema, ServiceSurface,
        };
        let mut identities = ProviderIdentityTable::default();
        let requirement = identities.intern("Service::call");
        let schema = ServiceSchema {
            declaration: std::sync::Arc::new(ServiceSurface {
                trait_name: identities.intern("Service"),
                identities,
                trait_package_identity: None,
                receiver_applications: vec![],
                methods: vec![ServiceMethod {
                    name: "call".into(),
                    requirement_identity: requirement,
                    ..Default::default()
                }],
            }),
            calling_applications: applications
                .iter()
                .map(|application| ServiceCallingApplication {
                    requirement,
                    abi: application.abi.clone(),
                    plan: ServiceCallingPlan {
                        report_fingerprint: application.report_fingerprint,
                        commitment: BoundaryCallingPlanCommitment::from_digest(
                            application.commitment,
                        ),
                    },
                })
                .collect(),
        };
        let mut output = RecordWriter::new(Default::default());
        schema.write(&mut output).unwrap();
        let bytes = output.finish();
        let mut input = RecordReader::new(&bytes, Default::default()).unwrap();
        let recovered = ServiceSchema::read(&mut input).unwrap();
        input.finish().unwrap();
        assert_eq!(schema, recovered);
        assert_eq!(recovered.calling_applications.len(), 2);
        for application in &applications {
            assert_eq!(
                recovered
                    .calling_application(&application.abi)
                    .unwrap()
                    .commitment
                    .as_bytes(),
                application.commitment
            );
        }
    }

    #[test]
    fn native_recovery_rejects_full_key_mutations_with_stale_coordinates() {
        let application = application("u32");
        for mutation in 0..3 {
            let mut changed = application.clone();
            match mutation {
                0 => changed.abi.operands[0].identity = "u64".into(),
                1 => changed.abi.operands[0].ordinal = 3,
                _ => changed.abi.declaring_application = "Service<u64>".into(),
            }
            let mut output = RecordWriter::new(Default::default());
            changed.write(&mut output).unwrap();
            let mut bytes = output.finish();
            // Keep the old compact coordinate as well as the old commitment.
            bytes[..8].copy_from_slice(&application.abi.id().get().to_le_bytes());
            assert!(
                BoundaryCallingApplication::read(
                    &mut RecordReader::new(&bytes, Default::default()).unwrap()
                )
                .is_err()
            );
            let mut output = RecordWriter::new(Default::default());
            changed.write(&mut output).unwrap();
            assert!(
                BoundaryCallingApplication::read(
                    &mut RecordReader::new(&output.finish(), Default::default()).unwrap()
                )
                .is_err(),
                "a recomputed compact coordinate cannot replace the selected full commitment"
            );
        }
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
            abi,
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
        abi.write(output)?;
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
                abi: RecordValue::read(input)?,
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

impl RecordValue for semantic_vocabulary::CallingAbiIdentity {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.validate().map_err(|reason| output.error(reason))?;
        self.id().get().write(output)?;
        self.requirement_identity.write(output)?;
        self.declaring_application.write(output)?;
        self.operands.write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        let coordinate = u64::read(input)?;
        let identity = Self {
            requirement_identity: RecordValue::read(input)?,
            declaring_application: RecordValue::read(input)?,
            operands: RecordValue::read(input)?,
        };
        identity.validate().map_err(|reason| input.error(reason))?;
        if identity.id().get() != coordinate {
            return Err(input.error("declaration ABI coordinate differs from its full key"));
        }
        Ok(identity)
    }
}

impl RecordValue for semantic_vocabulary::CallingStaticOperand {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.ordinal.write(output)?;
        (self.kind as u8).write(output)?;
        self.identity.write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        use semantic_vocabulary::CallingOperandKind;
        let ordinal = u32::read(input)?;
        let kind = match u8::read(input)? {
            0 => CallingOperandKind::Type,
            1 => CallingOperandKind::Const,
            2 => CallingOperandKind::Value,
            3 => CallingOperandKind::Machine,
            4 => CallingOperandKind::Implementation,
            5 => CallingOperandKind::Proposition,
            _ => return Err(input.error("unknown declaration ABI operand kind")),
        };
        Ok(Self {
            ordinal,
            kind,
            identity: RecordValue::read(input)?,
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
