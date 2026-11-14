/*
Mathematical proof declarations, theorem obligations (`TheoremObligation`, `ObligationPhase`),
callable contracts, integer certificates, and proof-term representations in `TerminalModule`.
*/

pub(crate) mod content {
    use crate::StructuralPlaceDeclaration;
    use semantic_vocabulary::{
        ClaimId, ContentAlgebra, ContentConservation, ContentProjectionIdentity,
        ContentStructuralPlace, OperationId,
    };

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ClaimContentProjection {
        pub projection: ContentProjectionIdentity,
        pub algebra: ContentAlgebra,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ContentEntryClaim {
        pub claim: ClaimId,
        pub input: ContentStructuralPlace,
        pub projections: Vec<ClaimContentProjection>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ContentIdentityReshuffle {
        pub claim: ClaimId,
        pub input: ContentStructuralPlace,
        pub output: ContentStructuralPlace,
        pub projections: Vec<ClaimContentProjection>,
    }

    impl ContentIdentityReshuffle {}

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ContentPlaceSubstitution {
        pub source: ContentStructuralPlace,
        pub target: ContentStructuralPlace,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ContentPartitionComposition {
        pub producer_operation: OperationId,
        pub source_report_fingerprint: u64,
        pub source_structural_places: Vec<StructuralPlaceDeclaration>,
        pub source: ContentConservation,
        pub input_claims: Vec<ClaimId>,
        pub substitutions: Vec<ContentPlaceSubstitution>,
        pub derived: ContentConservation,
    }

    impl ContentPartitionComposition {}

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ContentClaimProjection {
        pub claim: ClaimId,
        pub projection: ContentProjectionIdentity,
        pub subject: ContentStructuralPlace,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ContentTransformationDerivation {
        pub conservation: ContentConservation,
        pub lineage_roots: Vec<ClaimId>,
        pub ancestry: Vec<ContentLineageStep>,
        pub inputs: Vec<ContentClaimProjection>,
        pub outputs: Vec<ContentClaimProjection>,
        pub custody: ContentTransformationCustody,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum ContentTransformationCustody {
        Derived,
        Boundary {
            requirements: Vec<ContentBoundaryBodyRequirement>,
            addresses: Vec<ContentBackingAddressTransport>,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ContentBoundaryBodyRequirement {
        pub boundary: semantic_vocabulary::BoundaryMachineId,
        pub guarantee: crate::ContentConservationGuarantee,
        pub substitutions: Vec<ContentBodyProjectionSubstitution>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ContentBodyProjectionSubstitution {
        pub source: semantic_vocabulary::ContentTerm,
        pub target: semantic_vocabulary::ContentTerm,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ContentBackingAddressTransport {
        pub input: ContentClaimAddress,
        pub output: ContentClaimAddress,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ContentClaimAddress {
        pub claim: ClaimId,
        pub subject: ContentStructuralPlace,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ContentLineageStep {
        pub producer_operation: OperationId,
        pub inputs: Vec<ClaimId>,
        pub outputs: Vec<ClaimId>,
    }

    impl ContentTransformationDerivation {
        pub fn map_places(
            &mut self,
            map: &mut impl FnMut(semantic_vocabulary::PlaceId) -> semantic_vocabulary::PlaceId,
        ) {
            fn term(
                value: &semantic_vocabulary::ContentTerm,
                map: &mut impl FnMut(semantic_vocabulary::PlaceId) -> semantic_vocabulary::PlaceId,
            ) -> semantic_vocabulary::ContentTerm {
                match value {
                    semantic_vocabulary::ContentTerm::Projection {
                        projection,
                        subject,
                    } => {
                        let mut subject = subject.clone();
                        subject.root = map(subject.root);
                        semantic_vocabulary::ContentTerm::Projection {
                            projection: *projection,
                            subject,
                        }
                    }
                    semantic_vocabulary::ContentTerm::Separate(children) => {
                        let mut children = children
                            .iter()
                            .map(|child| term(child, map))
                            .collect::<Vec<_>>();
                        children.sort();
                        semantic_vocabulary::ContentTerm::Separate(children)
                    }
                }
            }
            self.conservation = ContentConservation::new(
                self.conservation.algebra().clone(),
                term(self.conservation.left(), map),
                term(self.conservation.right(), map),
            );
            for binding in self.inputs.iter_mut().chain(&mut self.outputs) {
                binding.subject.root = map(binding.subject.root);
            }
            if let ContentTransformationCustody::Boundary {
                requirements,
                addresses,
            } = &mut self.custody
            {
                for requirement in requirements.iter_mut() {
                    for substitution in &mut requirement.substitutions {
                        substitution.target = term(&substitution.target, map);
                    }
                    requirement.substitutions.sort();
                }
                requirements.sort();
                for address in addresses.iter_mut() {
                    address.input.subject.root = map(address.input.subject.root);
                    address.output.subject.root = map(address.output.subject.root);
                }
                addresses.sort();
            }
            self.inputs.sort();
            self.outputs.sort();
        }
    }
}
pub(crate) mod contracts {
    use crate::{CrashCause, EvidenceInterfaceIdentity};
    use semantic_vocabulary::{
        ContractId, EvidenceTermId, MachineId, ObligationId, PlaceId, Proposition, PropositionId,
        StructuralCaseId, StructuralTypeId,
    };

    /// One declaration's signature and published obligations. Keeping them
    /// together lets a requirement retain its contract without inventing a body.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct MachineContract {
        pub id: ContractId,
        pub declaration: Option<crate::CheckedMachineDeclaration>,
        pub parameters: Vec<crate::ValueDeclaration>,
        pub parameter_order: Vec<crate::BoundaryParameterKind>,
        pub structural_parameters: Vec<crate::StructuralParameterDeclaration>,
        pub result: crate::TerminalMachineResult,
        pub entry_claims: Vec<crate::EntryClaim>,
        // Omitted reach and crash clauses can leave requirements open;
        // an explicit empty clause promises none. Entries follow their own
        // rule: omitted requirement invokes means the empty semantic ceiling.
        pub declared_service_reach: Option<Vec<semantic_vocabulary::ServiceId>>,
        pub declared_synchronous_entries: Option<Vec<semantic_vocabulary::ServiceId>>,
        pub declared_crash_routes: Option<Vec<CrashRouteBucket>>,
        pub closed_reach_application: Option<crate::ClosedReachApplication>,
        pub published_service_ceiling: Vec<semantic_vocabulary::ServiceId>,
        /// Exact published synchronous entries, strictly increasing. Empty
        /// enters nothing; a boundary's parents are not added implicitly.
        pub published_synchronous_entries: Vec<semantic_vocabulary::ServiceId>,
        pub termination: semantic_vocabulary::Termination,
        pub content_entry_claims: Vec<crate::ContentEntryClaim>,
        pub crash_routes: Vec<CrashRouteBucket>,
        pub erased_scalar_formals: Vec<crate::ValueDeclaration>,
        pub erased_proof_formals: Vec<crate::ErasedProofFormal>,
        pub requires: Vec<Proposition>,
        pub source_checked_requires: Vec<Proposition>,
        pub ensures: Vec<ContractClause>,
        pub outcome_specific_ensures: Vec<OutcomeSpecificEnsure>,
    }

    impl MachineContract {
        pub fn validate_parameter_order(&self) -> Result<(), &'static str> {
            let scalar_count = self
                .parameter_order
                .iter()
                .filter(|kind| **kind == crate::BoundaryParameterKind::Scalar)
                .count();
            let structural_count = self.parameter_order.len() - scalar_count;
            if scalar_count != self.parameters.len()
                || structural_count != self.structural_parameters.len()
            {
                return Err("machine runtime parameter order does not cover its signature");
            }
            Ok(())
        }

        pub fn runtime_parameter_formals(&self) -> Result<Vec<u32>, &'static str> {
            self.validate_parameter_order()?;
            let positions =
                |kind| {
                    self.parameter_order.iter().enumerate().filter_map(
                        move |(position, candidate)| (*candidate == kind).then_some(position),
                    )
                };
            positions(crate::BoundaryParameterKind::Scalar)
                .chain(positions(crate::BoundaryParameterKind::Structural))
                .map(|position| {
                    u32::try_from(position)
                        .map_err(|_| "machine runtime formal ordinal cannot be represented")
                })
                .collect()
        }

        pub fn empty(id: ContractId) -> Self {
            Self {
                id,
                declaration: None,
                parameters: Vec::new(),
                parameter_order: Vec::new(),
                structural_parameters: Vec::new(),
                result: crate::TerminalMachineResult::Unit,
                entry_claims: Vec::new(),
                declared_service_reach: None,
                declared_synchronous_entries: None,
                declared_crash_routes: None,
                closed_reach_application: None,
                published_service_ceiling: Vec::new(),
                published_synchronous_entries: Vec::new(),
                termination: semantic_vocabulary::Termination::NoGuarantee,
                content_entry_claims: Vec::new(),
                crash_routes: Vec::new(),
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                requires: Vec::new(),
                source_checked_requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct OutcomeSpecificGuard {
        pub result_type: StructuralTypeId,
        pub result_case: StructuralCaseId,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct OutcomeSpecificEnsure {
        pub guard: OutcomeSpecificGuard,
        pub position: u32,
        pub obligation: ObligationId,
        pub proposition: Proposition,
        pub evidence: Option<OutcomeSpecificEvidence>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct OutcomeSpecificEvidence {
        pub term: EvidenceTermId,
        pub output_field: String,
        pub argument_subjects: Vec<OutcomeSpecificCallArgumentSubstitution>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct OutcomeSpecificCallEvidence {
        pub guard: OutcomeSpecificGuard,
        pub position: u32,
        pub callee_obligation: ObligationId,
        pub callee_term: EvidenceTermId,
        pub output_field: String,
        pub callee_proposition: PropositionId,
        pub instantiated_proposition: PropositionId,
        pub output: EvidenceTermId,
        pub argument_substitutions: Vec<OutcomeSpecificCallArgumentSubstitution>,
        pub result_substitution: Option<OutcomeSpecificCallResultSubstitution>,
        pub validity: OutcomeSpecificCallEvidenceValidity,
        pub expected_use_count: u32,
        pub uses: Vec<OutcomeSpecificEvidenceUse>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct OutcomeSpecificEvidenceUse {
        pub target: MachineId,
        pub input_position: u32,
        pub target_requirement: PropositionId,
        pub target_term: EvidenceTermId,
        pub source: EvidenceTermId,
        pub instantiated_proposition: PropositionId,
        pub target_parameter: PlaceId,
        pub caller_result: PlaceId,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct OutcomeSpecificCallArgumentSubstitution {
        pub argument_position: u32,
        pub parameter_position: u32,
        pub structural_path: Option<Vec<crate::StructuralPathSegment>>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct OutcomeSpecificCallResultSubstitution {
        pub argument_position: u32,
        pub callee_result: PlaceId,
        pub caller_result: PlaceId,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct OutcomeSpecificCallEvidenceValidity {
        pub result: PlaceId,
        pub proposition_dependencies: Vec<PlaceId>,
        pub evidence_interface: EvidenceInterfaceIdentity,
        pub interface_dependencies: Vec<PlaceId>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct CrashRouteBucket {
        pub cause: CrashCause,
        pub alternatives: Vec<CrashRouteGuard>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum CrashRouteGuard {
        Truth,
        Predicate(CrashPredicateTerm),
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct CrashPredicateTerm(Proposition);

    impl CrashPredicateTerm {
        pub const fn new(proposition: Proposition) -> Self {
            Self(proposition)
        }

        pub const fn proposition(&self) -> &Proposition {
            &self.0
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ContractClause {
        pub obligation: ObligationId,
        pub proposition: Proposition,
    }
}
pub(crate) mod declarations {
    use crate::terminal_module::TerminalIdentityIndex;
    use semantic_vocabulary::{EvidenceTermId, MachineId};

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct PropositionDeclaration {
        pub id: semantic_vocabulary::PropositionId,
        pub name: String,
        pub binders: Vec<PropositionBinderDeclaration>,
        pub parameter_types: Vec<String>,
        pub evidence: PropositionEvidence,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct PropositionBinderDeclaration {
        pub name: String,
        pub kind: PropositionBinderKind,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum PropositionBinderKind {
        Type,
        Const {
            type_identity: TerminalIdentityIndex,
        },
        Machine,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum PropositionEvidence {
        FactOnly,
        Witness { evidence_type: String },
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct PropositionApplicationIdentity {
        pub id: semantic_vocabulary::PropositionId,
        pub declaration: semantic_vocabulary::PropositionId,
        pub binder_arguments: Vec<PropositionBinderArgumentIdentity>,
        pub arguments: Vec<String>,
        pub evidence_interface: Option<EvidenceInterfaceIdentity>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct PropositionBinderArgumentIdentity {
        pub kind: PropositionBinderArgumentKind,
        pub identity: TerminalIdentityIndex,
        pub evidence_projection: Option<EvidenceProjectionIdentity>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct EvidenceProjectionIdentity {
        pub term: EvidenceTermId,
        pub declaring_trait_identity: TerminalIdentityIndex,
        pub declaring_trait_arguments: Vec<String>,
        pub requirement_identity: TerminalIdentityIndex,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct EvidenceTermDeclaration {
        pub id: EvidenceTermId,
        pub proposition: semantic_vocabulary::PropositionId,
        pub interface: EvidenceInterfaceIdentity,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct EvidenceInterfaceIdentity {
        pub trait_identity: TerminalIdentityIndex,
        pub arguments: Vec<String>,
        pub requirements: Vec<EvidenceRequirementIdentity>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct EvidenceRequirementIdentity {
        pub declaring_trait_identity: TerminalIdentityIndex,
        pub declaring_trait_arguments: Vec<String>,
        pub requirement_identity: TerminalIdentityIndex,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum EvidenceContractLaneKind {
        Requires,
        Ensures,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum EvidenceArgumentSubject {
        Parameter(super::contracts::OutcomeSpecificCallArgumentSubstitution),
        Result { argument_position: u32 },
        Closed { argument_position: u32 },
        Unknown { argument_position: u32 },
    }

    impl EvidenceArgumentSubject {
        pub fn argument_position(&self) -> u32 {
            match self {
                Self::Parameter(parameter) => parameter.argument_position,
                Self::Result { argument_position }
                | Self::Closed { argument_position }
                | Self::Unknown { argument_position } => *argument_position,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct EvidenceContractLane {
        pub machine: MachineId,
        pub kind: EvidenceContractLaneKind,
        pub position: u32,
        pub term: EvidenceTermId,
        pub output_field: Option<String>,
        pub argument_subjects: Vec<EvidenceArgumentSubject>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum PropositionBinderArgumentKind {
        Type,
        Const,
        Machine,
    }
}
pub(crate) mod operation_crash_contracts {

    use crate::CrashRouteBucket;
    use semantic_vocabulary::{MachineId, OperationId};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalOperationCrashContract {
        pub machine: MachineId,
        pub operation: OperationId,
        pub published_routes: Vec<CrashRouteBucket>,
        pub crash_continuations: Vec<CrashRouteBucket>,
    }
}
pub(crate) mod outputs {
    use crate::ClosedConformanceApplicationCommitment;
    use crate::terminal_module::TerminalIdentityIndex;
    use semantic_vocabulary::{EvidenceTermId, MachineId, PropositionId, ScalarType};

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ProofOutputCall {
        pub caller: MachineId,
        pub ordinal: u32,
        pub target_machine_identity: TerminalIdentityIndex,
        pub static_requirement_dispatch: Option<StaticRequirementDispatch>,
        pub runtime_result: Option<ProofOutputRuntimeResult>,
        pub runtime_call: Option<ProofOutputRuntimeCall>,
        pub evidence_arguments: Vec<ProofOutputEvidenceArgument>,
        pub outputs: Vec<ProofOutput>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct StaticRequirementDispatch {
        pub conformance_application_report_fingerprint: u64,
        pub conformance_application_commitment: ClosedConformanceApplicationCommitment,
        pub public_requirement_identity: TerminalIdentityIndex,
        pub declaring_trait_identity: TerminalIdentityIndex,
        pub requirement_identity: TerminalIdentityIndex,
        pub realization_identity: TerminalIdentityIndex,
        pub realization_callable_identity: TerminalIdentityIndex,
        pub realization: MachineId,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ProofOutputEvidenceArgument {
        pub input_position: u32,
        pub callee_proposition: PropositionId,
        pub source: EvidenceTermId,
        pub instantiated_proposition: PropositionId,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum ProofOutputRuntimeResult {
        Unit,
        Scalar(ScalarType),
        NoNormal,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ProofOutputRuntimeCall {
        pub site: semantic_vocabulary::InvocationSite,
        pub callee: MachineId,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ProofOutput {
        pub output_position: u32,
        pub output_field: String,
        pub callee_proposition: PropositionId,
        pub callee_output: Option<EvidenceTermId>,
        pub instantiated_proposition: PropositionId,
        pub forwarded_input_position: Option<u32>,
        pub output: Option<EvidenceTermId>,
    }
}
pub(crate) mod recursion {
    use crate::terminal_module::TerminalIdentityIndex;
    use semantic_vocabulary::ContractId;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TerminalProofRankingRelation {
        StructuralSubterm,
        StructuralSubtermCycles,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct TerminalProofRecursiveMember {
        pub contract: ContractId,
        pub machine_identity: TerminalIdentityIndex,
        pub rank_parameter_identity: TerminalIdentityIndex,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct TerminalProofRecursiveField {
        pub identity: TerminalIdentityIndex,
        pub type_identity: TerminalIdentityIndex,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct TerminalProofRecursiveType {
        pub identity: TerminalIdentityIndex,
        pub fields: Vec<TerminalProofRecursiveField>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TerminalProofRecursiveTransitionLane {
        Target,
        Continuation,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TerminalProofRecursiveCallSite {
        Statement {
            state_identity: TerminalIdentityIndex,
            statement_index: u64,
        },
        Expression {
            state_identity: TerminalIdentityIndex,
            statement_index: u64,
            expression_ordinal: u64,
        },
        Transition {
            state_identity: TerminalIdentityIndex,
            statement_index: u64,
            lane: TerminalProofRecursiveTransitionLane,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct TerminalProofRecursiveEdge {
        pub caller: ContractId,
        pub callee: ContractId,
        pub site: TerminalProofRecursiveCallSite,
        pub strict_member_path: Vec<String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct TerminalProofRecursiveComponent {
        pub ranking_relation: TerminalProofRankingRelation,
        pub rank_type_identity: TerminalIdentityIndex,
        pub types: Vec<TerminalProofRecursiveType>,
        pub members: Vec<TerminalProofRecursiveMember>,
        pub edges: Vec<TerminalProofRecursiveEdge>,
    }
}
pub(crate) mod scalar_block_invariants {

    use semantic_vocabulary::{BlockId, EdgeId, MachineId, ObligationId, Proposition};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ScalarBlockInvariant {
        pub machine: MachineId,
        pub header: BlockId,
        pub predicate: Proposition,
        pub arrivals: Vec<ScalarBlockInvariantArrival>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ScalarBlockInvariantArrival {
        pub edge: EdgeId,
        pub obligation: ObligationId,
    }
}
pub(crate) mod values {

    use semantic_vocabulary::{
        BlockId, IeeeFloatFormat, IeeeFloatStructuralField, MachineId, OperationId, ValueId,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ProofValueId(pub u32);

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ProofPropositionId(pub u32);

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct FloatProjectionInputId(pub u32);

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct FloatProjectionContractIdentity {
        pub format: u16,
        pub operation: u8,
        pub declaration: u8,
        pub catalog_version: u16,
        pub commitment: [u8; 32],
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum FloatMeaningProjectionOperation {
        Meaning32,
        Meaning64,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum ProofOnlyValueType {
        FloatMeaning,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ProofValueDeclaration {
        pub id: ProofValueId,
        pub value_type: ProofOnlyValueType,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct FloatProjectionInput {
        pub id: FloatProjectionInputId,
        pub format: IeeeFloatFormat,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct DirectMachineFloatParameter {
        pub owner: MachineId,
        pub parameter: ValueId,
        pub format: IeeeFloatFormat,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct DirectMachineFloatResult {
        pub owner: MachineId,
        pub result: ValueId,
        pub format: IeeeFloatFormat,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct DirectBlockFloatParameter {
        pub owner: MachineId,
        pub block: BlockId,
        pub parameter: ValueId,
        pub format: IeeeFloatFormat,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct DirectOperationFloatResult {
        pub owner: MachineId,
        pub producer: OperationId,
        pub result: ValueId,
        pub format: IeeeFloatFormat,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct DirectCallFloatResult {
        pub owner: MachineId,
        pub producer: OperationId,
        pub result: ValueId,
        pub format: IeeeFloatFormat,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct DirectStructuralFloatLeaf {
        pub owner: MachineId,
        pub field: IeeeFloatStructuralField,
        pub format: IeeeFloatFormat,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct FloatSemanticContractIdentity {
        pub row: u8,
        pub catalog_version: u16,
        pub commitment: [u8; 32],
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum FloatSemanticApplicationOperand {
        Format(IeeeFloatFormat),
        Meaning(ProofValueId),
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct FloatSemanticApplication {
        pub contract: FloatSemanticContractIdentity,
        pub format: IeeeFloatFormat,
        pub operands: Vec<FloatSemanticApplicationOperand>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum FloatMeaningSource {
        TransitionalInput(FloatProjectionInput),
        DirectMachineParameter(DirectMachineFloatParameter),
        DirectMachineResult(DirectMachineFloatResult),
        DirectBlockParameter(DirectBlockFloatParameter),
        DirectOperationResult(DirectOperationFloatResult),
        DirectCallResult(DirectCallFloatResult),
        DirectStructuralLeaf(DirectStructuralFloatLeaf),
        ExactBinary32Literal(u32),
        ExactBinary64Literal(u64),
        SemanticApplication(FloatSemanticApplication),
    }

    impl FloatMeaningSource {
        pub const fn format(&self) -> IeeeFloatFormat {
            match self {
                Self::TransitionalInput(input) => input.format,
                Self::DirectMachineParameter(parameter) => parameter.format,
                Self::DirectMachineResult(result) => result.format,
                Self::DirectBlockParameter(parameter) => parameter.format,
                Self::DirectOperationResult(result) => result.format,
                Self::DirectCallResult(result) => result.format,
                Self::DirectStructuralLeaf(leaf) => leaf.format,
                Self::ExactBinary32Literal(_) => IeeeFloatFormat::Binary32,
                Self::ExactBinary64Literal(_) => IeeeFloatFormat::Binary64,
                Self::SemanticApplication(application) => application.format,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct FloatMeaningProjection {
        pub result: ProofValueDeclaration,
        pub source: FloatMeaningSource,
        pub operation: FloatMeaningProjectionOperation,
        pub contract: FloatProjectionContractIdentity,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FloatMeaningEqualityProposition {
        pub id: ProofPropositionId,
        pub left: ProofValueId,
        pub right: ProofValueId,
    }

    pub fn float_meaning_equality_proposition_id(index: u32) -> semantic_vocabulary::PropositionId {
        let mut encoder = crate::digest_encoder::DomainDigestEncoder::new(
            b"omega.terminal_psi.float_meaning_equality_proposition.v1",
        );
        encoder.raw(&index.to_le_bytes());
        let digest = encoder.finish();
        let raw = u64::from_le_bytes(
            digest[..8]
                .try_into()
                .expect("sha256 digests cover at least eight bytes"),
        ) | 1;
        semantic_vocabulary::PropositionId::new(raw).expect("a digest bit-or 1 is nonzero")
    }
}

pub use content::{
    ClaimContentProjection, ContentBackingAddressTransport, ContentBodyProjectionSubstitution,
    ContentBoundaryBodyRequirement, ContentClaimAddress, ContentClaimProjection, ContentEntryClaim,
    ContentIdentityReshuffle, ContentLineageStep, ContentPartitionComposition,
    ContentPlaceSubstitution, ContentTransformationCustody, ContentTransformationDerivation,
};
pub use contracts::{
    ContractClause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard, MachineContract,
    OutcomeSpecificCallArgumentSubstitution, OutcomeSpecificCallEvidence,
    OutcomeSpecificCallEvidenceValidity, OutcomeSpecificCallResultSubstitution,
    OutcomeSpecificEnsure, OutcomeSpecificEvidence, OutcomeSpecificEvidenceUse,
    OutcomeSpecificGuard,
};
pub use declarations::{
    EvidenceArgumentSubject, EvidenceContractLane, EvidenceContractLaneKind,
    EvidenceInterfaceIdentity, EvidenceProjectionIdentity, EvidenceRequirementIdentity,
    EvidenceTermDeclaration, PropositionApplicationIdentity, PropositionBinderArgumentIdentity,
    PropositionBinderArgumentKind, PropositionBinderDeclaration, PropositionBinderKind,
    PropositionDeclaration, PropositionEvidence,
};
pub use operation_crash_contracts::TerminalOperationCrashContract;
pub use outputs::{
    ProofOutput, ProofOutputCall, ProofOutputEvidenceArgument, ProofOutputRuntimeCall,
    ProofOutputRuntimeResult, StaticRequirementDispatch,
};
pub use recursion::{
    TerminalProofRankingRelation, TerminalProofRecursiveCallSite, TerminalProofRecursiveComponent,
    TerminalProofRecursiveEdge, TerminalProofRecursiveField, TerminalProofRecursiveMember,
    TerminalProofRecursiveTransitionLane, TerminalProofRecursiveType,
};
pub use scalar_block_invariants::{ScalarBlockInvariant, ScalarBlockInvariantArrival};
pub use values::{
    DirectBlockFloatParameter, DirectCallFloatResult, DirectMachineFloatParameter,
    DirectMachineFloatResult, DirectOperationFloatResult, DirectStructuralFloatLeaf,
    FloatMeaningEqualityProposition, FloatMeaningProjection, FloatMeaningProjectionOperation,
    FloatMeaningSource, FloatProjectionContractIdentity, FloatProjectionInput,
    FloatProjectionInputId, FloatSemanticApplication, FloatSemanticApplicationOperand,
    FloatSemanticContractIdentity, ProofOnlyValueType, ProofPropositionId, ProofValueDeclaration,
    ProofValueId, float_meaning_equality_proposition_id,
};
