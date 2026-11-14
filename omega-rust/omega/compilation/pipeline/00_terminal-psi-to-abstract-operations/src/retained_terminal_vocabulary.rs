/*
  Compiler-private wire vocabulary (`record_variants!`) for retained native
  derivation inputs across calling plans, provider selections, receipts,
  recovered proposals, semantic identifiers, and targets.

  Decoding produces inert candidate records; calling-plan, entry, provider, and
  artifact admission remain with their receiving stage owners.
*/
use super::records::record_fields;
use super::value;

macro_rules! record_variants {
    ($value:path { $( $tag:literal => $variant:ident
        $( ( $($element:ident),* $(,)? ) )?
        $( { $($field:ident),* $(,)? } )?
    ),* $(,)? }) => {
        impl super::value::RecordValue for $value {
            fn write(&self, output: &mut super::value::RecordWriter)
                -> Result<(), super::value::RecordError>
            {
                match self {
                    $(Self::$variant $( ( $($element),* ) )? $( { $($field),* } )? => {
                        super::value::RecordValue::write(&($tag as u16), output)?;
                        $( $(super::value::RecordValue::write($element, output)?;)* )?
                        $( $(super::value::RecordValue::write($field, output)?;)* )?
                        Ok(())
                    }),*
                }
            }
            fn read(input: &mut super::value::RecordReader<'_>)
                -> Result<Self, super::value::RecordError>
            {
                input.record(|input| match <u16 as super::value::RecordValue>::read(input)? {
                    $($tag => Ok(Self::$variant
                        $( ( $({ let $element = super::value::RecordValue::read(input)?; $element }),* ) )?
                        $( { $($field: super::value::RecordValue::read(input)?),* } )?
                    ),)*
                    _ => Err(input.error(concat!("invalid ", stringify!($value), " variant"))),
                })
            }
        }
    };
}

mod calling {
    use super::record_fields;

    record_fields!(calling_conventions::BoundaryEntryPlan { call, state });

    record_fields!(calling_conventions::BoundaryNativeParameterGeometry {
        identity,
        native_ordinal,
        shape,
        origin
    });

    record_variants!(calling_conventions::BoundaryNativeParameterOrigin {
        0 => SemanticFormal { formal_ordinal },
        1 => PrivateCallback { binder, requirement },
    });

    record_variants!(calling_conventions::BoundaryNativeParameterShape {
        0 => Semantic(value0),
        1 => TargetFunctionPointer { byte_size, alignment },
    });

    record_fields!(calling_conventions::BoundaryOpaqueApplication {
        shape_root,
        application_report_fingerprint,
        selected_application_commitment,
        conformance_application_commitment,
        representation_schema_version,
        checked_semantic_copy
    });

    record_fields!(calling_conventions::BoundarySignatureGeometry {
        shapes,
        fields,
        parameters,
        result,
        native_parameters,
        native_parameter_rows,
        callback_layouts,
        callback_layout_rows,
        callback_bindings,
        callback_binding_rows
    });

    record_fields!(calling_conventions::BoundaryCallbackBinding {
        binder,
        requirement,
        static_machine_ordinal,
        requirement_identity,
        invokes_synchronously
    });

    record_variants!(calling_conventions::BoundaryValueClass {
        0 => Integer,
        1 => Float,
        2 => Reference,
        3 => FixedArray { element, length },
        4 => Record { first_field, field_count },
    });

    record_fields!(calling_conventions::BoundaryValueField { shape, byte_offset });

    record_fields!(calling_conventions::BoundaryValueShape {
        class,
        byte_size,
        alignment
    });

    record_fields!(calling_conventions::CallPlan {
        policy,
        parameters,
        result,
        callback_materializations,
        ordinary_clobbers,
        stack_alignment,
        shadow_bytes,
        entry_control
    });

    record_fields!(calling_conventions::CallSignature { parameters, result });

    record_fields!(calling_conventions::CallbackBinderRequirement {
        binder,
        requirement
    });

    record_fields!(calling_conventions::CallbackDataGeometry {
        identity,
        policy,
        layout,
        size,
        alignment
    });

    record_fields!(calling_conventions::CallbackInlineFieldGeometry {
        identity,
        offset,
        child
    });

    record_fields!(calling_conventions::CallbackMaterialization {
        binder,
        destination
    });

    record_fields!(calling_conventions::CallbackMaterializationContext { binders, demands });

    record_fields!(calling_conventions::CallbackSlotGeometry {
        identity,
        policy,
        requirement_identity,
        layout,
        slot,
        requirement,
        conformance_commitment,
        placements,
        size,
        alignment
    });

    record_variants!(calling_conventions::CallingPolicy {
        0 => MicrosoftX64,
        1 => SystemVAMD64,
        2 => Aapcs64,
        3 => LinuxSyscallX86_64,
        4 => LinuxSyscallAarch64,
    });

    record_variants!(calling_conventions::EntryControl {
        0 => CallReturn,
        1 => SupervisorCall { number_register, immediate },
        2 => InterruptReturn,
    });

    record_variants!(calling_conventions::EntryStack {
        0 => Interrupted,
        1 => Dedicated { class },
        2 => ProviderSelected,
    });

    record_variants!(calling_conventions::ExternalBindingKind {
        0 => Syscall { number },
        1 => Import { locator },
        2 => CompilerIntrinsic { machine },
        3 => VtableSlot { index },
        4 => VtableField { field, byte_offset },
        5 => TableFunction { field, byte_offset },
    });

    record_fields!(calling_conventions::ExternalBindingRow {
        target_name,
        trait_name,
        method,
        requirement_identity,
        provider_type,
        calling_applications,
        binding
    });

    record_variants!(calling_conventions::IndirectPointerLocation {
        0 => Register(value0),
        1 => Stack { stack_byte_offset, alignment },
    });

    record_variants!(calling_conventions::MachineRegime {
        0 => X86Long64,
        1 => Aarch64A64 { exception_level },
    });

    record_variants!(calling_conventions::MachineRegister {
        0 => X86Rax,
        1 => X86Rcx,
        2 => X86Rdx,
        3 => X86Rbx,
        4 => X86Rsp,
        5 => X86Rbp,
        6 => X86Rsi,
        7 => X86Rdi,
        8 => X86R8,
        9 => X86R9,
        10 => X86R10,
        11 => X86R11,
        12 => X86R12,
        13 => X86R13,
        14 => X86R14,
        15 => X86R15,
        16 => X86Xmm(value0),
        17 => Aarch64X(value0),
        18 => Aarch64V(value0),
    });

    record_variants!(calling_conventions::MachineState {
        0 => GeneralRegisters,
        1 => VectorRegisters,
        2 => Flags,
        3 => InstructionPointer,
        4 => StackPointer,
        5 => SegmentState,
        6 => ControlState,
        7 => DebugState,
        8 => ExtendedState,
    });

    record_fields!(calling_conventions::NativeCallbackDemand {
        destination,
        requirement
    });

    record_variants!(calling_conventions::NativePlace {
        0 => Parameter(value0),
        1 => Field { parameter, layout, field_path },
    });

    record_variants!(calling_conventions::Preemption {
        0 => NotApplicable,
        1 => Masked,
        2 => Nestable { maximum_depth },
        3 => ProviderDefined,
    });

    record_fields!(calling_conventions::StatePlan {
        initial_regime,
        interrupted_state,
        saved_state,
        restored_state,
        permitted_transitive_use,
        stack,
        preemption
    });

    record_variants!(calling_conventions::SystemVEightbyteClass {
        0 => Integer,
        1 => Sse,
    });

    record_variants!(calling_conventions::ValueClass {
        0 => Integer,
        1 => Float,
        2 => BorrowedReference,
        3 => BorrowedScalarReference,
        4 => HomogeneousFloatAggregate { members },
        5 => SystemVAggregate { first, second },
    });

    record_variants!(calling_conventions::ValueLocation {
        0 => Register { register, value_byte_offset, byte_size },
        1 => Stack { stack_byte_offset, value_byte_offset, byte_size, alignment },
        2 => Indirect { pointer, copy_stack_byte_offset, byte_size, alignment },
    });

    record_fields!(calling_conventions::ValuePlacement { shape, locations });

    record_fields!(calling_conventions::ValueShape {
        class,
        byte_size,
        alignment
    });

    record_variants!(layout_plans::LayoutPlacementReport {
        0 => At { offset },
        1 => IntegerAt { offset, stored_width, interpretation },
        2 => Bits { container, container_width, destination_lsb, source_lsb, width },
    });

    record_variants!(layout_plans::IntegerInterpretation {
        0 => Signed,
        1 => Unsigned,
    });
}
mod providers {
    use super::super::value::{RecordError, RecordWriter};
    use super::record_fields;

    record_fields!(effects::AssumedBindingContractSelection { required, supplied });

    record_fields!(effects::AuthoredBehaviorExclusion {
        kind,
        selecting_machine,
        source_span
    });

    record_variants!(effects::AuthoredBehaviorExclusionKind {
        0 => CrashCause { cause, case_symbol },
        1 => PhysicalAuthorityClass { class, case_symbol },
        2 => Service { identity, name },
    });

    record_variants!(effects::CompilerIntrinsicExecutionIdentity {
        0 => HostedExitProcessI32,
        1 => HostedWriteByteI32,
        2 => HostedReadByte,
        3 => ProgramImageRetainedSuffixStart,
        4 => ProgramStorageEntry,
        5 => AccessLifecycle(value0),
        6 => AccessorScalarAccess { contract, placement_commitment, access_slot, operation },
        7 => BuiltinFunction(value0),
        8 => PrimitiveFloatBinary { operation, format },
        9 => PrimitiveIntegerComparison { operation, integer_type },
        10 => NamedFloatNegation(value0),
        11 => NamedFloatConversion { source, target, domain },
        12 => TaskRegisterActivate,
        13 => TaskRegisterRead,
        14 => SegmentReload(value0),
        15 => SegmentRead(value0),
        16 => EmptyRunRelease,
        17 => StaticInterrupt(value0),
    });

    #[cfg(test)]
    mod execution_tag_tests {
        use crate::retained_terminal::codec::value::{
            RecordLimits, RecordReader, RecordValue, RecordWriter,
        };
        use effects::CompilerIntrinsicExecutionIdentity as Execution;
        use language_core::static_interrupts::StaticInterruptOperation as Interrupt;
        use std::collections::BTreeSet;

        #[test]
        fn integer_width_records_retain_exact_carriers_and_reject_invalid_metadata() {
            use effects::CompilerNumericType as Numeric;
            use numerics::integer_carrier::FixedInteger;
            for width in [1, 13, 257, u64::MAX] {
                for carrier in [FixedInteger::signed(width), FixedInteger::unsigned(width)] {
                    let value = Numeric::Integer(carrier);
                    let mut writer = RecordWriter::new(RecordLimits::default());
                    value.write(&mut writer).unwrap();
                    let bytes = writer.finish();
                    assert_eq!(bytes, value.canonical_encoding());
                    let mut reader = RecordReader::new(&bytes, RecordLimits::default()).unwrap();
                    assert_eq!(Numeric::read(&mut reader).unwrap(), value);
                    reader.finish().unwrap();
                    for length in 0..bytes.len() {
                        let mut reader =
                            RecordReader::new(&bytes[..length], RecordLimits::default()).unwrap();
                        assert!(Numeric::read(&mut reader).is_err());
                    }
                }
            }
            for invalid in [[0; Numeric::ENCODED_SIZE], [255; Numeric::ENCODED_SIZE]] {
                let mut reader = RecordReader::new(&invalid, RecordLimits::default()).unwrap();
                assert!(Numeric::read(&mut reader).is_err());
            }
        }

        #[test]
        fn release_and_interrupt_execution_records_do_not_alias() {
            let cases = [
                (Execution::EmptyRunRelease, 16_u16),
                (Execution::StaticInterrupt(Interrupt::Install), 17),
                (Execution::StaticInterrupt(Interrupt::Context), 17),
                (Execution::StaticInterrupt(Interrupt::BeginCompletion), 17),
                (Execution::StaticInterrupt(Interrupt::FinishCompletion), 17),
            ];
            let mut distinct = BTreeSet::new();
            for (value, tag) in cases {
                let mut writer = RecordWriter::new(RecordLimits::default());
                value.write(&mut writer).unwrap();
                let bytes = writer.finish();
                assert_eq!(&bytes[..2], &tag.to_le_bytes());
                let mut reader = RecordReader::new(&bytes, RecordLimits::default()).unwrap();
                assert_eq!(Execution::read(&mut reader).unwrap(), value);
                reader.finish().unwrap();
                for length in 0..bytes.len() {
                    let mut reader =
                        RecordReader::new(&bytes[..length], RecordLimits::default()).unwrap();
                    assert!(Execution::read(&mut reader).is_err());
                }
                assert!(distinct.insert(bytes));
            }
            for bytes in [[255, 255, 0, 0], [17, 0, 255, 255]] {
                let mut reader = RecordReader::new(&bytes, RecordLimits::default()).unwrap();
                assert!(Execution::read(&mut reader).is_err());
            }
        }
    }

    impl super::value::RecordValue for effects::CompilerNumericType {
        fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
            super::value::RecordValue::write(&self.canonical_encoding(), output)
        }

        fn read(input: &mut super::value::RecordReader<'_>) -> Result<Self, RecordError> {
            let bytes = <[u8; Self::ENCODED_SIZE] as super::value::RecordValue>::read(input)?;
            Self::from_canonical_encoding(bytes)
                .ok_or_else(|| input.error("invalid numeric intrinsic carrier"))
        }
    }

    record_variants!(effects::CompilerAccessorScalarOperation {
        0 => Read,
        1 => Take,
        2 => Write,
    });

    record_variants!(effects::CompilerPrimitiveFloatBinaryOperation {
        0 => Add,
        1 => Subtract,
        2 => Multiply,
        3 => Divide,
        4 => Equal,
        5 => NotEqual,
        6 => Less,
        7 => LessOrEqual,
        8 => Greater,
        9 => GreaterOrEqual,
    });

    record_variants!(effects::CompilerPrimitiveIntegerComparisonOperation {
        0 => Equal,
        1 => NotEqual,
        2 => Less,
        3 => LessOrEqual,
        4 => Greater,
        5 => GreaterOrEqual,
    });

    record_variants!(backend_plan::DirectBoundaryExecution {
        0 => Foreign(value0),
        1 => Intrinsic(value0),
    });

    record_fields!(effects::InstallationReachResolution {
        requirement_identity,
        provider_plan_report_identity,
        upper_bound,
        resolved_row
    });

    record_fields!(effects::provider_plan::TableFieldStorage {
        table_application,
        field_declaration
    });

    record_variants!(effects::provider_plan::ProviderBinding {
        0 => Import { evaluated },
        1 => Syscall { number },
        2 => CompilerIntrinsic { machine },
        3 => VtableSlot { index },
        4 => VtableField { table, field, storage },
        5 => TableFunction { table, field, storage },
        6 => CheckedAdapter { machine_identity, machine_package_identity },
    });

    record_fields!(terminal_installation::ProviderInstallationSelection {
        boundary,
        provider_identity,
        provider_declaration,
        candidate
    });

    record_fields!(effects::provider_plan::ProviderPlan {
        provider_application,
        name,
        provider_type,
        provider_type_package_identity,
        target,
        schema,
        identities,
        rows,
        origin_package_identity,
        origin_package,
        digest_memo
    });

    record_fields!(effects::provider_plan::ProviderPlanRow {
        method,
        requirement_identity,
        declared_requirement_identity,
        calling_signature,
        requirement_lifetime_partition,
        admitted_same_stack_bytes,
        binding
    });

    record_fields!(effects::SelectedExternalRequirementApplication {
        provider_plan_report_identity,
        provider_plan_digest,
        selected_requirement_identity,
        requirement_identity
    });

    record_fields!(backend_plan::SelectedFamilyRealizationSource {
        head_declaration_identity,
        head_callable_identity,
        target,
        selected_requirement_identity,
        provider_plan_report_identity,
        provider_plan_digest
    });

    record_fields!(effects::SelectedRequirementSpecialization {
        provider_plan_report_identity,
        selected_requirement_identity,
        selected_machine_identity,
        requirement_identity,
        machine_identity
    });

    record_fields!(effects::SelectedTargetFamilyRealization {
        terminal,
        target,
        head,
        member,
        head_identity,
        member_identity,
        selected_requirement_identity,
        provider_plan_report_identity,
        provider_plan_digest
    });

    record_variants!(effects::provider_plan::ServiceEntryAuthorityFlow {
        0 => Accepts,
    });

    record_fields!(effects::provider_plan::ServiceEntryClaim {
        parameter_index,
        carrier_identity,
        domain,
        predicate_presence,
        effective_carry,
        authority_flow
    });

    record_fields!(effects::provider_plan::ServiceMethod {
        completion,
        name,
        requirement_owner,
        requirement_owner_package_identity,
        requirement_identity,
        parameter_count,
        parameter_type_identities,
        entry_claims,
        has_result,
        result_type_identity,
        result_claims,
        service_reach,
        synchronous_invocations,
        may_suspend,
        may_block,
        terminates_guarantee,
        termination_premises
    });

    record_fields!(effects::provider_plan::ServiceProgressEstablishmentRoute {
        kind,
        requirement_identity
    });

    record_variants!(effects::provider_plan::ServiceProgressEstablishmentRouteKind {
        0 => CheckedRequirement,
        1 => BoundaryRequirement,
    });

    record_fields!(effects::provider_plan::ServiceProgressPremise {
        profile,
        subject,
        subject_projections,
        establishment_routes
    });

    record_variants!(effects::provider_plan::ServiceProgressSubject {
        0 => Parameter(value0),
    });

    record_fields!(effects::provider_plan::ServiceReceiverApplication {
        application,
        trait_name,
        trait_package_identity,
        argument_identities,
        lifetime_argument_identities,
        requirement_identities
    });

    record_fields!(effects::provider_plan::ServiceResultClaim {
        domain,
        effective_carry
    });

    impl super::super::value::RecordValue for effects::provider_plan::ServiceSchema {
        fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
            self.validate_calling_applications()
                .map_err(|reason| output.error(reason))?;
            let effects::provider_plan::ServiceSchema {
                declaration,
                calling_applications,
            } = self;
            let effects::provider_plan::ServiceSurface {
                trait_name,
                identities,
                trait_package_identity,
                receiver_applications,
                methods,
            } = declaration.as_ref();
            trait_name.write(output)?;
            identities.write(output)?;
            trait_package_identity.write(output)?;
            receiver_applications.write(output)?;
            methods.write(output)?;
            calling_applications.write(output)
        }

        fn read(input: &mut super::super::value::RecordReader<'_>) -> Result<Self, RecordError> {
            use super::super::value::RecordValue;
            use effects::provider_plan::{
                ServiceCallingApplication, ServiceMethod, ServiceSurface,
            };
            input.record(|input| {
                input.allocate(
                    std::mem::size_of::<ServiceSurface>()
                        .saturating_add(2 * std::mem::size_of::<usize>()),
                )?;
                let trait_name = RecordValue::read(input)?;
                let identities = RecordValue::read(input)?;
                let trait_package_identity = RecordValue::read(input)?;
                let receiver_applications = RecordValue::read(input)?;
                let methods = Vec::<ServiceMethod>::read(input)?;
                let calling_applications = Vec::<ServiceCallingApplication>::read(input)?;
                let schema = Self {
                    declaration: std::sync::Arc::new(ServiceSurface {
                        trait_name,
                        identities,
                        trait_package_identity,
                        receiver_applications,
                        methods,
                    }),
                    calling_applications,
                };
                schema
                    .validate_calling_applications()
                    .map_err(|reason| input.error(reason))?;
                Ok(schema)
            })
        }
    }

    record_fields!(effects::provider_plan::ServiceCallingApplication {
        requirement,
        abi,
        plan
    });
    record_fields!(effects::provider_plan::ServiceCallingPlan {
        report_fingerprint,
        commitment,
        promise
    });

    record_variants!(effects::TerminalAuthorityClass {
        0 => FilesystemContentRead,
        1 => FilesystemContentWrite,
        2 => FilesystemMetadataQuery,
        3 => DirectoryEnumeration,
        4 => FilesystemNamespaceMutation,
        5 => FilesystemMetadataMutation,
        6 => ProcessOutput,
        7 => ProcessTermination,
        8 => MachineControl,
        9 => PortIo,
        10 => InterruptControl,
        11 => InterruptEntry,
        12 => RootMemoryAccess,
        13 => ProcessInput,
    });

    record_fields!(backend_plan::TerminalTargetProviderPlan {
        plan,
        provider_declaration,
        row_numeric_realizations
    });

    record_fields!(backend_plan::TerminalTargetProviderProposal {
        target,
        terminal_psi,
        optimization_selection,
        application_roots,
        application_required,
        selection
    });

    record_fields!(backend_plan::TerminalTargetProviderSelection {
        plans,
        assumed_bindings,
        checked_candidates
    });

    record_variants!(terminal_psi::SegmentReload { 0 => Code, 1 => Data, });
    record_variants!(terminal_psi::SegmentRegister { 0 => Code, 1 => Data, 2 => Extra, 3 => Stack, 4 => Fs, 5 => Gs, });
}
mod receipts {
    use super::super::value::{RecordError, RecordReader, RecordValue, RecordWriter};
    use super::record_fields;

    struct ReceiptRecord {
        build_origin: Option<effects::provider_plan::BuildNativeBindingOrigin>,
        producer_package: Option<semantic_vocabulary::PackageKeyIdentity>,
        producer_callable_identity: terminal_psi::TerminalIdentityIndex,
        producer_closure_digest: effects::provider_plan::EvaluatedBindingProducerClosureDigest,
        evaluator_semantics_marker: u32,
        evaluation_usage: effects::provider_plan::EvaluatedBindingUsage,
        evaluation_digest: effects::provider_plan::EvaluatedBindingEvaluationDigest,
        materializer_schema_version: u32,
        materialization_digest: effects::provider_plan::EvaluatedBindingMaterializationDigest,
        locator_identity_digest: [u8; 32],
        identity_digest: [u8; 32],
    }

    record_fields!(ReceiptRecord {
        build_origin,
        producer_package,
        producer_callable_identity,
        producer_closure_digest,
        evaluator_semantics_marker,
        evaluation_usage,
        evaluation_digest,
        materializer_schema_version,
        materialization_digest,
        locator_identity_digest,
        identity_digest,
    });

    impl RecordValue for effects::provider_plan::EvaluatedBindingReceipt {
        fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
            ReceiptRecord {
                build_origin: self.build_origin().cloned(),
                producer_package: self.producer_package(),
                producer_callable_identity: self.producer_callable_identity(),
                producer_closure_digest: self.producer_closure_digest(),
                evaluator_semantics_marker: self.evaluator_semantics_marker(),
                evaluation_usage: self.evaluation_usage(),
                evaluation_digest: self.evaluation_digest(),
                materializer_schema_version: self.materializer_schema_version(),
                materialization_digest: self.materialization_digest(),
                locator_identity_digest: self.locator_identity_digest().as_bytes(),
                identity_digest: self.identity_digest(),
            }
            .write(output)
        }

        fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
            input.record(|input| {
                let ReceiptRecord {
                    build_origin,
                    producer_package,
                    producer_callable_identity,
                    producer_closure_digest,
                    evaluator_semantics_marker,
                    evaluation_usage,
                    evaluation_digest,
                    materializer_schema_version,
                    materialization_digest,
                    locator_identity_digest,
                    identity_digest,
                } = ReceiptRecord::read(input)?;
                let identities = input.context.binding_identities.clone().ok_or_else(|| {
                    input.error(
                        "evaluated binding receipt is missing its producer identity namespace",
                    )
                })?;
                let producer = identities
                    .text(producer_callable_identity)
                    .filter(|identity| !identity.is_empty())
                    .ok_or_else(|| {
                        input.error("binding receipt has no exact producer in its identity table")
                    })?;

                let text_bytes = identities
                    .entries()
                    .iter()
                    .fold(0usize, |total, text| total.saturating_add(text.len()));
                input.allocate(text_bytes.saturating_mul(2).saturating_add(
                    identities.entries().len().saturating_mul(
                        2 * std::mem::size_of::<String>() + 8 * std::mem::size_of::<usize>(),
                    ),
                ))?;
                let mut builder = identities.as_ref().clone().into_builder();
                let recovered = Self::from_evaluation(
                    producer_package,
                    producer,
                    &mut builder,
                    producer_closure_digest,
                    evaluator_semantics_marker,
                    evaluation_usage,
                    evaluation_digest,
                    materializer_schema_version,
                    materialization_digest,
                    target::ForeignLocatorIdentityDigest::from_digest(locator_identity_digest),
                )
                .map_err(|reason| input.error(reason))?;
                let recovered = match build_origin {
                    Some(origin) => recovered
                        .with_build_origin(origin, builder.as_table())
                        .map_err(|reason| input.error(reason))?,
                    None => recovered,
                };
                if recovered.producer_callable_identity() != producer_callable_identity {
                    return Err(input.error("binding receipt recovery changed its producer index"));
                }
                if builder.as_table() != identities.as_ref() {
                    return Err(
                        input.error("binding receipt recovery changed its producer identity table")
                    );
                }
                if recovered.identity_digest() != identity_digest {
                    return Err(input.error(
                        "binding receipt commitment differs from its retained evaluation fields",
                    ));
                }
                Ok(recovered)
            })
        }
    }

    impl RecordValue for effects::provider_plan::BuildNativeBindingOrigin {
        fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
            self.requirement_identity.write(output)?;
            self.source_occurrence_digest.write(output)?;
            self.terminal_code.vocabulary_marker.get().write(output)?;
            self.terminal_code
                .program_fingerprint
                .as_bytes()
                .write(output)?;
            self.work_schedule.write(output)?;
            self.storage_schedule.write(output)?;
            self.observation_digest.write(output)
        }
        fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
            input.record(|input| {
                let requirement_identity = RecordValue::read(input)?;
                let source_occurrence_digest = RecordValue::read(input)?;
                let vocabulary = u16::read(input)?;
                let vocabulary_marker = terminal_psi::VocabularyMarker::new(vocabulary)
                    .ok_or_else(|| {
                        input.error("native Build receipt has a different Terminal vocabulary")
                    })?;
                let program_fingerprint =
                    terminal_psi::SemanticFingerprint::from_bytes(RecordValue::read(input)?);
                Ok(Self {
                    requirement_identity,
                    source_occurrence_digest,
                    terminal_code: terminal_psi::TerminalPsiIdentity {
                        vocabulary_marker,
                        program_fingerprint,
                    },
                    work_schedule: RecordValue::read(input)?,
                    storage_schedule: RecordValue::read(input)?,
                    observation_digest: RecordValue::read(input)?,
                })
            })
        }
    }

    #[cfg(test)]
    mod tests {
        use super::super::super::value::{RecordLimits, RecordReader, RecordValue, RecordWriter};
        use effects::provider_plan::{
            BuildNativeBindingOrigin, EvaluatedBindingEvaluationDigest,
            EvaluatedBindingMaterializationDigest, EvaluatedBindingProducerClosureDigest,
            EvaluatedBindingReceipt, EvaluatedBindingUsage,
        };
        use std::sync::Arc;

        const REQUIREMENT: &str = "package::Console::write";
        const PRODUCER: &str = "package::build::console_binding";

        fn build_receipt(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        ) -> EvaluatedBindingReceipt {
            let requirement_identity = identities.intern(REQUIREMENT);
            EvaluatedBindingReceipt::from_evaluation(
                semantic_vocabulary::PackageKeyIdentity::from_digest([7; 32]),
                PRODUCER,
                identities,
                EvaluatedBindingProducerClosureDigest::from_bytes([1; 32]).unwrap(),
                3,
                EvaluatedBindingUsage::from_evaluator(1, 1, 10, 100, 0, 0, 4, 8, 1, 2).unwrap(),
                EvaluatedBindingEvaluationDigest::from_bytes([2; 32]).unwrap(),
                2,
                EvaluatedBindingMaterializationDigest::from_bytes([3; 32]).unwrap(),
                target::ForeignLocatorIdentityDigest::from_digest([4; 32]),
            )
            .unwrap()
            .with_build_origin(
                BuildNativeBindingOrigin {
                    requirement_identity,
                    source_occurrence_digest: [5; 32],
                    terminal_code: terminal_psi::TerminalPsiIdentity {
                        vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
                        program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([6; 32]),
                    },
                    work_schedule: 1,
                    storage_schedule: 1,
                    observation_digest: [8; 32],
                },
                identities.as_table(),
            )
            .unwrap()
        }

        fn round_trip(
            receipt: &EvaluatedBindingReceipt,
            identities: terminal_psi::TerminalIdentityTable,
        ) -> Result<EvaluatedBindingReceipt, String> {
            let mut output = RecordWriter::new(RecordLimits::default());
            receipt.write(&mut output).unwrap();
            let bytes = output.finish();
            let mut input = RecordReader::new(&bytes, RecordLimits::default()).unwrap();
            input.context.binding_identities = Some(Arc::new(identities));
            let recovered = EvaluatedBindingReceipt::read(&mut input).map_err(|e| e.to_string())?;
            input.finish().unwrap();
            Ok(recovered)
        }

        #[test]
        fn build_origin_receipt_round_trips_through_its_identity_table() {
            let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
            let receipt = build_receipt(&mut identities);
            let table = identities.build();
            let recovered = round_trip(&receipt, table.clone()).unwrap();
            assert_eq!(recovered, receipt);
            let origin = recovered.build_origin().unwrap();
            assert_eq!(table.text(origin.requirement_identity), Some(REQUIREMENT));
        }

        #[test]
        fn build_origin_requirement_absent_from_its_table_is_refused() {
            let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
            identities.intern(PRODUCER);
            let receipt = build_receipt(&mut identities);
            let mut producer_only = terminal_psi::TerminalIdentityTableBuilder::default();
            producer_only.intern(PRODUCER);
            let error = round_trip(&receipt, producer_only.build()).unwrap_err();
            assert!(error.contains("no exact requirement identity"), "{error}");
        }

        #[test]
        fn build_origin_requirement_with_substituted_text_is_refused() {
            let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
            let receipt = build_receipt(&mut identities);
            let mut substituted = terminal_psi::TerminalIdentityTableBuilder::default();
            substituted.intern("package::Console::read");
            substituted.intern(PRODUCER);
            let error = round_trip(&receipt, substituted.build()).unwrap_err();
            assert!(error.contains("commitment"), "{error}");
        }

        #[test]
        fn reinterned_build_origin_keeps_its_commitment() {
            let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
            let receipt = build_receipt(&mut identities);
            let source = identities.build();
            let mut destination = terminal_psi::TerminalIdentityTableBuilder::default();
            destination.intern("padding shifts every ordinal");
            let moved = receipt
                .reintern_producer(&source, &mut destination)
                .unwrap();
            let destination = destination.build();
            assert_eq!(moved.identity_digest(), receipt.identity_digest());
            assert_eq!(
                destination.text(moved.build_origin().unwrap().requirement_identity),
                Some(REQUIREMENT)
            );
            assert_eq!(round_trip(&moved, destination).unwrap(), moved);
        }
    }
}
mod recovered {
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
        semantic_vocabulary::OperationId,
        semantic_vocabulary::BoundaryMachineId,
        semantic_vocabulary::CallingSignatureId,
    );

    impl RecordValue for terminal_psi::ProofDeclarationId {
        fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
            self.0.write(output)
        }
        fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
            Ok(Self(u32::read(input)?))
        }
    }

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
        effects::provider_plan::BoundaryCallingPlanPromise,
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
                input.allocate(fields.len().saturating_add(1).saturating_mul(
                    std::mem::size_of::<calling_conventions::CallbackInlineFieldGeometry>(),
                ))?;
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
            self.work_units().write(output)?;
            self.work_budget().write(output)?;
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
                let form =
                    diagnostics::CoverageForm::from_resource(&resource).ok_or_else(|| {
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

    impl RecordValue for calling_conventions::PrivateCallbackPlacements {
        fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
            output.sequence(self.as_slice().len())?;
            for placement in self.as_slice() {
                placement.write(output)?;
            }
            Ok(())
        }
        fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
            let placements = Vec::<layout_plans::LayoutPlacementReport>::read(input)?;
            input.allocate(
                placements
                    .len()
                    .saturating_mul(std::mem::size_of::<layout_plans::LayoutPlacementReport>()),
            )?;
            Ok(Self::new(&placements))
        }
    }
}
mod semantic {
    use super::record_fields;

    record_variants!(numerics::arithmetic::ArithmeticDomain {
        0 => Exact,
        1 => Wrapping,
        2 => Saturating,
        3 => Trapping,
    });

    record_variants!(semantic_vocabulary::BoundaryCompletion {
        0 => NormalReturn,
        1 => External(value0),
    });

    record_variants!(symbols::BuiltinFunction {
        0 => FloatMaximum,
        1 => FloatMinimum,
        2 => FloatSquareRoot,
        3 => AsmHlt,
        4 => AsmPortOut,
        5 => AsmPortIn,
        6 => AsmLoadFence,
        7 => AsmStoreFence,
        8 => AsmFullFence,
        9 => AsmAarch64LoadFence,
        10 => AsmAarch64StoreFence,
        11 => AsmAarch64FullFence,
        12 => AsmDisableInterrupts,
        13 => AsmEnableInterrupts,
        14 => AsmSnapshotFlags,
        15 => AsmRestoreFlags,
        16 => AsmReadMsr,
        17 => AsmWriteMsr,
        18 => AsmReadCr0,
        19 => AsmReadCr2,
        20 => AsmReadCr3,
        21 => AsmReadCr4,
        22 => AsmWriteCr0,
        23 => AsmWriteCr3,
        24 => AsmWriteCr4,
        25 => AsmSerialize,
        26 => AsmInstructionSyncBarrier,
        27 => AsmSpinPause,
        28 => AsmYieldHint,
        29 => AsmNop,
        30 => AsmWaitForEvent,
        31 => AsmWaitForInterrupt,
        32 => AsmSendEvent,
        33 => AsmSendEventLocal,
        34 => AsmWriteBackInvalidate,
        35 => AsmInvalidate,
        36 => AsmWriteBackNoInvalidate,
        37 => AsmReadSctlrEl1,
        38 => AsmReadTcrEl1,
        39 => AsmReadTtbr0El1,
        40 => AsmReadTtbr1El1,
        41 => AsmReadMairEl1,
        42 => AsmReadVbarEl1,
        43 => AsmReadTpidrEl1,
        44 => AsmReadEsrEl1,
        45 => AsmReadFarEl1,
        46 => AsmWriteSctlrEl1,
        47 => AsmWriteTcrEl1,
        48 => AsmWriteTtbr0El1,
        49 => AsmWriteTtbr1El1,
        50 => AsmWriteMairEl1,
        51 => AsmWriteVbarEl1,
        52 => AsmWriteTpidrEl1,
        53 => FloatIsNan,
        54 => FloatMultiplyThenAddF32,
        55 => FloatMultiplyThenAddF64,
        56 => FloatFusedMultiplyAddF32,
        57 => FloatFusedMultiplyAddF64,
        58 => FloatIsFinite,
        59 => FloatIsInfinite,
        60 => FloatIsNormal,
        61 => FloatIsSubnormal,
        62 => FloatClassifyF32,
        63 => FloatClassifyF64,
        64 => ContentOld,
        65 => ContentSeparate,
        66 => FloatAddTowardZeroF32,
        67 => FloatAddTowardZeroF64,
        68 => FloatAddTowardPositiveF32,
        69 => FloatAddTowardPositiveF64,
        70 => FloatAddTowardNegativeF32,
        71 => FloatAddTowardNegativeF64,
        72 => FloatSubtractTowardZeroF32,
        73 => FloatSubtractTowardZeroF64,
        74 => FloatSubtractTowardPositiveF32,
        75 => FloatSubtractTowardPositiveF64,
        76 => FloatSubtractTowardNegativeF32,
        77 => FloatSubtractTowardNegativeF64,
        78 => FloatMultiplyTowardZeroF32,
        79 => FloatMultiplyTowardZeroF64,
        80 => FloatMultiplyTowardPositiveF32,
        81 => FloatMultiplyTowardPositiveF64,
        82 => FloatMultiplyTowardNegativeF32,
        83 => FloatMultiplyTowardNegativeF64,
        84 => FloatDivideTowardZeroF32,
        85 => FloatDivideTowardZeroF64,
        86 => FloatDivideTowardPositiveF32,
        87 => FloatDivideTowardPositiveF64,
        88 => FloatDivideTowardNegativeF32,
        89 => FloatDivideTowardNegativeF64,
        90 => FloatSqrtTowardZeroF32,
        91 => FloatSqrtTowardZeroF64,
        92 => FloatSqrtTowardPositiveF32,
        93 => FloatSqrtTowardPositiveF64,
        94 => FloatSqrtTowardNegativeF32,
        95 => FloatSqrtTowardNegativeF64,
        96 => FloatFusedMultiplyAddTowardZeroF32,
        97 => FloatFusedMultiplyAddTowardZeroF64,
        98 => FloatFusedMultiplyAddTowardPositiveF32,
        99 => FloatFusedMultiplyAddTowardPositiveF64,
        100 => FloatFusedMultiplyAddTowardNegativeF32,
        101 => FloatFusedMultiplyAddTowardNegativeF64,
        102 => IntegerEmbed,
        103 => AsmStoreGdtr,
        104 => AsmLoadGdtr,
    });

    record_variants!(language_semantics::CarryAddress {
        0 => Stable,
        1 => Movable,
    });

    record_variants!(language_semantics::CarryCpu {
        0 => Origin,
        1 => Any,
    });

    record_variants!(language_semantics::CarryHostThread {
        0 => Origin,
        1 => Any,
    });

    record_fields!(language_semantics::CarryPolicy {
        suspension,
        cpu,
        host_thread,
        address
    });

    record_variants!(language_semantics::CarrySuspension {
        0 => Forbidden,
        1 => Allowed,
    });

    record_variants!(terminal_psi::CrashCause {
        0 => Trap,
        1 => Abort,
    });

    record_fields!(diagnostics::Diagnostic {
        severity,
        message,
        exceeded_provision,
        source_span,
        resolved_location
    });

    record_variants!(diagnostics::DiagnosticSeverity {
        0 => Error,
        1 => Warning,
        2 => Note,
    });

    record_variants!(language_semantics::DomainPredicatePresence {
        0 => Absent,
        1 => Present,
    });

    record_variants!(semantic_vocabulary::ExternalCompletionKind {
        0 => ProcessExit,
    });

    record_variants!(numerics::literals::FloatFormat {
        0 => F32,
        1 => F64,
    });

    record_fields!(diagnostics::ResolvedSourceLocation {
        package,
        path,
        line,
        column
    });

    record_fields!(source::SourceSpan { source_id, span });

    record_variants!(terminal_psi::TerminalAccessLifecycleOperation {
        0 => Initialize,
        1 => MoveOut,
        2 => View,
        3 => Relocate,
    });

    record_variants!(language_core::static_interrupts::StaticInterruptOperation {
        0 => Install,
        1 => Context,
        2 => BeginCompletion,
        3 => FinishCompletion,
    });
}
mod targets {
    use super::record_fields;

    record_variants!(target::Architecture {
        0 => Aarch64,
        1 => X86_64,
    });

    record_variants!(target::ForeignLocatorCandidate {
        0 => PeByName { library, export },
        1 => PeByOrdinal { library, ordinal },
        2 => ElfVersioned { object, symbol, version },
        3 => MachODylibSymbol { install_name, symbol },
    });

    record_variants!(target::HostedApplicationIntent {
        0 => Console,
        1 => Gui,
    });

    record_fields!(target::NativeApplicationMetadata {
        name,
        intent,
        identifier,
        subsystem,
        elf_interpreter
    });

    record_fields!(target::NativeTarget {
        architecture,
        object_format,
        pointer_size,
        pointer_alignment
    });

    record_variants!(target::ObjectFormat {
        0 => Elf,
        1 => MachO,
        2 => Coff,
    });

    record_variants!(target::TargetProfile {
        0 => LinuxArm64,
        1 => LinuxX64,
        2 => MacosArm64,
        3 => MacosX64,
        4 => WindowsX64,
        5 => UefiX64,
        6 => CrossPlatformCli,
        7 => LocalUnchecked,
        8 => AlphaBootstrap,
    });

    record_variants!(target::X86TargetFeature {
        0 => Avx,
        1 => Fma3,
    });

    record_variants!(target::X86ScalarFmaSlot {
        0 => Binary32,
        1 => Binary64,
    });

    record_variants!(target::ProgramEntryPhysicalContractPackage {
        0 => UefiX64,
        1 => MacosArm64,
        2 => MacosX64,
        3 => LinuxX86_64,
        4 => LinuxArm64,
        5 => WindowsX64,
    });
}
