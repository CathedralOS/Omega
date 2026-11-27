/*
Ordered top-level section encoder and decoder (`encode_raw`, `decode_module_body`) for
`TerminalModule`, including interned identities, domain declarations, boundary requirements,
access views/lifecycles, quotients, native layouts, and scalar conditions.
*/

pub(crate) mod borrow_wire {

    use crate::sections::semantic_module::wire_fields::{
        Access, Id, Index, List, U64, wire_enum, wire_record,
    };
    use terminal_psi::{
        TerminalBorrowBoundarySource, TerminalBorrowOwnerSegment, TerminalBorrowPlace,
        TerminalBorrowPlaceSegment, TerminalReborrowRestorationClass,
        TerminalReborrowRestoredCallUse, TerminalReborrowSharedCohortMember,
    };

    wire_enum! {
        Boundary: TerminalBorrowBoundarySource, "TerminalBorrowBoundarySource" {
            1 => Statement { statement_index: U64 },
            2 => Call { statement_index: U64, call_ordinal: U64, target_identity: Index },
        }
        OwnerSegment: TerminalBorrowOwnerSegment, "TerminalBorrowOwnerSegment" {
            1 => Field(identity: Index),
            2 => Case(identity: Index),
            3 => FixedIndex(index: U64),
            4 => DynamicIndex,
        }
        PlaceSegment: TerminalBorrowPlaceSegment, "TerminalBorrowPlaceSegment" {
            1 => Field(identity: Index),
            2 => Case(identity: Index),
            3 => FixedIndex(index: U64),
            4 => FixedRange { start: U64, end: U64 },
            5 => CapturedSelector(identity: Index),
        }
        RestorationClass: TerminalReborrowRestorationClass, "TerminalReborrowRestorationClass" {
            1 => ExclusiveReactivation,
            2 => SharedFreezeRestoration,
        }
    }

    wire_record! {
        Place: TerminalBorrowPlace { root_identity: Index, segments: List<PlaceSegment> }
        CohortMember: TerminalReborrowSharedCohortMember {
            child_owner_identity: Index,
            child_owner_path: List<OwnerSegment>,
            child_place: Place,
            child_access: Access,
            child_activation: Boundary,
            child_weakening: Boundary,
        }
        RestoredCallUse: TerminalReborrowRestoredCallUse {
            machine: Id,
            operation: Id,
            restoration_class: RestorationClass,
            call_boundary: Boundary,
            call_target_machine: Id,
            source_machine_identity: Index,
            source_state_identity: Index,
            direct_root_owner_identity: Index,
            direct_root_owner_path: List<OwnerSegment>,
            direct_root_place: Place,
            direct_root_activation: Boundary,
            direct_root_weakening: Boundary,
            direct_root_lifetime_identity: Index,
            child_owner_identity: Index,
            child_owner_path: List<OwnerSegment>,
            child_place: Place,
            projection_remainder: List<PlaceSegment>,
            child_access: Access,
            child_activation: Boundary,
            formation_boundary: Boundary,
            child_weakening: Boundary,
            shared_cohort: List<CohortMember>,
        }
    }
}

mod theorem_parameter_wire {
    use crate::obligation_evidence::Certificate;
    use crate::wire_fields::{
        Id, InvocationSite, List, Proposition, ScalarTermField, U32, wire_enum, wire_record,
    };
    use psi_callable_encoding::signatures::Declaration;
    use terminal_psi::{
        TheoremApplication, TheoremCitation, TheoremContract, TheoremInstantiation,
        TheoremParameter, TheoremSource,
    };

    wire_enum! {
        Source: TheoremSource, "TheoremSource" {
            1 => Named(machine: Id),
            2 => Formal(position: U32),
        }
    }

    wire_record! {
        Contract: TheoremContract {
            parameters: List<Declaration>,
            requires: List<Proposition>,
            ensures: List<Proposition>,
        }
        Parameter: TheoremParameter {
            machine: Id,
            proof_position: U32,
            contract: Contract,
        }
        Application: TheoremApplication {
            machine: Id,
            edge: Id,
            successor: Id,
            source: Source,
            arguments: List<ScalarTermField>,
            requirements: List<Id>,
        }
        Citation: TheoremCitation {
            machine: Id,
            site: InvocationSite,
            proof_position: U32,
            source: Source,
            derivations: List<Certificate>,
            instantiation: List<Instantiation>,
        }
        Instantiation: TheoremInstantiation {
            source: Source,
            derivations: List<Certificate>,
        }
    }
}
pub(crate) mod boundary_application_wire {
    use super::super::CodecError;
    use super::super::wire::{Reader, Writer, decode_counted};
    use super::super::wire_fields::{
        BoundaryPackage, Field, Index, MachineDeclaration, Optional, Text,
    };
    use terminal_psi::{
        BoundaryApplication, BoundaryApplicationArgument, BoundaryApplicationCandidate,
        BoundaryOperatorRequirement, BoundaryTypeIdentity, NominalDeclarationIdentity,
        TerminalBoundaryApplicationDemand, TerminalIdentityIndex,
    };

    pub(crate) fn encode_demand(
        writer: &mut Writer,
        demand: &TerminalBoundaryApplicationDemand,
    ) -> Result<(), CodecError> {
        writer.id(demand.terminal_operation());
        encode_requirement(writer, demand.requirement())?;
        encode_application(writer, demand.application())
    }

    pub(crate) fn decode_demand(
        reader: &mut Reader<'_>,
    ) -> Result<TerminalBoundaryApplicationDemand, CodecError> {
        Ok(TerminalBoundaryApplicationDemand::new(
            reader.id("OperationId")?,
            decode_requirement(reader)?,
            decode_application(reader)?,
        ))
    }

    pub(crate) fn encode_requirement(
        writer: &mut Writer,
        requirement: &BoundaryOperatorRequirement,
    ) -> Result<(), CodecError> {
        writer.u32(requirement.declaration().identity().0);
        writer.string("boundary requirement overload", requirement.overload())?;
        BoundaryPackage::write(writer, &requirement.package())
    }

    pub(crate) fn decode_requirement(
        reader: &mut Reader<'_>,
    ) -> Result<BoundaryOperatorRequirement, CodecError> {
        let declaration = NominalDeclarationIdentity::new(TerminalIdentityIndex(reader.u32()?));
        BoundaryOperatorRequirement::new(
            declaration,
            reader.string("boundary requirement overload")?,
            BoundaryPackage::read(reader)?,
        )
        .map_err(CodecError::MalformedStructuralFoundation)
    }

    fn encode_application(
        writer: &mut Writer,
        application: &BoundaryApplication,
    ) -> Result<(), CodecError> {
        writer.len(
            "boundary application arguments",
            application.arguments().len(),
        )?;
        for argument in application.arguments() {
            match argument {
                BoundaryApplicationArgument::Type {
                    binder_ordinal,
                    type_identity,
                } => {
                    writer.u8(1);
                    writer.u32(*binder_ordinal);
                    writer.u32(type_identity.identity().0);
                }
                BoundaryApplicationArgument::Const {
                    binder_ordinal,
                    declared_carrier,
                    value_type,
                    value_encoding,
                } => {
                    writer.u8(2);
                    writer.u32(*binder_ordinal);
                    writer.u32(declared_carrier.identity().0);
                    writer.string("boundary const value type", value_type)?;
                    writer.string("boundary const value", value_encoding)?;
                }
            }
        }
        Ok(())
    }

    fn decode_application(reader: &mut Reader<'_>) -> Result<BoundaryApplication, CodecError> {
        let arguments = decode_counted(reader, |reader| {
            let tag = reader.u8()?;
            let ordinal = reader.u32()?;
            match tag {
                1 => Ok(BoundaryApplicationArgument::type_argument(
                    ordinal,
                    BoundaryTypeIdentity::new(TerminalIdentityIndex(reader.u32()?)),
                )),
                2 => BoundaryApplicationArgument::const_argument(
                    ordinal,
                    BoundaryTypeIdentity::new(TerminalIdentityIndex(reader.u32()?)),
                    reader.string("boundary const value type")?,
                    reader.string("boundary const value")?,
                )
                .map_err(CodecError::MalformedStructuralFoundation),
                tag => Err(CodecError::InvalidTag("BoundaryApplicationArgument", tag)),
            }
        })?;
        if arguments.is_empty() {
            Ok(BoundaryApplication::Empty)
        } else {
            BoundaryApplication::exact(arguments).map_err(CodecError::MalformedStructuralFoundation)
        }
    }

    pub(crate) fn encode_candidate(
        writer: &mut Writer,
        candidate: &BoundaryApplicationCandidate,
    ) -> Result<(), CodecError> {
        encode_requirement(writer, &candidate.requirement)?;
        encode_application(writer, &candidate.application)?;
        Index::write(writer, &candidate.selection_identity)?;
        BoundaryPackage::write(writer, &candidate.selection_package)?;
        Optional::<Text>::write(writer, &candidate.target)?;
        MachineDeclaration::write(writer, &candidate.body)
    }

    pub(crate) fn decode_candidate(
        reader: &mut Reader<'_>,
    ) -> Result<BoundaryApplicationCandidate, CodecError> {
        let requirement = decode_requirement(reader)?;
        let application = decode_application(reader)?;
        let selection_identity = Index::read(reader)?;
        let selection_package = BoundaryPackage::read(reader)?;
        let target = Optional::<Text>::read(reader)?;
        let body = MachineDeclaration::read(reader)?;
        if selection_identity.is_absent() || target.as_ref().is_some_and(String::is_empty) {
            return Err(CodecError::MalformedStructuralFoundation(
                "empty boundary candidate identity or commitment",
            ));
        }
        Ok(BoundaryApplicationCandidate {
            requirement,
            application,
            selection_identity,
            selection_package,
            target,
            body,
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use semantic_vocabulary::OperationId;

        #[test]
        fn boundary_demand_wire_round_trip_and_truncation() {
            let applications = [
                BoundaryApplication::Empty,
                BoundaryApplication::exact(vec![
                    BoundaryApplicationArgument::type_argument(
                        0,
                        BoundaryTypeIdentity::new(TerminalIdentityIndex(1)),
                    ),
                    BoundaryApplicationArgument::const_argument(
                        1,
                        BoundaryTypeIdentity::new(TerminalIdentityIndex(2)),
                        "u64".into(),
                        "integer(u64,4)".into(),
                    )
                    .unwrap(),
                ])
                .unwrap(),
            ];
            for application in applications {
                let demand = TerminalBoundaryApplicationDemand::new(
                    OperationId::new(1).unwrap(),
                    BoundaryOperatorRequirement::new(
                        NominalDeclarationIdentity::new(TerminalIdentityIndex(3)),
                        "operator::compare(Element,Element)->bool".into(),
                        None,
                    )
                    .unwrap(),
                    application,
                );
                let mut writer = Writer::default();
                encode_demand(&mut writer, &demand).unwrap();
                let bytes = writer.finish();
                let mut reader = Reader::new(&bytes);
                assert_eq!(decode_demand(&mut reader).unwrap(), demand);
                assert_eq!(reader.remaining(), 0);
                for end in 0..bytes.len() {
                    assert!(decode_demand(&mut Reader::new(&bytes[..end])).is_err());
                }
            }
        }

        #[test]
        fn boundary_demand_wire_rejects_noncanonical_binder_order() {
            let demand = TerminalBoundaryApplicationDemand::new(
                OperationId::new(1).unwrap(),
                BoundaryOperatorRequirement::new(
                    NominalDeclarationIdentity::new(TerminalIdentityIndex(3)),
                    "operator::compare".into(),
                    None,
                )
                .unwrap(),
                BoundaryApplication::Exact(vec![BoundaryApplicationArgument::type_argument(
                    1,
                    BoundaryTypeIdentity::new(TerminalIdentityIndex(2)),
                )]),
            );
            let mut writer = Writer::default();
            encode_demand(&mut writer, &demand).unwrap();
            assert!(decode_demand(&mut Reader::new(&writer.finish())).is_err());
        }

        #[test]
        fn boundary_candidate_wire_preserves_owner_and_specialization() {
            let nominal = |name: &str| {
                NominalDeclarationIdentity::new(TerminalIdentityIndex(
                    name.bytes()
                        .fold(1u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32)),
                ))
            };
            for specialization in [
                None,
                Some(terminal_psi::CheckedMachineSpecialization {
                    capture: None,
                    template_entry: nominal("owner::template::entry"),
                    template: nominal("owner::template"),
                    template_callable: TerminalIdentityIndex(8),
                    template_semantic_declaration: TerminalIdentityIndex(9),
                    commitment: [4; 32],
                }),
            ] {
                let candidate = terminal_psi::BoundaryApplicationCandidate {
                    requirement: BoundaryOperatorRequirement::new(
                        nominal("owner::requirement"),
                        "operator::compare".into(),
                        semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]),
                    )
                    .unwrap(),
                    application: BoundaryApplication::Empty,
                    selection_identity: TerminalIdentityIndex(9),
                    selection_package: semantic_vocabulary::PackageKeyIdentity::from_digest(
                        [2; 32],
                    ),
                    target: Some("windows_x86_64".into()),
                    body: terminal_psi::CheckedMachineDeclaration {
                        semantic_declaration: Default::default(),
                        parameter_types: Vec::new(),
                        callable: Default::default(),
                        result_type: Default::default(),
                        machine: nominal("owner::body"),
                        state: nominal("owner::entry"),
                        contract: [3; 32],
                        published_may_suspend: false,
                        published_may_block: false,
                        specialization,
                    },
                };
                let mut writer = Writer::default();
                encode_candidate(&mut writer, &candidate).unwrap();
                let bytes = writer.finish();
                let mut reader = Reader::new(&bytes);
                assert_eq!(decode_candidate(&mut reader).unwrap(), candidate);
                assert_eq!(reader.remaining(), 0);
                for end in 0..bytes.len() {
                    assert!(decode_candidate(&mut Reader::new(&bytes[..end])).is_err());
                }
                let mut malformed = candidate;
                malformed.body.contract = [0; 32];
                let mut writer = Writer::default();
                assert!(encode_candidate(&mut writer, &malformed).is_err());
            }
        }
    }
}
pub(crate) mod callback_application_wire {

    use super::super::wire_fields::{
        Digest, Flag, Id, MachineDeclaration, OptionalId, Text, U32, wire_record,
    };
    use terminal_psi::TerminalCallbackApplication;

    wire_record! {
        Application: TerminalCallbackApplication {
            application_index: U32,
            registrar_machine: Id,
            registrar: Id,
            static_machine_ordinal: U32,
            requirement: Text,
            calling_owner_application: Text,
            boundary: OptionalId,
            published_contract: Digest,
            entry_admitted: Flag,
            body: MachineDeclaration,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::sections::semantic_module::wire::{Reader, Writer};
        use crate::sections::semantic_module::wire_fields::Field;
        use semantic_vocabulary::OperationId;

        #[test]
        fn callback_application_wire_preserves_identity_and_rejects_truncation() {
            let application = TerminalCallbackApplication {
                application_index: 17,
                registrar_machine: semantic_vocabulary::MachineId::new(3).unwrap(),
                registrar: OperationId::new(42).expect("nonzero operation"),
                static_machine_ordinal: 2,
                requirement: "Hooks::call(u64)".to_owned(),
                calling_owner_application: "ChosenHooks".to_owned(),
                boundary: semantic_vocabulary::BoundaryMachineId::new(3),
                published_contract: [3; 32],
                entry_admitted: true,
                body: terminal_psi::CheckedMachineDeclaration {
                    semantic_declaration: Default::default(),
                    parameter_types: Vec::new(),
                    callable: Default::default(),
                    result_type: Default::default(),
                    machine: terminal_psi::NominalDeclarationIdentity::new(
                        terminal_psi::TerminalIdentityIndex(11),
                    ),
                    state: terminal_psi::NominalDeclarationIdentity::new(
                        terminal_psi::TerminalIdentityIndex(12),
                    ),
                    contract: [5; 32],
                    published_may_suspend: false,
                    published_may_block: false,
                    specialization: None,
                },
            };
            let mut writer = Writer::default();
            Application::write(&mut writer, &application).expect("encode callback");
            let bytes = writer.finish();
            let mut reader = Reader::new(&bytes);
            assert_eq!(
                Application::read(&mut reader).expect("decode callback"),
                application
            );
            assert_eq!(reader.remaining(), 0);
            let mut another_owner = application.clone();
            another_owner.registrar_machine = semantic_vocabulary::MachineId::new(4).unwrap();
            let mut changed_writer = Writer::default();
            Application::write(&mut changed_writer, &another_owner).unwrap();
            let changed_bytes = changed_writer.finish();
            assert_ne!(
                bytes, changed_bytes,
                "the owner participates in the encoded occurrence"
            );
            assert_eq!(
                Application::read(&mut Reader::new(&changed_bytes)).unwrap(),
                another_owner
            );
            let mut absent_owner = bytes.clone();
            absent_owner[4..12].fill(0);
            assert!(Application::read(&mut Reader::new(&absent_owner)).is_err());
            for byte_count in 0..bytes.len() {
                assert!(
                    Application::read(&mut Reader::new(&bytes[..byte_count])).is_err(),
                    "accepted {byte_count} bytes"
                );
            }
            let mut unadmitted = application;
            unadmitted.entry_admitted = false;
            unadmitted.boundary = None;
            let mut writer = Writer::default();
            Application::write(&mut writer, &unadmitted).unwrap();
            assert_eq!(
                Application::read(&mut Reader::new(&writer.finish())).unwrap(),
                unadmitted
            );
            let mut module = terminal_psi::TerminalModule::declarations();
            module.interfaces.callback_applications.push(unadmitted);
            super::super::super::canonical_order::validate_canonical_order(&module).unwrap();
            module.interfaces.callback_applications[0].entry_admitted = true;
            assert!(
                super::super::super::canonical_order::validate_canonical_order(&module).is_err()
            );
            module.interfaces.callback_applications[0].entry_admitted = false;
            module.interfaces.callback_applications[0].boundary =
                semantic_vocabulary::BoundaryMachineId::new(3);
            assert!(
                super::super::super::canonical_order::validate_canonical_order(&module).is_err()
            );
        }
    }
}
pub(crate) mod calling_signature_wire {

    use super::super::wire_fields::{
        Field, Flag, Id, Index, List, Optional, ResultField, Text, U32, wire_record,
    };
    use super::{CodecError, Reader, Writer};
    use semantic_vocabulary::CallingGenericFormal;
    use terminal_psi::{
        CallingAbiDeclaration, CallingAbiParameter, CallingAbiStorage, CallingCallbackBinder,
        CallingDirectCallback, CallingPolicyInstance, CallingPolicyRequest, CallingRuntimeFormal,
        CallingSignatureDeclaration, CallingSignatureTelescope, CallingStaticOperand,
    };

    wire_record! {
        AbiDeclaration: CallingAbiDeclaration {
            id: Id,
            requirement_identity: Index,
            declaring_application: Index,
            operands: List<StaticOperand>,
            formals: List<GenericFormal>,
            default_eligible: Flag,
            physical_owner: ResultField<DeclarationOwner, Text>,
            storage: ResultField<AbiStorage, Text>,
        }
        AbiStorage: CallingAbiStorage {
            parameters: List<AbiParameter>,
            result: Optional<Id>,
            telescope: Telescope,
        }
        StaticOperand: CallingStaticOperand { ordinal: U32, kind: OperandKind, identity: Index }
        AbiParameter: CallingAbiParameter { structural_type: Id, callback_backing: Optional<Id>, access: super::super::wire_fields::Access }
        GenericFormal: CallingGenericFormal { ordinal: U32, kind: OperandKind, callback: Optional<BinderId> }
        PolicyRequest: CallingPolicyRequest {
            declaration: ResultField<Index, Text>,
            display_name: Text,
            instances: ResultField<List<PolicyInstance>, List<Text>>,
        }
        PolicyInstance: CallingPolicyInstance {
            declaring_application: Index,
            policy_declaration: ResultField<ProofDeclaration, Text>,
            policy: ResultField<super::callable_wire::Callable, Text>,
            signatures: ResultField<List<Id>, Text>,
        }
        Declaration: CallingSignatureDeclaration {
            boundary: Id,
            application: ResultField<Id, Text>,
        }
        Telescope: CallingSignatureTelescope {
            parameters: List<Formal>,
            binders: List<Binder>,
            direct_callbacks: List<DirectCallback>,
        }
        Formal: CallingRuntimeFormal { identity: Parameter, native_ordinal: U32 }
        Binder: CallingCallbackBinder {
            binder: BinderId,
            requirement: Requirement,
            requirement_identity: Index,
            static_machine_ordinal: U32,
            invokes_synchronously: Flag,
        }
        DirectCallback: CallingDirectCallback {
            identity: Parameter,
            native_ordinal: U32,
            binder: BinderId,
            requirement: Requirement,
        }
    }

    struct DeclarationOwner;
    impl Field<semantic_vocabulary::CallingDeclarationOwner> for DeclarationOwner {
        fn write(
            writer: &mut Writer,
            value: &semantic_vocabulary::CallingDeclarationOwner,
        ) -> Result<(), CodecError> {
            use semantic_vocabulary::CallingDeclarationOwner;
            writer.u8(match value {
                CallingDeclarationOwner::BoundaryTrait => 0,
                CallingDeclarationOwner::TopLevelRequirement => 1,
            });
            Ok(())
        }

        fn read(
            reader: &mut Reader<'_>,
        ) -> Result<semantic_vocabulary::CallingDeclarationOwner, CodecError> {
            use semantic_vocabulary::CallingDeclarationOwner;
            match reader.u8()? {
                0 => Ok(CallingDeclarationOwner::BoundaryTrait),
                1 => Ok(CallingDeclarationOwner::TopLevelRequirement),
                _ => Err(CodecError::MalformedStructuralFoundation(
                    "unknown physical calling owner",
                )),
            }
        }
    }

    struct OperandKind;
    struct ProofDeclaration;
    impl Field<terminal_psi::ProofDeclarationId> for ProofDeclaration {
        fn write(
            writer: &mut Writer,
            value: &terminal_psi::ProofDeclarationId,
        ) -> Result<(), CodecError> {
            writer.u32(value.0);
            Ok(())
        }
        fn read(reader: &mut Reader<'_>) -> Result<terminal_psi::ProofDeclarationId, CodecError> {
            Ok(terminal_psi::ProofDeclarationId(reader.u32()?))
        }
    }
    impl Field<semantic_vocabulary::CallingOperandKind> for OperandKind {
        fn write(
            writer: &mut Writer,
            value: &semantic_vocabulary::CallingOperandKind,
        ) -> Result<(), CodecError> {
            use semantic_vocabulary::CallingOperandKind::*;
            writer.u8(match value {
                Type => 0,
                Const => 1,
                Value => 2,
                Machine => 3,
                Implementation => 4,
                Proposition => 5,
                Proof => 6,
            });
            Ok(())
        }
        fn read(
            reader: &mut Reader<'_>,
        ) -> Result<semantic_vocabulary::CallingOperandKind, CodecError> {
            use semantic_vocabulary::CallingOperandKind::*;
            Ok(match reader.u8()? {
                0 => Type,
                1 => Const,
                2 => Value,
                3 => Machine,
                4 => Implementation,
                5 => Proposition,
                6 => Proof,
                _ => {
                    return Err(CodecError::MalformedStructuralFoundation(
                        "unknown calling operand kind",
                    ));
                }
            })
        }
    }

    macro_rules! native_ids {
        ($($marker:ident: $ty:ident, $zero:literal;)*) => {$(
            struct $marker;
            impl Field<semantic_vocabulary::$ty> for $marker {
                fn write(writer: &mut Writer, value: &semantic_vocabulary::$ty) -> Result<(), CodecError> {
                    writer.u64(value.get());
                    Ok(())
                }
                fn read(reader: &mut Reader<'_>) -> Result<semantic_vocabulary::$ty, CodecError> {
                    semantic_vocabulary::$ty::new(reader.u64()?)
                        .ok_or(CodecError::MalformedStructuralFoundation($zero))
                }
            }
        )*};
    }
    native_ids! {
        Parameter: NativeParameterId, "zero native parameter identity";
        BinderId: StaticMachineBinderId, "zero callback binder identity";
        Requirement: CallbackRequirementId, "zero callback requirement identity";
    }
}
pub(crate) mod suspension_wire {

    use crate::sections::semantic_module::CodecError;
    use crate::sections::semantic_module::structural_place_wire::{
        decode_structural_path, encode_structural_path,
    };
    use crate::sections::semantic_module::wire::{Reader, Writer};
    use crate::sections::semantic_module::wire_fields::{
        Access, Argument, Digest, Field, Flag, Id, Ids, List, Scalar, U32, wire_enum, wire_record,
    };
    use arena::{Handle, HandleSpan};
    use terminal_psi::{
        TerminalSuspensionCallPlan, TerminalSuspensionCallSite, TerminalSuspensionCallTarget,
        TerminalSuspensionLiveValue, TerminalSuspensionLoan, TerminalSuspensionLoanBacking,
        TerminalSuspensionLoanCapture, TerminalSuspensionLoanResource, TerminalSuspensionLoans,
        TerminalSuspensionPlace, TerminalSuspensionStorage, TerminalSuspensionValueLoans,
        TerminalSuspensionValueType,
    };

    wire_record! {
        LiveValue: TerminalSuspensionLiveValue {
            place: Place,
            value_type: ValueType,
            storage: Storage,
            claim_count: U32,
            claims: Ids,
            loans: ValueLoans,
        }
        CallSite: TerminalSuspensionCallSite {
            operation: Id,
            crossing: Id,
            target: CallTarget,
            frontier_commitment: Digest,
        }
        Loan: TerminalSuspensionLoan {
            capture: Capture,
            backing: Backing,
            resource: Resource,
            parent: HandleKind,
            suppliers: SpanKind,
            inputs: SpanKind,
            access: Access,
            active_at_call: Flag,
        }
    }

    wire_enum! {
        Place: TerminalSuspensionPlace, "TerminalSuspensionPlace" {
            1 => Scalar(value: Id),
            2 => Structural(argument: Argument),
        }
        ValueType: TerminalSuspensionValueType, "TerminalSuspensionValueType" {
            1 => Scalar(scalar: Scalar),
            2 => Structural(identity: Id),
        }
        Storage: TerminalSuspensionStorage, "TerminalSuspensionStorage" {
            2 => Parameter,
            3 => Local,
            4 => CallArgument,
        }
        ValueLoans: TerminalSuspensionValueLoans, "TerminalSuspensionValueLoans" {
            0 => Unresolved,
            1 => ViewFree,
            2 => Retained { links: SpanKind },
        }
        CallTarget: TerminalSuspensionCallTarget, "TerminalSuspensionCallTarget" {
            1 => Machine(machine: Id),
            2 => Boundary(boundary: Id),
            5 => Dynamic { interface: U32, slot: U32 },
        }
        Capture: TerminalSuspensionLoanCapture, "TerminalSuspensionLoanCapture" {
            0 => Unresolved,
            1 => Reference {
                formation: Id,
                carrier: Id,
                source: Id,
                source_access: Access,
                source_path: SpanKind,
            },
            2 => RetainedValue { live_value: U32, path: SpanKind },
            3 => CompletedOutput { call: Id, argument: Argument, path: SpanKind },
            4 => CallInput { call: Id, argument: Argument, path: SpanKind, stored: Flag },
            5 => InputDependencies,
            6 => DescriptorTransfer,
        }
        Backing: TerminalSuspensionLoanBacking, "TerminalSuspensionLoanBacking" {
            0 => Unresolved,
            1 => RetainedOwner { live_value: U32 },
            2 => IncomingReferent { argument: Argument },
            3 => CompletedOutput { loan: HandleKind },
            4 => InputDependencies,
            5 => CapturedInput,
        }
        Resource: TerminalSuspensionLoanResource, "TerminalSuspensionLoanResource" {
            0 => Unresolved,
            1 => DirectRoot,
            2 => Reborrow,
            3 => IncomingReference,
            4 => CompletedOutput,
            5 => LifetimeDependency,
            6 => OutputInput,
            7 => DescriptorTransfer,
        }
    }

    struct HandleKind;
    impl<T> Field<Handle<T>> for HandleKind {
        fn write(writer: &mut Writer, value: &Handle<T>) -> Result<(), CodecError> {
            writer.u32(value.arena_index());
            writer.u32(value.generation());
            Ok(())
        }
        fn read(reader: &mut Reader<'_>) -> Result<Handle<T>, CodecError> {
            let slot = reader.u32()?;
            let generation = reader.u32()?;
            if (slot == 0 && generation != 0) || (slot != 0 && generation != 1) {
                return Err(CodecError::NonCanonicalEncoding);
            }
            Ok(Handle::from_parts(slot, generation))
        }
    }

    struct SpanKind;
    impl<T> Field<HandleSpan<T>> for SpanKind {
        fn write(writer: &mut Writer, value: &HandleSpan<T>) -> Result<(), CodecError> {
            HandleKind::write(writer, &value.start())?;
            writer.u32(value.count());
            Ok(())
        }
        fn read(reader: &mut Reader<'_>) -> Result<HandleSpan<T>, CodecError> {
            let start: Handle<T> = HandleKind::read(reader)?;
            let count = reader.count()?;
            if (count == 0) == start.is_valid() {
                return Err(CodecError::NonCanonicalEncoding);
            }
            Ok(HandleSpan::from_parts_or_empty(start, count))
        }
    }

    pub(crate) fn encode_suspension_call_plan(
        writer: &mut Writer,
        plan: &TerminalSuspensionCallPlan,
    ) -> Result<(), CodecError> {
        writer.id(plan.operation);
        writer.id(plan.crossing);
        CallTarget::write(writer, &plan.target)?;
        writer.u32(plan.live_value_count);
        List::<LiveValue>::write(writer, &plan.live_values)?;
        encode_loans(writer, &plan.loans)
    }

    pub(crate) fn decode_suspension_call_plan(
        reader: &mut Reader<'_>,
    ) -> Result<TerminalSuspensionCallPlan, CodecError> {
        let operation = reader.id("OperationId")?;
        let crossing = reader.id("SuspensionCrossingId")?;
        let target = CallTarget::read(reader)?;
        let live_value_count = reader.u32()?;
        let live_values = List::<LiveValue>::read(reader)?;
        let loans = decode_loans(reader)?;
        validate_loan_coordinates(&loans, &live_values)?;
        Ok(TerminalSuspensionCallPlan {
            operation,
            crossing,
            target,
            live_value_count,
            live_values,
            loans,
        })
    }

    fn encode_loans(
        writer: &mut Writer,
        loans: &TerminalSuspensionLoans,
    ) -> Result<(), CodecError> {
        let paths: Vec<_> = loans
            .paths
            .iter()
            .map(|(_, segment)| segment.clone())
            .collect();
        encode_structural_path(writer, "suspension loan paths", &paths)?;
        writer.len("suspension loan links", loans.value_links.len())?;
        for (_, link) in loans.value_links.iter() {
            HandleKind::write(writer, link)?;
        }
        writer.len("suspension loan suppliers", loans.supplier_links.len())?;
        for (_, supplier) in loans.supplier_links.iter() {
            HandleKind::write(writer, supplier)?;
        }
        writer.len("suspension loan inputs", loans.input_links.len())?;
        for (_, input) in loans.input_links.iter() {
            HandleKind::write(writer, input)?;
        }
        writer.len("suspension loans", loans.occurrences.len())?;
        for (_, loan) in loans.occurrences.iter() {
            Loan::write(writer, loan)?;
        }
        Ok(())
    }

    fn decode_loans(reader: &mut Reader<'_>) -> Result<TerminalSuspensionLoans, CodecError> {
        let mut loans = TerminalSuspensionLoans::default();
        for segment in decode_structural_path(reader)? {
            loans.paths.insert(segment);
        }
        for link in List::<HandleKind>::read(reader)? {
            loans.value_links.insert(link);
        }
        for supplier in List::<HandleKind>::read(reader)? {
            loans.supplier_links.insert(supplier);
        }
        for input in List::<HandleKind>::read(reader)? {
            loans.input_links.insert(input);
        }
        for loan in List::<Loan>::read(reader)? {
            loans.occurrences.insert(loan);
        }
        Ok(loans)
    }

    fn validate_loan_coordinates(
        loans: &TerminalSuspensionLoans,
        live_values: &[TerminalSuspensionLiveValue],
    ) -> Result<(), CodecError> {
        for (_, link) in loans.value_links.iter() {
            if !physical_loan(loans, *link) {
                return Err(CodecError::NonCanonicalEncoding);
            }
        }
        for (_, supplier) in loans.supplier_links.iter() {
            if !loans.occurrences.is_valid(*supplier) {
                return Err(CodecError::NonCanonicalEncoding);
            }
        }
        for (_, input) in loans.input_links.iter() {
            if !loans.occurrences.is_valid(*input)
                || loans.occurrences.get(*input).resource
                    != TerminalSuspensionLoanResource::OutputInput
            {
                return Err(CodecError::NonCanonicalEncoding);
            }
        }
        for (_, loan) in loans.occurrences.iter() {
            if (loan.parent.is_valid() && !physical_loan(loans, loan.parent))
                || loans.supplier_links.span(loan.suppliers).is_none()
                || loans.input_links.span(loan.inputs).is_none()
            {
                return Err(CodecError::NonCanonicalEncoding);
            }
            match loan.capture {
                TerminalSuspensionLoanCapture::CompletedOutput { path, .. }
                | TerminalSuspensionLoanCapture::CallInput { path, .. }
                    if loans.paths.span(path).is_none() =>
                {
                    return Err(CodecError::NonCanonicalEncoding);
                }
                TerminalSuspensionLoanCapture::Reference { source_path, .. }
                    if loans.paths.span(source_path).is_none() =>
                {
                    return Err(CodecError::NonCanonicalEncoding);
                }
                TerminalSuspensionLoanCapture::RetainedValue { live_value, path }
                    if loans.paths.span(path).is_none()
                        || !live_values.get(live_value as usize).is_some_and(|live| {
                            live.storage
                                == if loan.resource
                                    == TerminalSuspensionLoanResource::IncomingReference
                                {
                                    TerminalSuspensionStorage::Parameter
                                } else {
                                    TerminalSuspensionStorage::CallArgument
                                }
                                && matches!(live.place, TerminalSuspensionPlace::Structural(_))
                        }) =>
                {
                    return Err(CodecError::NonCanonicalEncoding);
                }
                _ => {}
            }
            if loan.resource == TerminalSuspensionLoanResource::IncomingReference
                && (!matches!(
                    loan.capture,
                    TerminalSuspensionLoanCapture::RetainedValue { .. }
                ) || !matches!(
                    loan.backing,
                    TerminalSuspensionLoanBacking::IncomingReferent { .. }
                ) || loan.parent.is_valid()
                    || !loan.suppliers.is_empty()
                    || !loan.active_at_call)
            {
                return Err(CodecError::NonCanonicalEncoding);
            }
            let valid_role = match loan.resource {
                TerminalSuspensionLoanResource::CompletedOutput => {
                    matches!(
                        loan.capture,
                        TerminalSuspensionLoanCapture::CompletedOutput { .. }
                    ) && matches!(
                        loan.backing,
                        TerminalSuspensionLoanBacking::CompletedOutput { .. }
                    ) && !loan.parent.is_valid()
                }
                TerminalSuspensionLoanResource::LifetimeDependency => {
                    matches!(
                        loan.capture,
                        TerminalSuspensionLoanCapture::InputDependencies
                    ) && matches!(
                        loan.backing,
                        TerminalSuspensionLoanBacking::InputDependencies
                    ) && !loan.parent.is_valid()
                }
                TerminalSuspensionLoanResource::OutputInput => {
                    matches!(
                        loan.capture,
                        TerminalSuspensionLoanCapture::CallInput { .. }
                    ) && !loan.parent.is_valid()
                        && loan.inputs.is_empty()
                        && !loan.active_at_call
                }
                TerminalSuspensionLoanResource::DescriptorTransfer => {
                    matches!(
                        loan.capture,
                        TerminalSuspensionLoanCapture::DescriptorTransfer
                            | TerminalSuspensionLoanCapture::RetainedValue { .. }
                            | TerminalSuspensionLoanCapture::CallInput { .. }
                    ) && !loan.parent.is_valid()
                        && !loan.suppliers.is_empty()
                        && loan.inputs.is_empty()
                }
                _ => loan.inputs.is_empty(),
            };
            if !valid_role {
                return Err(CodecError::NonCanonicalEncoding);
            }
            if matches!(
                loan.backing,
                TerminalSuspensionLoanBacking::InputDependencies
            ) && loan.resource != TerminalSuspensionLoanResource::LifetimeDependency
                || matches!(loan.backing, TerminalSuspensionLoanBacking::CapturedInput)
                    && (loan.active_at_call
                        || !matches!(
                            loan.capture,
                            TerminalSuspensionLoanCapture::CallInput { .. }
                        )
                        || !matches!(
                            loan.resource,
                            TerminalSuspensionLoanResource::OutputInput
                                | TerminalSuspensionLoanResource::DirectRoot
                                | TerminalSuspensionLoanResource::Reborrow
                                | TerminalSuspensionLoanResource::DescriptorTransfer
                        ))
            {
                return Err(CodecError::NonCanonicalEncoding);
            }
            if let TerminalSuspensionLoanBacking::CompletedOutput { loan: output } = loan.backing
                && (!loans.occurrences.is_valid(output)
                    || loans.occurrences.get(output).resource
                        != TerminalSuspensionLoanResource::CompletedOutput)
            {
                return Err(CodecError::NonCanonicalEncoding);
            }
            if let TerminalSuspensionLoanBacking::RetainedOwner { live_value } = loan.backing
                && live_value as usize >= live_values.len()
            {
                return Err(CodecError::NonCanonicalEncoding);
            }
        }
        for live in live_values {
            if let TerminalSuspensionValueLoans::Retained { links } = live.loans
                && (links.is_empty() || loans.value_links.span(links).is_none())
            {
                return Err(CodecError::NonCanonicalEncoding);
            }
        }
        Ok(())
    }

    fn physical_loan(
        loans: &TerminalSuspensionLoans,
        loan: Handle<TerminalSuspensionLoan>,
    ) -> bool {
        loans.occurrences.is_valid(loan)
            && !matches!(
                loans.occurrences.get(loan).resource,
                TerminalSuspensionLoanResource::LifetimeDependency
                    | TerminalSuspensionLoanResource::OutputInput
            )
    }

    #[cfg(test)]
    mod tests {
        use super::{CallTarget, Field, decode_suspension_call_plan, encode_suspension_call_plan};
        use crate::codec_error::CodecError;
        use crate::sections::semantic_module::wire::{Reader, Writer};
        use semantic_vocabulary::{
            MachineId, ObligationId, OperationId, PlaceId, StructuralCaseId, StructuralTypeId,
            SuspensionCrossingId, ValueId,
        };
        use terminal_psi::{
            StructuralAccess, StructuralArgument, StructuralPathSegment,
            TerminalSuspensionCallPlan, TerminalSuspensionCallTarget, TerminalSuspensionLiveValue,
            TerminalSuspensionPlace, TerminalSuspensionStorage, TerminalSuspensionValueType,
            suspension_frontier_commitment,
        };

        #[test]
        fn dynamic_suspension_target_keeps_its_slot_and_rejects_retired_tags() {
            for target in [
                TerminalSuspensionCallTarget::Dynamic {
                    interface: 0,
                    slot: 0,
                },
                TerminalSuspensionCallTarget::Dynamic {
                    interface: 7,
                    slot: 13,
                },
            ] {
                let mut writer = Writer::default();
                CallTarget::write(&mut writer, &target).unwrap();
                let bytes = writer.finish();
                let mut reader = Reader::new(&bytes);
                assert_eq!(
                    CallTarget::read(&mut reader).expect("selector target"),
                    target
                );
                assert_eq!(reader.remaining(), 0);
                assert_eq!(bytes[0], 5);
            }
            for retired in [3, 4] {
                let bytes = [retired];
                assert!(matches!(
                    CallTarget::read(&mut Reader::new(&bytes)),
                    Err(CodecError::InvalidTag("TerminalSuspensionCallTarget", tag)) if tag == retired
                ));
            }
        }

        #[test]
        fn a_suspension_plan_records_the_live_value_and_its_loans() {
            let plan = TerminalSuspensionCallPlan {
                operation: OperationId::new(1).unwrap(),
                crossing: SuspensionCrossingId::new(2).unwrap(),
                target: TerminalSuspensionCallTarget::Machine(MachineId::new(3).unwrap()),
                live_value_count: 1,
                live_values: vec![TerminalSuspensionLiveValue {
                    place: TerminalSuspensionPlace::Scalar(ValueId::new(4).unwrap()),
                    value_type: TerminalSuspensionValueType::Scalar(
                        semantic_vocabulary::ScalarType::Boolean,
                    ),
                    storage: TerminalSuspensionStorage::Local,
                    claim_count: 0,
                    claims: Vec::new(),
                    loans: terminal_psi::TerminalSuspensionValueLoans::ViewFree,
                }],
                loans: Default::default(),
            };
            let mut expected = Vec::new();
            expected.extend(1u64.to_le_bytes());
            expected.extend(2u64.to_le_bytes());
            expected.push(1); // Machine call target
            expected.extend(3u64.to_le_bytes());
            expected.extend(1u32.to_le_bytes()); // Live value count
            expected.extend(1u32.to_le_bytes()); // One value follows
            expected.push(1); // Scalar place
            expected.extend(4u64.to_le_bytes());
            expected.extend([1, 1, 3]); // Scalar value type, Boolean, local storage
            expected.extend([0; 8]); // Declared claim count and empty claim list
            expected.push(1); // View-free value
            expected.extend([0; 20]); // Five empty loan tables

            let mut writer = Writer::default();
            encode_suspension_call_plan(&mut writer, &plan).unwrap();
            assert_eq!(writer.finish(), expected);
            let mut reader = Reader::new(&expected);
            assert_eq!(decode_suspension_call_plan(&mut reader).unwrap(), plan);
            assert_eq!(reader.remaining(), 0);
        }

        #[test]
        fn suspension_storage_access_survives_wire_and_changes_identity() {
            let table = terminal_psi::TerminalIdentityTable::from(
                (0..64).map(|i| format!("identity-{i}")).collect::<Vec<_>>(),
            );
            let mut encodings = Vec::new();
            let mut commitments = Vec::new();
            for access in [
                StructuralAccess::Owned,
                StructuralAccess::SharedBorrow,
                StructuralAccess::MutableBorrow,
                StructuralAccess::WriteOnlyBorrow,
            ] {
                let selector = ValueId::new(7).expect("nonzero selector");
                let plan = TerminalSuspensionCallPlan {
                    operation: OperationId::new(1).expect("nonzero operation"),
                    crossing: SuspensionCrossingId::new(2).expect("nonzero crossing"),
                    target: TerminalSuspensionCallTarget::Machine(
                        MachineId::new(3).expect("nonzero machine"),
                    ),
                    live_value_count: 1,
                    live_values: vec![TerminalSuspensionLiveValue {
                        place: TerminalSuspensionPlace::Structural(StructuralArgument {
                            place: PlaceId::new(4).expect("nonzero place"),
                            access,
                            path: vec![
                                StructuralPathSegment::Case(
                                    StructuralCaseId::new(5).expect("nonzero case"),
                                ),
                                StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(
                                    13,
                                )),
                                StructuralPathSegment::Referent,
                                StructuralPathSegment::FixedIndex(2),
                                StructuralPathSegment::FixedRange { start: 1, end: 4 },
                                StructuralPathSegment::RuntimeIndex {
                                    index: selector,
                                    obligation: ObligationId::new(8).expect("nonzero obligation"),
                                },
                            ],
                        }),
                        value_type: TerminalSuspensionValueType::Structural(
                            StructuralTypeId::new(6).expect("nonzero structural type"),
                        ),
                        storage: TerminalSuspensionStorage::Local,
                        claim_count: 0,
                        claims: Vec::new(),
                        loans: terminal_psi::TerminalSuspensionValueLoans::Unresolved,
                    }],
                    loans: terminal_psi::TerminalSuspensionLoans::default(),
                };
                let mut writer = Writer::default();
                encode_suspension_call_plan(&mut writer, &plan).expect("encode frontier");
                let encoded = writer.finish();
                let mut reader = Reader::new(&encoded);
                let decoded = decode_suspension_call_plan(&mut reader).expect("decode frontier");
                assert_eq!(decoded, plan);
                assert_eq!(reader.remaining(), 0);

                let mut retained_values = Vec::new();
                decoded.live_values[0]
                    .place
                    .visit_value_ids(|value| retained_values.push(value));
                assert_eq!(retained_values, vec![selector]);
                let mut repeated = plan.clone();
                repeated.live_values.push(repeated.live_values[0].clone());
                repeated.live_value_count = 2;
                let mut writer = Writer::default();
                encode_suspension_call_plan(&mut writer, &repeated)
                    .expect("encode repeated storage");
                let bytes = writer.finish();
                assert_eq!(
                    decode_suspension_call_plan(&mut Reader::new(&bytes))
                        .expect("decode repeated storage"),
                    repeated
                );
                assert_ne!(
                    suspension_frontier_commitment(&table, &repeated),
                    suspension_frontier_commitment(&table, &plan)
                );
                let commitment = suspension_frontier_commitment(&table, &decoded);
                assert!(!encodings.contains(&encoded));
                assert!(!commitments.contains(&commitment));
                encodings.push(encoded);
                commitments.push(commitment);
            }
        }

        #[test]
        fn suspension_access_wire_rejects_accessless_module_format() {
            let mut encoded = super::super::super::MAGIC.to_vec();
            encoded.extend_from_slice(&130_u16.to_le_bytes());
            assert!(matches!(
                super::super::super::decode_module(&encoded),
                Err(CodecError::UnsupportedFormatMarker(130))
            ));
        }

        #[test]
        fn module_format_rejects_previous_layout() {
            let mut encoded = super::super::super::MAGIC.to_vec();
            let previous = super::super::super::FORMAT_MARKER - 1;
            encoded.extend_from_slice(&previous.to_le_bytes());
            assert!(matches!(
                super::super::super::decode_module(&encoded),
                Err(CodecError::UnsupportedFormatMarker(marker)) if marker == previous
            ));
        }

        #[test]
        fn suspension_loan_graph_round_trips_capture_and_dependency_identity() {
            let table = terminal_psi::TerminalIdentityTable::from(
                (0..64).map(|i| format!("identity-{i}")).collect::<Vec<_>>(),
            );
            use arena::HandleSpan;
            use terminal_psi::{
                TerminalSuspensionLoan, TerminalSuspensionLoanBacking,
                TerminalSuspensionLoanCapture, TerminalSuspensionLoanResource,
                TerminalSuspensionLoans, TerminalSuspensionValueLoans,
            };
            let selector = ValueId::new(17).expect("selector");
            let mut loans = TerminalSuspensionLoans::default();
            let path = loans
                .paths
                .insert_many([StructuralPathSegment::RuntimeIndex {
                    index: selector,
                    obligation: ObligationId::new(18).expect("bounds obligation"),
                }]);
            let parent = loans.occurrences.insert(TerminalSuspensionLoan {
                capture: TerminalSuspensionLoanCapture::Reference {
                    formation: OperationId::new(5).expect("formation"),
                    carrier: PlaceId::new(6).expect("carrier"),
                    source: PlaceId::new(7).expect("backing"),
                    source_access: StructuralAccess::Owned,
                    source_path: path,
                },
                backing: TerminalSuspensionLoanBacking::RetainedOwner { live_value: 0 },
                resource: TerminalSuspensionLoanResource::DirectRoot,
                ..Default::default()
            });
            let alternative = loans.occurrences.insert(TerminalSuspensionLoan {
                resource: TerminalSuspensionLoanResource::DirectRoot,
                ..Default::default()
            });
            let suppliers = loans.supplier_links.insert_many([parent, alternative]);
            let child = loans.occurrences.insert(TerminalSuspensionLoan {
                parent,
                suppliers,
                resource: TerminalSuspensionLoanResource::Reborrow,
                access: StructuralAccess::WriteOnlyBorrow,
                active_at_call: true,
                ..Default::default()
            });
            let links = loans.value_links.insert_many([child]);
            let plan = TerminalSuspensionCallPlan {
                operation: OperationId::new(1).expect("call"),
                crossing: SuspensionCrossingId::new(2).expect("crossing"),
                target: TerminalSuspensionCallTarget::Machine(MachineId::new(3).expect("machine")),
                live_value_count: 1,
                live_values: vec![TerminalSuspensionLiveValue {
                    place: TerminalSuspensionPlace::Structural(StructuralArgument {
                        place: PlaceId::new(7).expect("backing"),
                        access: StructuralAccess::Owned,
                        path: vec![],
                    }),
                    value_type: TerminalSuspensionValueType::Structural(
                        StructuralTypeId::new(8).expect("type"),
                    ),
                    storage: TerminalSuspensionStorage::Local,
                    claim_count: 0,
                    claims: vec![],
                    loans: TerminalSuspensionValueLoans::Retained { links },
                }],
                loans,
            };
            let mut writer = Writer::default();
            encode_suspension_call_plan(&mut writer, &plan).expect("encode graph");
            let bytes = writer.finish();
            let mut reader = Reader::new(&bytes);
            let decoded = decode_suspension_call_plan(&mut reader).expect("decode graph");
            assert_eq!(decoded, plan);
            assert_eq!(reader.remaining(), 0);
            let mut selectors = Vec::new();
            decoded.visit_value_ids(|value| selectors.push(value));
            assert_eq!(selectors, vec![selector]);
            let mut output_plan = plan.clone();
            let call = OperationId::new(11).expect("completed helper call");
            let argument = StructuralArgument {
                place: PlaceId::new(12).expect("original evaluated output argument"),
                path: vec![],
                access: StructuralAccess::MutableBorrow,
            };
            let input = output_plan
                .loans
                .occurrences
                .insert(TerminalSuspensionLoan {
                    capture: TerminalSuspensionLoanCapture::CallInput {
                        call,
                        argument: argument.clone(),
                        path,
                        stored: true,
                    },
                    backing: TerminalSuspensionLoanBacking::CapturedInput,
                    resource: TerminalSuspensionLoanResource::OutputInput,
                    ..Default::default()
                });
            let inputs = output_plan.loans.input_links.insert_many([input]);
            let output = output_plan
                .loans
                .occurrences
                .insert(TerminalSuspensionLoan {
                    capture: TerminalSuspensionLoanCapture::CompletedOutput {
                        call,
                        argument,
                        path: HandleSpan::empty(),
                    },
                    inputs,
                    resource: TerminalSuspensionLoanResource::CompletedOutput,
                    ..Default::default()
                });
            output_plan.loans.occurrences.get_mut(output).backing =
                TerminalSuspensionLoanBacking::CompletedOutput { loan: output };
            let debt = output_plan
                .loans
                .occurrences
                .insert(TerminalSuspensionLoan {
                    capture: TerminalSuspensionLoanCapture::InputDependencies,
                    backing: TerminalSuspensionLoanBacking::InputDependencies,
                    resource: TerminalSuspensionLoanResource::LifetimeDependency,
                    inputs,
                    ..Default::default()
                });
            let output_links = output_plan.loans.value_links.insert_many([output]);
            output_plan.live_values[0].loans = TerminalSuspensionValueLoans::Retained {
                links: output_links,
            };
            let encode = |plan: &TerminalSuspensionCallPlan| {
                let mut writer = Writer::default();
                encode_suspension_call_plan(&mut writer, plan)
                    .expect("encode completed output graph");
                writer.finish()
            };
            assert_eq!(
                decode_suspension_call_plan(&mut Reader::new(&encode(&output_plan)))
                    .expect("decode completed output graph"),
                output_plan
            );
            let mut ordinary_plan = output_plan.clone();
            let ordinary_suppliers = ordinary_plan.loans.supplier_links.insert_many([parent]);
            let mut ordinary_input = ordinary_plan.loans.occurrences.get(input).clone();
            ordinary_input.resource = TerminalSuspensionLoanResource::DescriptorTransfer;
            ordinary_input.suppliers = ordinary_suppliers;
            let ordinary_input = ordinary_plan.loans.occurrences.insert(ordinary_input);
            assert_eq!(
                decode_suspension_call_plan(&mut Reader::new(&encode(&ordinary_plan)))
                    .expect("decode retained ordinary call input"),
                ordinary_plan
            );
            for kind in [
                TerminalSuspensionLoanResource::DirectRoot,
                TerminalSuspensionLoanResource::Reborrow,
                TerminalSuspensionLoanResource::DescriptorTransfer,
            ] {
                let mut plan = ordinary_plan.clone();
                let input = plan.loans.occurrences.get_mut(ordinary_input);
                input.resource = kind;
                if kind == TerminalSuspensionLoanResource::Reborrow {
                    input.parent = parent;
                }
                assert_eq!(
                    decode_suspension_call_plan(&mut Reader::new(&encode(&plan)))
                        .expect("decode original incoming call input"),
                    plan
                );
                plan.loans
                    .occurrences
                    .get_mut(ordinary_input)
                    .active_at_call = true;
                assert!(decode_suspension_call_plan(&mut Reader::new(&encode(&plan))).is_err());
            }
            let mut wrong_capture = ordinary_plan.clone();
            wrong_capture
                .loans
                .occurrences
                .get_mut(ordinary_input)
                .capture = TerminalSuspensionLoanCapture::DescriptorTransfer;
            assert!(
                decode_suspension_call_plan(&mut Reader::new(&encode(&wrong_capture))).is_err()
            );
            ordinary_plan
                .loans
                .occurrences
                .get_mut(ordinary_input)
                .suppliers = HandleSpan::empty();
            assert!(
                decode_suspension_call_plan(&mut Reader::new(&encode(&ordinary_plan))).is_err()
            );
            let output_commitment = suspension_frontier_commitment(&table, &output_plan);
            for nonphysical in [input, debt] {
                let mut invalid = output_plan.clone();
                *invalid.loans.value_links.get_mut(output_links.start()) = nonphysical;
                assert!(decode_suspension_call_plan(&mut Reader::new(&encode(&invalid))).is_err());
                let mut invalid = output_plan.clone();
                invalid.loans.occurrences.get_mut(child).parent = nonphysical;
                assert!(decode_suspension_call_plan(&mut Reader::new(&encode(&invalid))).is_err());
            }
            let mut changed_input = output_plan.clone();
            let TerminalSuspensionLoanCapture::CallInput { stored, .. } =
                &mut changed_input.loans.occurrences.get_mut(input).capture
            else {
                unreachable!()
            };
            *stored = false;
            assert_ne!(
                suspension_frontier_commitment(&table, &changed_input),
                output_commitment
            );
            let mut absent_input = output_plan.clone();
            *absent_input.loans.input_links.get_mut(inputs.start()) = parent;
            assert!(decode_suspension_call_plan(&mut Reader::new(&encode(&absent_input))).is_err());
            let commitment = suspension_frontier_commitment(&table, &plan);
            let mut changed = plan.clone();
            changed.loans.occurrences.get_mut(child).suppliers = Default::default();
            assert_ne!(suspension_frontier_commitment(&table, &changed), commitment);
            changed = plan.clone();
            *changed.loans.supplier_links.get_mut(suppliers.start()) = alternative;
            assert_ne!(suspension_frontier_commitment(&table, &changed), commitment);
            changed = plan.clone();
            changed.loans.occurrences.get_mut(child).suppliers =
                HandleSpan::from_parts(suppliers.start(), suppliers.count() + 1);
            let mut writer = Writer::default();
            encode_suspension_call_plan(&mut writer, &changed)
                .expect("encode invalid supplier span");
            assert!(decode_suspension_call_plan(&mut Reader::new(&writer.finish())).is_err());
            changed = plan.clone();
            *changed.loans.supplier_links.get_mut(suppliers.start()) = Default::default();
            let mut writer = Writer::default();
            encode_suspension_call_plan(&mut writer, &changed).expect("encode absent supplier");
            assert!(decode_suspension_call_plan(&mut Reader::new(&writer.finish())).is_err());
            changed = plan.clone();
            changed.loans.occurrences.get_mut(parent).capture =
                TerminalSuspensionLoanCapture::Reference {
                    formation: OperationId::new(9).expect("different formation"),
                    carrier: PlaceId::new(6).expect("carrier"),
                    source: PlaceId::new(7).expect("backing"),
                    source_access: StructuralAccess::Owned,
                    source_path: HandleSpan::empty(),
                };
            assert_ne!(suspension_frontier_commitment(&table, &changed), commitment);
            let mut ingress = plan.clone();
            ingress.live_values[0].storage = TerminalSuspensionStorage::Parameter;
            let argument = StructuralArgument {
                place: PlaceId::new(7).expect("ingress place"),
                path: Vec::new(),
                access: StructuralAccess::MutableBorrow,
            };
            ingress.live_values[0].place = TerminalSuspensionPlace::Structural(argument.clone());
            *ingress.loans.occurrences.get_mut(child) = TerminalSuspensionLoan {
                capture: TerminalSuspensionLoanCapture::RetainedValue {
                    live_value: 0,
                    path: HandleSpan::empty(),
                },
                backing: TerminalSuspensionLoanBacking::IncomingReferent { argument },
                resource: TerminalSuspensionLoanResource::IncomingReference,
                access: StructuralAccess::MutableBorrow,
                active_at_call: true,
                ..Default::default()
            };
            let mut writer = Writer::default();
            encode_suspension_call_plan(&mut writer, &ingress).expect("encode ingress capture");
            assert_eq!(
                decode_suspension_call_plan(&mut Reader::new(&writer.finish()))
                    .expect("decode ingress capture"),
                ingress
            );
            assert_ne!(suspension_frontier_commitment(&table, &ingress), commitment);
            ingress.live_values[0].storage = TerminalSuspensionStorage::CallArgument;
            let mut writer = Writer::default();
            encode_suspension_call_plan(&mut writer, &ingress).expect("encode wrong ingress role");
            assert!(decode_suspension_call_plan(&mut Reader::new(&writer.finish())).is_err());
            ingress.live_values[0].storage = TerminalSuspensionStorage::Parameter;
            ingress.loans.occurrences.get_mut(child).backing =
                TerminalSuspensionLoanBacking::RetainedOwner { live_value: 0 };
            let mut writer = Writer::default();
            encode_suspension_call_plan(&mut writer, &ingress).expect("encode descriptor as owner");
            assert!(decode_suspension_call_plan(&mut Reader::new(&writer.finish())).is_err());

            changed = plan;
            changed.live_values[0].storage = TerminalSuspensionStorage::CallArgument;
            changed.loans.occurrences.get_mut(child).capture =
                TerminalSuspensionLoanCapture::RetainedValue {
                    live_value: 0,
                    path: HandleSpan::empty(),
                };
            let mut writer = Writer::default();
            encode_suspension_call_plan(&mut writer, &changed).expect("encode actual capture");
            let decoded = decode_suspension_call_plan(&mut Reader::new(&writer.finish()))
                .expect("decode actual capture");
            assert_eq!(decoded, changed);
            let actual_commitment = suspension_frontier_commitment(&table, &changed);
            changed.loans.occurrences.get_mut(child).capture =
                TerminalSuspensionLoanCapture::RetainedValue {
                    live_value: 1,
                    path: HandleSpan::empty(),
                };
            assert_ne!(
                suspension_frontier_commitment(&table, &changed),
                actual_commitment
            );
            let mut writer = Writer::default();
            encode_suspension_call_plan(&mut writer, &changed).expect("encode absent actual");
            assert!(decode_suspension_call_plan(&mut Reader::new(&writer.finish())).is_err());
            changed.loans.occurrences.get_mut(child).capture =
                TerminalSuspensionLoanCapture::RetainedValue {
                    live_value: 0,
                    path,
                };
            assert_ne!(
                suspension_frontier_commitment(&table, &changed),
                actual_commitment
            );
            changed.live_values[0].storage = TerminalSuspensionStorage::Local;
            let mut writer = Writer::default();
            encode_suspension_call_plan(&mut writer, &changed).expect("encode wrong storage role");
            assert!(decode_suspension_call_plan(&mut Reader::new(&writer.finish())).is_err());
            changed.live_values[0].loans = TerminalSuspensionValueLoans::Retained {
                links: HandleSpan::from_parts(links.start(), links.count() + 1),
            };
            let mut writer = Writer::default();
            encode_suspension_call_plan(&mut writer, &changed)
                .expect("encode invalid foreign coordinates");
            assert!(decode_suspension_call_plan(&mut Reader::new(&writer.finish())).is_err());
        }
    }
}
pub(crate) mod installed_interrupt_wire {
    use super::super::wire_fields::{Bytes, Digest, Id, List, Nominal, Strings, Text, wire_record};
    use terminal_psi::TerminalInstalledInterruptRegistration;

    wire_record! {
        Registration: TerminalInstalledInterruptRegistration {
            machine: Id,
            operation: Id,
            installer: Id,
            service: Id,
            registrar_boundary: Id,
            registrar: Nominal,
            registrar_owner: Nominal,
            registrar_application: Text,
            registrar_declaring_application: Text,
            registrar_arguments: Strings,
            registrar_lifetimes: Strings,
            registrar_requirement: Text,
            slot_declaration: Nominal,
            slot_commitment: Digest,
            slot_encoding: Bytes,
            callbacks: List<super::callback_application_wire::Application>,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::sections::semantic_module::wire::{Reader, Writer};
        use crate::sections::semantic_module::wire_fields::Field;
        use semantic_vocabulary::{BoundaryMachineId, MachineId, OperationId, ServiceId};

        #[test]
        fn installed_interrupt_wire_retains_machine_local_operation_and_rejects_truncation() {
            let nominal = terminal_psi::NominalDeclarationIdentity::new(
                terminal_psi::TerminalIdentityIndex(1),
            );
            let registration = TerminalInstalledInterruptRegistration {
                machine: MachineId::new(3).unwrap(),
                operation: OperationId::new(7).unwrap(),
                installer: BoundaryMachineId::new(1).unwrap(),
                service: ServiceId::new(1).unwrap(),
                registrar_boundary: BoundaryMachineId::new(2).unwrap(),
                registrar: nominal.clone(),
                registrar_owner: nominal.clone(),
                registrar_application: "Registrar".into(),
                registrar_declaring_application: "Registrar".into(),
                registrar_arguments: Vec::new(),
                registrar_lifetimes: Vec::new(),
                registrar_requirement: "Registrar::install".into(),
                slot_declaration: nominal,
                slot_commitment: [3; 32],
                slot_encoding: vec![1, 2],
                callbacks: Vec::new(),
            };
            let mut writer = Writer::default();
            Registration::write(&mut writer, &registration).unwrap();
            let bytes = writer.finish();
            let mut reader = Reader::new(&bytes);
            assert_eq!(Registration::read(&mut reader).unwrap(), registration);
            assert_eq!(reader.remaining(), 0);
            for length in 0..bytes.len() {
                assert!(Registration::read(&mut Reader::new(&bytes[..length])).is_err());
            }
            let mut another_owner = registration.clone();
            another_owner.machine = MachineId::new(4).unwrap();
            let mut writer = Writer::default();
            Registration::write(&mut writer, &another_owner).unwrap();
            let changed = writer.finish();
            assert_ne!(bytes, changed);
            assert_eq!(
                Registration::read(&mut Reader::new(&changed)).unwrap(),
                another_owner
            );
            let mut absent_owner = bytes;
            absent_owner[..8].fill(0);
            assert!(Registration::read(&mut Reader::new(&absent_owner)).is_err());
        }
    }
}
pub(crate) mod opaque_representation_wire {

    use super::super::wire_fields::{
        Bytes, Field, Id, Index, Text, U16, U64, wire_enum, wire_record,
    };
    use super::{CodecError, Reader, Writer};
    use application_identity::opaque_representations::OpaqueRepresentationApplicationOrigin;
    use application_identity::opaque_representations::OpaqueRepresentationCopyDisposition;
    use application_identity::opaque_representations::OpaqueRepresentationLifecycleDisposition;
    use application_identity::opaque_representations::SelectedOpaqueApplication;
    use terminal_psi::TerminalOpaqueRepresentation;

    wire_record! {
        Representation: TerminalOpaqueRepresentation {
            opaque: Id, carrier: Id,
            opaque_storage_identity: Index, carrier_storage_identity: Index,
            application: Application
        }
        Application: SelectedOpaqueApplication {
            opaque_identity: Text, carrier_identity: Text, conformance_identity: Text,
            application_report_fingerprint: U64, conformance_application_commitment: Digest,
            identity_encoding: Bytes, schema_version: U16,
            origin: Origin, lifecycle: Lifecycle, copy_disposition: CopyDisposition,
            selected_application_commitment: Digest,
        }
    }

    wire_enum! {
        Origin: OpaqueRepresentationApplicationOrigin, "opaque representation origin" { 1 => NamedConformance, }
        Lifecycle: OpaqueRepresentationLifecycleDisposition, "opaque representation lifecycle" { 1 => Inert, }
        CopyDisposition: OpaqueRepresentationCopyDisposition, "opaque representation copy disposition" {
            1 => PlacementOnly, 2 => CheckedSemanticCopy,
        }
    }

    struct Digest;
    impl Field<[u8; 32]> for Digest {
        fn write(writer: &mut Writer, value: &[u8; 32]) -> Result<(), CodecError> {
            writer.bytes(value);
            Ok(())
        }
        fn read(reader: &mut Reader<'_>) -> Result<[u8; 32], CodecError> {
            reader.array()
        }
    }
}
pub(crate) use psi_declaration_encoding::quotients as quotient_wire;
pub(crate) mod recursive_component_wire {

    use crate::sections::semantic_module::wire_fields::{
        Id, Index, List, Strings, U64, wire_enum, wire_record,
    };
    use terminal_psi::{
        TerminalProofRankingRelation, TerminalProofRecursiveCallSite,
        TerminalProofRecursiveComponent, TerminalProofRecursiveEdge, TerminalProofRecursiveField,
        TerminalProofRecursiveMember, TerminalProofRecursiveTransitionLane,
        TerminalProofRecursiveType,
    };

    wire_record! {
        Component: TerminalProofRecursiveComponent {
            ranking_relation: Ranking,
            rank_type_identity: Index,
            types: List<RecursiveType>,
            members: List<Member>,
            edges: List<Edge>,
        }
        RecursiveType: TerminalProofRecursiveType { identity: Index, fields: List<RecursiveField> }
        RecursiveField: TerminalProofRecursiveField { identity: Index, type_identity: Index }
        Member: TerminalProofRecursiveMember {
            contract: Id,
            machine_identity: Index,
            rank_parameter_identity: Index,
        }
        Edge: TerminalProofRecursiveEdge {
            caller: Id,
            callee: Id,
            site: CallSite,
            strict_member_path: Strings,
        }
    }

    wire_enum! {
        Ranking: TerminalProofRankingRelation, "TerminalProofRankingRelation" {
            1 => StructuralSubterm,
            2 => StructuralSubtermCycles,
        }
        CallSite: TerminalProofRecursiveCallSite, "TerminalProofRecursiveCallSite" {
            1 => Statement { state_identity: Index, statement_index: U64 },
            2 => Expression { state_identity: Index, statement_index: U64, expression_ordinal: U64 },
            3 => Transition { state_identity: Index, statement_index: U64, lane: Lane },
        }
        Lane: TerminalProofRecursiveTransitionLane, "TerminalProofRecursiveTransitionLane" {
            1 => Target,
            2 => Continuation,
        }
    }
}
pub(crate) mod unoptimized_behavior_wire {

    use super::super::wire_fields::{Id, InvocationSite, List, wire_enum, wire_record};
    use semantic_vocabulary::CrashCause;
    use terminal_psi::{UnoptimizedBehaviorOperation, UnoptimizedMachineBehavior};

    wire_record! {
        Behavior: UnoptimizedMachineBehavior { machine: Id, operations: List<Operation> }
    }

    wire_enum! {
        Operation: UnoptimizedBehaviorOperation, "unoptimized semantic operation" {
            0 => Invocation { block: Id, site: InvocationSite, callee: Id },
            1 => BoundaryInvocation { block: Id, site: InvocationSite, boundary: Id },
            2 => InstructionService { block: Id, operation: Id, service: Id },
            3 => Crash { block: Id, cause: Cause },
            4 => NominalCleanup { block: Id, callee: Id },
        }
        Cause: CrashCause, "semantic crash cause" {
            0 => Trap,
            1 => Abort,
        }
    }
}

pub(crate) use psi_declaration_encoding::access::inputs as access_view_wire;
pub(crate) use psi_declaration_encoding::access::lifecycles as access_lifecycle_wire;
pub(crate) use psi_declaration_encoding::access::views as access_view_plan_wire;
pub(crate) mod closed_conformance_wire {
    pub(crate) use psi_callable_encoding::signatures::ResultLifetime;

    use super::super::wire_fields::{
        Bytes, Commitment, Digest, Id, Index, List, Nominal, Optional, ReachArgument, Scalar,
        Strings, Text, U32, U64, wire_enum, wire_record,
    };
    use super::callable_wire::Callable;
    use terminal_psi::{
        ClosedConformanceApplication, ClosedConformanceCallableResult,
        ClosedConformanceParameterBinding, ClosedConformanceParameterKind,
        ClosedConformanceRealizationCallable, ClosedConformanceRow, ClosedConformanceSchema,
        ClosedConformanceSchemaKind, ClosedConformanceStaticApplication,
    };

    wire_record! {
        Application: ClosedConformanceApplication {
            owner: Id,
            declaration_identity: Index,
            telescope: List<Binding>,
            trait_identity: Index,
            trait_lifetime_arguments: Strings,
            trait_arguments: Strings,
            realization_callables: List<RealizationCallable>,
            schemas: List<Schema>,
            static_applications: List<StaticApplication>,
            rows: List<Row>,
            report_fingerprint: U64,
            commitment: Commitment,
        }
        Binding: ClosedConformanceParameterBinding { parameter: Text, kind: ParameterKind, argument: Text }
        RealizationCallable: ClosedConformanceRealizationCallable {
            source_callable_identity: Index,
            callable: Callable,
            result: CallableResult,
        }
        Schema: ClosedConformanceSchema {
            requirement_declaration: Nominal,
            declaring_trait_identity: Index,
            public_requirement_identity: Index,
            requirement_identity: Index,
            realization_identity: Index,
            definition: Bytes,
            kind: SchemaKind,
            lifetime_parameter_count: U32,
            parameter_kinds: Bytes,
            finite_tuples: Optional<List<List<Index>>>,
        }
        StaticApplication: ClosedConformanceStaticApplication {
            public_requirement_identity: Index,
            realization_identity: Index,
            source_callable_identity: Index,
            callable: Callable,
            application_commitment: Optional<Digest>,
            method_arguments: List<ReachArgument>,
        }
        Row: ClosedConformanceRow {
            requirement_declaration: Nominal,
            declaring_trait_identity: Index,
            public_requirement_identity: Index,
            family_tuple: List<Index>,
            requirement_identity: Index,
            realization_identity: Index,
            realization_callable_identity: Optional<Index>,
        }
    }

    wire_enum! {
        ParameterKind: ClosedConformanceParameterKind, "ClosedConformanceParameterKind" {
            1 => Lifetime,
            2 => Type,
            3 => Const,
            4 => Machine,
        }
        CallableResult: ClosedConformanceCallableResult, "ClosedConformanceCallableResult" {
            1 => Unit,
            2 => Scalar(scalar: Scalar),
            3 => Structural(identity: Id),
        }
        SchemaKind: ClosedConformanceSchemaKind, "ClosedConformanceSchemaKind" {
            1 => Nongeneric,
            2 => Finite,
            3 => Open,
        }
    }
}
pub(crate) use psi_callable_encoding::catalogs as callable_values_wire;
pub(crate) use psi_declaration_encoding::dispatch as dynamic_dispatch_wire;
pub(crate) use psi_declaration_encoding::domains as declaration_wire;

pub(crate) use psi_callable_encoding::evidence as evidence_wire;
pub(crate) use psi_declaration_encoding::terms::float_meaning as float_meaning_wire;
pub(crate) mod folded_constant_wire {
    use super::super::wire_fields::{
        Digest, Id, Index, List, Nominal, Optional, Text, wire_record,
    };
    use terminal_psi::{FoldedConstantCandidate, FoldedConstantClosure, FoldedConstantDependency};

    wire_record! {
        Closure: FoldedConstantClosure { target: Optional<Text>, dependencies: List<Dependency> }
        Dependency: FoldedConstantDependency {
            target: Text,
            requirement: Index,
            plan: Digest,
            candidate: Optional<Candidate>,
        }
        Candidate: FoldedConstantCandidate { boundary: Id, machine: Id, declaration: Nominal }
    }
}
pub(crate) mod native_layout_wire {

    use super::super::wire_fields::{
        Field, Id, Index, List, Optional, Text, U64, wire_enum, wire_record,
    };
    use super::super::{
        CodecError,
        wire::{Reader, Writer, decode_bytes},
    };
    use application_identity::callback_declarations::PrivateCallbackSlotSelection;
    use layout_plans::{
        IntegerInterpretation, LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport,
        NativeLayoutPlanReport, PrivateCallbackLayoutDemandReport,
    };
    use terminal_psi::{NativeLayoutCatalog, NativeLayoutFieldIdentity, NativeLayoutRealization};
    type NativeReport = NativeLayoutPlanReport<PrivateCallbackSlotSelection>;
    type PrivateDemand = PrivateCallbackLayoutDemandReport<PrivateCallbackSlotSelection>;

    pub(crate) struct Catalog;
    impl Field<NativeLayoutCatalog> for Catalog {
        fn write(writer: &mut Writer, value: &NativeLayoutCatalog) -> Result<(), CodecError> {
            writer.len("native layout records", value.records().count())?;
            for record in value.records() {
                Id::write(writer, &record.record)?;
                Index::write(writer, &record.data_identity)?;
                Index::write(writer, &record.policy_identity)?;
                Index::write(writer, &record.semantic_identity)?;
                let fields = value.fields(record).map_err(CodecError::NativeLayout)?;
                writer.len("native nominal fields", fields.len())?;
                for field in fields {
                    NominalField::write(writer, field)?;
                }
                let realizations = value
                    .realizations(record)
                    .map_err(CodecError::NativeLayout)?;
                writer.len("native layout realizations", realizations.len())?;
                for realization in realizations {
                    Realization::write(writer, realization)?;
                }
            }
            Ok(())
        }
        fn read(reader: &mut Reader<'_>) -> Result<NativeLayoutCatalog, CodecError> {
            let mut catalog = NativeLayoutCatalog::default();
            for _ in 0..reader.count()? {
                let record = Id::read(reader)?;
                let data = Index::read(reader)?;
                let policy = Index::read(reader)?;
                let semantic = Index::read(reader)?;
                let fields = List::<NominalField>::read(reader)?;
                let realizations = List::<Realization>::read(reader)?;
                catalog.insert(record, data, policy, semantic, fields, realizations);
            }
            Ok(catalog)
        }
    }

    wire_record! {
        NominalField: NativeLayoutFieldIdentity { field: Id, declaration_identity: Index }
        Realization: NativeLayoutRealization {
            target: Optional<super::target_layout_wire::Input>,
            report: Report,
        }
        Report: NativeReport {
            layout: Layout,
            private_callback_demands: List<Demand>,
        }
        Layout: LayoutPlanReport {
            schema_report_fingerprint: U64,
            entries: List<Entry>,
            offsets: Optional<List<U64>>,
            size: Optional<U64>,
            align: U64,
        }
        Entry: LayoutFieldEntryReport {
            field: Text,
            member_identity: Optional<U64>,
            placement: Placement,
        }
        Demand: PrivateDemand {
            slot_application: Selection,
            slot_identity: Text,
            layout_subject_identity: Text,
            callback_requirement_identity: Text,
            placements: List<Placement>,
        }
        Selection: PrivateCallbackSlotSelection {
            declaration_identity: Text,
            trait_identity: Text,
            semantic_identity: Text,
            layout_identity: Text,
            requirement_identity: Text,
            application_report_fingerprint: U64,
            application_commitment: Digest,
            application_encoding: Preimage,
        }
    }

    wire_enum! {
        Interpretation: IntegerInterpretation, "native integer interpretation" {
            0 => Signed,
            1 => Unsigned,
        }
        Placement: LayoutPlacementReport, "native layout placement" {
            0 => At { offset: U64 },
            1 => IntegerAt { offset: U64, stored_width: U64, interpretation: Interpretation },
            2 => Bits { container: U64, container_width: U64, destination_lsb: U64, source_lsb: U64, width: U64 },
        }
    }

    struct Digest;
    impl Field<[u8; 32]> for Digest {
        fn write(writer: &mut Writer, value: &[u8; 32]) -> Result<(), CodecError> {
            writer.bytes(value);
            Ok(())
        }
        fn read(reader: &mut Reader<'_>) -> Result<[u8; 32], CodecError> {
            reader.array()
        }
    }
    struct Preimage;
    impl Field<std::sync::Arc<[u8]>> for Preimage {
        fn write(writer: &mut Writer, value: &std::sync::Arc<[u8]>) -> Result<(), CodecError> {
            writer.len("Source conformance identity preimage", value.len())?;
            writer.bytes(value);
            Ok(())
        }
        fn read(reader: &mut Reader<'_>) -> Result<std::sync::Arc<[u8]>, CodecError> {
            Ok(decode_bytes(reader)?.into())
        }
    }
}
pub(crate) mod operation_crash_contract_wire {

    use super::super::wire_fields::{CrashRoutes, Id, wire_record};
    use terminal_psi::TerminalOperationCrashContract;

    wire_record! {
        Contract: TerminalOperationCrashContract {
            machine: Id,
            operation: Id,
            published_routes: CrashRoutes,
            crash_continuations: CrashRoutes,
        }
    }
}
pub(crate) mod proof_output_wire {
    pub(crate) use psi_callable_encoding::signatures::Dispatch;

    use super::super::wire_fields::{
        Id, Index, InvocationSite, List, Optional, Scalar, Text, U32, wire_enum, wire_record,
    };
    use terminal_psi::{
        EvidenceCallableTarget, ProofOutput, ProofOutputCall, ProofOutputEvidenceArgument,
        ProofOutputRuntimeCall, ProofOutputRuntimeResult,
    };

    wire_record! {
        Call: ProofOutputCall {
            caller: Id,
            ordinal: U32,
            target_identity: Index,
            static_requirement_dispatch: Optional<Dispatch>,
            runtime_result: Optional<RuntimeResult>,
            runtime_call: Optional<RuntimeCall>,
            evidence_arguments: List<EvidenceArgument>,
            outputs: List<Output>,
        }
        RuntimeCall: ProofOutputRuntimeCall { site: InvocationSite, target: CallableTarget }
        EvidenceArgument: ProofOutputEvidenceArgument {
            input_position: U32,
            callee_proposition: Id,
            source: Id,
            instantiated_proposition: Id,
        }
        Output: ProofOutput {
            output_position: U32,
            output_field: Text,
            callee_proposition: Id,
            callee_output: Optional<Id>,
            instantiated_proposition: Id,
            forwarded_input_position: Optional<U32>,
            output: Optional<Id>,
        }
    }

    wire_enum! {
        CallableTarget: EvidenceCallableTarget, "EvidenceCallableTarget" {
            1 => Machine(machine: Id),
            2 => Runtime { contract: Id, callee: Id },
        }
        RuntimeResult: ProofOutputRuntimeResult, "ProofOutputRuntimeResult" {
            0 => Unit,
            1 => Scalar(scalar: Scalar),
            2 => NoNormal,
            3 => Structural(structural_type: Id),
        }
    }
}
pub(crate) mod proof_scope_wire {
    use super::super::CodecError;
    use super::super::wire::{Reader, Writer, bounded_count, decode_counted};
    use super::super::wire_fields::{
        Custody, Field, Flag, Index, List, MachineDeclaration, Optional, ProviderPackage,
        ResultField, Strings, Text, U32, wire_record,
    };
    use super::boundary_application_wire::{decode_requirement, encode_requirement};
    use crate::term_table::{decode_rows, encode_rows};
    use arena::{Arena, HandleSpan};
    use proof_applications::{
        ProofApplicationCall, ProofApplicationMachine, ProofApplicationTargetMember,
        ProofCallGraph, ProofCallMachine, ProofDeclarationDependencies, ProofStaticBinding,
    };
    use semantic_vocabulary::compiler_evaluation::{
        CompilerEvaluationDeclaration, CompilerEvaluationEffects, DeclarationAuthorityRequest,
    };
    use terminal_psi::{
        DeclaredSynchronousBoundaryTarget, ProofApplicationScope, ProofCallableAssociation,
        ProofCallableHeader, ProofCheckedRecursiveCall, ProofCheckedRecursiveComponent,
        ProofDeclarationId, ProofScopeDeclaration,
    };

    struct DeclId;
    impl Field<ProofDeclarationId> for DeclId {
        fn write(writer: &mut Writer, value: &ProofDeclarationId) -> Result<(), CodecError> {
            writer.u32(value.0);
            Ok(())
        }
        fn read(reader: &mut Reader<'_>) -> Result<ProofDeclarationId, CodecError> {
            Ok(ProofDeclarationId(reader.u32()?))
        }
    }

    struct UsizeCount;
    impl Field<usize> for UsizeCount {
        fn write(writer: &mut Writer, value: &usize) -> Result<(), CodecError> {
            writer.len("callable type parameters", *value)
        }
        fn read(reader: &mut Reader<'_>) -> Result<usize, CodecError> {
            Ok(reader.count()? as usize)
        }
    }

    struct AuthorityRequest;
    impl Field<DeclarationAuthorityRequest> for AuthorityRequest {
        fn write(
            writer: &mut Writer,
            value: &DeclarationAuthorityRequest,
        ) -> Result<(), CodecError> {
            ProviderPackage::write(writer, &Some(value.requester))?;
            ProviderPackage::write(writer, &Some(value.owner))
        }
        fn read(reader: &mut Reader<'_>) -> Result<DeclarationAuthorityRequest, CodecError> {
            Ok(DeclarationAuthorityRequest {
                requester: ProviderPackage::read(reader)?.ok_or(
                    CodecError::MalformedStructuralFoundation(
                        "compiler-evaluation requester is absent",
                    ),
                )?,
                owner: ProviderPackage::read(reader)?.ok_or(
                    CodecError::MalformedStructuralFoundation(
                        "compiler-evaluation owner is absent",
                    ),
                )?,
            })
        }
    }

    #[cfg(test)]
    use terminal_psi::TerminalIdentityIndex;

    type ScopeTargetMember = ProofApplicationTargetMember<ProofDeclarationId>;

    wire_record! {
        EvalEffects: CompilerEvaluationEffects {
            services: Strings,
            may_suspend: Flag,
            may_block: Flag,
        }
        EvalDecl: CompilerEvaluationDeclaration {
            name: Text,
            custody: Custody,
            effects: EvalEffects,
            local_terminates: Flag,
            entry_requires: Flag,
            linear_carrier: Optional<Text>,
            invocation_refusal: Optional<Text>,
            selections: ResultField<List<AuthorityRequest>, Text>,
        }
        SyncTarget: DeclaredSynchronousBoundaryTarget {
            declaration: Index,
            package: ProviderPackage,
            application: Optional<Index>,
        }
        CallableHeader: ProofCallableHeader {
            declaration: DeclId,
            callable: ResultField<Index, Text>,
            package: ProviderPackage,
            source_target: Optional<Text>,
            product_eligible: Flag,
            supplies_checked_body: Flag,
            type_parameter_count: UsizeCount,
            compiler_evaluation: ResultField<EvalDecl, Text>,
            synchronous_invocations: ResultField<List<SyncTarget>, Text>,
        }
        TargetMember: ScopeTargetMember { target: Text, machine: DeclId }
        RecursiveCall: ProofCheckedRecursiveCall {
            caller: DeclId,
            call_ordinal: U32,
            callee: DeclId,
        }
    }

    fn references(
        writer: &mut Writer,
        rows: &Arena<ProofDeclarationId>,
        span: HandleSpan<ProofDeclarationId>,
    ) -> Result<(), CodecError> {
        let entries = rows
            .span(span)
            .ok_or(CodecError::MalformedStructuralFoundation(
                "proof scope lost its reference span",
            ))?;
        writer.counted("proof scope references", entries, DeclId::write)
    }

    fn read_references(
        reader: &mut Reader<'_>,
        rows: &mut Arena<ProofDeclarationId>,
    ) -> Result<HandleSpan<ProofDeclarationId>, CodecError> {
        Ok(rows.insert_many(List::<DeclId>::read(reader)?))
    }

    pub(crate) fn encode(
        writer: &mut Writer,
        scope: &ProofApplicationScope,
    ) -> Result<(), CodecError> {
        writer.len("proof scope declarations", scope.declarations.len())?;
        for (_, declaration) in scope.declarations.iter() {
            match declaration {
                ProofScopeDeclaration::Missing => {
                    return Err(CodecError::MalformedStructuralFoundation(
                        "proof scope declaration is absent",
                    ));
                }
                ProofScopeDeclaration::Nominal(identity) => {
                    writer.u8(1);
                    Index::write(writer, identity)?;
                }
                ProofScopeDeclaration::Callable(declaration) => {
                    writer.u8(2);
                    MachineDeclaration::write(writer, declaration)?;
                }
                ProofScopeDeclaration::Unavailable(reason) => {
                    writer.u8(3);
                    writer.string("unavailable proof declaration", reason)?;
                }
                ProofScopeDeclaration::Operator(requirement) => {
                    writer.u8(4);
                    encode_requirement(writer, requirement)?;
                }
            }
        }
        writer.len("proof scope callable headers", scope.callable_headers.len())?;
        for (_, header) in scope.callable_headers.iter() {
            CallableHeader::write(writer, header)?;
        }
        writer.len("proof scope callables", scope.callables.len())?;
        for (_, row) in scope.callables.iter() {
            let ProofCallableAssociation::Callable {
                declaration,
                callable,
            } = row
            else {
                return Err(CodecError::MalformedStructuralFoundation(
                    "proof scope callable association is absent",
                ));
            };
            DeclId::write(writer, declaration)?;
            super::callable_wire::encode_callable(writer, *callable);
        }
        let plan = &scope.plan;
        writer.boolean(plan.has_obligations);
        writer.boolean(plan.unscoped_recipe);
        writer.len("proof scope machines", plan.machines.len())?;
        for (_, row) in plan.machines.iter() {
            DeclId::write(writer, &row.machine)?;
            references(writer, &plan.symbols, row.states)?;
            references(writer, &plan.symbols, row.binders)?;
            references(writer, &plan.symbols, row.dynamic_realizations)?;
            Optional::<Text>::write(writer, &row.authored_target)?;
            Optional::<Text>::write(writer, &row.source_target)?;
            writer.boolean(row.family_interface);
            writer.boolean(row.product_selected);
            let members = plan.target_members.span(row.family_members).ok_or(
                CodecError::MalformedStructuralFoundation(
                    "proof scope lost its family member span",
                ),
            )?;
            writer.counted("proof scope target members", members, TargetMember::write)?;
        }
        writer.len(
            "proof scope declaration dependencies",
            plan.declaration_dependencies.len(),
        )?;
        for (_, row) in plan.declaration_dependencies.iter() {
            DeclId::write(writer, &row.declaration)?;
            references(writer, &plan.symbols, row.dependencies)?;
            references(writer, &plan.symbols, row.signatures)?;
        }
        references(writer, &plan.symbols, plan.declarations)?;
        references(writer, &plan.symbols, plan.obligation_owners)?;
        writer.len(
            "proof scope candidate providers",
            plan.provider_candidates.len(),
        )?;
        for (_, (requirement, realization)) in plan.provider_candidates.iter() {
            DeclId::write(writer, requirement)?;
            DeclId::write(writer, realization)?;
        }
        let graph = &scope.calls;
        writer.len("proof scope call summaries", graph.machines().len())?;
        for (_, row) in graph.machines().iter() {
            DeclId::write(writer, &row.machine)?;
            let calls =
                graph
                    .calls
                    .span(row.calls)
                    .ok_or(CodecError::MalformedStructuralFoundation(
                        "proof scope lost its call span",
                    ))?;
            writer.len("proof scope calls", calls.len())?;
            for call in calls {
                DeclId::write(writer, &call.binder)?;
                DeclId::write(writer, &call.callee)?;
                DeclId::write(writer, &call.state)?;
                DeclId::write(writer, &call.operator)?;
                bindings(writer, graph, call.arguments)?;
            }
            references(writer, &graph.cleanup, row.cleanup)?;
        }
        writer.len(
            "checked recursive components",
            scope.recursive_components.len(),
        )?;
        writer.len(
            "pending compiler-evaluation conditions",
            scope.pending_evaluation_conditions.len(),
        )?;
        for (_, owner) in scope.pending_evaluation_conditions.iter() {
            DeclId::write(writer, owner)?;
        }
        for (_, component) in scope.recursive_components.iter() {
            references(writer, &scope.recursive_members, component.members)?;
            let calls = scope.recursive_calls.span(component.calls).ok_or(
                CodecError::MalformedStructuralFoundation(
                    "checked recursive component lost its call span",
                ),
            )?;
            writer.counted("checked recursive calls", calls, RecursiveCall::write)?;
        }
        Ok(())
    }

    /// A call's static bindings form one postorder table (`crate::term_table`):
    /// each row is the parameter, the selected declaration and its argument
    /// count, then one handle per argument binding; the call's own bindings are
    /// the table's roots.
    fn bindings(
        writer: &mut Writer,
        graph: &ProofCallGraph<ProofDeclarationId>,
        span: HandleSpan<ProofStaticBinding<ProofDeclarationId>>,
    ) -> Result<(), CodecError> {
        let span_entries = |span| {
            graph
                .bindings
                .span(span)
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "proof scope lost its binding span",
                ))
        };
        encode_rows(
            writer,
            "proof scope static binding",
            span_entries(span)?,
            |entry: &ProofStaticBinding<ProofDeclarationId>, children| {
                children.extend(span_entries(entry.arguments)?);
                Ok(())
            },
            |entry, writer| {
                DeclId::write(writer, &entry.parameter)?;
                DeclId::write(writer, &entry.selected)?;
                writer.len(
                    "proof scope static bindings",
                    span_entries(entry.arguments)?.len(),
                )
            },
        )
    }

    fn read_bindings(
        reader: &mut Reader<'_>,
        graph: &mut ProofCallGraph<ProofDeclarationId>,
    ) -> Result<HandleSpan<ProofStaticBinding<ProofDeclarationId>>, CodecError> {
        let roots = decode_rows(
            reader,
            "proof scope static binding",
            12,
            |reader, children| {
                let parameter = DeclId::read(reader)?;
                let selected = DeclId::read(reader)?;
                let count = bounded_count(reader, 4)?;
                let arguments = children.take(reader, count)?;
                Ok(ProofStaticBinding {
                    parameter,
                    selected,
                    arguments: graph.bindings.insert_many(arguments),
                })
            },
        )?;
        Ok(graph.bindings.insert_many(roots))
    }

    pub(crate) fn decode(reader: &mut Reader<'_>) -> Result<ProofApplicationScope, CodecError> {
        let mut result = ProofApplicationScope::default();
        for _ in 0..reader.count()? {
            let declaration = match reader.u8()? {
                1 => ProofScopeDeclaration::Nominal(Index::read(reader)?),
                2 => ProofScopeDeclaration::Callable(MachineDeclaration::read(reader)?),
                3 => ProofScopeDeclaration::Unavailable(
                    reader.string("unavailable proof declaration")?,
                ),
                4 => ProofScopeDeclaration::Operator(decode_requirement(reader)?),
                tag => return Err(CodecError::InvalidTag("ProofScopeDeclaration", tag)),
            };
            result.declarations.insert(declaration);
        }
        for _ in 0..reader.count()? {
            result
                .callable_headers
                .insert(CallableHeader::read(reader)?);
        }
        for _ in 0..reader.count()? {
            result.callables.insert(ProofCallableAssociation::Callable {
                declaration: DeclId::read(reader)?,
                callable: super::callable_wire::decode_callable(reader)?,
            });
        }
        let plan = &mut result.plan;
        plan.has_obligations = reader.boolean()?;
        plan.unscoped_recipe = reader.boolean()?;
        for _ in 0..reader.count()? {
            let machine = DeclId::read(reader)?;
            let states = read_references(reader, &mut plan.symbols)?;
            let binders = read_references(reader, &mut plan.symbols)?;
            let dynamic_realizations = read_references(reader, &mut plan.symbols)?;
            let authored_target = Optional::<Text>::read(reader)?;
            let source_target = Optional::<Text>::read(reader)?;
            let family_interface = reader.boolean()?;
            let product_selected = reader.boolean()?;
            let family_members = plan
                .target_members
                .insert_many(List::<TargetMember>::read(reader)?);
            plan.machines.insert(ProofApplicationMachine {
                machine,
                states,
                binders,
                dynamic_realizations,
                authored_target,
                source_target,
                family_interface,
                product_selected,
                family_members,
            });
        }
        for _ in 0..reader.count()? {
            let declaration = DeclId::read(reader)?;
            let dependencies = read_references(reader, &mut plan.symbols)?;
            let signatures = read_references(reader, &mut plan.symbols)?;
            plan.declaration_dependencies
                .insert(ProofDeclarationDependencies {
                    declaration,
                    dependencies,
                    signatures,
                });
        }
        plan.declarations = read_references(reader, &mut plan.symbols)?;
        plan.obligation_owners = read_references(reader, &mut plan.symbols)?;
        for _ in 0..reader.count()? {
            plan.provider_candidates
                .insert((DeclId::read(reader)?, DeclId::read(reader)?));
        }
        let graph = &mut result.calls;
        for _ in 0..reader.count()? {
            let machine = DeclId::read(reader)?;
            let calls = decode_counted(reader, |reader| {
                Ok(ProofApplicationCall {
                    binder: DeclId::read(reader)?,
                    callee: DeclId::read(reader)?,
                    state: DeclId::read(reader)?,
                    operator: DeclId::read(reader)?,
                    arguments: read_bindings(reader, graph)?,
                })
            })?;
            let calls = graph.calls.insert_many(calls);
            let cleanup = read_references(reader, &mut graph.cleanup)?;
            graph.insert_machine(ProofCallMachine {
                machine,
                calls,
                cleanup,
            });
        }
        graph.index_machines();
        let component_count = reader.count()?;
        for _ in 0..reader.count()? {
            result
                .pending_evaluation_conditions
                .insert(DeclId::read(reader)?);
        }
        for _ in 0..component_count {
            let members = read_references(reader, &mut result.recursive_members)?;
            let calls = result
                .recursive_calls
                .insert_many(List::<RecursiveCall>::read(reader)?);
            result
                .recursive_components
                .insert(ProofCheckedRecursiveComponent { members, calls });
        }
        Ok(result)
    }

    #[cfg(test)]
    mod tests {
        use semantic_vocabulary::{BoundaryMachineId, MachineId, PackageKeyIdentity};

        use super::*;

        fn scope() -> ProofApplicationScope {
            let mut scope = ProofApplicationScope::default();
            for _ in 0..8 {
                scope
                    .declarations
                    .insert(ProofScopeDeclaration::Nominal(TerminalIdentityIndex(1)));
            }
            scope
                .declarations
                .insert(ProofScopeDeclaration::Unavailable(
                    "symbolic declaration".into(),
                ));
            for ordinal in [1, 2, 3, 4, 7, 8] {
                scope.plan.machines.insert(ProofApplicationMachine {
                    machine: ProofDeclarationId(ordinal),
                    authored_target: (ordinal == 1 || ordinal == 8).then(|| "macos_arm64".into()),
                    source_target: (ordinal == 1 || ordinal == 8).then(|| "macos_arm64".into()),
                    product_selected: ordinal != 8,
                    ..Default::default()
                });
            }
            let dependency = scope.plan.symbols.insert_many([ProofDeclarationId(9)]);
            scope
                .plan
                .declaration_dependencies
                .insert(ProofDeclarationDependencies {
                    declaration: ProofDeclarationId(4),
                    dependencies: dependency,
                    ..Default::default()
                });
            scope.plan.has_obligations = true;
            scope.plan.obligation_owners = scope.plan.symbols.insert_many([ProofDeclarationId(4)]);
            let nested = scope.calls.bindings.insert_many([ProofStaticBinding {
                parameter: ProofDeclarationId(6),
                selected: ProofDeclarationId(4),
                ..Default::default()
            }]);
            let bindings = scope.calls.bindings.insert_many([ProofStaticBinding {
                parameter: ProofDeclarationId(5),
                selected: ProofDeclarationId(3),
                arguments: nested,
            }]);
            for (owner, callee, binder, arguments) in [
                (1, 2, 0, bindings),
                (2, 0, 5, HandleSpan::default()),
                (3, 0, 6, HandleSpan::default()),
            ] {
                let calls = scope.calls.calls.insert_many([ProofApplicationCall {
                    callee: ProofDeclarationId(callee),
                    binder: ProofDeclarationId(binder),
                    arguments,
                    ..Default::default()
                }]);
                let cleanup = if owner == 1 {
                    scope.calls.cleanup.insert_many([ProofDeclarationId(7)])
                } else {
                    HandleSpan::default()
                };
                scope.calls.insert_machine(ProofCallMachine {
                    machine: ProofDeclarationId(owner),
                    calls,
                    cleanup,
                });
            }
            scope
        }

        #[test]
        fn proof_scope_round_trip_preserves_distinct_declarations_and_nested_bindings() {
            let mut scope = scope();
            for (declaration, callable) in [
                (
                    1,
                    terminal_psi::TargetMachineCallable::Machine(MachineId::new(41).unwrap()),
                ),
                (
                    2,
                    terminal_psi::TargetMachineCallable::Machine(MachineId::new(42).unwrap()),
                ),
                (
                    9,
                    terminal_psi::TargetMachineCallable::Boundary(
                        BoundaryMachineId::new(41).unwrap(),
                    ),
                ),
            ] {
                scope
                    .callables
                    .insert(terminal_psi::ProofCallableAssociation::Callable {
                        declaration: ProofDeclarationId(declaration),
                        callable,
                    });
            }
            scope.declarations.insert(ProofScopeDeclaration::Operator(
                terminal_psi::BoundaryOperatorRequirement::new(
                    terminal_psi::NominalDeclarationIdentity::new(TerminalIdentityIndex(1)),
                    "operator::compare(u64,u64)->bool".into(),
                    PackageKeyIdentity::from_digest([7; 32]),
                )
                .unwrap(),
            ));
            let mut writer = Writer::default();
            encode(&mut writer, &scope).unwrap();
            let bytes = writer.finish();
            let mut reader = Reader::new(&bytes);
            let decoded = decode(&mut reader).unwrap();
            assert_eq!(decoded.declarations, scope.declarations);
            assert_eq!(decoded.callables, scope.callables);
            assert_eq!(reader.remaining(), 0);
            let before = scope
                .applications()
                .membership("macos_arm64", false, &[], None);
            let after = decoded
                .applications()
                .membership("macos_arm64", false, &[], None);
            assert_eq!(before, after);
            let mut machines = after.0;
            machines.sort_unstable();
            assert_eq!(machines, [1, 2, 3, 4, 7].map(ProofDeclarationId));
            assert_eq!(after.1, [ProofDeclarationId(9)]);
            assert!(!machines.contains(&ProofDeclarationId(8)));
            let mut writer = Writer::default();
            encode(&mut writer, &decoded).unwrap();
            assert_eq!(writer.finish(), bytes);
            assert!(decode(&mut Reader::new(&bytes[..bytes.len() - 1])).is_err());
        }

        #[test]
        fn proof_scope_refuses_lost_spans_instead_of_erasing_edges() {
            let mut scope = scope();
            scope.plan.obligation_owners =
                HandleSpan::from_parts(arena::Handle::from_parts(u32::MAX, 1), 1);
            assert!(matches!(
                encode(&mut Writer::default(), &scope),
                Err(CodecError::MalformedStructuralFoundation(
                    "proof scope lost its reference span"
                ))
            ));
        }
    }
}
pub(crate) mod scalar_application_wire {
    use super::super::CodecError;
    use super::super::wire::{Reader, Writer};
    use super::super::wire_fields::{Field, Id, Scalar, U32, wire_enum};
    use super::callable_wire::Callable;
    use terminal_psi::{LayoutMeasurement, ScalarApplication, ScalarApplications};

    wire_enum! {
        Measurement: LayoutMeasurement, "LayoutMeasurement" {
            1 => Size,
            2 => Alignment,
        }
    }

    pub(crate) fn encode(
        writer: &mut Writer,
        applications: &ScalarApplications,
    ) -> Result<(), CodecError> {
        writer.len("scalar applications", applications.declarations().len())?;
        for application in applications.declarations() {
            match application {
                ScalarApplication::Missing => {
                    return Err(CodecError::MalformedStructuralFoundation(
                        "scalar application catalog contains the absent sentinel",
                    ));
                }
                ScalarApplication::LayoutObservation {
                    subject,
                    measurement,
                    semantics_version,
                    scalar_type,
                } => {
                    writer.u8(1);
                    Id::write(writer, subject)?;
                    Measurement::write(writer, measurement)?;
                    U32::write(writer, semantics_version)?;
                    Scalar::write(writer, scalar_type)?;
                }
                ScalarApplication::Evaluate { entry, scalar_type } => {
                    writer.u8(2);
                    Callable::write(writer, entry)?;
                    Scalar::write(writer, scalar_type)?;
                }
            }
        }
        super::scalar_condition_wire::encode(writer, &applications.conditions)?;
        super::proof_scope_wire::encode(writer, &applications.proof_scope)
    }

    pub(crate) fn decode(reader: &mut Reader<'_>) -> Result<ScalarApplications, CodecError> {
        let mut applications = ScalarApplications::default();
        for _ in 0..reader.count()? {
            let declaration = match reader.u8()? {
                1 => ScalarApplication::LayoutObservation {
                    subject: Id::read(reader)?,
                    measurement: Measurement::read(reader)?,
                    semantics_version: U32::read(reader)?,
                    scalar_type: Scalar::read(reader)?,
                },
                2 => ScalarApplication::Evaluate {
                    entry: Callable::read(reader)?,
                    scalar_type: Scalar::read(reader)?,
                },
                tag => return Err(CodecError::InvalidTag("ScalarApplication", tag)),
            };
            applications
                .insert(declaration)
                .map_err(CodecError::MalformedStructuralFoundation)?;
        }
        applications.conditions = super::scalar_condition_wire::decode(reader)?;
        applications.proof_scope = super::proof_scope_wire::decode(reader)?;
        Ok(applications)
    }
}
pub(crate) mod scalar_block_invariant_wire {
    use super::super::wire_fields::{Id, List, Proposition, wire_record};
    use terminal_psi::{ScalarBlockInvariant, ScalarBlockInvariantArrival};

    wire_record! {
        Invariant: ScalarBlockInvariant {
            machine: Id,
            header: Id,
            predicate: Proposition,
            arrivals: List<Arrival>,
        }
        Arrival: ScalarBlockInvariantArrival { edge: Id, obligation: Id }
    }
}
pub(crate) mod scalar_condition_wire {
    use super::super::CodecError;
    use super::super::wire::{Reader, Writer};
    use super::super::wire_fields::{Field, Id, List, Scalar, U64, wire_enum};
    use bignum::BigInt;
    use numerics::integer_bounds::IntegerBoundStep;
    use semantic_vocabulary::ScalarConditionRole;
    use terminal_psi::{
        ProofDeclarationId, ScalarCompletionCondition, ScalarConditionSubject, ScalarConditions,
        ScalarRangeCondition, ScalarRangeObservation, ScalarRangeRequirement,
    };

    struct DeclId;
    impl Field<ProofDeclarationId> for DeclId {
        fn write(writer: &mut Writer, value: &ProofDeclarationId) -> Result<(), CodecError> {
            writer.u32(value.0);
            Ok(())
        }
        fn read(reader: &mut Reader<'_>) -> Result<ProofDeclarationId, CodecError> {
            Ok(ProofDeclarationId(reader.u32()?))
        }
    }

    pub(crate) struct BigIntField;
    impl Field<BigInt> for BigIntField {
        fn write(writer: &mut Writer, value: &BigInt) -> Result<(), CodecError> {
            writer.boolean(value.is_negative());
            writer.len("bound magnitude words", value.magnitude_words().len())?;
            for word in value.magnitude_words() {
                writer.u64(*word);
            }
            Ok(())
        }
        fn read(reader: &mut Reader<'_>) -> Result<BigInt, CodecError> {
            let negative = reader.boolean()?;
            let words = List::<U64, 8>::read(reader)?;
            BigInt::from_canonical_words(negative, words)
                .map_err(CodecError::MalformedStructuralFoundation)
        }
    }

    #[cfg(test)]
    pub(crate) fn encode_integer(writer: &mut Writer, value: &BigInt) -> Result<(), CodecError> {
        BigIntField::write(writer, value)
    }

    #[cfg(test)]
    pub(crate) fn decode_integer(reader: &mut Reader<'_>) -> Result<BigInt, CodecError> {
        BigIntField::read(reader)
    }

    wire_enum! {
        Role: ScalarConditionRole, "ScalarConditionRole" {
            1 => Prerequisite,
            2 => Result,
        }
        Subject: ScalarConditionSubject, "ScalarConditionSubject" {
            1 => Module,
            2 => Declaration(declaration: DeclId),
            4 => Application(application: Id),
        }
        RangeReq: ScalarRangeRequirement, "ScalarRangeRequirement" {
            1 => Fixed { minimum: BigIntField, maximum: BigIntField },
            2 => AddressModel,
        }
    }

    #[cfg(test)]
    pub(crate) fn encode_role(writer: &mut Writer, role: ScalarConditionRole) {
        let _ = Role::write(writer, &role);
    }

    #[cfg(test)]
    pub(crate) fn encode_subject(writer: &mut Writer, subject: ScalarConditionSubject) {
        let _ = Subject::write(writer, &subject);
    }

    pub(crate) fn encode_observation(
        writer: &mut Writer,
        observation: &ScalarRangeObservation,
    ) -> Result<(), CodecError> {
        match observation {
            ScalarRangeObservation::Missing => Err(CodecError::MalformedStructuralFoundation(
                "scalar condition contains an absent observation",
            )),
            ScalarRangeObservation::Literal(value) => {
                writer.u8(1);
                BigIntField::write(writer, value)
            }
            ScalarRangeObservation::Application(application) => {
                writer.u8(2);
                Id::write(writer, application)
            }
            ScalarRangeObservation::EntryResult {
                application,
                machine,
                result,
                scalar_type,
            } => {
                writer.u8(3);
                Id::write(writer, application)?;
                Id::write(writer, machine)?;
                Id::write(writer, result)?;
                Scalar::write(writer, scalar_type)
            }
        }
    }

    pub(crate) fn decode_observation(
        reader: &mut Reader<'_>,
    ) -> Result<ScalarRangeObservation, CodecError> {
        Ok(match reader.u8()? {
            1 => ScalarRangeObservation::Literal(BigIntField::read(reader)?),
            2 => ScalarRangeObservation::Application(Id::read(reader)?),
            3 => ScalarRangeObservation::EntryResult {
                application: Id::read(reader)?,
                machine: Id::read(reader)?,
                result: Id::read(reader)?,
                scalar_type: Scalar::read(reader)?,
            },
            tag => return Err(CodecError::InvalidTag("ScalarRangeObservation", tag)),
        })
    }

    pub(crate) fn encode(
        writer: &mut Writer,
        conditions: &ScalarConditions,
    ) -> Result<(), CodecError> {
        writer.len("scalar range conditions", conditions.ranges().len())?;
        for condition in conditions.ranges() {
            let ScalarRangeCondition::Range {
                subject,
                role,
                requirement,
                ..
            } = condition
            else {
                return Err(CodecError::MalformedStructuralFoundation(
                    "scalar condition is absent",
                ));
            };
            Subject::write(writer, subject)?;
            Role::write(writer, role)?;
            RangeReq::write(writer, requirement)?;
            let steps = conditions
                .steps(condition)
                .map_err(CodecError::MalformedStructuralFoundation)?;
            writer.len("scalar bound steps", steps.len())?;
            for step in steps {
                match step {
                    IntegerBoundStep::Missing => {
                        return Err(CodecError::MalformedStructuralFoundation(
                            "scalar condition contains an absent step",
                        ));
                    }
                    IntegerBoundStep::Bounds { low, high } => {
                        writer.u8(1);
                        BigIntField::write(writer, low)?;
                        BigIntField::write(writer, high)?;
                    }
                    IntegerBoundStep::Observation(obs) => {
                        writer.u8(2);
                        encode_observation(writer, obs)?;
                    }
                    IntegerBoundStep::Add => writer.u8(3),
                    IntegerBoundStep::Subtract => writer.u8(4),
                    IntegerBoundStep::Multiply => writer.u8(5),
                    IntegerBoundStep::Union => writer.u8(6),
                    IntegerBoundStep::Interval => writer.u8(7),
                    IntegerBoundStep::Intersect => writer.u8(8),
                    IntegerBoundStep::Equal => writer.u8(9),
                    IntegerBoundStep::LessThan => writer.u8(10),
                }
            }
        }
        super::scalar_predicate_wire::encode(writer, conditions)?;
        writer.len(
            "scalar completion conditions",
            conditions.completions().len(),
        )?;
        for condition in conditions.completions() {
            let ScalarCompletionCondition::Complete {
                subject,
                application,
            } = condition
            else {
                return Err(CodecError::MalformedStructuralFoundation(
                    "scalar completion is absent",
                ));
            };
            Subject::write(writer, subject)?;
            Id::write(writer, application)?;
        }
        Ok(())
    }

    pub(crate) fn decode(reader: &mut Reader<'_>) -> Result<ScalarConditions, CodecError> {
        let mut conditions = ScalarConditions::default();
        for _ in 0..reader.count()? {
            let subject = Subject::read(reader)?;
            let role = Role::read(reader)?;
            let requirement = RangeReq::read(reader)?;
            let mut steps = Vec::new();
            for _ in 0..reader.count()? {
                steps.push(match reader.u8()? {
                    1 => IntegerBoundStep::Bounds {
                        low: BigIntField::read(reader)?,
                        high: BigIntField::read(reader)?,
                    },
                    2 => IntegerBoundStep::Observation(decode_observation(reader)?),
                    3 => IntegerBoundStep::Add,
                    4 => IntegerBoundStep::Subtract,
                    5 => IntegerBoundStep::Multiply,
                    6 => IntegerBoundStep::Union,
                    7 => IntegerBoundStep::Interval,
                    8 => IntegerBoundStep::Intersect,
                    9 => IntegerBoundStep::Equal,
                    10 => IntegerBoundStep::LessThan,
                    tag => return Err(CodecError::InvalidTag("IntegerBoundStep", tag)),
                });
            }
            conditions
                .retain_range(subject, role, requirement, steps)
                .map_err(CodecError::MalformedStructuralFoundation)?;
        }
        super::scalar_predicate_wire::decode(reader, &mut conditions)?;
        for _ in 0..reader.count()? {
            let subject = Subject::read(reader)?;
            let application = Id::read(reader)?;
            conditions.retain_completion(subject, application);
        }
        Ok(conditions)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use semantic_vocabulary::{
            IntegerSign, IntegerType, MachineId, ScalarApplicationId, ScalarConditionRole,
            ScalarType, ValueId,
        };
        use terminal_psi::ProofDeclarationId;

        #[test]
        fn scalar_conditions_round_trip_subjects_observations_and_bound_operations() {
            let application = ScalarApplicationId::new(3).unwrap();
            let machine = MachineId::new(7).unwrap();
            let entry_result = ScalarRangeObservation::EntryResult {
                application,
                machine,
                result: ValueId::new(11).unwrap(),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                ),
            };
            let wide = BigInt::from_u64(1).shl_bits(260);
            let mut steps = vec![IntegerBoundStep::Observation(
                ScalarRangeObservation::Application(application),
            )];
            for operation in [
                IntegerBoundStep::Add,
                IntegerBoundStep::Subtract,
                IntegerBoundStep::Multiply,
                IntegerBoundStep::Union,
                IntegerBoundStep::Interval,
                IntegerBoundStep::Intersect,
                IntegerBoundStep::Equal,
                IntegerBoundStep::LessThan,
            ] {
                steps.push(IntegerBoundStep::Observation(entry_result.clone()));
                steps.push(operation);
            }
            steps.extend([
                IntegerBoundStep::Bounds {
                    low: wide.negate(),
                    high: wide.clone(),
                },
                IntegerBoundStep::Intersect,
                IntegerBoundStep::Observation(ScalarRangeObservation::Literal(BigInt::from_u64(5))),
                IntegerBoundStep::Union,
            ]);
            let mut conditions = ScalarConditions::default();
            for subject in [
                ScalarConditionSubject::Module,
                ScalarConditionSubject::Declaration(ProofDeclarationId(7)),
                ScalarConditionSubject::Application(application),
            ] {
                conditions.retain_completion(subject, application);
                for (requirement, role) in [
                    ScalarRangeRequirement::AddressModel,
                    ScalarRangeRequirement::Fixed {
                        minimum: wide.negate(),
                        maximum: wide.clone(),
                    },
                ]
                .into_iter()
                .zip([
                    ScalarConditionRole::Prerequisite,
                    ScalarConditionRole::Result,
                ]) {
                    conditions
                        .retain_range(subject, role, requirement, steps.clone())
                        .unwrap();
                }
            }
            let mut writer = Writer::default();
            encode(&mut writer, &conditions).unwrap();
            let bytes = writer.finish();
            let mut reader = Reader::new(&bytes);
            let decoded = decode(&mut reader).unwrap();
            assert_eq!(decoded, conditions);
            for (condition, role) in decoded.ranges().iter().zip(
                [
                    ScalarConditionRole::Prerequisite,
                    ScalarConditionRole::Result,
                ]
                .into_iter()
                .cycle(),
            ) {
                assert!(
                    matches!(condition, ScalarRangeCondition::Range { role: retained, .. } if *retained == role)
                );
            }
            assert_eq!(reader.remaining(), 0);
            let mut writer = Writer::default();
            encode(&mut writer, &decoded).unwrap();
            assert_eq!(writer.finish(), bytes);
        }

        #[test]
        fn scalar_conditions_reject_semantic_marker_without_explicit_roles() {
            for marker in [236_u16, 237] {
                let mut bytes = super::super::super::MAGIC.to_vec();
                bytes.extend_from_slice(&marker.to_le_bytes());
                assert!(matches!(
                    super::super::super::decode_module(&bytes),
                    Err(CodecError::UnsupportedFormatMarker(actual)) if actual == marker
                ));
            }
        }

        #[test]
        fn scalar_conditions_reject_absent_or_unknown_roles() {
            for role in [0, 3, u8::MAX] {
                let mut writer = Writer::default();
                writer.u32(1);
                encode_subject(&mut writer, ScalarConditionSubject::Module);
                writer.u8(role);
                assert!(matches!(
                    decode(&mut Reader::new(&writer.finish())),
                    Err(CodecError::InvalidTag("ScalarConditionRole", rejected)) if rejected == role
                ));
            }
        }

        #[test]
        fn scalar_conditions_reject_malformed_bound_stack() {
            let mut writer = Writer::default();
            writer.u32(1);
            writer.u8(1);
            encode_role(
                &mut writer,
                semantic_vocabulary::ScalarConditionRole::Prerequisite,
            );
            writer.u8(2);
            writer.u32(1);
            writer.u8(3);
            assert!(matches!(
                decode(&mut Reader::new(&writer.finish())),
                Err(CodecError::MalformedStructuralFoundation(
                    "scalar condition has missing bound operands"
                ))
            ));
        }

        #[test]
        fn scalar_conditions_reject_noncanonical_and_truncated_integers() {
            for (negative, words) in [(true, vec![]), (false, vec![0]), (false, vec![9, 0])] {
                let mut writer = Writer::default();
                writer.boolean(negative);
                writer.len("words", words.len()).unwrap();
                for word in words {
                    writer.u64(word);
                }
                assert!(matches!(
                    decode_integer(&mut Reader::new(&writer.finish())),
                    Err(CodecError::MalformedStructuralFoundation(
                        "integer magnitude is not canonical"
                    ))
                ));
            }
            let mut writer = Writer::default();
            writer.boolean(false);
            writer.u32(u32::MAX);
            assert!(matches!(
                decode_integer(&mut Reader::new(&writer.finish())),
                Err(CodecError::UnexpectedEnd)
            ));
        }
    }
}
pub(crate) mod scalar_predicate_wire {
    use super::super::wire_fields::{Field, wire_enum};
    use super::super::{
        CodecError,
        wire::{Reader, Writer},
    };
    use super::scalar_condition_wire::{
        BigIntField, Role, Subject, decode_observation, encode_observation,
    };
    #[cfg(test)]
    use super::scalar_condition_wire::{encode_integer, encode_role, encode_subject};
    use arena::{Arena, Handle};
    use numerics::scalar_predicates::{PredicateNode, PredicateOperator, ScalarValue};
    use terminal_psi::{ScalarConditions, ScalarPredicateNode};

    wire_enum! {
        Operator: PredicateOperator, "PredicateOperator" {
            1 => And, 2 => Or, 3 => Equal, 4 => NotEqual,
            5 => Less, 6 => LessOrEqual, 7 => Greater, 8 => GreaterOrEqual,
        }
    }

    fn coordinate(
        references: &[Handle<ScalarPredicateNode>],
        reference: Handle<ScalarPredicateNode>,
    ) -> Result<u32, CodecError> {
        references
            .iter()
            .position(|candidate| *candidate == reference)
            .and_then(|ordinal| u32::try_from(ordinal).ok())
            .ok_or(CodecError::MalformedStructuralFoundation(
                "scalar predicate lost a preceding child",
            ))
    }

    fn reference(
        reader: &mut Reader<'_>,
        references: &[Handle<ScalarPredicateNode>],
    ) -> Result<Handle<ScalarPredicateNode>, CodecError> {
        references.get(reader.u32()? as usize).copied().ok_or(
            CodecError::MalformedStructuralFoundation("scalar predicate lost a preceding child"),
        )
    }

    pub(crate) fn encode(
        writer: &mut Writer,
        conditions: &ScalarConditions,
    ) -> Result<(), CodecError> {
        writer.len("scalar predicates", conditions.predicates().len())?;
        for condition in conditions.predicates() {
            Subject::write(writer, &condition.subject)?;
            Role::write(writer, &condition.role)?;
            writer.string("scalar predicate description", &condition.description)?;
            writer.len("scalar predicate nodes", condition.nodes().len())?;
            let mut references = Vec::new();
            for (handle, node) in condition.nodes().iter() {
                match &node.0 {
                    PredicateNode::Missing | PredicateNode::Value(ScalarValue::Unknown) => {
                        return Err(CodecError::MalformedStructuralFoundation(
                            "scalar predicate contains an absent value",
                        ));
                    }
                    PredicateNode::Value(ScalarValue::Integer(value)) => {
                        writer.u8(1);
                        BigIntField::write(writer, value)?;
                    }
                    PredicateNode::Value(ScalarValue::Boolean(value)) => {
                        writer.u8(2);
                        writer.boolean(*value);
                    }
                    PredicateNode::Leaf(observation) => {
                        writer.u8(3);
                        encode_observation(writer, observation)?;
                    }
                    PredicateNode::Forward(child) | PredicateNode::Not(child) => {
                        writer.u8(if matches!(node.0, PredicateNode::Forward(_)) {
                            4
                        } else {
                            5
                        });
                        writer.u32(coordinate(&references, *child)?);
                    }
                    PredicateNode::Binary {
                        operator,
                        left,
                        right,
                    } => {
                        writer.u8(6);
                        Operator::write(writer, operator)?;
                        writer.u32(coordinate(&references, *left)?);
                        writer.u32(coordinate(&references, *right)?);
                    }
                }
                references.push(handle);
            }
            writer.u32(coordinate(&references, condition.root())?);
        }
        Ok(())
    }

    pub(crate) fn decode(
        reader: &mut Reader<'_>,
        conditions: &mut ScalarConditions,
    ) -> Result<(), CodecError> {
        for _ in 0..reader.count()? {
            let subject = Subject::read(reader)?;
            let role = Role::read(reader)?;
            let description = reader.string("scalar predicate description")?;
            let mut nodes = Arena::new();
            let mut references = Vec::new();
            for _ in 0..reader.count()? {
                let node = match reader.u8()? {
                    1 => PredicateNode::Value(ScalarValue::Integer(BigIntField::read(reader)?)),
                    2 => PredicateNode::Value(ScalarValue::Boolean(reader.boolean()?)),
                    3 => PredicateNode::Leaf(decode_observation(reader)?),
                    4 => PredicateNode::Forward(reference(reader, &references)?),
                    5 => PredicateNode::Not(reference(reader, &references)?),
                    6 => PredicateNode::Binary {
                        operator: Operator::read(reader)?,
                        left: reference(reader, &references)?,
                        right: reference(reader, &references)?,
                    },
                    tag => return Err(CodecError::InvalidTag("ScalarPredicateNode", tag)),
                };
                references.push(nodes.insert(ScalarPredicateNode(node)));
            }
            let root = reference(reader, &references)?;
            conditions
                .retain_predicate(subject, role, description, nodes, root)
                .map_err(CodecError::MalformedStructuralFoundation)?;
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use bignum::BigInt;
        use semantic_vocabulary::{ScalarApplicationId, ScalarConditionRole};
        use terminal_psi::{ScalarConditionSubject, ScalarRangeObservation};

        #[test]
        fn scalar_predicate_round_trip_retains_both_equality_operands_and_short_circuiting() {
            let first = ScalarApplicationId::new(13).unwrap();
            let second = ScalarApplicationId::new(19).unwrap();
            let mut nodes = Arena::new();
            let left = nodes.insert(ScalarPredicateNode(PredicateNode::Leaf(
                ScalarRangeObservation::Application(first),
            )));
            let right = nodes.insert(ScalarPredicateNode(PredicateNode::Leaf(
                ScalarRangeObservation::Application(second),
            )));
            let equality = nodes.insert(ScalarPredicateNode(PredicateNode::Binary {
                operator: PredicateOperator::Equal,
                left,
                right,
            }));
            let mut conditions = ScalarConditions::default();
            conditions
                .retain_predicate(
                    ScalarConditionSubject::Module,
                    ScalarConditionRole::Result,
                    "both recipes agree".into(),
                    nodes,
                    equality,
                )
                .unwrap();
            let mut nodes = Arena::new();
            let known = nodes.insert(ScalarPredicateNode(PredicateNode::Value(
                ScalarValue::Boolean(true),
            )));
            let unknown = nodes.insert(ScalarPredicateNode(PredicateNode::Leaf(
                ScalarRangeObservation::Application(second),
            )));
            let bound = nodes.insert(ScalarPredicateNode(PredicateNode::Value(
                ScalarValue::Integer(BigInt::from_u64(24)),
            )));
            let comparison = nodes.insert(ScalarPredicateNode(PredicateNode::Binary {
                operator: PredicateOperator::Equal,
                left: unknown,
                right: bound,
            }));
            let root = nodes.insert(ScalarPredicateNode(PredicateNode::Binary {
                operator: PredicateOperator::Or,
                left: known,
                right: comparison,
            }));
            conditions
                .retain_predicate(
                    ScalarConditionSubject::Module,
                    ScalarConditionRole::Prerequisite,
                    "selected disjunction".into(),
                    nodes,
                    root,
                )
                .unwrap();
            let mut writer = Writer::default();
            encode(&mut writer, &conditions).unwrap();
            let bytes = writer.finish();
            drop(conditions);
            let mut decoded = ScalarConditions::default();
            let mut reader = Reader::new(&bytes);
            decode(&mut reader, &mut decoded).unwrap();
            assert_eq!(reader.remaining(), 0);
            assert_eq!(decoded.predicates()[0].role, ScalarConditionRole::Result);
            assert_eq!(
                decoded.predicates()[1].role,
                ScalarConditionRole::Prerequisite
            );
            assert_eq!(decoded.observations().count(), 3);
            for (second_value, expected) in [
                (Some(24), Some(true)),
                (Some(16), Some(false)),
                (None, None),
            ] {
                let mut observed = Vec::new();
                let result = decoded.predicates()[0].evaluate(&mut |observation| {
                    let ScalarRangeObservation::Application(application) = observation else {
                        panic!("expected application");
                    };
                    observed.push(application);
                    let value = if application == first {
                        Some(24)
                    } else {
                        second_value
                    }?;
                    Some(ScalarValue::Integer(BigInt::from_u64(value)))
                });
                assert_eq!(observed, vec![first, second]);
                assert_eq!(result, expected.map(ScalarValue::Boolean));
            }
            assert_eq!(
                decoded.predicates()[1].evaluate(&mut |_| panic!("skipped observation")),
                Some(ScalarValue::Boolean(true))
            );
            let mut writer = Writer::default();
            encode(&mut writer, &decoded).unwrap();
            assert_eq!(writer.finish(), bytes);
        }

        #[test]
        fn scalar_predicate_codec_rejects_forward_edges_and_missing_roots() {
            for forward in [false, true] {
                let mut writer = Writer::default();
                writer.u32(1);
                encode_subject(&mut writer, ScalarConditionSubject::Module);
                encode_role(
                    &mut writer,
                    semantic_vocabulary::ScalarConditionRole::Prerequisite,
                );
                writer.string("description", "malformed graph").unwrap();
                writer.u32(1);
                if forward {
                    writer.u8(4);
                    writer.u32(0);
                } else {
                    writer.u8(2);
                    writer.boolean(true);
                    writer.u32(1);
                }
                assert!(matches!(
                    decode(
                        &mut Reader::new(&writer.finish()),
                        &mut ScalarConditions::default()
                    ),
                    Err(CodecError::MalformedStructuralFoundation(
                        "scalar predicate lost a preceding child"
                    ))
                ));
            }
        }

        #[test]
        fn scalar_predicate_codec_rejects_ill_typed_skipped_operands() {
            let mut writer = Writer::default();
            writer.u32(1);
            encode_subject(&mut writer, ScalarConditionSubject::Module);
            encode_role(
                &mut writer,
                semantic_vocabulary::ScalarConditionRole::Prerequisite,
            );
            writer
                .string("description", "true or integer is not formed")
                .unwrap();
            writer.u32(3);
            writer.u8(2);
            writer.boolean(true);
            writer.u8(1);
            encode_integer(&mut writer, &BigInt::from_u64(7)).unwrap();
            writer.u8(6);
            writer.u8(2);
            writer.u32(0);
            writer.u32(1);
            writer.u32(2);
            assert!(matches!(
                decode(
                    &mut Reader::new(&writer.finish()),
                    &mut ScalarConditions::default()
                ),
                Err(CodecError::MalformedStructuralFoundation(
                    "scalar predicate operator has incompatible operand kinds"
                ))
            ));
        }
    }
}
pub(crate) mod callable_wire {
    use super::super::CodecError;
    use super::super::wire::{Reader, Writer};
    use super::super::wire_fields::{Field, Id, wire_enum};
    use terminal_psi::TargetMachineCallable;

    wire_enum! {
        Callable: TargetMachineCallable, "TargetMachineCallable" {
            1 => Machine(machine: Id),
            2 => Boundary(boundary: Id),
        }
    }

    pub(crate) fn encode_callable(writer: &mut Writer, callable: TargetMachineCallable) {
        let _ = Callable::write(writer, &callable);
    }

    pub(crate) fn decode_callable(
        reader: &mut Reader<'_>,
    ) -> Result<TargetMachineCallable, CodecError> {
        Callable::read(reader)
    }
}
pub(crate) mod target_layout_wire {
    use super::super::structural_type_wire::{
        decode_authored_field_storage, encode_authored_field_storage,
    };
    use super::super::wire_fields::{
        Field, Flag, Id, Index, List, Optional, Text, U16, U64, wire_enum, wire_record,
    };
    use super::super::{
        CodecError,
        wire::{Reader, Writer},
    };
    use semantic_vocabulary::{AuthoredFieldStorage, AuthoredRecordStorage};
    use terminal_psi::{
        LayoutApplicationId, TargetLayoutApplication, TargetLayoutFieldLayoutSource,
        TargetLayoutFieldStorage, TargetLayoutInput, TargetLayoutRealization,
        TargetLayoutSchemaArgument, TargetLayoutSchemaField, TargetLayoutSchemaFieldKind,
    };

    wire_record! {
        Application: TargetLayoutApplication {
            id: ApplicationId,
            record: Id,
            schema: Id,
            policy_identity: Index,
            plan_identity: Index,
            target_inputs: List<Input>,
            realizations: List<Realization>,
        }
        Input: TargetLayoutInput {
            target: Text,
            pointer_size: U64,
            pointer_alignment: U64,
            case_tag_size: U64,
            case_tag_alignment: U64,
            little_endian: Flag,
        }
        Realization: TargetLayoutRealization {
            schema_argument: SchemaArgument,
            target: Text,
            pointer_size: U64,
            pointer_alignment: U64,
            case_tag_size: U64,
            case_tag_alignment: U64,
            little_endian: Flag,
            storage: RecordStorage,
            fields: List<FieldStorage>,
        }
        RecordStorage: AuthoredRecordStorage { byte_size: U64, byte_alignment: U64 }
        FieldStorage: TargetLayoutFieldStorage { field: Id, storage: RequiredStorage }
        SchemaArgument: TargetLayoutSchemaArgument {
            abi_version: U16,
            fields: List<SchemaField>,
            retired_field_identities: List<U64>,
        }
        SchemaField: TargetLayoutSchemaField {
            record_field: Id,
            schema_field: Id,
            key: U64,
            identity: Optional<U64>,
            byte_size: U64,
            byte_alignment: U64,
            kind: SchemaFieldKind,
            layout_source: FieldLayoutSource,
        }
    }

    wire_enum! {
        SchemaFieldKind: TargetLayoutSchemaFieldKind, "TargetLayoutSchemaFieldKind" {
            0 => Scalar,
            1 => Nested,
            2 => Repeated,
        }
        FieldLayoutSource: TargetLayoutFieldLayoutSource, "TargetLayoutFieldLayoutSource" {
            0 => Default,
            1 => Scalar,
            2 => Explicit,
        }
    }

    struct ApplicationId;
    impl Field<LayoutApplicationId> for ApplicationId {
        fn write(writer: &mut Writer, value: &LayoutApplicationId) -> Result<(), CodecError> {
            writer.u32(value.0);
            Ok(())
        }
        fn read(reader: &mut Reader<'_>) -> Result<LayoutApplicationId, CodecError> {
            Ok(LayoutApplicationId(reader.u32()?))
        }
    }

    struct RequiredStorage;
    impl Field<AuthoredFieldStorage> for RequiredStorage {
        fn write(writer: &mut Writer, value: &AuthoredFieldStorage) -> Result<(), CodecError> {
            encode_authored_field_storage(writer, Some(value))
        }
        fn read(reader: &mut Reader<'_>) -> Result<AuthoredFieldStorage, CodecError> {
            decode_authored_field_storage(reader)?.ok_or(CodecError::MalformedStructuralFoundation(
                "layout alternative has absent field storage",
            ))
        }
    }
}
pub(crate) mod task_activation_wire {

    use super::super::CodecError;
    use super::super::wire::{Reader, Writer};
    use super::super::wire_fields::{
        Digest, Field, Flag, Id, Index, List, Referent, Scalar, StructuralType, TaskPackage, Text,
        U32, U64, wire_enum, wire_record,
    };
    use semantic_vocabulary::AuthoredRecordStorage;
    use terminal_psi::{
        LayoutApplicationId, TerminalTaskActivationDemand, TerminalTaskActivationDemands,
        TerminalTaskCarryStorage, TerminalTaskFrame, TerminalTaskFrameCall, TerminalTaskFrameState,
        TerminalTaskLayoutRealization, TerminalTaskLiveCarry, TerminalTaskResidentValue,
        TerminalTaskSignatureValue, TerminalTaskStartOperation, TerminalTaskSuspensionCrossing,
        TerminalTaskTargetLayout, TerminalTaskUnresolvedCall, TerminalTaskUnresolvedCallKind,
        TerminalTaskValueType,
    };

    wire_record! {
        Demands: TerminalTaskActivationDemands {
            demands: List<Demand>,
            structural_types: List<StructuralType>,
            structural_referents: List<Referent>,
            target_layout_applications: List<TargetLayout>,
        }
        Demand: TerminalTaskActivationDemand {
            operation: StartOperation,
            report_fingerprint: U64,
            requirement_callable_identity: Index,
            requirement_owner_path: Index,
            requirement_owner_package: TaskPackage,
            requirement_in_scope: Flag,
            authored_requirement_identity: Index,
            machine_callable_identity: Index,
            entry: Index,
            entry_report_fingerprint: U64,
            contract_report_fingerprint: U64,
            specialization_commitment: Digest,
            specialization_report_fingerprint: U64,
            may_suspend: Flag,
            may_block: Flag,
            cancellation_required: Flag,
            signature_arguments: List<SignatureValue>,
            signature_outcome: SignatureValue,
            root_frame: U64,
            frames: List<Frame>,
            crossings: List<Crossing>,
        }
        SignatureValue: TerminalTaskSignatureValue {
            value_type: ValueType,
            normalized_identity: Index,
            is_mutable: Flag,
        }
        Resident: TerminalTaskResidentValue { value_type: ValueType, normalized_identity: Index }
        LiveCarry: TerminalTaskLiveCarry {
            place: U64,
            value_type: U64,
            storage: CarryStorage,
            claims: List<U64>,
        }
        Crossing: TerminalTaskSuspensionCrossing {
            identity: U64,
            live_carry: List<LiveCarry>,
        }
        Frame: TerminalTaskFrame {
            identity: U64,
            machine_callable_identity: Index,
            entry: Index,
            states: List<FrameState>,
            calls: List<FrameCall>,
            unresolved_calls: List<UnresolvedCall>,
        }
        FrameState: TerminalTaskFrameState {
            state: Index,
            resident: List<Resident>,
            crossing_arguments: List<List<Resident>>,
        }
        FrameCall: TerminalTaskFrameCall { callee: U64 }
        UnresolvedCall: TerminalTaskUnresolvedCall {
            state: Index,
            statement_index: U32,
            call_ordinal: U32,
            kind: UnresolvedCallKind,
        }
        TargetLayout: TerminalTaskTargetLayout {
            id: ApplicationId,
            record: Id,
            realizations: List<LayoutRealization>,
        }
        LayoutRealization: TerminalTaskLayoutRealization { target: Text, storage: RecordStorage }
        RecordStorage: AuthoredRecordStorage { byte_size: U64, byte_alignment: U64 }
    }

    wire_enum! {
        ValueType: TerminalTaskValueType, "TerminalTaskValueType" {
            0 => Unit,
            1 => Scalar(scalar: Scalar),
            2 => Structural(identity: Id),
        }
        CarryStorage: TerminalTaskCarryStorage, "TerminalTaskCarryStorage" {
            1 => Persistent,
            2 => Parameter,
            3 => Local,
            4 => CallArgument,
        }
        UnresolvedCallKind: TerminalTaskUnresolvedCallKind, "TerminalTaskUnresolvedCallKind" {
            1 => UnresolvedTarget,
            2 => NonCheckedSupply,
        }
        StartOperation: TerminalTaskStartOperation, "TerminalTaskStartOperation" {
            1 => Start,
            2 => TryStart,
        }
    }

    struct ApplicationId;
    impl Field<LayoutApplicationId> for ApplicationId {
        fn write(writer: &mut Writer, value: &LayoutApplicationId) -> Result<(), CodecError> {
            writer.u32(value.0);
            Ok(())
        }
        fn read(reader: &mut Reader<'_>) -> Result<LayoutApplicationId, CodecError> {
            Ok(LayoutApplicationId(reader.u32()?))
        }
    }
}
use super::external_provider_wire;

use terminal_psi::{
    TerminalIdentityFormation, TerminalIdentityFormations, TerminalIdentityIndex,
    TerminalIdentityTable, TerminalIndexedTheorem, TerminalMathematicalCarrier, TerminalModule,
    TerminalRootServiceReach, VocabularyMarker,
};

use super::wire::{Reader, Writer, decode_bytes, strictly_increasing};
use super::wire_fields::{
    Field, Ids, List, MachineDeclaration, Optional, Referent, StructuralType, U32,
    delegated_fields, wire_record,
};
use super::{CodecError, FORMAT_MARKER, MAGIC};

delegated_fields! {
    ScalarQualifications: terminal_psi::ScalarQualificationCatalog
        => super::scalar_qualification_wire::encode, super::scalar_qualification_wire::decode;
    ScalarApplications: terminal_psi::ScalarApplications
        => scalar_application_wire::encode, scalar_application_wire::decode;
    AccessViewPlan: terminal_psi::TerminalAccessViewPlan
        => access_view_plan_wire::encode, access_view_plan_wire::decode;
    AccessLifecycle: terminal_psi::TerminalAccessLifecyclePlan
        => access_lifecycle_wire::encode, access_lifecycle_wire::decode;
    ExternalProvider: terminal_psi::ExternalProviderConformance
        => external_provider_wire::encode, external_provider_wire::decode;
    ProviderCandidate: terminal_psi::ProviderCandidateConformance
        => super::provider_candidate_wire::encode_provider_candidate,
           super::provider_candidate_wire::decode_provider_candidate;
    SuspensionPlan: terminal_psi::TerminalSuspensionCallPlan
        => suspension_wire::encode_suspension_call_plan,
           suspension_wire::decode_suspension_call_plan;
    Machine: terminal_psi::TerminalMachine
        => super::machine_wire::encode_machine, super::machine_wire::decode_machine;
    BoundaryDemand: terminal_psi::TerminalBoundaryApplicationDemand
        => boundary_application_wire::encode_demand, boundary_application_wire::decode_demand;
    BoundaryCandidate: terminal_psi::BoundaryApplicationCandidate
        => boundary_application_wire::encode_candidate, boundary_application_wire::decode_candidate;
}

// The saved module interleaves fields from several parts of the in-memory model.
// Use the same exhaustive pattern to borrow fields for writing and assemble them
// after reading. The field codecs below run in the order of the published format.
macro_rules! module_wire_record {
    ($model:ident $layout:tt; $($field:ident: $codec:ty,)*) => {
        struct Module;

        impl Field<$model> for Module {
            fn write(writer: &mut Writer, module: &$model) -> Result<(), CodecError> {
                let $model $layout = module;
                $(<$codec as Field<_>>::write(writer, $field)?;)*
                Ok(())
            }

            fn read(reader: &mut Reader<'_>) -> Result<$model, CodecError> {
                $(let $field = <$codec as Field<_>>::read(reader)?;)*
                Ok($model $layout)
            }
        }
    };
}

module_wire_record! {
    TerminalModule {
        vocabulary_marker,
        identities,
        entry,
        machines,
        types: terminal_psi::TypeDeclarations {
            scalar_qualifications,
            structural_types,
            structural_referents,
            structural_domains,
            structural_qualification_changes,
            opaque_representations,
            mathematical_carriers,
            identity_formations,
            quotients,
        },
        scalars: terminal_psi::ScalarDeclarations {
            scalar_applications,
            folded_constants,
            float_meaning_projections,
            float_meaning_equalities,
            scalar_predicate_definitions,
        },
        interfaces: terminal_psi::ProgramInterfaces {
            services,
            root_service_reach,
            boundary_machines,
            calling_signatures,
            calling_abi_signatures,
            calling_policy_requests,
            service_schemas,
            program_entry_contracts,
            provider_selections,
            external_providers,
            provider_candidates,
            provider_instance_requirements,
            boundary_application_candidates,
            callback_applications,
            installed_interrupt_registrations,
            boundary_application_demands,
        },
        access: terminal_psi::AccessDeclarations {
            access_view_inputs,
            access_view_plans,
            access_lifecycle_plans,
            reborrow_restored_call_uses,
        },
        execution: terminal_psi::ExecutionDeclarations {
            dynamic_dispatch,
            callables,
            unoptimized_behavior,
            declaration_contracts,
            suspension_call_plan_count,
            suspension_call_sites,
            suspension_call_plans,
            task_activation_demands,
        },
        proofs: terminal_psi::ProgramProofs {
            theorem_parameters,
            theorem_applications,
            theorem_citations,
            indexed_theorems,
            proposition_declarations,
            proposition_applications,
            evidence_terms,
            evidence_contract_lanes,
            proof_output_calls,
            proof_recursive_components,
            scalar_block_invariants,
            operation_crash_contracts,
            closed_conformance_applications,
        },
        layouts: terminal_psi::LayoutDeclarations {
            target_layout_applications,
            native_layouts,
        },
    };
    vocabulary_marker: Vocabulary,
    entry: Optional<callable_wire::Callable>,
    identities: Identities,
    theorem_parameters: List<theorem_parameter_wire::Parameter>,
    theorem_applications: List<theorem_parameter_wire::Application>,
    theorem_citations: List<theorem_parameter_wire::Citation>,
    indexed_theorems: List<IndexedTheorem>,
    mathematical_carriers: List<MathematicalCarrier>,
    identity_formations: IdentityFormations,
    scalar_qualifications: ScalarQualifications,
    quotients: quotient_wire::Catalog,
    folded_constants: folded_constant_wire::Closure,
    scalar_applications: ScalarApplications,
    structural_types: List<StructuralType>,
    structural_referents: List<Referent>,
    structural_domains: List<declaration_wire::Domain>,
    structural_qualification_changes: List<super::structural_result_wire::QualificationChange>,
    services: List<declaration_wire::Service>,
    root_service_reach: RootServiceReach,
    access_view_plans: List<AccessViewPlan>,
    access_lifecycle_plans: List<AccessLifecycle>,
    access_view_inputs: List<access_view_wire::Input>,
    reborrow_restored_call_uses: List<borrow_wire::RestoredCallUse>,
    boundary_machines: List<super::structural_signature_wire::BoundaryMachine>,
    calling_signatures: List<calling_signature_wire::Declaration>,
    calling_abi_signatures: List<calling_signature_wire::AbiDeclaration>,
    calling_policy_requests: List<calling_signature_wire::PolicyRequest>,
    service_schemas: List<crate::service_schema_wire::Schema>,
    program_entry_contracts: List<crate::service_schema_wire::EntryContract>,
    provider_selections: List<crate::service_schema_wire::ProviderSelection>,
    opaque_representations: List<opaque_representation_wire::Representation>,
    unoptimized_behavior: List<unoptimized_behavior_wire::Behavior>,
    external_providers: List<ExternalProvider>,
    provider_candidates: List<ProviderCandidate>,
    provider_instance_requirements: List<super::provider_candidate_wire::InstanceRequirements>,
    dynamic_dispatch: dynamic_dispatch_wire::Catalog,
    callables: callable_values_wire::Catalog,
    float_meaning_projections: List<float_meaning_wire::Projection>,
    float_meaning_equalities: List<float_meaning_wire::Equality>,
    proposition_declarations: List<super::proof_declaration_wire::Declaration>,
    scalar_predicate_definitions: List<super::proof_declaration_wire::ScalarPredicate>,
    proposition_applications: List<super::proof_declaration_wire::Application>,
    evidence_terms: List<evidence_wire::Term>,
    evidence_contract_lanes: List<evidence_wire::Lane>,
    proof_output_calls: List<proof_output_wire::Call>,
    proof_recursive_components: List<recursive_component_wire::Component>,
    closed_conformance_applications: List<closed_conformance_wire::Application>,
    suspension_call_plan_count: U32,
    suspension_call_sites: List<suspension_wire::CallSite>,
    suspension_call_plans: List<SuspensionPlan>,
    scalar_block_invariants: List<scalar_block_invariant_wire::Invariant>,
    operation_crash_contracts: List<operation_crash_contract_wire::Contract>,
    declaration_contracts: List<super::contract_wire::Contract>,
    machines: List<Machine>,
    target_layout_applications: List<target_layout_wire::Application>,
    native_layouts: native_layout_wire::Catalog,
    boundary_application_demands: List<BoundaryDemand>,
    callback_applications: List<callback_application_wire::Application>,
    installed_interrupt_registrations: List<installed_interrupt_wire::Registration>,
    boundary_application_candidates: List<BoundaryCandidate>,
    task_activation_demands: task_activation_wire::Demands,
}

wire_record! {
    RootServiceReach: TerminalRootServiceReach {
        concrete: Ids,
        installation_dependencies: List<declaration_wire::ReachDependency>,
    }
}

struct Vocabulary;
impl Field<VocabularyMarker> for Vocabulary {
    fn write(writer: &mut Writer, value: &VocabularyMarker) -> Result<(), CodecError> {
        writer.u16(value.get());
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<VocabularyMarker, CodecError> {
        let raw = reader.u16()?;
        if raw != VocabularyMarker::CURRENT.get() {
            return Err(CodecError::UnsupportedVocabularyMarker(raw));
        }
        Ok(VocabularyMarker::CURRENT)
    }
}

struct Identities;
impl Field<TerminalIdentityTable> for Identities {
    fn write(writer: &mut Writer, value: &TerminalIdentityTable) -> Result<(), CodecError> {
        writer.strings("identity", value.entries())
    }
    fn read(reader: &mut Reader<'_>) -> Result<TerminalIdentityTable, CodecError> {
        let entries = super::wire::decode_counted(reader, |reader| reader.string("identity"))?;
        if entries.first().map(String::as_str) != Some("") {
            return Err(CodecError::MalformedStructuralFoundation(
                "identity table must begin with the empty identity",
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for identity in &entries[1..] {
            if identity.is_empty() || !seen.insert(identity.as_str()) {
                return Err(CodecError::MalformedStructuralFoundation(
                    "identity table entries must be non-empty and unique",
                ));
            }
        }
        Ok(TerminalIdentityTable::from(entries))
    }
}

struct IndexedTheorem;
impl Field<TerminalIndexedTheorem> for IndexedTheorem {
    fn write(writer: &mut Writer, value: &TerminalIndexedTheorem) -> Result<(), CodecError> {
        MachineDeclaration::write(writer, &value.source_contract)?;
        writer.u32(value.ensure_ordinal);
        writer.u32(value.evidence_source_position);
        writer.u32(value.type_identity.0);
        writer.len("indexed theorem certificate", value.certificate.len())?;
        writer.bytes(&value.certificate);
        List::<U32>::write(writer, &value.assumption_closure)
    }
    fn read(reader: &mut Reader<'_>) -> Result<TerminalIndexedTheorem, CodecError> {
        let source_contract = MachineDeclaration::read(reader)?;
        let ensure_ordinal = reader.u32()?;
        let evidence_source_position = reader.u32()?;
        let type_identity = TerminalIdentityIndex(reader.u32()?);
        let certificate = decode_bytes(reader)?;
        super::mathematical_certificate_wire::decode_mathematical_certificate(&certificate)?;
        let assumption_closure = List::<U32>::read(reader)?;
        if !strictly_increasing(assumption_closure.iter().copied()) || type_identity.is_absent() {
            return Err(CodecError::MalformedMathematicalCertificate(
                "invalid indexed theorem root",
            ));
        }
        Ok(TerminalIndexedTheorem {
            source_contract,
            ensure_ordinal,
            evidence_source_position,
            type_identity,
            certificate,
            assumption_closure,
        })
    }
}

struct MathematicalCarrier;
impl Field<TerminalMathematicalCarrier> for MathematicalCarrier {
    fn write(writer: &mut Writer, value: &TerminalMathematicalCarrier) -> Result<(), CodecError> {
        writer.u32(value.type_identity.0);
        writer.len("mathematical carrier key", value.carrier_key.len())?;
        writer.bytes(&value.carrier_key);
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<TerminalMathematicalCarrier, CodecError> {
        let type_identity = TerminalIdentityIndex(reader.u32()?);
        let carrier_key = decode_bytes(reader)?;
        let decoded =
            super::mathematical_certificate_wire::decode_mathematical_certificate(&carrier_key)?;
        if !decoded
            .arena
            .structurally_equal(decoded.certificate.term, decoded.certificate.expected)
            || !decoded.certificate.context.is_empty()
            || decoded.certificate.level_arity != 0
            || type_identity.is_absent()
        {
            return Err(CodecError::MalformedMathematicalCertificate(
                "invalid mathematical carrier key",
            ));
        }
        Ok(TerminalMathematicalCarrier {
            type_identity,
            carrier_key,
        })
    }
}

wire_record! {
    IdentityFormation: TerminalIdentityFormation {
        source_owner: super::wire_fields::Index,
        source_carrier: super::wire_fields::Index,
        declaration: U32,
        subject_origins: List<(U32, U32)>,
        subject_carriers: List<super::wire_fields::Index>,
    }
    IdentityFormations: TerminalIdentityFormations {
        rows: List<IdentityFormation>,
        certificate: super::wire_fields::Bytes,
    }
}

#[cfg(any(test, feature = "test-support"))]
pub fn encode_raw(module: &TerminalModule) -> Result<Vec<u8>, CodecError> {
    encode_raw_closed(module).map(|(bytes, _)| bytes)
}

pub(crate) fn encode_raw_closed(
    module: &TerminalModule,
) -> Result<(Vec<u8>, crate::ClosedTypeCatalog), CodecError> {
    encode_raw_with_writer(module, Writer::default(), MAGIC)
}

pub(crate) fn encode_invocation_prefix(
    module: &TerminalModule,
) -> Result<(Vec<u8>, crate::ClosedTypeCatalog), CodecError> {
    encode_raw_with_writer(module, Writer::invocation_prefix(), b"PSIPREFIX\0")
}

fn encode_raw_with_writer(
    module: &TerminalModule,
    mut writer: Writer,
    format: &[u8],
) -> Result<(Vec<u8>, crate::ClosedTypeCatalog), CodecError> {
    writer.bytes(format);
    writer.u16(FORMAT_MARKER);
    Module::write(&mut writer, module)?;
    let (bytes, integers) = writer.finish_with_integer_types();
    Ok((
        bytes,
        crate::ClosedTypeCatalog::from_encoded_integers(integers),
    ))
}

pub(crate) fn decode_module_body(reader: &mut Reader<'_>) -> Result<TerminalModule, CodecError> {
    Module::read(reader)
}
