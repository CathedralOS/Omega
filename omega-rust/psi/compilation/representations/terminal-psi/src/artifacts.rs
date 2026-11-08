/*
Replaceable proof, debug-map, machine-provenance, and program-entry companion records carried
alongside `TerminalModule` in canonical Terminal Psi artifacts (`TerminalDebugMap`,
`TerminalMachineProvenance`, `CheckedProgramEntryTerminalReceipt`, `CertificateEnvelope`, and
`ProofNode` trees). These records are pure data schemas; canonical codecs and independent verifiers
validate them at artifact boundaries without trusting producer construction.
*/

pub(crate) mod checked_program_entry {

    use semantic_vocabulary::MachineId;

    use crate::TerminalPsiIdentity;

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedProgramEntryTerminalReceipt {
        source_signature_identity: [u8; 32],
        source_machine_name: String,
        declaration: crate::NominalDeclarationIdentity,
        terminal_psi_identity: TerminalPsiIdentity,
        terminal_entry: MachineId,
    }

    impl CheckedProgramEntryTerminalReceipt {
        pub fn new(
            source_signature_identity: [u8; 32],
            source_machine_name: String,
            declaration: crate::NominalDeclarationIdentity,
            terminal_psi_identity: TerminalPsiIdentity,
            terminal_entry: MachineId,
        ) -> Self {
            Self {
                source_signature_identity,
                source_machine_name,
                declaration,
                terminal_psi_identity,
                terminal_entry,
            }
        }

        pub const fn source_signature_identity(&self) -> [u8; 32] {
            self.source_signature_identity
        }

        pub fn source_machine_name(&self) -> &str {
            &self.source_machine_name
        }

        pub const fn declaration(&self) -> &crate::NominalDeclarationIdentity {
            &self.declaration
        }

        pub const fn terminal_psi_identity(&self) -> TerminalPsiIdentity {
            self.terminal_psi_identity
        }

        pub const fn terminal_entry(&self) -> MachineId {
            self.terminal_entry
        }
    }
}
pub(crate) mod debug_map {

    use crate::TerminalPsiIdentity;
    use semantic_vocabulary::{
        BlockId, ClaimId, ContractId, EdgeId, MachineId, ObligationId, OperationId, PlaceId,
        ValueId,
    };
    use std::num::NonZeroU32;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct DebugFileId(NonZeroU32);

    impl DebugFileId {
        pub const fn new(raw: u32) -> Option<Self> {
            match NonZeroU32::new(raw) {
                Some(value) => Some(Self(value)),
                None => None,
            }
        }

        pub const fn get(self) -> u32 {
            self.0.get()
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct DebugSourceDigest([u8; 32]);

    impl DebugSourceDigest {
        pub const fn from_bytes(bytes: [u8; 32]) -> Self {
            Self(bytes)
        }

        pub const fn as_bytes(&self) -> &[u8; 32] {
            &self.0
        }
    }

    impl std::fmt::Debug for DebugSourceDigest {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            for byte in self.0 {
                write!(formatter, "{byte:02x}")?;
            }
            Ok(())
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum DebugSourceOrigin {
        User,
        Toolchain,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct DebugSourceFile {
        pub id: DebugFileId,
        pub origin: DebugSourceOrigin,
        pub byte_len: u64,
        pub digest: DebugSourceDigest,
        pub path: String,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct DebugSourceSpan {
        pub file: DebugFileId,
        pub start: u64,
        pub end: u64,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum DebugSubject {
        Machine(MachineId),
        Block(BlockId),
        Operation(OperationId),
        Edge(EdgeId),
        Value(ValueId),
        Contract(ContractId),
        Obligation(ObligationId),
        Place(PlaceId),
        Claim { machine: MachineId, claim: ClaimId },
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct DebugSite {
        pub subject: DebugSubject,
        pub span: DebugSourceSpan,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalDebugMap {
        pub semantic: TerminalPsiIdentity,
        pub files: Vec<DebugSourceFile>,
        pub sites: Vec<DebugSite>,
    }
}
pub(crate) mod machine_provenance {

    use semantic_vocabulary::MachineId;
    use source::SourceSpan;
    use std::collections::BTreeMap;

    #[derive(Debug, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct TerminalMachineProvenance {
        machines: BTreeMap<MachineId, (String, Option<SourceSpan>)>,
        callers: BTreeMap<MachineId, MachineId>,
    }

    impl TerminalMachineProvenance {
        pub fn for_root(
            &self,
            module: &crate::TerminalModule,
            entry: MachineId,
            name: Option<(&str, Option<SourceSpan>)>,
        ) -> Self {
            let mut provenance = Self {
                machines: self.machines.clone(),
                callers: BTreeMap::new(),
            };
            if let Some((name, span)) = name {
                provenance.machines.insert(entry, (name.to_owned(), span));
            }
            let mut pending = std::collections::VecDeque::from([entry]);
            let mut visited = std::collections::BTreeSet::from([entry]);
            while let Some(current) = pending.pop_front() {
                let Some(body) = module
                    .unoptimized_behavior
                    .iter()
                    .find(|body| body.machine == current)
                else {
                    continue;
                };
                for operation in &body.operations {
                    let mut visit = |callee| {
                        if visited.insert(callee) {
                            provenance.record_caller(callee, current);
                            pending.push_back(callee);
                        }
                    };
                    match operation {
                        crate::UnoptimizedBehaviorOperation::Invocation { callee, .. }
                        | crate::UnoptimizedBehaviorOperation::NominalCleanup { callee, .. } => {
                            visit(*callee)
                        }
                        crate::UnoptimizedBehaviorOperation::BoundaryInvocation {
                            boundary,
                            ..
                        } => {
                            for candidate in module
                                .provider_candidates
                                .iter()
                                .filter(|candidate| candidate.boundary == *boundary)
                            {
                                visit(candidate.candidate);
                            }
                        }
                        _ => {}
                    }
                }
            }
            provenance
        }

        pub fn name(&mut self, machine: MachineId, name: String, span: Option<SourceSpan>) {
            self.machines.entry(machine).or_insert((name, span));
        }

        pub fn record_caller(&mut self, machine: MachineId, caller: MachineId) {
            self.callers.entry(machine).or_insert(caller);
        }

        fn named_ancestor(
            &self,
            machine: MachineId,
        ) -> Option<(MachineId, &(String, Option<SourceSpan>))> {
            let mut current = machine;
            for _ in 0..=self.callers.len() {
                if let Some(named) = self.machines.get(&current) {
                    return Some((current, named));
                }
                current = *self.callers.get(&current)?;
            }
            None
        }

        pub fn label(&self, machine: MachineId) -> String {
            match self.named_ancestor(machine) {
                Some((named, (name, _))) if named == machine => {
                    format!("machine `{name}` (Terminal machine {machine})")
                }
                Some((named, (name, _))) => format!(
                    "Terminal machine {machine}, reached through machine `{name}` (Terminal machine {named})"
                ),
                None => format!("Terminal machine {machine}"),
            }
        }

        pub fn source_span(&self, machine: MachineId) -> Option<SourceSpan> {
            let (_, (_, span)) = self.named_ancestor(machine)?;
            *span
        }
    }

    #[cfg(test)]
    mod tests {
        use super::TerminalMachineProvenance;
        use crate::{TerminalModule, UnoptimizedBehaviorOperation, UnoptimizedMachineBehavior};
        use semantic_vocabulary::{BlockId, InvocationSite, MachineId, OperationId};

        #[test]
        fn root_attribution_uses_retained_edges_without_source_or_live_bodies() {
            let entry = MachineId::new(1).unwrap();
            let helper = MachineId::new(2).unwrap();
            let cleanup = MachineId::new(3).unwrap();
            let other = MachineId::new(4).unwrap();
            let block = BlockId::new(1).unwrap();
            let site = InvocationSite::Operation(OperationId::new(1).unwrap());
            let mut module = TerminalModule::declarations();
            module.unoptimized_behavior = vec![
                UnoptimizedMachineBehavior {
                    machine: entry,
                    operations: vec![UnoptimizedBehaviorOperation::Invocation {
                        block,
                        site,
                        callee: helper,
                    }],
                },
                UnoptimizedMachineBehavior {
                    machine: helper,
                    operations: vec![
                        UnoptimizedBehaviorOperation::Invocation {
                            block,
                            site,
                            callee: entry,
                        },
                        UnoptimizedBehaviorOperation::NominalCleanup {
                            block,
                            callee: cleanup,
                        },
                    ],
                },
                UnoptimizedMachineBehavior {
                    machine: other,
                    operations: Vec::new(),
                },
            ];
            let mut names = TerminalMachineProvenance::default();
            names.name(entry, "generic declaration label".into(), None);
            let rooted = names.for_root(&module, entry, Some(("selected application", None)));
            assert!(rooted.label(cleanup).contains("selected application"));
            assert!(!rooted.label(other).contains("selected application"));
            let another = names.for_root(&module, other, Some(("other application", None)));
            assert!(!another.label(cleanup).contains("selected application"));
            assert!(!another.label(cleanup).contains("other application"));
            assert!(
                names.label(entry).contains("generic declaration label"),
                "rooting mutated shared names"
            );
        }
    }
}
pub use machine_provenance::TerminalMachineProvenance;
pub(crate) mod proof_kernel {

    pub(crate) mod admission {
        use super::{EvidenceIdentity, PrimitiveJudgment, ProofNode};
        use semantic_vocabulary::{AdmissionSiteId, ProfileDecisionId};

        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct ProofSystemMarker;

        impl ProofSystemMarker {
            pub const CURRENT: Self = Self;

            pub const fn new(raw: u16) -> Option<Self> {
                if raw == Self::CURRENT.get() {
                    Some(Self::CURRENT)
                } else {
                    None
                }
            }

            pub const fn get(self) -> u16 {
                5
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum AdmissionKind {
            ForeignBoundaryGuarantee,
            ProviderFact,
            CheckedAssemblyClaim,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct CertificateEnvelope {
            pub identity: EvidenceIdentity,
            pub proof_system_marker: ProofSystemMarker,
            pub proof: ProofNode,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct AdmissionEvidence {
            pub site: AdmissionSiteId,
            pub kind: AdmissionKind,
            pub authority_identity: EvidenceIdentity,
            pub evidence_identity: EvidenceIdentity,
            pub profile_decision: ProfileDecisionId,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum EvidenceRoute {
            KernelDerived(PrimitiveJudgment),
            CertificateDerived(CertificateEnvelope),
            Admitted(AdmissionEvidence),
            SourceChecked,
        }
    }
    pub(crate) mod nodes {
        use super::{
            IntegerAffineWitness, IntegerCastChainWitness, IntegerCorrelatedForbiddenRootWitness,
            Proposition,
        };
        use semantic_vocabulary::IntegerMathTerm;
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum PrimitiveJudgment {
            Truth,
            ReflexiveEquality,
            ClosedIntegerRelation,
            IntegerCarrierBound,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct ProofNode {
            pub conclusion: Proposition,
            pub rule: ProofRule,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum ProofRule {
            ValueEqualityTransport {
                premise: Box<ProofNode>,
                equalities: Vec<ProofNode>,
            },
            PredicateDenotation {
                premise: Box<ProofNode>,
            },
            Primitive(PrimitiveJudgment),
            SemanticAxiom {
                index: usize,
            },
            Assumption {
                index: usize,
            },
            ConjunctionIntroduction(Vec<ProofNode>),
            ConjunctionElimination {
                conjunction: Box<ProofNode>,
                conjunct: usize,
            },
            DisjunctionIntroduction {
                disjunct: Box<ProofNode>,
                index: usize,
            },
            DisjunctionElimination {
                disjunction: Box<ProofNode>,
                branches: Vec<ProofNode>,
            },
            ImplicationIntroduction {
                body: Box<ProofNode>,
            },
            ImplicationElimination {
                implication: Box<ProofNode>,
                premise: Box<ProofNode>,
            },
            EqualityTransitivity {
                left_equals_middle: Box<ProofNode>,
                middle_equals_right: Box<ProofNode>,
            },
            EqualitySymmetry {
                equality: Box<ProofNode>,
            },
            IntegerOrderWeakening {
                relation: Box<ProofNode>,
            },
            IntegerOrderDiscreteness {
                relation: Box<ProofNode>,
            },
            IntegerMathOrderTranslation {
                relation: Box<ProofNode>,
                addend: IntegerMathTerm,
            },
            IntegerOrderAntisymmetry {
                forward: Box<ProofNode>,
                backward: Box<ProofNode>,
            },
            IntegerSubtractOrder {
                difference: Box<ProofNode>,
                positive: Box<ProofNode>,
            },
            IntegerAddOrder {
                sum: Box<ProofNode>,
                positive: Box<ProofNode>,
            },
            IntegerSubtractAntitone {
                smaller: Box<ProofNode>,
                larger: Box<ProofNode>,
                order: Box<ProofNode>,
            },
            IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: Box<ProofNode>,
                middle_less_or_equal_right: Box<ProofNode>,
            },
            IntegerStrictOrderTransitivity {
                left_to_middle: Box<ProofNode>,
                middle_to_right: Box<ProofNode>,
            },
            IntegerOrderSubstitution {
                relation: Box<ProofNode>,
                equality: Box<ProofNode>,
                endpoint: usize,
            },
            IntegerAffineBound {
                root_bound: Box<ProofNode>,
                witness: IntegerAffineWitness,
            },
            IntegerExactAddDefinitionBound {
                left_bound: Box<ProofNode>,
                right_bound: Box<ProofNode>,
                definition_axiom: usize,
            },
            IntegerExactSubtractDefinitionBound {
                left_bound: Box<ProofNode>,
                right_bound: Box<ProofNode>,
                definition_axiom: usize,
            },
            IntegerCastBound {
                root_bound: Box<ProofNode>,
                witness: IntegerCastChainWitness,
            },
            IntegerCorrelatedForbiddenRoots {
                witness: IntegerCorrelatedForbiddenRootWitness,
            },
        }
    }
    pub(crate) mod witnesses {
        use semantic_vocabulary::{Proposition, ScalarTerm};

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct IntegerAffineWitness {
            pub root: ScalarTerm,
            pub target: ScalarTerm,
            pub definition_axioms: Vec<usize>,
            pub literal_axioms: Vec<Option<usize>>,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct IntegerCastChainWitness {
            pub root: ScalarTerm,
            pub target: ScalarTerm,
            pub definition_axioms: Vec<usize>,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct CorrelatedAffineStepWitness {
            pub definition_axiom: usize,
            pub literal_axiom: Option<usize>,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct CorrelatedAffineBranchWitness {
            pub root: ScalarTerm,
            pub target: ScalarTerm,
            pub steps: Vec<CorrelatedAffineStepWitness>,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct IntegerCorrelatedForbiddenRootWitness {
            pub dividend: CorrelatedAffineBranchWitness,
            pub divisor: CorrelatedAffineBranchWitness,
            pub definition_axiom_count: usize,
            pub lower_bound_axiom: usize,
            pub upper_bound_axiom: usize,
            pub conclusion: Proposition,
        }
    }
    pub use admission::{
        AdmissionEvidence, AdmissionKind, CertificateEnvelope, EvidenceRoute, ProofSystemMarker,
    };
    pub use nodes::{PrimitiveJudgment, ProofNode, ProofRule};
    use semantic_vocabulary::{EvidenceIdentity, Proposition};
    pub use witnesses::{
        CorrelatedAffineBranchWitness, CorrelatedAffineStepWitness, IntegerAffineWitness,
        IntegerCastChainWitness, IntegerCorrelatedForbiddenRootWitness,
    };
}
pub use checked_program_entry::CheckedProgramEntryTerminalReceipt;
pub use debug_map::{
    DebugFileId, DebugSite, DebugSourceDigest, DebugSourceFile, DebugSourceOrigin, DebugSourceSpan,
    DebugSubject, TerminalDebugMap,
};
pub use proof_kernel::{
    AdmissionEvidence, AdmissionKind, CertificateEnvelope, CorrelatedAffineBranchWitness,
    CorrelatedAffineStepWitness, EvidenceRoute, IntegerAffineWitness, IntegerCastChainWitness,
    IntegerCorrelatedForbiddenRootWitness, PrimitiveJudgment, ProofNode, ProofRule,
    ProofSystemMarker,
};
