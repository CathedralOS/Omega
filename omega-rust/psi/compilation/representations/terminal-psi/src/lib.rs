#![forbid(unsafe_code)]

/*
We define the canonical target-neutral Terminal Psi representation (`TerminalModule`,
`TerminalMachine`, `Block`, `Operation`, `Terminator`) together with its companion proof, debug-map,
and provenance schemas (`artifacts`) and its target/calling/callback/interrupt boundary descriptors.
Every source-level construct, ownership loan, affine cleanup obligation, termination ranking witness,
and proof obligation produced by the Psi front-end is represented here as dense, interned,
flat-table data with no hidden parser or type-checker state.
*/

/*
Why isolate `terminal-psi` as a pure data-schema crate separate from both its canonical wire codec
(`terminal-codec`) and its receiving semantic judgments (`terminal-semantics`)? Keeping the
published IR representation free of serialization cursors and verification engines lets the Psi
producer (`08_lowered-psi-to-terminal-psi`), the reference interpreter (`terminal-interpreter`), the
work meter (`psi-work`), and the Omega native realization pipeline (`Omega 00`) share one exact
in-memory vocabulary while enforcing the representation-to-checker trust boundary at package manifest
level.
*/

/*
Constructed by `08_lowered-psi-to-terminal-psi` at the end of the target-neutral Psi pipeline,
encoded and decoded as canonical `PSITERM\0` artifacts by `terminal-codec`, validated by
`terminal-semantics` and `terminal-verification`, executed by `terminal-interpreter`, and lowered to
target-neutral abstract operations by `00_terminal-psi-to-abstract-operations`.
*/

pub mod artifacts;
pub mod boundary;
pub mod callable_values;
#[macro_use]
pub mod control_flow;
pub mod identity;
pub mod observation;
#[macro_use]
pub mod ownership;
pub mod proof;
pub mod terminal_module;
pub mod types_and_values;

pub use artifacts::{
    AdmissionEvidence, AdmissionKind, CertificateEnvelope, CheckedProgramEntryTerminalReceipt,
    CorrelatedAffineBranchWitness, CorrelatedAffineStepWitness, DebugFileId, DebugSite,
    DebugSourceDigest, DebugSourceFile, DebugSourceOrigin, DebugSourceSpan, DebugSubject,
    EvidenceRoute, IntegerAffineWitness, IntegerCastChainWitness,
    IntegerCorrelatedForbiddenRootWitness, PrimitiveJudgment, ProofNode, ProofRule,
    ProofSystemMarker, TerminalDebugMap, TerminalMachineProvenance,
};
pub use language_core::inline_assembly::{
    AsmControlRegister as ControlRegister, AsmFenceKind as MemoryFenceKind,
    AsmGlobalDescriptorTableOperation as GlobalDescriptorTableOperation,
    AsmInterruptControlKind as InterruptControlKind, AsmRegisterCarrier as RegisterCarrier,
    AsmRegisterOperation as RegisterOperation, AsmSystemRegister as SystemRegister,
    AsmTargetApplicability as RegisterTargetApplicability,
};
pub use language_semantics::byte_predicates::{ByteSequencePredicate, ByteSequencePredicateSet};
pub(crate) use optimization::digest_encoder;

pub mod prepared_roots {

    use crate::{CheckedMachineDeclaration, TerminalIdentityTable, TerminalModule};
    use arena::Arena;
    use semantic_vocabulary::MachineId;
    use source::SourceSpan;
    use std::sync::Arc;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum NominalRootError {
        MissingEntryDeclaration,
        TerminalEntryMultiplicity(usize),
    }

    impl std::fmt::Display for NominalRootError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(formatter, "{self:?}")
        }
    }

    impl std::error::Error for NominalRootError {}

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct PreparedNominalRoot {
        identities: TerminalIdentityTable,
        declaration: crate::NominalDeclarationIdentity,
        source_machine_name: String,
        span: Option<SourceSpan>,
    }

    impl PreparedNominalRoot {
        pub fn new(
            declaration: &str,
            source_machine_name: &str,
            span: Option<SourceSpan>,
        ) -> Result<Self, NominalRootError> {
            if declaration.is_empty() {
                return Err(NominalRootError::MissingEntryDeclaration);
            }
            let mut identities = crate::TerminalIdentityTableBuilder::default();
            let declaration =
                crate::NominalDeclarationIdentity::new(identities.intern(declaration));
            Ok(Self {
                identities: identities.build(),
                declaration,
                source_machine_name: source_machine_name.to_owned(),
                span,
            })
        }

        pub fn resolve<'module>(
            &self,
            module: &'module TerminalModule,
        ) -> Result<&'module crate::TerminalMachine, NominalRootError> {
            let mut matching = module.machines.iter().filter(|machine| {
                machine.declaration.as_ref().is_some_and(|declaration| {
                    self.declaration.same_identity_as(
                        &self.identities,
                        &declaration.machine,
                        &module.identities,
                    )
                })
            });
            let Some(entry) = matching.next() else {
                return Err(NominalRootError::TerminalEntryMultiplicity(0));
            };
            let remaining = matching.count();
            if remaining != 0 {
                return Err(NominalRootError::TerminalEntryMultiplicity(remaining + 1));
            }
            Ok(entry)
        }

        pub fn source_machine_name(&self) -> &str {
            &self.source_machine_name
        }

        pub fn diagnostic_name(&self) -> Option<(&str, Option<SourceSpan>)> {
            (!self.source_machine_name.is_empty()).then_some((&self.source_machine_name, self.span))
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub enum TerminalRootDeclaration {
        #[default]
        Missing,
        Available(CheckedMachineDeclaration),
        Unavailable(String),
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub struct TerminalRootRequest {
        pub declaration: TerminalRootDeclaration,
        pub name: Option<String>,
        pub span: Option<SourceSpan>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct PreparedTerminalRoots {
        identities: Arc<TerminalIdentityTable>,
        roots: Arena<TerminalRootRequest>,
    }

    pub struct RetainedTerminalRoot<'a> {
        pub machine: MachineId,
        pub name: Option<&'a str>,
        pub span: Option<SourceSpan>,
    }

    impl PreparedTerminalRoots {
        pub fn new(
            identities: Arc<TerminalIdentityTable>,
            requests: impl IntoIterator<Item = TerminalRootRequest>,
        ) -> Self {
            let mut roots = Arena::new();
            for mut request in requests {
                if let TerminalRootDeclaration::Available(declaration) = &request.declaration {
                    let has_portable_owner = !identities
                        .is_invocation_local(declaration.machine.identity())
                        && !identities.is_invocation_local(declaration.state.identity())
                        && !identities.is_invocation_local(declaration.semantic_declaration)
                        && declaration
                            .specialization
                            .as_ref()
                            .is_none_or(|specialization| {
                                !identities.is_invocation_local(specialization.template.identity())
                                    && !identities.is_invocation_local(
                                        specialization.template_semantic_declaration,
                                    )
                            });
                    if !has_portable_owner {
                        request.declaration = TerminalRootDeclaration::Unavailable(
                            "checked declaration has no exact package or toolchain owner".into(),
                        );
                    }
                }
                roots.insert(request);
            }
            Self { identities, roots }
        }

        pub fn retained_roots<'a>(
            &'a self,
            module: &'a TerminalModule,
        ) -> impl Iterator<Item = Result<RetainedTerminalRoot<'a>, String>> + 'a {
            self.roots.iter().map(|(_, root)| {
                let declaration = match &root.declaration {
                    TerminalRootDeclaration::Available(declaration) => declaration,
                    TerminalRootDeclaration::Unavailable(reason) => return Err(reason.clone()),
                    TerminalRootDeclaration::Missing => {
                        return Err("selected root lost its checked declaration".into());
                    }
                };
                let mut matches = module.machines.iter().filter(|machine| {
                    machine.declaration.as_ref().is_some_and(|candidate| {
                        declaration.same_identity_as(
                            &self.identities,
                            candidate,
                            &module.identities,
                        )
                    })
                });
                let machine = matches
                    .next()
                    .ok_or("selected root has no exact retained declaration")?;
                if matches.next().is_some() {
                    return Err("selected root has multiple exact retained declarations".into());
                }
                Ok(RetainedTerminalRoot {
                    machine: machine.id,
                    name: root.name.as_deref(),
                    span: root.span,
                })
            })
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::{
            CheckedMachineSpecialization, NominalDeclarationIdentity, TerminalIdentityTableBuilder,
        };

        fn declaration(table: &mut TerminalIdentityTableBuilder) -> CheckedMachineDeclaration {
            CheckedMachineDeclaration {
                machine: NominalDeclarationIdentity::new(table.intern("package::Machine")),
                state: NominalDeclarationIdentity::new(table.intern("package::Machine::entry")),
                semantic_declaration: table.intern("entry-signature"),
                parameter_types: vec![table.intern("qualified-input")],
                callable: table.intern("qualified-callable"),
                result_type: table.intern("qualified-result"),
                contract: [17; 32],
                published_may_suspend: false,
                published_may_block: false,
                specialization: Some(CheckedMachineSpecialization {
                    capture: None,
                    template: NominalDeclarationIdentity::new(table.intern("package::Template")),
                    template_callable: table.intern("template-callable"),
                    template_semantic_declaration: table.intern("template-signature"),
                    commitment: [29; 32],
                }),
            }
        }

        #[test]
        fn preparation_retains_complete_declarations_and_deferred_occurrences() {
            let mut table = TerminalIdentityTableBuilder::default();
            let declaration = declaration(&mut table);
            let requests = [
                TerminalRootRequest::default(),
                TerminalRootRequest {
                    declaration: TerminalRootDeclaration::Unavailable(
                        "unselected declaration unavailable".into(),
                    ),
                    ..Default::default()
                },
                TerminalRootRequest {
                    declaration: TerminalRootDeclaration::Available(declaration.clone()),
                    name: Some("selected".into()),
                    span: None,
                },
            ];
            let prepared = PreparedTerminalRoots::new(Arc::new(table.build()), requests.clone());
            let retained: Vec<_> = prepared
                .roots
                .iter()
                .map(|(_, request)| request.clone())
                .collect();
            assert_eq!(retained, requests);
        }

        #[test]
        fn every_invocation_local_declaration_owner_is_refused_at_the_boundary() {
            for owner in 0..5 {
                let mut table = TerminalIdentityTableBuilder::for_invocation();
                let mut declaration = declaration(&mut table);
                let local = table.intern_invocation_local("private-coordinate").unwrap();
                match owner {
                    0 => declaration.machine = NominalDeclarationIdentity::new(local),
                    1 => declaration.state = NominalDeclarationIdentity::new(local),
                    2 => declaration.semantic_declaration = local,
                    3 => {
                        declaration.specialization.as_mut().unwrap().template =
                            NominalDeclarationIdentity::new(local)
                    }
                    4 => {
                        declaration
                            .specialization
                            .as_mut()
                            .unwrap()
                            .template_semantic_declaration = local
                    }
                    _ => unreachable!(),
                }
                let prepared = PreparedTerminalRoots::new(
                    Arc::new(table.build()),
                    [TerminalRootRequest {
                        declaration: TerminalRootDeclaration::Available(declaration),
                        ..Default::default()
                    }],
                );
                let (_, request) = prepared.roots.iter().next().unwrap();
                assert_eq!(
                    request.declaration,
                    TerminalRootDeclaration::Unavailable(
                        "checked declaration has no exact package or toolchain owner".into()
                    ),
                    "owner {owner}"
                );
            }
        }
    }
}
pub use prepared_roots::{
    NominalRootError, PreparedNominalRoot, PreparedTerminalRoots, RetainedTerminalRoot,
    TerminalRootDeclaration, TerminalRootRequest,
};

pub mod callback_application {

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalCallbackApplication {
        pub application_index: u32,
        pub registrar: semantic_vocabulary::OperationId,
        pub static_machine_ordinal: u32,
        pub requirement: String,
        pub calling_owner_application: String,
        pub boundary: Option<semantic_vocabulary::BoundaryMachineId>,
        pub published_contract: [u8; 32],
        pub entry_admitted: bool,
        pub body: crate::CheckedMachineDeclaration,
    }
}
pub mod installed_interrupt {

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalInstalledInterruptRegistration {
        pub operation: semantic_vocabulary::OperationId,
        pub installer: semantic_vocabulary::BoundaryMachineId,
        pub service: semantic_vocabulary::ServiceId,
        pub registrar_boundary: semantic_vocabulary::BoundaryMachineId,
        pub registrar: crate::NominalDeclarationIdentity,
        pub registrar_owner: crate::NominalDeclarationIdentity,
        pub registrar_application: String,
        pub registrar_declaring_application: String,
        pub registrar_arguments: Vec<String>,
        pub registrar_lifetimes: Vec<String>,
        pub registrar_requirement: String,
        pub slot_declaration: crate::NominalDeclarationIdentity,
        pub slot_commitment: [u8; 32],
        pub slot_encoding: Vec<u8>,
        pub callbacks: Vec<crate::TerminalCallbackApplication>,
    }
}
pub use installed_interrupt::TerminalInstalledInterruptRegistration;
pub mod calling_policies {

    use crate::TerminalIdentityIndex;
    use semantic_vocabulary::{
        BoundaryMachineId, CallbackRequirementId, CallingOperandKind, CallingSignatureId,
        NativeParameterId, StaticMachineBinderId, StructuralTypeId,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CallingAbiDeclaration {
        pub id: CallingSignatureId,
        pub requirement_identity: TerminalIdentityIndex,
        pub declaring_application: TerminalIdentityIndex,
        pub operands: Vec<CallingStaticOperand>,
        pub formals: Vec<semantic_vocabulary::CallingGenericFormal>,
        pub default_eligible: bool,
        pub physical_owner: Result<semantic_vocabulary::CallingDeclarationOwner, String>,
        pub storage: Result<CallingAbiStorage, String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CallingStaticOperand {
        pub ordinal: u32,
        pub kind: CallingOperandKind,
        pub identity: TerminalIdentityIndex,
    }

    impl CallingAbiDeclaration {
        pub fn identity(
            &self,
            identities: &crate::TerminalIdentityTable,
        ) -> Result<semantic_vocabulary::CallingAbiIdentity, String> {
            let text = |identity| {
                identities
                    .text(identity)
                    .filter(|text| !text.is_empty())
                    .map(str::to_owned)
                    .ok_or_else(|| "declaration ABI lost a portable identity".to_owned())
            };
            let identity = semantic_vocabulary::CallingAbiIdentity {
                requirement_identity: text(self.requirement_identity)?,
                declaring_application: text(self.declaring_application)?,
                operands: self
                    .operands
                    .iter()
                    .map(|operand| {
                        Ok(semantic_vocabulary::CallingStaticOperand {
                            ordinal: operand.ordinal,
                            kind: operand.kind,
                            identity: text(operand.identity)?,
                        })
                    })
                    .collect::<Result<_, String>>()?,
            };
            identity.validate()?;
            if identity.id() != self.id {
                return Err(
                    "declaration ABI compact coordinate differs from its complete key".into(),
                );
            }
            Ok(identity)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CallingAbiStorage {
        pub parameters: Vec<CallingAbiParameter>,
        pub result: Option<StructuralTypeId>,
        pub telescope: CallingSignatureTelescope,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CallingAbiParameter {
        pub structural_type: StructuralTypeId,
        pub callback_backing: Option<StructuralTypeId>,
        pub access: crate::StructuralAccess,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalOpaqueRepresentation {
        pub opaque: StructuralTypeId,
        pub carrier: StructuralTypeId,
        pub opaque_storage_identity: TerminalIdentityIndex,
        pub carrier_storage_identity: TerminalIdentityIndex,
        pub application: language_semantics::opaque_representations::SelectedOpaqueApplication,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CallingPolicyRequest {
        pub declaration: Result<TerminalIdentityIndex, String>,
        pub display_name: String,
        pub instances: Result<Vec<CallingPolicyInstance>, Vec<String>>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CallingPolicyInstance {
        pub declaring_application: TerminalIdentityIndex,
        pub policy_declaration: Result<crate::ProofDeclarationId, String>,
        pub policy: Result<crate::TargetMachineCallable, String>,
        pub signatures: Result<Vec<CallingSignatureId>, String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CallingSignatureDeclaration {
        pub boundary: BoundaryMachineId,
        pub application: Result<CallingSignatureId, String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CallingSignatureTelescope {
        pub parameters: Vec<CallingRuntimeFormal>,
        pub binders: Vec<CallingCallbackBinder>,
        pub direct_callbacks: Vec<CallingDirectCallback>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CallingRuntimeFormal {
        pub identity: NativeParameterId,
        pub native_ordinal: u32,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CallingCallbackBinder {
        pub binder: StaticMachineBinderId,
        pub requirement: CallbackRequirementId,
        pub requirement_identity: TerminalIdentityIndex,
        pub static_machine_ordinal: u32,
        pub invokes_synchronously: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CallingDirectCallback {
        pub identity: NativeParameterId,
        pub native_ordinal: u32,
        pub binder: StaticMachineBinderId,
        pub requirement: CallbackRequirementId,
    }
}
pub mod native_layouts {

    use crate::{TargetLayoutInput, TerminalIdentityIndex};
    use arena::{Arena, HandleSpan};
    use language_semantics::callback_declarations::PrivateCallbackSlotSelection;
    use layout_plans::NativeLayoutPlanReport;
    use semantic_vocabulary::{StructuralFieldId, StructuralTypeId};

    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub struct NativeLayoutCatalog {
        records: Arena<NativeRecordLayout>,
        fields: Arena<NativeLayoutFieldIdentity>,
        realizations: Arena<NativeLayoutRealization>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct NativeRecordLayout {
        pub record: StructuralTypeId,
        pub data_identity: TerminalIdentityIndex,
        pub policy_identity: TerminalIdentityIndex,
        pub semantic_identity: TerminalIdentityIndex,
        fields: HandleSpan<NativeLayoutFieldIdentity>,
        realizations: HandleSpan<NativeLayoutRealization>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct NativeLayoutFieldIdentity {
        pub field: StructuralFieldId,
        pub declaration_identity: TerminalIdentityIndex,
    }

    impl Default for NativeRecordLayout {
        fn default() -> Self {
            Self {
                record: StructuralTypeId::new(1).expect("arena sentinel"),
                data_identity: Default::default(),
                policy_identity: Default::default(),
                semantic_identity: Default::default(),
                fields: HandleSpan::empty(),
                realizations: HandleSpan::empty(),
            }
        }
    }

    impl Default for NativeLayoutFieldIdentity {
        fn default() -> Self {
            Self {
                field: StructuralFieldId::new(1).expect("arena sentinel"),
                declaration_identity: Default::default(),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct NativeLayoutRealization {
        pub target: Option<TargetLayoutInput>,
        pub report: NativeLayoutPlanReport<PrivateCallbackSlotSelection>,
    }

    impl Default for NativeLayoutRealization {
        fn default() -> Self {
            Self {
                target: None,
                report: NativeLayoutPlanReport {
                    layout: layout_plans::LayoutPlanReport {
                        schema_report_fingerprint: 0,
                        entries: Vec::new(),
                        offsets: None,
                        size: None,
                        align: 1,
                    },
                    private_callback_demands: Vec::new(),
                },
            }
        }
    }

    impl NativeLayoutCatalog {
        pub fn records(&self) -> impl Iterator<Item = &NativeRecordLayout> {
            self.records.iter().map(|(_, record)| record)
        }

        pub fn record(&self, identity: StructuralTypeId) -> Option<&NativeRecordLayout> {
            self.records().find(|record| record.record == identity)
        }

        pub fn fields(
            &self,
            record: &NativeRecordLayout,
        ) -> Result<&[NativeLayoutFieldIdentity], String> {
            self.fields
                .span(record.fields)
                .ok_or_else(|| "native layout has stale nominal field storage".into())
        }

        pub fn realizations(
            &self,
            record: &NativeRecordLayout,
        ) -> Result<&[NativeLayoutRealization], String> {
            self.realizations
                .span(record.realizations)
                .ok_or_else(|| "native layout has stale realization storage".into())
        }

        pub fn insert(
            &mut self,
            record: StructuralTypeId,
            data_identity: TerminalIdentityIndex,
            policy_identity: TerminalIdentityIndex,
            semantic_identity: TerminalIdentityIndex,
            fields: impl IntoIterator<Item = NativeLayoutFieldIdentity>,
            realizations: impl IntoIterator<Item = NativeLayoutRealization>,
        ) {
            let fields = self.fields.insert_many(fields);
            let realizations = self.realizations.insert_many(realizations);
            self.records.insert(NativeRecordLayout {
                record,
                data_identity,
                policy_identity,
                semantic_identity,
                fields,
                realizations,
            });
        }

        pub fn for_target(
            &self,
            record: &NativeRecordLayout,
            target: &str,
        ) -> Result<Option<&NativeLayoutRealization>, String> {
            let realizations = self.realizations(record)?;
            if record.policy_identity.is_absent() {
                if !realizations.is_empty() {
                    return Err("natural native layout carries authored realizations".into());
                }
                return Ok(None);
            }
            let mut selected = realizations.iter().filter(|realization| {
                realization
                    .target
                    .as_ref()
                    .is_none_or(|facts| facts.target == target)
            });
            let result = selected
                .next()
                .ok_or("native layout has no realization for the selected target")?;
            if selected.next().is_some() {
                return Err(
                    "native layout has multiple realizations for the selected target".into(),
                );
            }
            Ok(Some(result))
        }
    }
}
pub use native_layouts::{
    NativeLayoutCatalog, NativeLayoutFieldIdentity, NativeLayoutRealization, NativeRecordLayout,
};
pub mod unoptimized_behavior {

    use crate::{OperationKind, TerminalMachine, Terminator};
    use semantic_vocabulary::{
        BlockId, BoundaryMachineId, CrashCause, InvocationSite, MachineId, OperationId, ServiceId,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct UnoptimizedMachineBehavior {
        pub machine: MachineId,
        pub operations: Vec<UnoptimizedBehaviorOperation>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum UnoptimizedBehaviorOperation {
        Invocation {
            block: BlockId,
            site: InvocationSite,
            callee: MachineId,
        },
        BoundaryInvocation {
            block: BlockId,
            site: InvocationSite,
            boundary: BoundaryMachineId,
        },
        InstructionService {
            block: BlockId,
            operation: OperationId,
            service: ServiceId,
        },
        Crash {
            block: BlockId,
            cause: CrashCause,
        },
        NominalCleanup {
            block: BlockId,
            callee: MachineId,
        },
    }

    impl UnoptimizedMachineBehavior {
        pub fn capture(machine: &TerminalMachine) -> Self {
            let mut operations = Vec::new();
            for block in &machine.blocks {
                for operation in &block.operations {
                    if let Some(callee) = operation.kind.direct_callee() {
                        operations.push(UnoptimizedBehaviorOperation::Invocation {
                            block: block.id,
                            site: InvocationSite::Operation(operation.id),
                            callee,
                        });
                    }
                    match &operation.kind {
                        OperationKind::BoundaryCall { boundary, .. } => {
                            operations.push(UnoptimizedBehaviorOperation::BoundaryInvocation {
                                block: block.id,
                                site: InvocationSite::Operation(operation.id),
                                boundary: *boundary,
                            })
                        }
                        OperationKind::ActivateTaskRegister { service, .. }
                        | OperationKind::ReadTaskRegister { service, .. }
                        | OperationKind::ReloadSegmentDescriptors { service, .. }
                        | OperationKind::ReadSegmentSelector { service, .. }
                        | OperationKind::GlobalDescriptorTable { service, .. }
                        | OperationKind::PortWrite { service, .. }
                        | OperationKind::RegisterAccess {
                            service: Some(service),
                            ..
                        } => operations.push(UnoptimizedBehaviorOperation::InstructionService {
                            block: block.id,
                            operation: operation.id,
                            service: *service,
                        }),
                        _ => {}
                    }
                }
                match &block.terminator {
                    Terminator::InvokeNoNormal { edge, callee, .. } => {
                        operations.push(UnoptimizedBehaviorOperation::Invocation {
                            block: block.id,
                            site: InvocationSite::InvokeNoNormal(*edge),
                            callee: *callee,
                        })
                    }
                    Terminator::ExternalCompletion { edge, boundary, .. } => {
                        operations.push(UnoptimizedBehaviorOperation::BoundaryInvocation {
                            block: block.id,
                            site: InvocationSite::ExternalCompletion(*edge),
                            boundary: *boundary,
                        })
                    }
                    Terminator::Crash { cause, .. } => {
                        operations.push(UnoptimizedBehaviorOperation::Crash {
                            block: block.id,
                            cause: *cause,
                        })
                    }
                    _ => {}
                }
                for (_, actions) in block.terminator.cleanup_rosters() {
                    for action in actions {
                        if let crate::TerminalAffineCleanupAction::InvokeNominal(cleanup) = action {
                            operations.push(UnoptimizedBehaviorOperation::NominalCleanup {
                                block: block.id,
                                callee: cleanup.cleanup_machine,
                            });
                        }
                    }
                }
            }
            Self {
                machine: machine.id,
                operations,
            }
        }
    }
}
pub use callback_application::TerminalCallbackApplication;
pub use calling_policies::{
    CallingAbiDeclaration, CallingAbiParameter, CallingAbiStorage, CallingCallbackBinder,
    CallingDirectCallback, CallingPolicyInstance, CallingPolicyRequest, CallingRuntimeFormal,
    CallingSignatureDeclaration, CallingSignatureTelescope, CallingStaticOperand,
    TerminalOpaqueRepresentation,
};
pub use unoptimized_behavior::{UnoptimizedBehaviorOperation, UnoptimizedMachineBehavior};
pub mod callback_declarations {

    mod prepared;
    pub use prepared::PreparedCallbackDeclarations;

    use crate::{CheckedMachineDeclaration, NominalDeclarationIdentity};
    use arena::{Arena, Handle};
    use language_semantics::callback_declarations::{
        CallbackDeclaration, CallbackRegistrationDeclaration,
    };

    pub type CallbackUseId = Handle<CallbackUseDeclaration>;
    pub type CallbackRequirementId = Handle<CallbackRequirementDeclaration>;

    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub struct CallbackDeclarationRoster {
        pub uses: Arena<CallbackUseDeclaration>,
        pub requirements: Arena<CallbackRequirementDeclaration>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CallbackRequirementDeclaration {
        pub owner: Result<NominalDeclarationIdentity, String>,
        pub member: Result<NominalDeclarationIdentity, String>,
        pub callable: Result<String, String>,
        pub is_boundary: bool,
    }

    impl Default for CallbackRequirementDeclaration {
        fn default() -> Self {
            Self {
                owner: Err("callback requirement has no declaration owner".into()),
                member: Err("callback requirement has no declaration member".into()),
                callable: Err("callback requirement has no callable identity".into()),
                is_boundary: false,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub struct CallbackContractEnvelope {
        pub report_fingerprint: u64,
        pub commitment: [u8; 32],
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub struct CallbackContractRefinement {
        pub published: CallbackContractEnvelope,
        pub actual: CallbackContractEnvelope,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CallbackEntryResources {
        pub machine: Result<NominalDeclarationIdentity, String>,
        pub entry: Result<NominalDeclarationIdentity, String>,
        pub contract: CallbackContractEnvelope,
        pub stack_report_fingerprint: u64,
        pub logical_work_report_fingerprint: u64,
        pub machine_state_report_fingerprint: u64,
        pub envelope_report_fingerprint: u64,
    }

    impl Default for CallbackEntryResources {
        fn default() -> Self {
            Self {
                machine: Err("callback entry has no machine declaration".into()),
                entry: Err("callback entry has no entry declaration".into()),
                contract: CallbackContractEnvelope::default(),
                stack_report_fingerprint: 0,
                logical_work_report_fingerprint: 0,
                machine_state_report_fingerprint: 0,
                envelope_report_fingerprint: 0,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CallbackUseDeclaration {
        pub application_index: u32,
        pub static_machine_ordinal: u32,
        pub registration: Result<CallbackRegistrationDeclaration, String>,
        pub registration_declaration: Result<NominalDeclarationIdentity, String>,
        pub requirement: CallbackRequirementId,
        pub requirement_overload: String,
        pub published: CallbackContractEnvelope,
        pub actual: CallbackContractEnvelope,
        pub refinement: CallbackContractRefinement,
        pub entry: Option<CallbackEntryResources>,
        pub selected_declaration: Result<CheckedMachineDeclaration, String>,
        pub selected_entry: Result<NominalDeclarationIdentity, String>,
    }

    impl Default for CallbackUseDeclaration {
        fn default() -> Self {
            Self {
                application_index: 0,
                static_machine_ordinal: 0,
                registration: Err("callback use has no registration declaration".into()),
                registration_declaration: Err("callback use has no registration owner".into()),
                requirement: Handle::default(),
                requirement_overload: String::new(),
                published: CallbackContractEnvelope::default(),
                actual: CallbackContractEnvelope::default(),
                refinement: CallbackContractRefinement::default(),
                entry: None,
                selected_declaration: Err("callback use has no selected declaration".into()),
                selected_entry: Err("callback use has no selected entry".into()),
            }
        }
    }

    impl CallbackDeclarationRoster {
        pub fn declaration<'a>(
            &self,
            usage: &'a CallbackUseDeclaration,
        ) -> CallbackDeclaration<'a> {
            CallbackDeclaration {
                registration: usage.registration.clone(),
                satisfaction_is_boundary: self.requirements.get(usage.requirement).is_boundary,
                has_checked_entry: usage.entry.is_some(),
                requirement_overload: &usage.requirement_overload,
            }
        }
    }
}
pub use callback_declarations::{
    CallbackContractEnvelope, CallbackContractRefinement, CallbackDeclarationRoster,
    CallbackEntryResources, CallbackRequirementDeclaration, CallbackRequirementId,
    CallbackUseDeclaration, CallbackUseId, PreparedCallbackDeclarations,
};

#[cfg(test)]
mod source_guards {

    use source_audit::violations;

    const CRATE_ROOT: &str = env!("CARGO_MANIFEST_DIR");

    const ALLOWED: &[(&str, &str)] = &[];

    #[test]
    fn terminal_module_carries_no_string_identity_fields() {
        let violations = violations(
            CRATE_ROOT,
            "",
            &[
                concat!("identity", ": String"),
                concat!("identity", ": Option<String>"),
                concat!("identity", ": Vec<String>"),
                concat!("identities", ": Vec<String>"),
                concat!("identities", ": HashSet<String>"),
                concat!("identities", ": BTreeSet<String>"),
            ],
            ALLOWED,
        );
        assert!(
            violations.is_empty(),
            "terminal rows must carry interned identity indices, not text: {violations:?}"
        );
    }
}

pub use callable_values::{
    TerminalCallableCatalog, TerminalCallableContract, TerminalCallableEnvelope,
    TerminalCallableEvidenceLane, TerminalCallableInvocation, TerminalCallableTermination,
    TerminalCallableTerminationPremise, TerminalMachineDescriptor, TerminalMachineDescriptorOrigin,
    TerminalMachineEnvelope,
};
pub use language_core::BindingRelevance;
pub use semantic_vocabulary::{
    BoundaryCompletion, BoundaryRequirementKind, ExternalCompletionKind,
};
pub use terminal_module::*;

pub mod target_layouts {
    use crate::TerminalIdentityIndex;
    use semantic_vocabulary::{
        AuthoredFieldStorage, AuthoredRecordStorage, StructuralFieldId, StructuralTypeId,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct LayoutApplicationId(pub u32);

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum StructuralRecordStorage {
        Packed,
        AccessView(AuthoredRecordStorage),
        Accessor(AuthoredRecordStorage),
        Authored(AuthoredRecordStorage),
        TargetLayout(LayoutApplicationId),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TargetLayoutApplication {
        pub id: LayoutApplicationId,
        pub record: StructuralTypeId,
        pub schema: StructuralTypeId,
        pub policy_identity: TerminalIdentityIndex,
        pub plan_identity: TerminalIdentityIndex,
        pub target_inputs: Vec<TargetLayoutInput>,
        pub realizations: Vec<TargetLayoutRealization>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TargetLayoutSchemaArgument {
        pub abi_version: u16,
        pub fields: Vec<TargetLayoutSchemaField>,
        pub retired_field_identities: Vec<u64>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TargetLayoutSchemaFieldKind {
        Scalar,
        Nested,
        Repeated,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TargetLayoutFieldLayoutSource {
        Default,
        Scalar,
        Explicit,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TargetLayoutSchemaField {
        pub record_field: StructuralFieldId,
        pub schema_field: StructuralFieldId,
        pub key: u64,
        pub identity: Option<u64>,
        pub byte_size: u64,
        pub byte_alignment: u64,
        pub kind: TargetLayoutSchemaFieldKind,
        pub layout_source: TargetLayoutFieldLayoutSource,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TargetLayoutInput {
        pub target: String,
        pub pointer_size: u64,
        pub pointer_alignment: u64,
        pub case_tag_size: u64,
        pub case_tag_alignment: u64,
        pub little_endian: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TargetLayoutRealization {
        pub schema_argument: TargetLayoutSchemaArgument,
        pub target: String,
        pub pointer_size: u64,
        pub pointer_alignment: u64,
        pub case_tag_size: u64,
        pub case_tag_alignment: u64,
        pub little_endian: bool,
        pub storage: AuthoredRecordStorage,
        pub fields: Vec<TargetLayoutFieldStorage>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TargetLayoutFieldStorage {
        pub field: StructuralFieldId,
        pub storage: AuthoredFieldStorage,
    }
}
pub use target_layouts::{
    LayoutApplicationId, StructuralRecordStorage, TargetLayoutApplication,
    TargetLayoutFieldLayoutSource, TargetLayoutFieldStorage, TargetLayoutInput,
    TargetLayoutRealization, TargetLayoutSchemaArgument, TargetLayoutSchemaField,
    TargetLayoutSchemaFieldKind,
};

pub use terminal_module::quotients::{
    TerminalQuotientAdaptation, TerminalQuotientAdaptationLicense, TerminalQuotientCatalog,
    TerminalQuotientOperation, TerminalQuotientRepresentation,
};
pub mod direct_calls {
    use crate::{OperationKind, OutcomeSpecificCallEvidence, StructuralArgument};
    use semantic_vocabulary::{MachineId, ValueId};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum DirectCallRole {
        Unit,
        Scalar,
        Structural,
        NoNormal,
    }

    #[derive(Clone, Copy, Debug)]
    pub struct DirectCallParts<'a> {
        pub callee: MachineId,
        pub role: DirectCallRole,
        pub arguments: &'a [ValueId],
        pub structural_arguments: &'a [StructuralArgument],
        pub selected_evidence: &'a [OutcomeSpecificCallEvidence],
    }

    pub fn direct_call_parts(kind: &OperationKind) -> Option<DirectCallParts<'_>> {
        let (callee, role, arguments, structural_arguments, selected_evidence) = match kind {
            OperationKind::Call {
                callee, arguments, ..
            } => (
                callee,
                DirectCallRole::Scalar,
                arguments.as_slice(),
                &[][..],
                &[][..],
            ),
            OperationKind::CallUnit {
                callee,
                arguments,
                structural_arguments,
                ..
            } => (
                callee,
                DirectCallRole::Unit,
                arguments.as_slice(),
                structural_arguments.as_slice(),
                &[][..],
            ),
            OperationKind::CallStructuralScalar {
                callee,
                arguments,
                structural_arguments,
                ..
            } => (
                callee,
                DirectCallRole::Scalar,
                arguments.as_slice(),
                structural_arguments.as_slice(),
                &[][..],
            ),
            OperationKind::CallStructural {
                callee,
                structural_arguments,
                selected_evidence,
                ..
            } => (
                callee,
                DirectCallRole::Structural,
                &[][..],
                structural_arguments.as_slice(),
                selected_evidence.as_slice(),
            ),
            OperationKind::CallStructuralWithScalarArguments {
                callee,
                arguments,
                structural_arguments,
                selected_evidence,
                ..
            } => (
                callee,
                DirectCallRole::Structural,
                arguments.as_slice(),
                structural_arguments.as_slice(),
                selected_evidence.as_slice(),
            ),
            _ => return None,
        };
        Some(DirectCallParts {
            callee: *callee,
            role,
            arguments,
            structural_arguments,
            selected_evidence,
        })
    }

    pub fn call_structural_arguments(kind: &OperationKind) -> &[StructuralArgument] {
        match kind {
            OperationKind::BoundaryCall {
                structural_arguments,
                ..
            }
            | OperationKind::CallDynamic {
                structural_arguments,
                ..
            } => structural_arguments,
            kind => direct_call_parts(kind).map_or(&[], |call| call.structural_arguments),
        }
    }
}
pub use direct_calls::{
    DirectCallParts, DirectCallRole, call_structural_arguments, direct_call_parts,
};

pub use language_core::descriptor_tables::{SegmentRegister, SegmentReload};

pub use flow_effects::{
    ConditionalCarryRequirement, ContractDependency, template_contract_commitment,
    validate_published_dependency,
};
