//! Invocation-local views of current executable data. These are construction
//! scratch for the common reader, not another IR or retained proof receipt.
//! Adapters copy actual operand/result roles and use the same independently
//! reconstructed descriptor graph. No source text or inferred call is allowed.
use crate::descriptor_lifetimes::DescriptorPosition;
use crate::descriptor_results::{DescriptorMachine, PayloadMembershipContract};
use semantic_vocabulary::{
    BlockId, InvocationSite, MachineId, OperationId, PlaceId, ScalarTerm, ValueId,
};
use terminal_psi::{
    CheckedMachineDeclaration, DirectCallParts, DirectCallRole, EvidenceContractLane,
    EvidenceTermDeclaration, OutcomeSpecificCallEvidence, OutcomeSpecificEnsure, ProofOutputCall,
    PropositionApplicationIdentity, StructuralArgument, StructuralDomainDeclaration,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralTypeDeclaration,
    TerminalIdentityTable,
};

pub enum RuntimeEvidenceResult {
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

pub struct RuntimeEvidenceOperation {
    pub site: InvocationSite,
    pub position: DescriptorPosition,
    pub callee: MachineId,
    pub role: DirectCallRole,
    pub arguments: Vec<ValueId>,
    pub structural_arguments: Vec<StructuralArgument>,
    pub result: RuntimeEvidenceResult,
    pub selected_evidence: Vec<OutcomeSpecificCallEvidence>,
}
impl RuntimeEvidenceOperation {
    pub fn parts(&self) -> DirectCallParts<'_> {
        DirectCallParts {
            callee: self.callee,
            role: self.role,
            arguments: &self.arguments,
            structural_arguments: &self.structural_arguments,
            selected_evidence: &self.selected_evidence,
        }
    }
}

pub struct RuntimeEvidenceBody {
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
    pub fn operation(&self, id: OperationId) -> Option<&RuntimeEvidenceOperation> {
        self.invocation(InvocationSite::Operation(id))
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
    pub fn payload_contract(&self, root: PlaceId) -> Option<&PayloadMembershipContract> {
        let mut rows = self
            .payload_contracts
            .iter()
            .filter(|(place, _)| *place == root);
        let (_, contract) = rows.next()?;
        rows.next().is_none().then_some(contract)
    }
}

pub struct RuntimeEvidenceInputs<'input> {
    pub identities: &'input TerminalIdentityTable,
    pub structural_types: &'input [StructuralTypeDeclaration],
    pub structural_domains: &'input [StructuralDomainDeclaration],
    pub proposition_applications: &'input [PropositionApplicationIdentity],
    pub evidence_terms: &'input [EvidenceTermDeclaration],
    pub evidence_contract_lanes: &'input [EvidenceContractLane],
    pub proof_output_calls: &'input [ProofOutputCall],
    pub machines: &'input [RuntimeEvidenceBody],
    pub graph: &'input [DescriptorMachine],
}
impl RuntimeEvidenceInputs<'_> {
    pub fn machine(&self, id: MachineId) -> Option<&RuntimeEvidenceBody> {
        let mut rows = self.machines.iter().filter(|row| row.id == id);
        let row = rows.next()?;
        rows.next().is_none().then_some(row)
    }
}
