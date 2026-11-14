//! Bodyless calls keep their own declaration and completion contract.
//! Provider selection settles that requirement later; it does not replace this
//! identity with a named body or an implicit service receiver.

use semantic_vocabulary::{BoundaryCompletion, BoundaryMachineId, IdTable, MachineId};
use symbols::SymbolHandle;
use terminal_check_tables::{Checked, Construct, Refusal, RefusalReason};
use terminal_psi::{
    BoundaryDeclarationRole, BoundaryMachineDeclaration, BoundaryMachineResult,
    BoundaryStructuralResultDeclaration, CheckedBoundaryCallable, CheckedBoundaryExecution,
    TerminalCallableTermination, TerminalMachineResult, TerminalModule,
};

use crate::{Outcome, Signature};

#[derive(Debug, Clone)]
pub(crate) struct BoundarySource {
    pub source: SymbolHandle,
    pub kind: semantic_vocabulary::BoundaryRequirementKind,
    pub owner: SymbolHandle,
    pub signature: Signature,
    pub envelope: crate::reach::SourceEnvelope,
    pub completion: BoundaryCompletion,
    pub owning_service: Option<language_semantics::ServiceReachId>,
    pub declaration: crate::declare::BoundaryDeclarationSource,
}
#[derive(Debug, Clone, Default)]
pub(crate) struct BoundaryNamespace {
    pub rows: IdTable<BoundaryMachineId, BoundarySource>,
}
impl BoundaryNamespace {
    pub fn row(&self, boundary: BoundaryMachineId) -> Option<&BoundarySource> {
        self.rows.get(boundary)
    }
    pub fn signature(&self, boundary: BoundaryMachineId) -> Option<&Signature> {
        Some(&self.row(boundary)?.signature)
    }
    pub fn completion(&self, boundary: BoundaryMachineId) -> Option<BoundaryCompletion> {
        Some(self.row(boundary)?.completion)
    }
    pub fn outcome(&self, boundary: BoundaryMachineId) -> Option<Outcome> {
        let source = self.row(boundary)?;
        let crash_routes = match &source.signature.crashes {
            crate::body::CrashCeiling::Authored(routes) => routes.clone(),
            crate::body::CrashCeiling::DeclaredNone => Vec::new(),
            crate::body::CrashCeiling::Inferred => return None,
        };
        Some(Outcome {
            completes_normally: matches!(source.completion, BoundaryCompletion::NormalReturn),
            crash_routes,
            service_reach: match &source.signature.reaches {
                crate::Ceiling::Authored(row) => row.clone(),
                crate::Ceiling::Inferred => return None,
            },
            synchronous_entries: match &source.signature.invokes {
                crate::Ceiling::Authored(row) => row.clone(),
                crate::Ceiling::Inferred => return None,
            },
        })
    }
    pub fn publish(
        &self,
        context: MachineId,
        signatures: &IdTable<MachineId, Signature>,
        outcomes: &IdTable<MachineId, Outcome>,
        records: &crate::lower::places::Records<'_>,
        callables: &crate::callables::CallableNamespace,
        services: &crate::reach::ServiceMap,
        module: &mut TerminalModule,
    ) -> Checked<()> {
        let refusal = |detail: &str| Refusal {
            machine: context,
            concerns: Construct::Machine,
            reason: RefusalReason::Unsupported,
            detail: detail.to_owned(),
        };
        if !module.boundary_machines.is_empty() {
            return Err(refusal("boundary declarations were already published"));
        }
        let mut identities = std::mem::take(&mut module.identities).into_builder();
        for (id, source) in self.rows.iter() {
            let envelope = services.envelope(context, &source.envelope)?;
            let header = crate::lower::signature_header(
                context,
                &source.signature,
                signatures,
                outcomes,
                records,
                callables,
                self,
            )?;
            if !header.proof.outcome_specific_ensures.is_empty() {
                return Err(refusal("boundary has an unsupported guarded guarantee"));
            }
            if !header.proof.erased_scalar_formals.is_empty()
                || !header.proof.erased_proof_formals.is_empty()
            {
                return Err(refusal("boundary needs retained erased actual lanes"));
            }
            let result = match header.result {
                TerminalMachineResult::Unit => BoundaryMachineResult::Unit,
                TerminalMachineResult::Scalar(result) => {
                    BoundaryMachineResult::Scalar(result.value_type())
                }
                TerminalMachineResult::Structural(result) => {
                    if !result.reference_sources.is_empty() {
                        return Err(refusal(
                            "boundary reference results need retained public region declarations",
                        ));
                    }
                    BoundaryMachineResult::Structural(BoundaryStructuralResultDeclaration {
                        structural_type: result.structural_type,
                        multiplicity: result.multiplicity,
                        qualifications: result.qualifications,
                        projected_qualifications: result.projected_qualifications,
                    })
                }
            };
            let declaration = crate::declare::provider_declaration(
                &source.declaration,
                &source.signature,
                &header.proof,
                &envelope,
                services,
                &mut identities,
            )
            .map_err(refusal)?;
            module.boundary_machines.push(BoundaryMachineDeclaration {
                id,
                declaration: Some(CheckedBoundaryCallable { source: declaration, execution: CheckedBoundaryExecution::Provider, method_arguments: Vec::new() }),
                numeric_intrinsic: None,
                role: BoundaryDeclarationRole::Callable,
                kind: source.kind,
                completion: source.completion,
                unconditional_termination: matches!(source.envelope.termination, TerminalCallableTermination::Terminates { ref premises } if premises.is_empty()),
                identity: identities.intern(&source.declaration.callable),
                owner: terminal_psi::NominalDeclarationIdentity::new(identities.intern(&source.declaration.owner)),
                attachment: None,
                parameter_order: header.parameter_order,
                scalar_parameters: header.scalar_parameters.iter().map(|parameter| parameter.scalar_type).collect(),
                crash_routes: header.proof.crash_routes,
                structural_parameters: header.structural_parameters,
                result,
                result_reference_regions: Vec::new(),
                requires: Vec::new(),
                normal_qualifications: Vec::new(),
                scalar_domain_requirements: Vec::new(),
                scalar_requires: header.proof.requires,
                scalar_ensures: header.proof.ensures.into_iter().map(|clause| clause.proposition).collect(),
                reference_outputs: Vec::new(),
                program_local_root_introductions: Vec::new(),
                content_guarantees: Vec::new(),
                fixed_service_reach: envelope.service_reach.clone(),
                owning_service: source.owning_service.map(|service| services.service(context, service)).transpose()?,
                published_service_ceiling: envelope.service_reach,
                published_synchronous_entries: envelope.invocations.iter().filter_map(|entry| match entry { terminal_psi::TerminalCallableInvocation::Service(service) => Some(*service), terminal_psi::TerminalCallableInvocation::Parameter(_) => None }).collect(),
            });
        }
        module.identities = identities.build();
        Ok(())
    }
}
