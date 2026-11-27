use crate::descriptors::descriptor_lifetimes::DescriptorPosition;
use crate::descriptors::descriptor_results::{DescriptorMachine, PayloadMembershipContract};
use semantic_vocabulary::{BlockId, InvocationSite, MachineId, PlaceId, ScalarTerm, ValueId};
use terminal_psi::{
    CheckedMachineDeclaration, DirectCallRole, EvidenceCallableOwner, EvidenceCallableTarget,
    EvidenceContractLane, EvidenceTermDeclaration, OutcomeSpecificCallEvidence,
    OutcomeSpecificEnsure, ProofOutputCall, PropositionApplicationIdentity, StructuralArgument,
    StructuralDomainDeclaration, StructuralOperationResult, StructuralParameterDeclaration,
    StructuralTypeDeclaration, TerminalIdentityTable,
};

pub(crate) enum RuntimeEvidenceResult {
    Unit,
    NoNormal,
    Scalar {
        id: ValueId,
        scalar_type: semantic_vocabulary::ScalarType,
    },
    Structural(StructuralOperationResult),
}
impl RuntimeEvidenceResult {
    pub fn scalar(&self) -> Option<(ValueId, semantic_vocabulary::ScalarType)> {
        match self {
            Self::Scalar { id, scalar_type } => Some((*id, *scalar_type)),
            _ => None,
        }
    }
    pub fn structural(&self) -> Option<&StructuralOperationResult> {
        match self {
            Self::Structural(result) => Some(result),
            _ => None,
        }
    }
}

pub(crate) struct RuntimeEvidenceOperation {
    pub site: InvocationSite,
    pub position: DescriptorPosition,
    pub target: EvidenceCallableTarget,
    pub role: DirectCallRole,
    pub arguments: Vec<ValueId>,
    pub structural_arguments: Vec<StructuralArgument>,
    pub result: RuntimeEvidenceResult,
    pub selected_evidence: Vec<OutcomeSpecificCallEvidence>,
}
#[derive(Clone, Copy)]
pub(crate) struct RuntimeEvidenceCallParts<'operation> {
    pub target: EvidenceCallableTarget,
    pub role: DirectCallRole,
    pub arguments: &'operation [ValueId],
    pub structural_arguments: &'operation [StructuralArgument],
    pub selected_evidence: &'operation [OutcomeSpecificCallEvidence],
}
impl RuntimeEvidenceOperation {
    pub fn parts(&self) -> RuntimeEvidenceCallParts<'_> {
        RuntimeEvidenceCallParts {
            target: self.target,
            role: self.role,
            arguments: &self.arguments,
            structural_arguments: &self.structural_arguments,
            selected_evidence: &self.selected_evidence,
        }
    }
}

pub(crate) struct RuntimeEvidenceBody {
    pub id: MachineId,
    pub entry: BlockId,
    pub declaration: Option<CheckedMachineDeclaration>,
    pub parameters: Vec<ValueId>,
    pub structural_parameters: Vec<StructuralParameterDeclaration>,
    pub outcome_specific_ensures: Vec<OutcomeSpecificEnsure>,
    pub calls: Vec<RuntimeEvidenceOperation>,
    pub scalar_expressions: Vec<(ValueId, ScalarTerm)>,
    pub payload_contracts: Vec<(PlaceId, PayloadMembershipContract)>,
}
impl RuntimeEvidenceBody {
    pub fn operations(&self) -> impl Iterator<Item = &RuntimeEvidenceOperation> {
        self.calls.iter()
    }
    pub fn invocation(&self, site: InvocationSite) -> Option<&RuntimeEvidenceOperation> {
        let mut rows = self.calls.iter().filter(|row| row.site == site);
        let row = rows.next()?;
        rows.next().is_none().then_some(row)
    }
    pub fn scalar_expression(&self, id: ValueId) -> Option<&ScalarTerm> {
        let mut rows = self
            .scalar_expressions
            .iter()
            .filter(|(value, _)| *value == id);
        let (_, expression) = rows.next()?;
        rows.next().is_none().then_some(expression)
    }
    pub(crate) fn payload_contract(&self, root: PlaceId) -> Option<&PayloadMembershipContract> {
        let mut rows = self
            .payload_contracts
            .iter()
            .filter(|(place, _)| *place == root);
        let (_, contract) = rows.next()?;
        rows.next().is_none().then_some(contract)
    }
}

#[derive(Clone, Copy)]
pub(crate) enum RuntimeEvidenceCallable<'input> {
    Machine(&'input RuntimeEvidenceBody),
    Runtime(&'input terminal_psi::TerminalCallableContract),
}
impl<'input> RuntimeEvidenceCallable<'input> {
    pub fn scalar_parameter(self, position: usize) -> Option<ValueId> {
        match self {
            Self::Machine(body) => body.parameters.get(position).copied(),
            Self::Runtime(contract) => contract
                .scalar_parameters
                .get(position)
                .map(|value| value.id),
        }
    }
    pub fn structural_parameters(self) -> &'input [StructuralParameterDeclaration] {
        match self {
            Self::Machine(body) => &body.structural_parameters,
            Self::Runtime(contract) => &contract.structural_parameters,
        }
    }
    pub fn outcome_specific_ensures(self) -> &'input [OutcomeSpecificEnsure] {
        match self {
            Self::Machine(body) => &body.outcome_specific_ensures,
            Self::Runtime(contract) => &contract.proof.outcome_specific_ensures,
        }
    }
    pub(crate) fn checked_identity(
        self,
        identities: &TerminalIdentityTable,
    ) -> Result<String, &'static str> {
        match self {
            Self::Machine(body) => {
                let declaration = body
                    .declaration
                    .as_ref()
                    .ok_or("evidence target has no checked callable declaration")?;
                let callable = identities
                    .text(declaration.callable)
                    .ok_or("evidence target callable identity is absent")?;
                Ok(CheckedMachineDeclaration::callable_evidence_identity(
                    callable,
                    declaration
                        .specialization
                        .as_ref()
                        .map(|application| &application.commitment),
                ))
            }
            Self::Runtime(contract) => identities
                .text(contract.identity)
                .map(str::to_owned)
                .ok_or("runtime evidence target contract identity is absent"),
        }
    }
}

pub(crate) struct RuntimeEvidenceInputs<'input> {
    pub identities: &'input TerminalIdentityTable,
    pub callables: &'input terminal_psi::TerminalCallableCatalog,
    pub structural_types: &'input [StructuralTypeDeclaration],
    pub structural_domains: &'input [StructuralDomainDeclaration],
    pub proposition_declarations: &'input [terminal_psi::PropositionDeclaration],
    pub proposition_applications: &'input [PropositionApplicationIdentity],
    pub evidence_terms: &'input [EvidenceTermDeclaration],
    pub evidence_contract_lanes: &'input [EvidenceContractLane],
    pub proof_output_calls: &'input [ProofOutputCall],
    pub machines: &'input [RuntimeEvidenceBody],
    pub graph: &'input [DescriptorMachine],
}
impl RuntimeEvidenceInputs<'_> {
    pub fn callable(
        &self,
        owner: EvidenceCallableOwner,
    ) -> Option<RuntimeEvidenceCallable<'_>> {
        match owner {
            EvidenceCallableOwner::Machine(machine) => {
                self.machine(machine).map(RuntimeEvidenceCallable::Machine)
            }
            EvidenceCallableOwner::Runtime(contract) => self
                .callables
                .contract(contract)
                .map(RuntimeEvidenceCallable::Runtime),
        }
    }
    pub fn machine(&self, id: MachineId) -> Option<&RuntimeEvidenceBody> {
        let mut rows = self.machines.iter().filter(|row| row.id == id);
        let row = rows.next()?;
        rows.next().is_none().then_some(row)
    }
}
