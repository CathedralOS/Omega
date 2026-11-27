//! Reconstruct the entries, requirements and physical effects of the embedded Psi module.
use crate::records::*;
use ::wire::digest_encoder::hex;
use semantic_vocabulary::{BoundaryMachineId, ServiceId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{OperationKind, TerminalIdentityIndex, TerminalModule};

const DESCRIPTION_IDENTITY_DOMAIN: &[u8] = b"omega-component-description-v1";
const REQUIREMENT_CONTRACT_DOMAIN: &[u8] = b"omega-component-requirement-v1";
const CANONICAL_ENTRY_CONTRACT_DOMAIN: &[u8] = b"omega-component-canonical-entry-v1";
const PORT_MECHANISM_DOMAIN: &[u8] = b"omega-component-port-mechanism-v1";
const REGISTER_MECHANISM_DOMAIN: &[u8] = b"omega-component-register-mechanism-v1";

pub(crate) fn requirement_export_identity(
    requirement_identity: &str,
    provider_identity: &str,
    candidate_identity: &str,
) -> String {
    format!("export:requirement:{requirement_identity}|{provider_identity}|{candidate_identity}")
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExportContract {
    pub identity: String,
    pub contract_identity: [u8; 32],
}

pub(crate) fn component_description_identity(canonical_bytes: &[u8]) -> [u8; 32] {
    ::wire::digest_encoder::DomainDigestEncoder::content(
        DESCRIPTION_IDENTITY_DOMAIN,
        canonical_bytes,
    )
}

pub fn requirement_contract_identity(requirement_identity: &str) -> [u8; 32] {
    ::wire::digest_encoder::DomainDigestEncoder::content(
        REQUIREMENT_CONTRACT_DOMAIN,
        requirement_identity.as_bytes(),
    )
}

fn canonical_entry_label(entry: terminal_psi::TargetMachineCallable) -> String {
    match entry {
        terminal_psi::TargetMachineCallable::Machine(machine) => {
            format!("machine:{}", machine.get())
        }
        terminal_psi::TargetMachineCallable::Boundary(boundary) => {
            format!("boundary:{}", boundary.get())
        }
    }
}

pub(crate) fn identity_text(module: &TerminalModule, identity: TerminalIdentityIndex) -> &str {
    module.identities.text(identity).unwrap_or_default()
}

pub(crate) fn canonical_entry_contract_identity(
    entry: terminal_psi::TargetMachineCallable,
) -> [u8; 32] {
    let mut digest =
        ::wire::digest_encoder::DomainDigestEncoder::new(CANONICAL_ENTRY_CONTRACT_DOMAIN);
    match entry {
        terminal_psi::TargetMachineCallable::Machine(machine) => {
            digest.raw(&[1]);
            digest.raw(&machine.get().to_le_bytes());
        }
        terminal_psi::TargetMachineCallable::Boundary(boundary) => {
            digest.raw(&[2]);
            digest.raw(&boundary.get().to_le_bytes());
        }
    }
    digest.finish()
}

pub fn port_mechanism_assumption(service: ServiceId, port: u16, value: u8) -> [u8; 32] {
    let mut digest = ::wire::digest_encoder::DomainDigestEncoder::new(PORT_MECHANISM_DOMAIN);
    digest.raw(&service.get().to_le_bytes());
    digest.raw(&port.to_le_bytes());
    digest.byte(value);
    digest.finish()
}

pub fn register_mechanism_assumption(
    service: ServiceId,
    operation: terminal_psi::RegisterOperation,
) -> [u8; 32] {
    let mut digest = ::wire::digest_encoder::DomainDigestEncoder::new(REGISTER_MECHANISM_DOMAIN);
    digest.raw(&service.get().to_le_bytes());
    digest.raw(&operation.catalog_tag().to_le_bytes());
    digest.finish()
}

pub struct DerivedInventory {
    pub called_requirement_identities: Vec<String>,
    pub entries: Vec<ComponentEntry>,
    pub export_contracts: Vec<ExportContract>,
    pub custody: Vec<CustodyConstraint>,
    pub port_writes: BTreeSet<(ServiceId, u16, u8)>,
    pub register_accesses: BTreeSet<(ServiceId, terminal_psi::RegisterOperation)>,
    pub concrete_service_reach: BTreeSet<ServiceId>,
    pub service_bounds: Vec<InstallationServiceBound>,
}

pub fn derive_component_inventory(
    module: &TerminalModule,
) -> Result<DerivedInventory, DescribeError> {
    let mut called_boundaries = BTreeSet::new();
    let mut port_writes = BTreeSet::new();
    let mut register_accesses = BTreeSet::new();
    let mut custody = Vec::new();

    for machine in &module.machines {
        for block in &machine.blocks {
            for operation in &block.operations {
                match &operation.kind {
                    OperationKind::EstablishNativeCodeLayout { .. }
                    | OperationKind::ReadScalarApplication { .. } => {}
                    OperationKind::BoundaryCall {
                        boundary,
                        completion_receipts,
                        ..
                    } => {
                        called_boundaries.insert(*boundary);
                        for receipt in completion_receipts {
                            custody.push(CustodyConstraint {
                                kind: CustodyKind::CompletionReceipt,
                                identity: format!(
                                    "completion-receipt:{}:{}:{}:{}",
                                    boundary.get(),
                                    operation.id.get(),
                                    receipt.claim.get(),
                                    receipt.argument_index,
                                ),
                                evidence: CustodyEvidence::ModuleDerived,
                            });
                        }
                    }
                    OperationKind::PortWrite {
                        service,
                        port,
                        value,
                    } => {
                        port_writes.insert((*service, *port, *value));
                    }
                    OperationKind::ActivateTaskRegister { service, .. } => {
                        register_accesses.insert((
                            *service,
                            terminal_psi::RegisterOperation::TaskRegisterActivate,
                        ));
                    }
                    OperationKind::ReloadSegmentDescriptors { service, kind, .. } => {
                        register_accesses.insert((
                            *service,
                            terminal_psi::RegisterOperation::SegmentReload(*kind),
                        ));
                    }
                    OperationKind::ReadSegmentSelector { service, register } => {
                        register_accesses.insert((
                            *service,
                            terminal_psi::RegisterOperation::SegmentRead(*register),
                        ));
                    }
                    OperationKind::ReadTaskRegister { service } => {
                        register_accesses
                            .insert((*service, terminal_psi::RegisterOperation::TaskRegisterRead));
                    }
                    OperationKind::GlobalDescriptorTable {
                        service, operation, ..
                    } => {
                        register_accesses.insert((
                            *service,
                            terminal_psi::RegisterOperation::GlobalDescriptorTable(*operation),
                        ));
                    }
                    OperationKind::RegisterAccess { service, event } => {
                        if let Some(service) = service {
                            register_accesses.insert((*service, event.operation()));
                        }
                    }
                    OperationKind::Call { .. }
                    | OperationKind::CallUnit { .. }
                    | OperationKind::CallStructural { .. }
                    | OperationKind::CallStructuralScalar { .. }
                    | OperationKind::CallStructuralWithScalarArguments { .. }
                    | OperationKind::CallDynamic { .. }
                    | OperationKind::CallMachine { .. } => {}
                    OperationKind::EstablishAccessReference { .. }
                    | OperationKind::EstablishReference { .. }
                    | OperationKind::ReleaseReference { .. }
                    | OperationKind::PackedIntegerUnary { .. }
                    | OperationKind::PackedIntegerBinary { .. }
                    | OperationKind::EstablishScalarArray { .. }
                    | OperationKind::EstablishStructuralArray { .. }
                    | OperationKind::EstablishPrimitiveLocal { .. }
                    | OperationKind::EstablishTrivialAffineLocal { .. }
                    | OperationKind::EstablishZeroedStructural { .. }
                    | OperationKind::EstablishStructuralLocal { .. }
                    | OperationKind::EstablishRecord { .. }
                    | OperationKind::MoveStructuralValue { .. }
                    | OperationKind::DecomposeStructuralValue { .. }
                    | OperationKind::TransformContent { .. }
                    | OperationKind::EstablishScalarCase { .. }
                    | OperationKind::EstablishStructuralCase { .. }
                    | OperationKind::EstablishActivationClaim { .. }
                    | OperationKind::SplitActivationClaim { .. }
                    | OperationKind::MergeActivationClaim { .. }
                    | OperationKind::ReleaseActivationClaim { .. }
                    | OperationKind::EstablishByteSequenceLiteral { .. }
                    | OperationKind::StorageAddress { .. }
                    | OperationKind::StorageAlignment { .. }
                    | OperationKind::PrimitiveScalarRead { .. }
                    | OperationKind::AtomicFence { .. }
                    | OperationKind::AtomicAccess { .. }
                    | OperationKind::StructuralScalarFieldStore { .. }
                    | OperationKind::StructuralByteSequenceFieldLength { .. }
                    | OperationKind::StructuralByteSequenceFieldByteStore { .. }
                    | OperationKind::StructuralByteSequenceFieldRead { .. }
                    | OperationKind::ByteSequencePredicate { .. }
                    | OperationKind::ByteSequenceEqual { .. }
                    | OperationKind::ByteSequenceStore { .. }
                    | OperationKind::StructuralByteSequenceFieldStore { .. }
                    | OperationKind::StructuralByteViewFieldStore { .. }
                    | OperationKind::StructuralCaseMembership { .. }
                    | terminal_psi::structural_leaf_copy_pair_pattern!()
                    | OperationKind::WriteOnlyPrimitiveStore { .. }
                    | OperationKind::ByteSequenceLength { .. }
                    | OperationKind::ByteSequenceRead { .. }
                    | OperationKind::ByteSequenceWrite { .. }
                    | OperationKind::ByteSequenceSubslice { .. }
                    | terminal_psi::element_view_establish_pair_pattern!(..)
                    | OperationKind::ElementViewLength { .. }
                    | OperationKind::ElementViewRead { .. }
                    | OperationKind::ElementViewWrite { .. }
                    | OperationKind::ElementViewSubslice { .. }
                    | OperationKind::ElementViewFixedArray { .. } => {}
                    OperationKind::IntegerConstant { .. }
                    | OperationKind::SelectImplementation { .. }
                    | OperationKind::SelectMachine { .. }
                    | OperationKind::BooleanConstant { .. }
                    | OperationKind::IeeeFloatConstant { .. }
                    | OperationKind::IeeeFloatCompare { .. }
                    | OperationKind::ScalarRepresentationView { .. }
                    | OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. }
                    | OperationKind::BooleanStructuralField { .. }
                    | OperationKind::ScalarStructuralField { .. }
                    | OperationKind::BooleanNot { .. }
                    | OperationKind::BooleanEqual { .. }
                    | OperationKind::IntegerEqual { .. }
                    | terminal_psi::integer_less_or_equal_pair_pattern!(..)
                    | OperationKind::IntegerBitwiseNot { .. }
                    | OperationKind::IntegerBitwiseAnd { .. }
                    | OperationKind::IntegerBitwiseOr { .. }
                    | OperationKind::IntegerBitwiseXor { .. }
                    | OperationKind::IntegerWiden { .. }
                    | OperationKind::IntegerExactCast { .. }
                    | OperationKind::WrappingIntegerShiftLeft { .. }
                    | OperationKind::WrappingIntegerShiftRight { .. }
                    | OperationKind::ExactIntegerShiftLeft { .. }
                    | OperationKind::ExactIntegerShiftRight { .. }
                    | OperationKind::WrappingIntegerAdd { .. }
                    | OperationKind::SaturatingIntegerAdd { .. }
                    | OperationKind::ExactIntegerAdd { .. }
                    | OperationKind::WrappingIntegerSubtract { .. }
                    | OperationKind::SaturatingIntegerSubtract { .. }
                    | OperationKind::ExactIntegerSubtract { .. }
                    | OperationKind::WrappingIntegerMultiply { .. }
                    | OperationKind::SaturatingIntegerMultiply { .. }
                    | OperationKind::ExactIntegerMultiply { .. }
                    | OperationKind::WrappingIntegerDivide { .. }
                    | OperationKind::SaturatingIntegerDivide { .. }
                    | OperationKind::ExactIntegerDivide { .. }
                    | OperationKind::WrappingIntegerRemainder { .. }
                    | OperationKind::SaturatingIntegerRemainder { .. }
                    | OperationKind::ExactIntegerRemainder { .. }
                    | OperationKind::TrappingInteger { .. }
                    | OperationKind::FloatResultPolicy { .. }
                    | terminal_psi::move_or_store_structural_field_pattern!()
                    | OperationKind::EstablishByteSequenceConcatenation { .. } => {}
                }
            }
        }
    }

    let declarations: BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration> =
        module
            .interfaces.boundary_machines
            .iter()
            .map(|declaration| (declaration.id, declaration))
            .collect();

    let mut called_requirement_identities = Vec::new();
    for boundary in &called_boundaries {
        let declaration = declarations
            .get(boundary)
            .ok_or(DescribeError::UndeclaredBoundary(*boundary))?;
        called_requirement_identities.push(identity_text(module, declaration.identity).to_string());
        for schema in &declaration.program_local_root_introductions {
            custody.push(CustodyConstraint {
                kind: CustodyKind::ProgramLocalRootIntroduction,
                identity: format!(
                    "root-introduction:{}:{}",
                    identity_text(module, declaration.identity),
                    schema.argument_index,
                ),
                evidence: CustodyEvidence::ModuleDerived,
            });
        }
        for (index, _guarantee) in declaration.content_guarantees.iter().enumerate() {
            custody.push(CustodyConstraint {
                kind: CustodyKind::BoundaryContentGuarantee,
                identity: format!(
                    "content-guarantee:{}:{index}",
                    identity_text(module, declaration.identity)
                ),
                evidence: CustodyEvidence::ModuleDerived,
            });
        }
    }
    called_requirement_identities.sort();
    called_requirement_identities.dedup();

    for input in &module.access.access_view_inputs {
        custody.push(CustodyConstraint {
            kind: CustodyKind::AccessViewInput,
            identity: format!(
                "access-view:{}:{}:{}",
                input.machine.get(),
                input.position,
                identity_text(module, input.view_identity),
            ),
            evidence: CustodyEvidence::ModuleDerived,
        });
    }
    for restored in &module.access.reborrow_restored_call_uses {
        custody.push(CustodyConstraint {
            kind: CustodyKind::ReborrowRestoredCall,
            identity: format!(
                "reborrow-call:{}:{}",
                restored.machine.get(),
                restored.operation.get(),
            ),
            evidence: CustodyEvidence::ModuleDerived,
        });
    }

    let mut entries = Vec::new();
    if let Some(entry) = module.entry {
        entries.push(ComponentEntry {
            kind: ComponentEntryKind::Canonical,
            identity: format!("canonical-entry:{}", canonical_entry_label(entry)),
            evidence: EntryEvidence::ModuleDerived,
        });
    }
    for site in &module.execution.suspension_call_sites {
        entries.push(ComponentEntry {
            kind: ComponentEntryKind::SuspensionResumption,
            identity: format!(
                "suspension-resumption:{}:{}",
                site.operation.get(),
                site.crossing.get(),
            ),
            evidence: EntryEvidence::ModuleDerived,
        });
        custody.push(CustodyConstraint {
            kind: CustodyKind::SuspensionFrontier,
            identity: format!(
                "suspension-frontier:{}:{}:{}",
                site.operation.get(),
                site.crossing.get(),
                hex(&site.frontier_commitment),
            ),
            evidence: CustodyEvidence::ModuleDerived,
        });
    }

    let concrete_service_reach: BTreeSet<ServiceId> =
        module.interfaces.root_service_reach.concrete.iter().copied().collect();
    let mut service_bounds: Vec<InstallationServiceBound> = module
        .interfaces.root_service_reach
        .installation_dependencies
        .iter()
        .map(|dependency| {
            let mut bound = dependency.upper_bound.clone();
            bound.sort();
            bound.dedup();
            InstallationServiceBound {
                requirement_identity: identity_text(module, dependency.requirement_identity)
                    .to_string(),
                bound,
            }
        })
        .collect();
    service_bounds.sort();
    service_bounds.dedup();

    let mut export_contracts = Vec::new();
    if let Some(entry) = module.entry {
        export_contracts.push(ExportContract {
            identity: format!("export:canonical:{}", canonical_entry_label(entry)),
            contract_identity: canonical_entry_contract_identity(entry),
        });
    }
    for candidate in &module.interfaces.provider_candidates {
        export_contracts.push(ExportContract {
            identity: requirement_export_identity(
                identity_text(module, candidate.requirement_identity),
                identity_text(module, candidate.provider_identity),
                identity_text(module, candidate.candidate_identity),
            ),
            contract_identity: requirement_contract_identity(identity_text(
                module,
                candidate.requirement_identity,
            )),
        });
    }
    export_contracts.sort();
    export_contracts.dedup();

    custody.sort();
    custody.dedup();
    entries.sort();
    entries.dedup();

    Ok(DerivedInventory {
        called_requirement_identities,
        entries,
        export_contracts,
        custody,
        port_writes,
        register_accesses,
        concrete_service_reach,
        service_bounds,
    })
}
