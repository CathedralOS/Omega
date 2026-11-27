use crate::{CrashCause, EvidenceInterfaceIdentity};
use semantic_vocabulary::{
    ContractId, EvidenceTermId, MachineId, ObligationId, PlaceId, Proposition, PropositionId,
    StructuralCaseId, StructuralFieldId, StructuralTypeId,
};

/// A logical membership of an exact runtime value. These facts do not
/// change its declared type or participate in result overload selection.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RuntimeDomainMembership {
    Scalar {
        subject: semantic_vocabulary::ScalarTerm,
        domain: semantic_vocabulary::ScalarDomainId,
    },
    Structural {
        root: PlaceId,
        path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
        version: semantic_vocabulary::ContentPlaceVersion,
        domain: semantic_vocabulary::StructuralDomainId,
    },
}

/// A structural-formal ordinal followed by zero or more field identities.
/// `None` retains the ownership ceiling; `Some([])` is a complete empty
/// frame. The owning contract supplies every parameter's exact type.
pub type PublishedWriteFrame = Option<Vec<(u32, Vec<StructuralFieldId>)>>;

/// One declaration's signature and published obligations. Keeping them
/// together lets a requirement retain its contract without inventing a body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineContract {
    pub id: ContractId,
    pub declaration: Option<crate::CheckedMachineDeclaration>,
    /// Closing an open effect promise depends on the exact selected
    /// application. Retain that association with this public header,
    /// rather than assigning the implementation's private contract to it.
    pub selected_application: Option<crate::StaticRequirementDispatch>,
    pub parameters: Vec<crate::ValueDeclaration>,
    pub parameter_order: Vec<crate::BoundaryParameterKind>,
    pub structural_parameters: Vec<crate::StructuralParameterDeclaration>,
    pub result: crate::TerminalMachineResult,
    /// The public signature's declared lifetime membership: every input a
    /// borrowed result leaf may borrow (lifetimes.md, "Returned views").
    /// Both a selected call (through its public header) and a rigid call
    /// (through the bodiless requirement) read their caller's loans from
    /// this one account; a body's actual origins stay in `result`. It
    /// grants no backing or loan. `None` leaves the frontier unpublished.
    pub result_lifetimes: Option<Vec<crate::DeclaredResultLifetime>>,
    pub entry_claims: Vec<crate::EntryClaim>,
    // Omitted reach and crash clauses can leave requirements open;
    // an explicit empty clause promises none. Entries follow their own
    // rule: omitted requirement invokes means the empty semantic ceiling.
    pub declared_service_reach: Option<Vec<semantic_vocabulary::ServiceId>>,
    pub declared_synchronous_entries: Option<Vec<semantic_vocabulary::ServiceId>>,
    pub declared_crash_routes: Option<Vec<CrashRouteBucket>>,
    /// Private omissions infer; explicit/public false forbids the effect.
    pub declared_suspension: Option<bool>,
    pub declared_blocking: Option<bool>,
    pub published_may_suspend: bool,
    pub published_may_block: bool,
    pub closed_reach_application: Option<crate::ClosedReachApplication>,
    pub published_service_ceiling: Vec<semantic_vocabulary::ServiceId>,
    /// Exact published synchronous entries, strictly increasing. Empty
    /// enters nothing; a boundary's parents are not added implicitly.
    pub published_synchronous_entries: Vec<semantic_vocabulary::ServiceId>,
    /// A call may invalidate only the existing storage it can write.
    /// Each row names a structural-formal ordinal and its field path.
    /// `None` retains the conservative ceiling; `Some([])` establishes
    /// an empty frame. This records effects, never invocation permission
    /// or a promise about the values left there.
    pub published_writes: PublishedWriteFrame,
    pub termination: semantic_vocabulary::Termination,
    pub content_entry_claims: Vec<crate::ContentEntryClaim>,
    pub crash_routes: Vec<CrashRouteBucket>,
    pub erased_scalar_formals: Vec<crate::ValueDeclaration>,
    pub erased_proof_formals: Vec<crate::ErasedProofFormal>,
    pub membership_requires: Vec<RuntimeDomainMembership>,
    pub membership_ensures: Vec<RuntimeDomainMembership>,
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
            selected_application: None,
            parameters: Vec::new(),
            parameter_order: Vec::new(),
            structural_parameters: Vec::new(),
            result: crate::TerminalMachineResult::Unit,
            result_lifetimes: None,
            entry_claims: Vec::new(),
            declared_service_reach: None,
            declared_synchronous_entries: None,
            declared_crash_routes: None,
            declared_suspension: None,
            declared_blocking: None,
            published_may_suspend: false,
            published_may_block: false,
            closed_reach_application: None,
            published_service_ceiling: Vec::new(),
            published_synchronous_entries: Vec::new(),
            published_writes: None,
            termination: semantic_vocabulary::Termination::NoGuarantee,
            content_entry_claims: Vec::new(),
            crash_routes: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            requires: Vec::new(),
            membership_requires: Vec::new(),
            membership_ensures: Vec::new(),
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
