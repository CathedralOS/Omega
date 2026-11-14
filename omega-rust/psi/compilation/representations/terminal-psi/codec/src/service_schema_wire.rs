//! Encode the complete normalized service surface beside its checked program.
//! Identity-table order and every contract field survive a round trip.

use crate::codec_error::CodecError;
use crate::module_wire::carry_and_suspension_wire::Carry;
use crate::semantic_module::structural_signature_wire::Completion;
use crate::wire_fields::wire::{Reader, Writer, decode_counted};
use crate::wire_fields::{
    BoundaryPackage, Custody, Digest, Field, Flag, Id, Index, List, Optional, Text, wire_enum,
    wire_record,
};
use language_core::sealed_intrinsics::{FixedImageOperation, HostedOperation};
use language_semantics::DomainPredicatePresence;
use language_semantics::declaration_selection::CompositionMode;
use language_semantics::service_surface::identities::{ProviderIdentityId, ProviderIdentityTable};
use language_semantics::service_surface::{
    ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceMethod, ServiceProgressEstablishmentRoute,
    ServiceProgressEstablishmentRouteKind, ServiceProgressPremise, ServiceProgressSubject,
    ServiceReceiverApplication, ServiceResultClaim, ServiceSurface,
};
use std::sync::Arc;
use terminal_psi::{
    TerminalProgramEntryContract, TerminalProviderSelection, TerminalSealedNativeIntrinsic,
    TerminalServiceSchema,
};

struct ProviderIdentity;
impl Field<ProviderIdentityId> for ProviderIdentity {
    fn write(writer: &mut Writer, value: &ProviderIdentityId) -> Result<(), CodecError> {
        writer.u32(value.index());
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<ProviderIdentityId, CodecError> {
        Ok(ProviderIdentityId::from_index(reader.u32()?))
    }
}

struct Count;
impl Field<usize> for Count {
    fn write(writer: &mut Writer, value: &usize) -> Result<(), CodecError> {
        writer.len("service parameter count", *value)
    }
    fn read(reader: &mut Reader<'_>) -> Result<usize, CodecError> {
        usize::try_from(reader.u32()?)
            .map_err(|_| CodecError::CollectionTooLong("service parameter count"))
    }
}

struct Identities;
impl Field<ProviderIdentityTable> for Identities {
    fn write(writer: &mut Writer, value: &ProviderIdentityTable) -> Result<(), CodecError> {
        writer.len("service identities", value.texts().count())?;
        for text in value.texts() {
            writer.string("service identity", text)?;
        }
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<ProviderIdentityTable, CodecError> {
        let texts = decode_counted(reader, |reader| reader.string("service identity"))?;
        let mut table = ProviderIdentityTable::default();
        for text in texts {
            if table.id_at(&text).is_some() {
                return Err(CodecError::NonCanonicalOrder("repeated service identity"));
            }
            table.intern(&text);
        }
        Ok(table)
    }
}

wire_enum! {
    Composition: CompositionMode, "CompositionMode" {
        1 => Fused,
        2 => Independent,
    }
    SealedIntrinsic: TerminalSealedNativeIntrinsic, "TerminalSealedNativeIntrinsic" {
        1 => Hosted(operation: Hosted),
        2 => FixedImage(operation: FixedImage),
    }
    Hosted: HostedOperation, "HostedOperation" {
        1 => ConsoleExitProcess,
        2 => ConsoleWriteByte,
        3 => ConsoleReadByte,
        4 => ProcessExit,
    }
    FixedImage: FixedImageOperation, "FixedImageOperation" {
        1 => ProgramStorageEntry,
        2 => ProgramImageRetainedSuffixStart,
    }
    PredicatePresence: DomainPredicatePresence, "DomainPredicatePresence" {
        1 => Absent,
        2 => Present,
    }
    AuthorityFlow: ServiceEntryAuthorityFlow, "ServiceEntryAuthorityFlow" {
        1 => Accepts,
    }
    ProgressKind: ServiceProgressEstablishmentRouteKind, "ServiceProgressEstablishmentRouteKind" {
        1 => CheckedRequirement,
        2 => BoundaryRequirement,
    }
    ProgressSubject: ServiceProgressSubject, "ServiceProgressSubject" {
        1 => Parameter(index: Count),
    }
}

wire_record! {
    Receiver: ServiceReceiverApplication {
        application: ProviderIdentity,
        trait_name: ProviderIdentity,
        trait_package_identity: BoundaryPackage,
        argument_identities: List<ProviderIdentity>,
        lifetime_argument_identities: List<ProviderIdentity>,
        requirement_identities: List<ProviderIdentity>,
    }
    EntryClaim: ServiceEntryClaim {
        parameter_index: Count,
        carrier_identity: ProviderIdentity,
        domain: Text,
        predicate_presence: PredicatePresence,
        effective_carry: Carry,
        authority_flow: AuthorityFlow,
    }
    ResultClaim: ServiceResultClaim {
        domain: Text,
        effective_carry: Carry,
    }
    ProgressRoute: ServiceProgressEstablishmentRoute {
        kind: ProgressKind,
        requirement_identity: ProviderIdentity,
    }
    ProgressPremise: ServiceProgressPremise {
        profile: Text,
        subject: ProgressSubject,
        subject_projections: List<Text>,
        establishment_routes: List<ProgressRoute>,
    }
    Method: ServiceMethod {
        completion: Completion,
        name: Text,
        requirement_owner: Text,
        requirement_owner_package_identity: BoundaryPackage,
        requirement_identity: ProviderIdentity,
        parameter_count: Count,
        parameter_type_identities: List<ProviderIdentity>,
        entry_claims: List<EntryClaim>,
        has_result: Flag,
        result_type_identity: ProviderIdentity,
        result_claims: List<ResultClaim>,
        service_reach: List<Text>,
        synchronous_invocations: List<Text>,
        may_suspend: Flag,
        may_block: Flag,
        terminates_guarantee: Flag,
        termination_premises: List<ProgressPremise>,
    }
    Surface: ServiceSurface {
        trait_name: ProviderIdentity,
        identities: Identities,
        trait_package_identity: BoundaryPackage,
        receiver_applications: List<Receiver>,
        methods: List<Method>,
    }
}

struct SharedSurface;
impl Field<Arc<ServiceSurface>> for SharedSurface {
    fn write(writer: &mut Writer, value: &Arc<ServiceSurface>) -> Result<(), CodecError> {
        Surface::write(writer, value)
    }
    fn read(reader: &mut Reader<'_>) -> Result<Arc<ServiceSurface>, CodecError> {
        Ok(Arc::new(Surface::read(reader)?))
    }
}

wire_record! {
    Schema: TerminalServiceSchema {
        declaration: Index,
        application: Index,
        surface: SharedSurface,
    }
    EntryContract: TerminalProgramEntryContract {
        target: Text,
        entry: Id,
        schema_declaration: Index,
        schema_application: Index,
        physical_source_digest: Digest,
    }
    ProviderSelection: TerminalProviderSelection {
        target: Text,
        composition_mode: Composition,
        schema_declaration: Index,
        schema_application: Index,
        provider_declaration: Index,
        provider_identity: Index,
        provider_application: Optional<Index>,
        provider_package: BoundaryPackage,
        provider_custody: Custody,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use language_semantics::CarryPolicy;
    use terminal_psi::{TerminalIdentityTableBuilder, TerminalModule};

    #[test]
    fn complete_service_contract_and_source_choices_survive_transport() {
        let mut identities = ProviderIdentityTable::default();
        let owner = identities.intern("Service");
        let application = identities.intern("applied(Service)");
        let requirement = identities.intern("Service::exchange");
        let scalar = identities.intern("named(u32)");
        let surface = Arc::new(ServiceSurface {
            trait_name: owner,
            identities,
            trait_package_identity: None,
            receiver_applications: vec![ServiceReceiverApplication {
                application,
                trait_name: owner,
                trait_package_identity: None,
                argument_identities: vec![scalar],
                lifetime_argument_identities: Vec::new(),
                requirement_identities: vec![requirement],
            }],
            methods: vec![ServiceMethod {
                completion: semantic_vocabulary::BoundaryCompletion::NormalReturn,
                name: "exchange".into(),
                requirement_owner: "Service".into(),
                requirement_owner_package_identity: None,
                requirement_identity: requirement,
                parameter_count: 1,
                parameter_type_identities: vec![scalar],
                entry_claims: vec![ServiceEntryClaim {
                    parameter_index: 0,
                    carrier_identity: scalar,
                    domain: "InputDomain".into(),
                    predicate_presence: DomainPredicatePresence::Present,
                    effective_carry: CarryPolicy::STRICT,
                    authority_flow: ServiceEntryAuthorityFlow::Accepts,
                }],
                has_result: true,
                result_type_identity: scalar,
                result_claims: vec![ServiceResultClaim {
                    domain: "ResultDomain".into(),
                    effective_carry: CarryPolicy::STRICT,
                }],
                service_reach: vec!["Console".into()],
                synchronous_invocations: vec!["Other::call".into()],
                may_suspend: true,
                may_block: false,
                terminates_guarantee: true,
                termination_premises: vec![ServiceProgressPremise {
                    profile: "Finite".into(),
                    subject: ServiceProgressSubject::Parameter(0),
                    subject_projections: vec!["remaining".into()],
                    establishment_routes: vec![ServiceProgressEstablishmentRoute {
                        kind: ServiceProgressEstablishmentRouteKind::BoundaryRequirement,
                        requirement_identity: requirement,
                    }],
                }],
            }],
        });
        let mut names = TerminalIdentityTableBuilder::default();
        let declaration = names.intern("toolchain::Service");
        let application = names.intern("applied(Service)");
        let nominal = names.intern("nominal(Provider)");
        let semantic = names.intern("toolchain::Provider");
        let schema = TerminalServiceSchema {
            declaration,
            application,
            surface,
        };
        let entry = TerminalProgramEntryContract {
            target: "linux_x86_64".into(),
            entry: semantic_vocabulary::MachineId::new(7).unwrap(),
            schema_declaration: declaration,
            schema_application: application,
            physical_source_digest: [23; 32],
        };
        let selection = TerminalProviderSelection {
            target: entry.target.clone(),
            composition_mode: CompositionMode::Independent,
            schema_declaration: declaration,
            schema_application: application,
            provider_declaration: nominal,
            provider_identity: semantic,
            provider_application: Some(semantic),
            provider_package: None,
            provider_custody:
                semantic_vocabulary::compiler_evaluation::DeclarationCustody::Toolchain,
        };
        let mut writer = Writer::default();
        Schema::write(&mut writer, &schema).unwrap();
        EntryContract::write(&mut writer, &entry).unwrap();
        ProviderSelection::write(&mut writer, &selection).unwrap();
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);
        let decoded = Schema::read(&mut reader).unwrap();
        assert_eq!(decoded, schema);
        assert_eq!(EntryContract::read(&mut reader).unwrap(), entry);
        assert_eq!(ProviderSelection::read(&mut reader).unwrap(), selection);
        assert_eq!(reader.remaining(), 0);
        assert_eq!(
            decoded.surface.identities.texts().collect::<Vec<_>>(),
            schema.surface.identities.texts().collect::<Vec<_>>()
        );

        let mut module = TerminalModule::declarations();
        module.identities = names.build();
        module.service_schemas.push(decoded.clone());
        assert!(module.service_schema(declaration, application).is_ok());
        module.service_schemas.push(decoded);
        assert!(module.service_schema(declaration, application).is_err());
        let mut no_result = schema.clone();
        let surface = Arc::make_mut(&mut no_result.surface);
        surface.methods[0].has_result = false;
        surface.methods[0].result_type_identity = ProviderIdentityId::default();
        surface.methods[0].result_claims.clear();
        module.service_schemas = vec![no_result];
        assert!(module.service_schema(declaration, application).is_ok());
        Arc::make_mut(&mut module.service_schemas[0].surface).methods[0].has_result = true;
        assert!(module.service_schema(declaration, application).is_err());
    }

    #[test]
    fn sealed_native_meaning_survives_transport() {
        for meaning in [
            TerminalSealedNativeIntrinsic::Hosted(HostedOperation::ConsoleExitProcess),
            TerminalSealedNativeIntrinsic::Hosted(HostedOperation::ConsoleReadByte),
            TerminalSealedNativeIntrinsic::Hosted(HostedOperation::ConsoleWriteByte),
            TerminalSealedNativeIntrinsic::Hosted(HostedOperation::ProcessExit),
            TerminalSealedNativeIntrinsic::FixedImage(FixedImageOperation::ProgramStorageEntry),
            TerminalSealedNativeIntrinsic::FixedImage(
                FixedImageOperation::ProgramImageRetainedSuffixStart,
            ),
        ] {
            let mut writer = Writer::default();
            SealedIntrinsic::write(&mut writer, &meaning).unwrap();
            let bytes = writer.finish();
            let mut reader = Reader::new(&bytes);
            assert_eq!(SealedIntrinsic::read(&mut reader).unwrap(), meaning);
            assert_eq!(reader.remaining(), 0);
        }
    }
}
