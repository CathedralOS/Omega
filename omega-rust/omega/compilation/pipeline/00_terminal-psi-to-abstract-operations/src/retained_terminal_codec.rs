/*
  Compiler-private binary persistence (`OMGNTI20`) for retained native
  derivation inputs (`retain_native_derivation_inputs`,
  `recover_native_derivation_inputs`) and installation environment assumptions.
*/
mod admissions {
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
                    return Err(
                        input.error("identity table repeats an entry or has an empty identity")
                    );
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
                program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes(
                    <[u8; 32]>::read(input)?,
                ),
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
            let declaration =
                terminal_psi::NominalDeclarationIdentity::new(RecordValue::read(input)?);
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
            let complete =
                optimization_core::OptimizationSelections::decode(&Vec::<u8>::read(input)?)
                    .map_err(|reason| {
                        input.error(format!("complete optimization selection: {reason}"))
                    })?;
            Ok(complete.project_post_terminal())
        }
    }

    impl RecordValue for calling_conventions::BoundaryCallingApplication {
        fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
            let Self {
                abi,
                calling_owner_application,
                requirement_identity,
                target,
                report_fingerprint,
                commitment,
                plan,
                dispatch_only_parameter_count,
                signature,
                opaque_applications,
                callback_context,
                callback_context_closed,
            } = self;
            abi.write(output)?;
            calling_owner_application.write(output)?;
            requirement_identity.write(output)?;
            target.write(output)?;
            report_fingerprint.write(output)?;
            commitment.write(output)?;
            plan.plan().write(output)?;
            dispatch_only_parameter_count.write(output)?;
            signature.write(output)?;
            opaque_applications.write(output)?;
            callback_context.write(output)?;
            callback_context_closed.write(output)
        }
        fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
            input.record(|input| {
                calling_conventions::BoundaryCallingApplicationCandidate {
                    abi: RecordValue::read(input)?,
                    calling_owner_application: RecordValue::read(input)?,
                    requirement_identity: RecordValue::read(input)?,
                    target: RecordValue::read(input)?,
                    report_fingerprint: RecordValue::read(input)?,
                    commitment: RecordValue::read(input)?,
                    plan: RecordValue::read(input)?,
                    dispatch_only_parameter_count: RecordValue::read(input)?,
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

    impl RecordValue for backend_plan::DirectBoundaryLeaf {
        fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
            self.validate_calling_custody()
                .map_err(|reason| output.error(reason))?;
            let Self {
                declaration_identity,
                callable_identity,
                satisfied_schema,
                plan,
                satisfied_requirement_identity,
                execution,
            } = self;
            declaration_identity.write(output)?;
            callable_identity.write(output)?;
            satisfied_schema.write(output)?;
            plan.write(output)?;
            satisfied_requirement_identity.write(output)?;
            execution.write(output)
        }
        fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
            input.record(|input| {
                let value = Self {
                    declaration_identity: RecordValue::read(input)?,
                    callable_identity: RecordValue::read(input)?,
                    satisfied_schema: RecordValue::read(input)?,
                    plan: RecordValue::read(input)?,
                    satisfied_requirement_identity: RecordValue::read(input)?,
                    execution: RecordValue::read(input)?,
                };
                value
                    .validate_calling_custody()
                    .map_err(|reason| input.error(reason))?;
                Ok(value)
            })
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
                calling_owner_application: None,
                requirement_identity: abi.requirement_identity.clone(),
                abi,
                target: target::NativeTarget::from_omega_target_name(Some("windows_x86_64"))
                    .unwrap(),
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
                dispatch_only_parameter_count: 0,
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
        fn native_recovery_preserves_sibling_calling_owners() {
            let mut first = application("u64");
            first.calling_owner_application = Some("TimerCallback".into());
            first.commitment = first.recomputed_commitment().unwrap();
            let mut sibling = first.clone();
            sibling.calling_owner_application = Some("UnusedTimerCallback".into());
            sibling.commitment = sibling.recomputed_commitment().unwrap();
            assert_eq!(first.abi, sibling.abi);
            assert_eq!(first.plan, sibling.plan);
            assert_ne!(first.commitment, sibling.commitment);
            let applications = vec![first, sibling];
            let mut output = RecordWriter::new(Default::default());
            applications.write(&mut output).unwrap();
            let bytes = output.finish();
            let mut input = RecordReader::new(&bytes, Default::default()).unwrap();
            let recovered = Vec::<BoundaryCallingApplication>::read(&mut input).unwrap();
            assert_eq!(recovered, applications);
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
                            promise:
                                effects::provider_plan::BoundaryCallingPlanPromise::from_digest(
                                    application.plan.contract_commitment_digest(),
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
        fn native_recovery_preserves_dispatch_projection_and_remaining_argument() {
            use calling_conventions::{
                BoundaryNativeParameterGeometry, BoundaryNativeParameterOrigin,
                BoundaryNativeParameterShape, BoundaryValueClass, BoundaryValueShape,
                NativeParameterId, ValueShape,
            };
            let mut application = application("u32");
            application.signature.shapes = vec![
                BoundaryValueShape {
                    class: BoundaryValueClass::Reference,
                    byte_size: 8,
                    alignment: 8,
                },
                BoundaryValueShape {
                    class: BoundaryValueClass::Integer,
                    byte_size: 8,
                    alignment: 8,
                },
            ];
            application.signature.parameters = vec![0, 1];
            application.signature.native_parameter_rows = application
                .signature
                .native_parameters
                .insert_many((0..2).map(|ordinal| BoundaryNativeParameterGeometry {
                    identity: NativeParameterId::new(u64::from(ordinal) + 1).unwrap(),
                    native_ordinal: ordinal,
                    shape: BoundaryNativeParameterShape::Semantic(ordinal as u16),
                    origin: BoundaryNativeParameterOrigin::SemanticFormal {
                        formal_ordinal: ordinal,
                    },
                }));
            application.dispatch_only_parameter_count = 1;
            application.plan = calling_conventions::evaluate_ordinary_boundary_entry_plan(
                CallingPolicy::MicrosoftX64,
                &CallSignature {
                    parameters: vec![ValueShape::integer(8, 8)],
                    result: None,
                },
            )
            .unwrap();
            application.commitment = application.recomputed_commitment().unwrap();
            let round_trip = |application: &BoundaryCallingApplication| {
                let mut output = RecordWriter::new(Default::default());
                application.write(&mut output).unwrap();
                let bytes = output.finish();
                let mut input = RecordReader::new(&bytes, Default::default()).unwrap();
                let recovered = BoundaryCallingApplication::read(&mut input);
                if recovered.is_ok() {
                    input.finish().unwrap();
                }
                recovered
            };
            let recovered = round_trip(&application).unwrap();
            assert_eq!(recovered, application);
            let projection = recovered.call_projection().unwrap();
            assert_eq!(projection.parameter_ordinal(0).unwrap(), None);
            assert_eq!(projection.parameter_ordinal(1).unwrap(), Some(0));
            assert_eq!(recovered.plan.plan().call.parameters.len(), 1);

            let mut changed = application.clone();
            changed.dispatch_only_parameter_count = 0;
            changed.plan = calling_conventions::evaluate_ordinary_boundary_entry_plan(
                CallingPolicy::MicrosoftX64,
                &CallSignature {
                    parameters: vec![ValueShape::integer(8, 8); 2],
                    result: None,
                },
            )
            .unwrap();
            assert_ne!(
                changed.recomputed_commitment().unwrap(),
                application.commitment
            );
            let error = round_trip(&changed).unwrap_err();
            assert!(error.reason.contains("commitment"), "{error}");
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
}
mod installation_environment {
    use super::value::{RecordError, RecordLimits, RecordReader, RecordValue, RecordWriter};
    use installation_evidence::{
        BootstrapAssumptionInterval, BootstrapEnvironmentAssumption,
        InstallationEnvironmentAssumption, InterruptDeliveryInterval,
        X86InterruptDeliveryAssumption, X86InterruptDeliveryProfile,
    };

    super::records::record_fields!(BootstrapEnvironmentAssumption {
        psi,
        profile,
        entry,
        installation,
        interval,
        environment_description_sha256,
        source_provenance,
        caller_provenance
    });

    impl RecordValue for BootstrapAssumptionInterval {
        fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
            match self {
                Self::EntryUntilInstallationPublished => 0u8.write(output),
            }
        }
        fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
            match u8::read(input)? {
                0 => Ok(Self::EntryUntilInstallationPublished),
                _ => Err(input.error("unknown bootstrap assumption interval")),
            }
        }
    }

    super::records::record_fields!(X86InterruptDeliveryAssumption {
        psi,
        profile,
        entry,
        installation,
        interval,
        delivery,
        environment_description_sha256,
        source_provenance,
        caller_provenance
    });

    impl RecordValue for InterruptDeliveryInterval {
        fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
            match self {
                Self::InstalledScopeLifetime => 0u8.write(output),
            }
        }
        fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
            match u8::read(input)? {
                0 => Ok(Self::InstalledScopeLifetime),
                _ => Err(input.error("unknown interrupt delivery interval")),
            }
        }
    }
    impl RecordValue for X86InterruptDeliveryProfile {
        fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
            match self {
                Self::ArchitecturalExceptionsAndExternalIrqs => 0u8.write(output),
            }
        }
        fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
            match u8::read(input)? {
                0 => Ok(Self::ArchitecturalExceptionsAndExternalIrqs),
                _ => Err(input.error("unknown interrupt delivery profile")),
            }
        }
    }
    impl RecordValue for InstallationEnvironmentAssumption {
        fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
            match self {
                Self::Bootstrap(row) => {
                    0u8.write(output)?;
                    row.write(output)
                }
                Self::InterruptDelivery(row) => {
                    1u8.write(output)?;
                    row.write(output)
                }
            }
        }
        fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
            match u8::read(input)? {
                0 => Ok(Self::Bootstrap(BootstrapEnvironmentAssumption::read(
                    input,
                )?)),
                1 => Ok(Self::InterruptDelivery(
                    X86InterruptDeliveryAssumption::read(input)?,
                )),
                _ => Err(input.error("unknown installation environment condition")),
            }
        }
    }

    const REVIEW_SCHEMA: &[u8; 8] = b"OMGINST1";

    pub fn encode_installation_assumptions(
        rows: &[InstallationEnvironmentAssumption],
    ) -> Result<Vec<u8>, String> {
        for row in rows {
            row.validate_description()?;
        }
        let mut output = RecordWriter::new(RecordLimits::default());
        output
            .extend(REVIEW_SCHEMA)
            .map_err(|error| error.to_string())?;
        super::value::write_values(rows, &mut output).map_err(|error| error.to_string())?;
        Ok(output.finish())
    }

    pub fn decode_installation_assumptions(
        bytes: &[u8],
    ) -> Result<Vec<InstallationEnvironmentAssumption>, String> {
        let mut input =
            RecordReader::new(bytes, RecordLimits::default()).map_err(|error| error.to_string())?;
        let schema = input
            .take(REVIEW_SCHEMA.len())
            .map_err(|error| error.to_string())?;
        if schema != REVIEW_SCHEMA {
            return Err("foreign installation assumption review schema".into());
        }
        let rows = Vec::<InstallationEnvironmentAssumption>::read(&mut input)
            .map_err(|error| error.to_string())?;
        input.finish().map_err(|error| error.to_string())?;
        for row in &rows {
            row.validate_description()?;
        }
        Ok(rows)
    }
}
pub use installation_environment::{
    decode_installation_assumptions, encode_installation_assumptions,
};
mod entry {
    use super::value::{
        RecordError, RecordReader, RecordValue, RecordWriter, write_text, write_values,
    };
    use program_entry_plan::*;
    use terminal_psi::TerminalIdentityTable;

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
            let plan =
                Self::from_target_slot(slot, RecordValue::read(input)?, String::read(input)?)
                    .map_err(|reason| input.error(reason.to_string()))?;
            match Option::<ProgramEntryPhysicalContractPlan>::read(input)? {
                None => Ok(plan),
                Some(physical) => plan
                    .with_physical_contract(physical)
                    .map_err(|reason| input.error(reason.to_string())),
            }
        }
    }

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
}
mod records {
    macro_rules! record_fields {
    ($value:path { $($field:ident),* $(,)? }) => {
        impl $crate::retained_terminal::codec::value::RecordValue for $value {
            fn write(&self, output: &mut $crate::retained_terminal::codec::value::RecordWriter)
                -> Result<(), $crate::retained_terminal::codec::value::RecordError>
            {
                let Self { $($field),* } = self;
                $($crate::retained_terminal::codec::value::RecordValue::write($field, output)?;)*
                Ok(())
            }
            fn read(input: &mut $crate::retained_terminal::codec::value::RecordReader<'_>)
                -> Result<Self, $crate::retained_terminal::codec::value::RecordError>
            {
                input.record(|input| Ok(Self {
                    $($field: $crate::retained_terminal::codec::value::RecordValue::read(input)?),*
                }))
            }
        }
    };
}
    pub(super) use record_fields;
}
mod selection {
    use super::records::record_fields;
    use super::value::{RecordError, RecordReader, RecordValue, RecordWriter};

    record_fields!(effects::SelectedProviderPlanCandidate {
        plans,
        assumed_bindings,
        report_fingerprint,
        installation_reach_resolutions,
        specialized_requirement_rows,
        external_requirement_applications,
        family_realization_rows,
        toolchain_settled_identities,
        identities,
    });

    impl RecordValue for effects::SelectedProviderPlanFacts {
        fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
            self.recovery_candidate().write(output)
        }
        fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
            effects::SelectedProviderPlanCandidate::read(input)?
                .validate()
                .map_err(|reason| input.error(reason))
        }
    }
}
mod storage {
    use super::value::{RecordError, RecordReader, RecordValue, RecordWriter};
    use arena::{Arena, ArenaSlotStorage, Handle, HandleSpan};

    impl<T> RecordValue for Handle<T> {
        fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
            self.arena_index().write(output)?;
            self.generation().write(output)
        }
        fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
            let index = u32::read(input)?;
            let generation = u32::read(input)?;
            if (index == 0) != (generation == 0) {
                return Err(input.error("handle has a partial absence identity"));
            }
            Ok(Self::from_parts(index, generation))
        }
    }

    impl<T> RecordValue for HandleSpan<T> {
        fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
            self.start().write(output)?;
            self.count().write(output)
        }
        fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
            let start = Handle::read(input)?;
            let count = u32::read(input)?;
            if (count != 0 && !start.is_valid())
                || (count != 0 && start.arena_index().checked_add(count - 1).is_none())
            {
                return Err(input.error("span has an invalid handle range"));
            }
            Ok(Self::from_parts(start, count))
        }
    }

    impl<T: RecordValue> RecordValue for Arena<T> {
        fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
            let storage = self.storage_view();
            storage.dummy.write(output)?;
            output.sequence(storage.items.len())?;
            for item in storage.items {
                item.write(output)?;
            }
            storage.fresh_generation.write(output)?;
            storage.fresh.write(output)?;
            storage.slots.is_some().write(output)?;
            if let Some(slots) = storage.slots {
                slots.generations.write(output)?;
                slots.occupied.write(output)?;
                slots.free_indices.write(output)?;
            }
            Ok(())
        }
        fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
            input.record(|input| {
                let dummy = T::read(input)?;
                let items = Vec::<T>::read(input)?;
                let generation = u32::read(input)?;
                let fresh = bool::read(input)?;
                let slots = if bool::read(input)? {
                    let generations = Vec::<u32>::read(input)?;
                    let occupied = Vec::<bool>::read(input)?;
                    let free_indices = Vec::<u32>::read(input)?;
                    input.allocate(occupied.len())?;
                    Some(ArenaSlotStorage {
                        generations,
                        occupied,
                        free_indices,
                    })
                } else {
                    None
                };
                Arena::from_storage(dummy, items, slots, generation, fresh)
                    .map_err(|reason| input.error(reason))
            })
        }
    }
}
mod value {
    pub(super) use compiler_storage::{RecordError, RecordLimits, RecordWriter};
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::Arc;
    pub(super) type RecordReader<'bytes> =
        compiler_storage::RecordReader<'bytes, NativeRecordContext>;

    #[derive(Default)]
    pub(super) struct NativeRecordContext {
        pub(super) binding_identities: Option<Arc<terminal_psi::TerminalIdentityTable>>,
    }

    pub(super) trait RecordValue: Sized {
        fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError>;
        fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError>;
    }

    macro_rules! integer_value {
    ($($value:ty),* $(,)?) => {$ (
        impl RecordValue for $value {
            fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
                output.extend(&self.to_le_bytes())
            }
            fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
                let bytes = input.take(std::mem::size_of::<Self>())?;
                let bytes = bytes.try_into().map_err(|_| input.error("integer width mismatch"))?;
                Ok(Self::from_le_bytes(bytes))
            }
        }
    )*};
}
    integer_value!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128);

    impl RecordValue for usize {
        fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
            u64::try_from(*self)
                .map_err(|_| output.error("integer exceeds record width"))?
                .write(output)
        }
        fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
            Self::try_from(u64::read(input)?).map_err(|_| input.error("integer exceeds this host"))
        }
    }

    impl RecordValue for bool {
        fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
            u8::from(*self).write(output)
        }
        fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
            match u8::read(input)? {
                0 => Ok(false),
                1 => Ok(true),
                _ => Err(input.error("invalid Boolean tag")),
            }
        }
    }

    impl RecordValue for String {
        fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
            output.sequence(self.len())?;
            output.extend(self.as_bytes())
        }
        fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
            let length = input.sequence::<u8>()?;
            let text = std::str::from_utf8(input.take(length)?)
                .map_err(|_| input.error("invalid UTF-8"))?;
            Ok(text.to_owned())
        }
    }

    impl<T: RecordValue> RecordValue for Vec<T> {
        fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
            output.sequence(self.len())?;
            for value in self {
                value.write(output)?;
            }
            Ok(())
        }
        fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
            input.record(|input| {
                let length = input.sequence::<T>()?;
                let mut values = Vec::with_capacity(length.min(1024));
                for _ in 0..length {
                    values.push(T::read(input)?);
                }
                Ok(values)
            })
        }
    }

    impl<T: RecordValue, const LENGTH: usize> RecordValue for [T; LENGTH] {
        fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
            for value in self {
                value.write(output)?;
            }
            Ok(())
        }
        fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
            input.allocate(std::mem::size_of::<Self>())?;
            let mut values = Vec::with_capacity(LENGTH);
            for _ in 0..LENGTH {
                values.push(T::read(input)?);
            }
            values
                .try_into()
                .map_err(|_| input.error("array width mismatch"))
        }
    }

    impl<T: RecordValue> RecordValue for Option<T> {
        fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
            match self {
                None => 0u8.write(output),
                Some(value) => {
                    1u8.write(output)?;
                    value.write(output)
                }
            }
        }
        fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
            input.record(|input| match u8::read(input)? {
                0 => Ok(None),
                1 => T::read(input).map(Some),
                _ => Err(input.error("invalid optional-value tag")),
            })
        }
    }

    impl<T: RecordValue, Error: RecordValue> RecordValue for Result<T, Error> {
        fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
            match self {
                Ok(value) => {
                    0u8.write(output)?;
                    value.write(output)
                }
                Err(error) => {
                    1u8.write(output)?;
                    error.write(output)
                }
            }
        }
        fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
            input.record(|input| match u8::read(input)? {
                0 => T::read(input).map(Ok),
                1 => Error::read(input).map(Err),
                _ => Err(input.error("invalid result tag")),
            })
        }
    }

    impl<T: RecordValue> RecordValue for Arc<T> {
        fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
            self.as_ref().write(output)
        }
        fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
            input.allocate(
                std::mem::size_of::<T>().saturating_add(2 * std::mem::size_of::<usize>()),
            )?;
            input.record(T::read).map(Arc::new)
        }
    }

    impl<T: RecordValue + Ord> RecordValue for BTreeSet<T> {
        fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
            output.sequence(self.len())?;
            for value in self {
                value.write(output)?;
            }
            Ok(())
        }
        fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
            let values = Vec::<T>::read(input)?;
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(input.error("set values are not strictly ordered"));
            }
            input.allocate(
                values
                    .len()
                    .saturating_mul(std::mem::size_of::<T>() + 4 * std::mem::size_of::<usize>()),
            )?;
            Ok(values.into_iter().collect())
        }
    }

    impl<Key: RecordValue + Ord, Value: RecordValue> RecordValue for BTreeMap<Key, Value> {
        fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
            output.sequence(self.len())?;
            for (key, value) in self {
                key.write(output)?;
                value.write(output)?;
            }
            Ok(())
        }
        fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
            input.record(|input| {
                let length = input.sequence::<(Key, Value)>()?;
                input.allocate(length.saturating_mul(4 * std::mem::size_of::<usize>()))?;
                let mut values = BTreeMap::new();
                for _ in 0..length {
                    let key = Key::read(input)?;
                    if values
                        .last_key_value()
                        .is_some_and(|(previous, _)| previous >= &key)
                    {
                        return Err(input.error("map keys are not strictly ordered"));
                    }
                    values.insert(key, Value::read(input)?);
                }
                Ok(values)
            })
        }
    }

    impl RecordValue for () {
        fn write(&self, _output: &mut RecordWriter) -> Result<(), RecordError> {
            Ok(())
        }
        fn read(_input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
            Ok(())
        }
    }
    impl<T: RecordValue> RecordValue for Box<T> {
        fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
            self.as_ref().write(output)
        }
        fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
            input.allocate(std::mem::size_of::<T>())?;
            input.record(T::read).map(Box::new)
        }
    }
    impl<Left: RecordValue, Right: RecordValue> RecordValue for (Left, Right) {
        fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
            self.0.write(output)?;
            self.1.write(output)
        }
        fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
            input.record(|input| Ok((Left::read(input)?, Right::read(input)?)))
        }
    }

    pub(super) fn write_values<T: RecordValue>(
        values: &[T],
        output: &mut RecordWriter,
    ) -> Result<(), RecordError> {
        output.sequence(values.len())?;
        for value in values {
            value.write(output)?;
        }
        Ok(())
    }

    pub(super) fn write_text(value: &str, output: &mut RecordWriter) -> Result<(), RecordError> {
        output.sequence(value.len())?;
        output.extend(value.as_bytes())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn encode<T: RecordValue>(value: &T) -> Vec<u8> {
            let mut writer = RecordWriter::new(RecordLimits::default());
            value.write(&mut writer).expect("bounded record");
            writer.finish()
        }

        #[test]
        fn record_values_preserve_wide_bits_and_deferred_outcomes() {
            let values = vec![
                None,
                Some(Ok(u128::MAX)),
                Some(Err("pending".to_owned())),
                Some(Ok(0)),
            ];
            let bytes = encode(&values);
            let mut reader =
                RecordReader::new(&bytes, RecordLimits::default()).expect("bounded bytes");
            assert_eq!(
                Vec::<Option<Result<u128, String>>>::read(&mut reader).expect("value"),
                values
            );
            reader.finish().expect("complete record");
        }

        #[test]
        fn record_values_reject_every_truncated_prefix_and_trailing_bytes() {
            let value = (i128::MIN, vec!["entry".to_owned(), "λ".to_owned()]);
            let mut bytes = encode(&value);
            for length in 0..bytes.len() {
                let mut reader = RecordReader::new(&bytes[..length], RecordLimits::default())
                    .expect("bounded prefix");
                assert!(<(i128, Vec<String>)>::read(&mut reader).is_err());
            }
            bytes.push(0);
            let mut reader =
                RecordReader::new(&bytes, RecordLimits::default()).expect("bounded bytes");
            <(i128, Vec<String>)>::read(&mut reader).expect("value");
            assert!(reader.finish().is_err());
        }

        #[test]
        fn record_limits_precede_allocation_and_nested_decoding() {
            let enormous_length = u64::MAX.to_le_bytes();
            let mut reader = RecordReader::new(&enormous_length, RecordLimits::default())
                .expect("bounded bytes");
            assert!(Vec::<u128>::read(&mut reader).is_err());
            let bytes = [0; 32];
            let limits = RecordLimits {
                allocation: 31,
                ..RecordLimits::default()
            };
            let mut reader = RecordReader::new(&bytes, limits).expect("bounded bytes");
            assert!(<[u8; 32]>::read(&mut reader).is_err());
            fn nested(reader: &mut RecordReader<'_>) -> Result<(), RecordError> {
                reader.record(|reader| {
                    if bool::read(reader)? {
                        nested(reader)
                    } else {
                        Ok(())
                    }
                })
            }
            let bytes = [1; 129];
            let mut reader =
                RecordReader::new(&bytes, RecordLimits::default()).expect("bounded bytes");
            assert!(
                nested(&mut reader)
                    .expect_err("depth bound")
                    .reason
                    .contains("nesting")
            );
        }

        #[test]
        fn record_values_refuse_unknown_tags_and_duplicate_set_rows() {
            for tag in [2u8, 255] {
                let bytes = [tag];
                let mut reader =
                    RecordReader::new(&bytes, RecordLimits::default()).expect("bounded bytes");
                assert!(Option::<u32>::read(&mut reader).is_err());
            }
            let bytes = encode(&vec![3u32, 3]);
            let mut reader =
                RecordReader::new(&bytes, RecordLimits::default()).expect("bounded bytes");
            assert!(BTreeSet::<u32>::read(&mut reader).is_err());
        }
    }
}
#[path = "retained_terminal_vocabulary.rs"]
mod vocabulary;

use super::{
    RetainedTerminalArtifact, TerminalNativeProposalRequest,
    project_terminal_native_realization_proposal,
};
use crate::providers::provider_admission::TerminalAuthorityPermissionPolicy;
use diagnostics::Diagnostic;
use entry::EntryRecord;
use std::sync::Arc;
use value::{RecordError, RecordLimits, RecordReader, RecordValue, RecordWriter, write_values};

const RECORD_SCHEMA: &[u8; 8] = b"OMGNTI22";
const RECORD_SCHEMA_FAMILY: &[u8; 6] = b"OMGNTI";
const SUPERSEDED_RECORD_SCHEMA: &str = "retained native derivation inputs use a superseded record \
    schema; rebuild the component to retain them again";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retired_entry_field_record_schema_is_rejected() {
        for bytes in [
            b"OMGNTI01",
            b"OMGNTI09",
            b"OMGNTI10",
            b"OMGNTI11",
            b"OMGNTI12",
            b"OMGNTI13",
            b"OMGNTI14",
            b"OMGNTI15",
            b"OMGNTI16",
            b"OMGNTI17",
            b"OMGNTI18",
            b"OMGNTI19",
            b"OMGNTI20",
        ] {
            let mut input =
                RecordReader::new(bytes, RecordLimits::default()).expect("bounded old schema");
            let Err(error) = DerivationInputs::read(&mut input) else {
                panic!("retired record schema accepted");
            };
            assert_eq!(error.reason, SUPERSEDED_RECORD_SCHEMA);
        }
    }

    #[test]
    fn superseded_text_requirement_schema_rejects_with_recovery_guidance() {
        let mut input =
            RecordReader::new(b"OMGNTI11", RecordLimits::default()).expect("bounded old schema");
        let Err(error) = DerivationInputs::read(&mut input) else {
            panic!("text-requirement record schema accepted");
        };
        assert!(error.reason.contains("rebuild the component"), "{error}");
    }

    #[test]
    fn newer_record_schema_is_not_called_superseded() {
        let mut input =
            RecordReader::new(b"OMGNTI22", RecordLimits::default()).expect("bounded new schema");
        let Err(error) = DerivationInputs::read(&mut input) else {
            panic!("newer record schema accepted");
        };
        assert_ne!(error.reason, SUPERSEDED_RECORD_SCHEMA);
    }

    #[test]
    fn unrelated_record_schema_is_foreign() {
        let mut input =
            RecordReader::new(b"OTHERREC", RecordLimits::default()).expect("bounded bytes");
        let Err(error) = DerivationInputs::read(&mut input) else {
            panic!("foreign record schema accepted");
        };
        assert_eq!(error.reason, "foreign native derivation record schema");
    }
}

pub fn retain_native_derivation_inputs(
    checked_entry: &terminal_psi::CheckedProgramEntryTerminalReceipt,
    physical_adapter_requested: bool,
    provider_proposals: &[backend_plan::TerminalTargetProviderProposal],
    request: &TerminalNativeProposalRequest<'_>,
    accepted_package_permissions: &TerminalAuthorityPermissionPolicy,
    installation_assumptions: &[installation_evidence::InstallationEnvironmentAssumption],
) -> Result<Vec<u8>, String> {
    capture(
        checked_entry,
        physical_adapter_requested,
        provider_proposals,
        request,
        accepted_package_permissions,
        installation_assumptions,
    )
    .map_err(|reason| reason.to_string())
}

fn capture(
    checked_entry: &terminal_psi::CheckedProgramEntryTerminalReceipt,
    physical_adapter_requested: bool,
    provider_proposals: &[backend_plan::TerminalTargetProviderProposal],
    request: &TerminalNativeProposalRequest<'_>,
    accepted_package_permissions: &TerminalAuthorityPermissionPolicy,
    installation_assumptions: &[installation_evidence::InstallationEnvironmentAssumption],
) -> Result<Vec<u8>, RecordError> {
    let TerminalNativeProposalRequest {
        evaluated_binding_identities,
        direct_boundary_leaves,
        provider_declarations,
        target_profile,
        family_realizations,
        boundaries,
        native_target,
        program_entry,
        application,
        optimizations,
        external_bindings,
        behavior_exclusions,
        package_terminal_authority_permissions,
        package_terminal_authority_permission_identities,
        x86_scalar_fma_provider,
    } = request;
    let mut output = RecordWriter::new(RecordLimits::default());
    output.extend(RECORD_SCHEMA)?;
    evaluated_binding_identities.write(&mut output)?;
    checked_entry.write(&mut output)?;
    physical_adapter_requested.write(&mut output)?;
    write_values(provider_proposals, &mut output)?;
    output.sequence(direct_boundary_leaves.len())?;
    for leaf in direct_boundary_leaves {
        leaf.write(&mut output)?;
    }
    write_values(provider_declarations, &mut output)?;
    target_profile.write(&mut output)?;
    write_values(family_realizations, &mut output)?;
    boundaries
        .selected_provider_plans
        .clone()
        .without_checked_specializations()
        .write(&mut output)?;
    output.sequence(boundaries.selected_provider_provenance.len())?;
    for crate::providers::provider_admission::CompilerIntrinsicPlanProvenance {
        plan,
        semantic_schema,
        row_executions,
        row_numeric_realizations,
    } in &boundaries.selected_provider_provenance
    {
        plan.write(&mut output)?;
        semantic_schema.write(&mut output)?;
        output.sequence(row_executions.len())?;
        for execution in *row_executions {
            // Transport cannot erase a supplied meaning before native
            // preparation reconciles it with the canonical declaration.
            execution.write(&mut output)?;
        }
        write_values(row_numeric_realizations, &mut output)?;
    }
    boundaries.calling_applications.write(&mut output)?;
    native_target.write(&mut output)?;
    match program_entry {
        None => 0u8.write(&mut output)?,
        Some(entry) => {
            1u8.write(&mut output)?;
            EntryRecord::write_borrowed(*entry, &mut output)?;
        }
    }
    application.write(&mut output)?;
    optimizations.write(&mut output)?;
    write_values(external_bindings, &mut output)?;
    write_values(behavior_exclusions, &mut output)?;
    package_terminal_authority_permissions.write(&mut output)?;
    package_terminal_authority_permission_identities.write(&mut output)?;
    x86_scalar_fma_provider.write(&mut output)?;
    write_values(accepted_package_permissions.rows(), &mut output)?;
    accepted_package_permissions
        .identities()
        .write(&mut output)?;
    write_values(installation_assumptions, &mut output)?;
    Ok(output.finish())
}

struct IntrinsicPlanRecord {
    plan: effects::provider_plan::ProviderPlan,
    semantic_schema: effects::provider_plan::ServiceSchema,
    row_executions: Vec<Option<effects::CompilerIntrinsicExecutionIdentity>>,
    row_numeric_realizations: Vec<Option<String>>,
}
records::record_fields!(IntrinsicPlanRecord {
    plan,
    semantic_schema,
    row_executions,
    row_numeric_realizations
});

struct DerivationInputs {
    evaluated_binding_identities: Arc<terminal_psi::TerminalIdentityTable>,
    checked_entry: terminal_psi::CheckedProgramEntryTerminalReceipt,
    physical_adapter_requested: bool,
    provider_proposals: Vec<backend_plan::TerminalTargetProviderProposal>,
    leaves: Vec<backend_plan::DirectBoundaryLeaf>,
    provider_declarations: Vec<backend_plan::TerminalTargetProviderPlan>,
    profile: Option<target::TargetProfile>,
    families: Vec<backend_plan::SelectedFamilyRealizationSource>,
    selected: effects::SelectedProviderPlanFacts,
    intrinsic_provenance: Vec<IntrinsicPlanRecord>,
    calling: Vec<calling_conventions::BoundaryCallingApplication>,
    native: Option<target::NativeTarget>,
    entry: Option<EntryRecord>,
    application: target::NativeApplicationMetadata,
    optimizations: optimization_core::PostTerminalOptimizationSelectionProjection,
    external: Vec<calling_conventions::ExternalBindingRow>,
    exclusions: Vec<effects::AuthoredBehaviorExclusion>,
    permissions: Vec<effects::ServiceTerminalAuthorityPermission>,
    permission_identities: terminal_psi::TerminalIdentityTable,
    fma: Option<target::AdmittedX86ScalarFmaProvider>,
    accepted_permissions: Vec<effects::ServiceTerminalAuthorityPermission>,
    accepted_identities: terminal_psi::TerminalIdentityTable,
    installation_assumptions: Vec<installation_evidence::InstallationEnvironmentAssumption>,
}
impl DerivationInputs {
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        let schema = input.take(RECORD_SCHEMA.len())?;
        if schema != RECORD_SCHEMA {
            let superseded =
                schema.starts_with(RECORD_SCHEMA_FAMILY) && schema < RECORD_SCHEMA.as_slice();
            return Err(input.error(if superseded {
                SUPERSEDED_RECORD_SCHEMA
            } else {
                "foreign native derivation record schema"
            }));
        }
        let evaluated_binding_identities =
            Arc::new(terminal_psi::TerminalIdentityTable::read(input)?);
        input.context.binding_identities = Some(Arc::clone(&evaluated_binding_identities));
        Ok(Self {
            evaluated_binding_identities,
            checked_entry: RecordValue::read(input)?,
            physical_adapter_requested: RecordValue::read(input)?,
            provider_proposals: RecordValue::read(input)?,
            leaves: RecordValue::read(input)?,
            provider_declarations: RecordValue::read(input)?,
            profile: RecordValue::read(input)?,
            families: RecordValue::read(input)?,
            selected: RecordValue::read(input)?,
            intrinsic_provenance: RecordValue::read(input)?,
            calling: RecordValue::read(input)?,
            native: RecordValue::read(input)?,
            entry: match u8::read(input)? {
                0 => None,
                1 => Some(EntryRecord::read(input)?),
                _ => return Err(input.error("invalid native entry presence tag")),
            },
            application: RecordValue::read(input)?,
            optimizations: RecordValue::read(input)?,
            external: RecordValue::read(input)?,
            exclusions: RecordValue::read(input)?,
            permissions: RecordValue::read(input)?,
            permission_identities: RecordValue::read(input)?,
            fma: RecordValue::read(input)?,
            accepted_permissions: RecordValue::read(input)?,
            accepted_identities: RecordValue::read(input)?,
            installation_assumptions: RecordValue::read(input)?,
        })
    }
}

pub fn recover_native_derivation_inputs(
    artifact: Arc<terminal_codec::CanonicalTerminalArtifact>,
    bytes: &[u8],
) -> Result<
    (
        RetainedTerminalArtifact,
        TerminalAuthorityPermissionPolicy,
        Vec<installation_evidence::InstallationEnvironmentAssumption>,
    ),
    Vec<Diagnostic>,
> {
    let error = |reason: String| vec![Diagnostic::error(reason)];
    let mut reader = RecordReader::new(bytes, RecordLimits::default())
        .map_err(|reason| error(reason.to_string()))?;
    let inputs = DerivationInputs::read(&mut reader).map_err(|reason| error(reason.to_string()))?;
    reader
        .finish()
        .map_err(|reason| error(reason.to_string()))?;
    let DerivationInputs {
        evaluated_binding_identities,
        checked_entry,
        physical_adapter_requested,
        provider_proposals,
        leaves,
        provider_declarations,
        profile,
        families,
        selected,
        intrinsic_provenance,
        calling,
        native,
        entry,
        application,
        optimizations,
        external,
        exclusions,
        permissions,
        permission_identities,
        fma,
        accepted_permissions,
        accepted_identities,
        installation_assumptions,
    } = inputs;
    crate::providers::provider_admission::terminal_authority_permissions::validate_package_terminal_authority_permission_custody(
        accepted_permissions.iter(), &accepted_identities)?;
    let accepted =
        crate::providers::provider_admission::terminal_authority_permission_policy_with_rows(
            accepted_permissions,
            accepted_identities,
        )
        .map_err(|reason| {
            error(format!(
                "recovered accepted package permission candidates: {reason:?}"
            ))
        })?;
    let entry_adapter_requested = entry
        .as_ref()
        .is_some_and(|entry| entry.physical_adapter_requested);
    let request = TerminalNativeProposalRequest {
        evaluated_binding_identities: &evaluated_binding_identities,
        direct_boundary_leaves: leaves.iter().collect(),
        provider_declarations: &provider_declarations,
        target_profile: profile,
        family_realizations: &families,
        boundaries: crate::BoundaryApplicationSettlement {
            selected_provider_plans: &selected,
            selected_provider_provenance: intrinsic_provenance
                .iter()
                .map(
                    |row| crate::providers::provider_admission::CompilerIntrinsicPlanProvenance {
                        plan: &row.plan,
                        semantic_schema: &row.semantic_schema,
                        row_executions: &row.row_executions,
                        row_numeric_realizations: &row.row_numeric_realizations,
                    },
                )
                .collect(),
            calling_applications: calling,
        },
        native_target: native,
        program_entry: entry.as_ref().map(EntryRecord::borrowed),
        application,
        optimizations,
        external_bindings: &external,
        behavior_exclusions: &exclusions,
        package_terminal_authority_permissions: permissions,
        package_terminal_authority_permission_identities: permission_identities,
        x86_scalar_fma_provider: fma,
    };
    let mut proposal =
        project_terminal_native_realization_proposal(&artifact, checked_entry, request)?;
    if physical_adapter_requested || entry_adapter_requested {
        proposal
            .bind_physical_adapter(&artifact)
            .map_err(|reason| error(reason.to_owned()))?;
    }
    Ok((
        RetainedTerminalArtifact::new_with_native_realization_proposal(
            artifact,
            proposal.with_target_provider_proposals(provider_proposals),
        ),
        accepted,
        installation_assumptions,
    ))
}
