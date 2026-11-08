/*
Built-in Psi optimization pass catalog (`PSI_PASS_CATALOG`), ordered rule
registries (`OrderedRuleRegistry`, `PsiOptimizationRule`, `RuleAnalysisView`),
shared unused-node elision and accepted-obligation helpers, copy propagation,
dead scalar elimination, and specialization pass registrations.
*/

pub(crate) const SCCP_PASS_NAME: &[u8] =
    b"omega.psi-pass.sparse-conditional-constant-propagation.v5";
pub(crate) const CONTROL_FLOW_CLEANUP_PASS_NAME: &[u8] = b"omega.psi-pass.control-flow-cleanup.v13";
pub(crate) const COPY_PROPAGATION_PASS_NAME: &[u8] = b"omega.psi-pass.copy-propagation.v1";
pub(crate) const DEAD_PURE_SCALAR_PASS_NAME: &[u8] =
    b"omega.psi-pass.dead-pure-scalar-elimination.v2";
pub(crate) const PROOF_CHECK_ELISION_PASS_NAME: &[u8] = b"omega.psi-pass.proof-check-elision.v12";
pub(crate) const GLOBAL_VALUE_NUMBERING_PASS_NAME: &[u8] =
    b"omega.psi-pass.global-value-numbering.v14";
pub(crate) const STATE_SPECIALIZATION_PASS_NAME: &[u8] = b"omega.psi-pass.state-specialization.v1";
pub(crate) const REPRESENTATION_SPECIALIZATION_PASS_NAME: &[u8] =
    b"omega.psi-pass.representation-specialization.v1";

pub(crate) fn standard_rule_contract(
    identity: &'static [u8],
    pass: &'static [u8],
    version: u32,
    required_analyses: AnalysisSet,
    invalidated_analyses: AnalysisInvalidationSet,
    safety_class: OptimizationSafetyClass,
) -> OptimizationRuleContract {
    OptimizationRuleContract::new(
        OptimizationRuleIdentity::from_canonical_bytes(identity),
        OptimizationPassIdentity::from_canonical_bytes(pass),
        version,
        required_analyses,
        invalidated_analyses,
        safety_class,
    )
    .expect("built-in rule has nonzero version")
}

fn representation_specialization_contract(identity: &'static [u8]) -> OptimizationRuleContract {
    standard_rule_contract(
        identity,
        REPRESENTATION_SPECIALIZATION_PASS_NAME,
        1,
        AnalysisSet::new([AnalysisKind::StronglyConnectedComponents]),
        AnalysisInvalidationSet::new([
            AnalysisKind::UseDefinition,
            AnalysisKind::ScalarConstants,
            AnalysisKind::ValueRanges,
            AnalysisKind::EffectSummaries,
        ]),
        OptimizationSafetyClass::StructuralIdentity,
    )
}

macro_rules! psi_rule_contract {
    (@adapter) => {
        fn contract(&self) -> OptimizationRuleContract {
            Self::contract()
        }
    };
    ($(#[$meta:meta])* $name:ident, $contract:expr $(,)?) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, Default)]
        pub(crate) struct $name;

        impl $name {
            pub fn contract() -> OptimizationRuleContract {
                $contract
            }
        }
    };
    ($(#[$meta:meta])* $name:ident, $contract:expr, propose = $propose:ident $(, $extra:expr)* $(,)?) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, Default)]
        pub(crate) struct $name;

        impl $name {
            pub fn contract() -> OptimizationRuleContract {
                $contract
            }
        }

        impl PsiOptimizationRule for $name {
            fn contract(&self) -> OptimizationRuleContract {
                Self::contract()
            }

            fn propose(
                &self,
                unit: &PsiOptimizationUnit,
                analyses: RuleAnalysisView<'_>,
            ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
                $propose(unit, analyses, Self::contract(), $($extra),*)
            }
        }
    };
}

macro_rules! psi_rule_impl {
    ($name:ident, |$unit:ident, $analyses:ident| $body:block $(,)?) => {
        impl PsiOptimizationRule for $name {
            psi_rule_contract!(@adapter);

            fn propose(
                &self,
                $unit: &PsiOptimizationUnit,
                $analyses: RuleAnalysisView<'_>,
            ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> $body
        }
    };
}

macro_rules! required_analysis {
    ($analyses:expr, $kind:expr => $($product:tt)*) => {
        let $($product)* = $analyses.get($kind) else {
            return Err(RuleProposalError::MissingAnalysis($kind));
        };
    };
}

pub(crate) mod accepted_obligations {
    use optimization_core::AcceptedObligationFactIdentity;
    use optimization_unit::{OptimizationFact, PsiOptimizationUnit};
    use semantic_vocabulary::{MachineId, OperationId};

    use crate::RuleProposalError;

    pub(crate) fn accepted_obligation_fact(
        unit: &PsiOptimizationUnit,
        machine: MachineId,
        operation: OperationId,
    ) -> Result<AcceptedObligationFactIdentity, RuleProposalError> {
        let obligation = unit.function(machine).and_then(|function| {
            function.facts.iter().find_map(|fact| match fact {
                OptimizationFact::OperationObligationReference {
                    obligation,
                    support,
                } if *support == operation => Some(*obligation),
                _ => None,
            })
        });
        let Some(obligation) = obligation else {
            return Err(RuleProposalError::MissingAcceptedObligation {
                machine,
                operation,
                obligation: None,
            });
        };
        unit.accepted_obligation_fact(
            machine,
            optimization_unit::PsiProvenance::Operation(operation),
            Some(obligation),
        )
        .map(|fact| fact.identity)
        .ok_or(RuleProposalError::MissingAcceptedObligation {
            machine,
            operation,
            obligation: Some(obligation),
        })
    }
}
pub(crate) mod catalog {
    use std::sync::Arc;

    use optimization::PsiOptimization;

    use crate::rules::PsiPassStep;
    use crate::rules::{
        control_flow_cleanup, copy_propagation, dead_scalar_elimination, global_value_numbering,
        proof_check_elision, representation_specialization_rule_registrations,
        sparse_conditional_constant_propagation, state_specialization,
    };
    use crate::{OrderedRuleRegistry, PsiOptimizationRule, RuleRegistryError};

    type RuleCatalog = fn() -> Vec<BuiltInRuleRegistration>;

    #[derive(Clone, Copy)]
    enum PsiPassCatalogPayload {
        RuleCatalog(RuleCatalog),
        LoopInvariantScalarMotion,
    }

    #[derive(Clone, Copy)]
    pub(crate) struct PsiPassCatalogEntry {
        optimization: PsiOptimization,
        payload: PsiPassCatalogPayload,
    }

    impl PsiPassCatalogEntry {
        const fn new(optimization: PsiOptimization, rule_catalog: RuleCatalog) -> Self {
            Self {
                optimization,
                payload: PsiPassCatalogPayload::RuleCatalog(rule_catalog),
            }
        }

        const fn session_step(optimization: PsiOptimization) -> Self {
            Self {
                optimization,
                payload: PsiPassCatalogPayload::LoopInvariantScalarMotion,
            }
        }

        pub const fn optimization(self) -> PsiOptimization {
            self.optimization
        }

        pub(crate) fn step(self) -> Result<PsiPassStep, RuleRegistryError> {
            match self.payload {
                PsiPassCatalogPayload::RuleCatalog(rule_catalog) => Ok(
                    PsiPassStep::RewriteRegistry(assemble_built_in_registry(rule_catalog())?),
                ),
                PsiPassCatalogPayload::LoopInvariantScalarMotion => {
                    Ok(PsiPassStep::LoopInvariantScalarMotion)
                }
            }
        }
    }

    pub(crate) const PSI_PASS_CATALOG: [PsiPassCatalogEntry; 9] = [
        PsiPassCatalogEntry::new(
            PsiOptimization::SparseConditionalConstantPropagation,
            sparse_conditional_constant_propagation::built_in_registrations,
        ),
        PsiPassCatalogEntry::new(
            PsiOptimization::ControlFlowCleanup,
            control_flow_cleanup::built_in_registrations,
        ),
        PsiPassCatalogEntry::new(
            PsiOptimization::CopyPropagation,
            copy_propagation::built_in_registrations,
        ),
        PsiPassCatalogEntry::new(
            PsiOptimization::GlobalValueNumbering,
            global_value_numbering::built_in_registrations,
        ),
        PsiPassCatalogEntry::new(
            PsiOptimization::ProofCheckElision,
            proof_check_elision::built_in_registrations,
        ),
        PsiPassCatalogEntry::new(
            PsiOptimization::DeadPureScalarElimination,
            dead_scalar_elimination::built_in_registrations,
        ),
        PsiPassCatalogEntry::new(
            PsiOptimization::StateSpecialization,
            state_specialization::built_in_registrations,
        ),
        PsiPassCatalogEntry::new(
            PsiOptimization::RepresentationSpecialization,
            representation_specialization_rule_registrations,
        ),
        PsiPassCatalogEntry::session_step(PsiOptimization::LoopInvariantScalarMotion),
    ];

    #[derive(Debug, Clone)]
    pub(crate) struct BuiltInRuleRegistration {
        schedule_ordinal: u16,
        rule: Arc<dyn PsiOptimizationRule>,
    }

    impl BuiltInRuleRegistration {
        pub(crate) fn new(schedule_ordinal: u16, rule: impl PsiOptimizationRule + 'static) -> Self {
            Self {
                schedule_ordinal,
                rule: Arc::new(rule),
            }
        }
    }

    pub(crate) fn assemble_built_in_registry(
        mut registrations: Vec<BuiltInRuleRegistration>,
    ) -> Result<OrderedRuleRegistry, RuleRegistryError> {
        registrations.sort_by_key(|registration| registration.schedule_ordinal);
        for (expected, registration) in registrations.iter().enumerate() {
            let expected = u16::try_from(expected).expect("built-in rule schedule fits u16");
            assert_eq!(
                registration.schedule_ordinal, expected,
                "built-in rule schedule ordinals must be unique and contiguous"
            );
        }
        OrderedRuleRegistry::new(
            registrations
                .into_iter()
                .map(|registration| registration.rule),
        )
    }
}
pub(crate) mod node_elision {
    use abstract_operations::AbstractOperation;
    use optimization_core::{AnalysisKind, OptimizationRuleContract};
    use optimization_unit::{
        DeadScalarNodeRewrite, NodeLocation, ProvenanceRewrite, PsiOptimizationFunction,
        PsiOptimizationUnit, PsiRealizationSite, PsiRewriteCandidate,
    };
    use semantic_vocabulary::{BlockId, OperationId, ScalarType, ValueId};

    use super::accepted_obligations::accepted_obligation_fact;
    use crate::rules::control_flow_cleanup::{
        node_realized_provenance, provenance_realized_at, record_block_provenance,
    };
    use crate::{AnalysisProduct, RuleAnalysisView, RuleProposalError};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct DeadScalarShape {
        pub(crate) source_operation: OperationId,
        pub(crate) result: ValueId,
        pub(crate) scalar_type: ScalarType,
    }

    type Classifier = fn(&AbstractOperation) -> Option<DeadScalarShape>;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Evidence {
        Structural,
        AcceptedObligation,
    }

    pub(crate) fn propose_unproved_dead_scalar_nodes(
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
        contract: OptimizationRuleContract,
        classify: Classifier,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose(unit, analyses, contract, classify, Evidence::Structural)
    }

    pub(crate) fn propose_proof_certified_dead_scalar_nodes(
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
        contract: OptimizationRuleContract,
        classify: Classifier,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose(
            unit,
            analyses,
            contract,
            classify,
            Evidence::AcceptedObligation,
        )
    }

    fn propose(
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
        contract: OptimizationRuleContract,
        classify: Classifier,
        evidence: Evidence,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        required_analysis!(analyses, AnalysisKind::ValueLiveness => Some(AnalysisProduct::ValueLiveness(liveness)));
        required_analysis!(analyses, AnalysisKind::EffectSummaries => Some(AnalysisProduct::EffectSummaries(effects)));
        let mut candidates = Vec::new();
        for function in &unit.functions {
            for block in &function.blocks {
                for (node_index, node) in block.nodes.iter().enumerate() {
                    let Some(shape) = classify(&node.operation) else {
                        continue;
                    };
                    let Some(next) = block.nodes.get(node_index + 1) else {
                        continue;
                    };
                    if next
                        .provenance
                        .iter()
                        .any(|source| node.provenance.contains(source))
                    {
                        continue;
                    }
                    let node_index =
                        u32::try_from(node_index).expect("optimization node index fits u32");
                    let live = liveness
                        .blocks
                        .iter()
                        .find(|row| row.machine == function.machine && row.block == block.id)
                        .and_then(|row| row.nodes.iter().find(|row| row.node == node_index));
                    let effect = effects.node(function.machine, block.id, node_index);
                    if live.is_none_or(|row| row.exit.contains(&shape.result))
                        || effect.is_none_or(|row| !row.is_pure_scalar_baseline(unit.identity))
                    {
                        continue;
                    }
                    let location = NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    };
                    let Some((affected_blocks, provenance)) =
                        node_elision_accounting(function, location, shape.result)
                    else {
                        continue;
                    };
                    let patch = DeadScalarNodeRewrite {
                        location,
                        source_operation: shape.source_operation,
                        result: shape.result,
                        scalar_type: shape.scalar_type,
                    };
                    let candidate = match evidence {
                        Evidence::Structural => PsiRewriteCandidate::new_dead_scalar_node(
                            unit.identity,
                            contract,
                            affected_blocks,
                            provenance,
                            -1,
                            patch,
                        ),
                        Evidence::AcceptedObligation => {
                            PsiRewriteCandidate::new_proof_certified_dead_scalar_node(
                                unit.identity,
                                contract,
                                affected_blocks,
                                provenance,
                                accepted_obligation_fact(
                                    unit,
                                    function.machine,
                                    shape.source_operation,
                                )?,
                                -1,
                                patch,
                            )
                        }
                    };
                    candidates.push(candidate.map_err(RuleProposalError::InvalidCandidate)?);
                }
            }
        }
        Ok(candidates)
    }

    pub(crate) fn node_elision_accounting(
        function: &PsiOptimizationFunction,
        removed_location: NodeLocation,
        removed_result: ValueId,
    ) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
        let block_position = function.block_index(removed_location.block)?;
        let node_position = usize::try_from(removed_location.node).ok()?;
        let block = &function.blocks[block_position];
        let removed = block.nodes.get(node_position)?;
        block.nodes.get(node_position.checked_add(1)?)?;
        let mut provenance = vec![node_realized_provenance(removed_location, removed)];
        for (index, node) in block.nodes.iter().enumerate().skip(node_position + 1) {
            if node.provenance.is_empty() {
                continue;
            }
            let old = NodeLocation {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(index).ok()?,
            };
            let new = NodeLocation {
                node: old.node.checked_sub(1)?,
                ..old
            };
            provenance.push(provenance_realized_at(
                PsiRealizationSite::Node(old),
                PsiRealizationSite::Node(new),
                &node.provenance,
                &node.work,
            ));
        }
        let mut affected = vec![block.id];
        for later in function.blocks.iter().skip(block_position + 1) {
            affected.push(later.id);
            record_block_provenance(function.machine, later, &mut provenance)?;
        }
        for use_block in &function.blocks {
            if affected.contains(&use_block.id)
                || !use_block.uses().any(|row| row.value == removed_result)
            {
                continue;
            }
            affected.push(use_block.id);
            record_block_provenance(function.machine, use_block, &mut provenance)?;
        }
        affected.sort();
        provenance.sort_by_key(ProvenanceRewrite::canonical_sort_key);
        Some((affected, provenance))
    }
}
pub(crate) mod registry {
    use std::{collections::BTreeSet, sync::Arc};

    use optimization::PsiOptimization;
    use optimization_core::{
        AnalysisKind, AnalysisSet, OptimizationPassIdentity, OptimizationRuleContract,
        OptimizationRuleIdentity, OptimizationRuleSetIdentity,
    };
    use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate, PsiRewriteCandidateError};
    use semantic_vocabulary::{MachineId, ObligationId, OperationId};

    use crate::analyses::analysis_index;
    use crate::{AnalysisProduct, EffectSummaryAnalysis, UseDefinitionAnalysis};

    #[derive(Debug, Clone, Copy)]
    pub(crate) struct RuleAnalysisView<'a> {
        products: &'a [Option<AnalysisProduct>],
        requested: AnalysisSet,
    }

    impl<'a> RuleAnalysisView<'a> {
        pub(crate) const fn new(
            products: &'a [Option<AnalysisProduct>],
            requested: AnalysisSet,
        ) -> Self {
            Self {
                products,
                requested,
            }
        }

        pub fn get(self, kind: AnalysisKind) -> Option<&'a AnalysisProduct> {
            if !self.requested.contains(kind) {
                return None;
            }
            self.products[analysis_index(kind)].as_ref()
        }

        pub(crate) fn use_definitions_and_effects(
            self,
        ) -> Result<(&'a UseDefinitionAnalysis, &'a EffectSummaryAnalysis), RuleProposalError>
        {
            required_analysis!(self, AnalysisKind::UseDefinition => Some(AnalysisProduct::UseDefinition(use_definitions)));
            required_analysis!(self, AnalysisKind::EffectSummaries => Some(AnalysisProduct::EffectSummaries(effects)));
            Ok((use_definitions, effects))
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum RuleProposalError {
        MissingAnalysis(AnalysisKind),
        InvalidCandidate(PsiRewriteCandidateError),
        MissingAcceptedObligation {
            machine: MachineId,
            operation: OperationId,
            obligation: Option<ObligationId>,
        },
    }

    failed_display!(RuleProposalError, "Psi optimizer rule proposal failed");

    pub(crate) trait PsiOptimizationRule: std::fmt::Debug + Send + Sync {
        fn contract(&self) -> OptimizationRuleContract;

        fn propose(
            &self,
            unit: &PsiOptimizationUnit,
            analyses: RuleAnalysisView<'_>,
        ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError>;
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub(crate) struct RuleScheduleKey {
        pub pass: OptimizationPassIdentity,
        pub rule: OptimizationRuleIdentity,
    }

    impl RuleScheduleKey {
        pub const fn from_contract(contract: OptimizationRuleContract) -> Self {
            Self {
                pass: contract.pass(),
                rule: contract.identity(),
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RuleRegistryError {
        UnsupportedOptimization(PsiOptimization),
        UnsupportedOptimizationCombination,
        DuplicateRule(OptimizationRuleIdentity),
        MixedPasses {
            expected: OptimizationPassIdentity,
            actual: OptimizationPassIdentity,
        },
    }

    failed_display!(RuleRegistryError, "invalid ordered Psi optimizer registry");

    #[derive(Debug, Clone)]
    pub(crate) struct OrderedRuleRegistry {
        rules: Arc<[Arc<dyn PsiOptimizationRule>]>,
        pass: Option<OptimizationPassIdentity>,
        identity: OptimizationRuleSetIdentity,
    }

    impl OrderedRuleRegistry {
        pub fn new(
            rules: impl IntoIterator<Item = Arc<dyn PsiOptimizationRule>>,
        ) -> Result<Self, RuleRegistryError> {
            let rules = rules.into_iter().collect::<Vec<_>>();
            let mut seen = BTreeSet::new();
            let mut pass = None;
            let mut identities = Vec::with_capacity(rules.len());
            for rule in &rules {
                let contract = rule.contract();
                let key = RuleScheduleKey::from_contract(contract);
                match pass {
                    None => pass = Some(key.pass),
                    Some(expected) if expected != key.pass => {
                        return Err(RuleRegistryError::MixedPasses {
                            expected,
                            actual: key.pass,
                        });
                    }
                    Some(_) => {}
                }
                if !seen.insert(key.rule) {
                    return Err(RuleRegistryError::DuplicateRule(key.rule));
                }
                identities.push(key.rule);
            }
            let identity = OptimizationRuleSetIdentity::from_ordered_rules(&identities)
                .map_err(|duplicate| RuleRegistryError::DuplicateRule(duplicate.0))?;
            Ok(Self {
                rules: rules.into(),
                pass,
                identity,
            })
        }

        pub const fn pass(&self) -> Option<OptimizationPassIdentity> {
            self.pass
        }

        pub const fn identity(&self) -> OptimizationRuleSetIdentity {
            self.identity
        }

        pub fn len(&self) -> usize {
            self.rules.len()
        }

        pub fn iter(&self) -> impl ExactSizeIterator<Item = &dyn PsiOptimizationRule> {
            self.rules.iter().map(Arc::as_ref)
        }

        pub fn contracts(&self) -> impl ExactSizeIterator<Item = OptimizationRuleContract> + '_ {
            self.iter().map(PsiOptimizationRule::contract)
        }
    }
}

pub(crate) mod case_membership_specialization {
    use optimization_core::{AnalysisKind, OptimizationRuleContract};
    use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

    use crate::rules::catalog::BuiltInRuleRegistration;
    use crate::{
        AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError,
        representation_specialization,
    };

    psi_rule_contract!(
        CaseMembershipSpecializationRule,
        crate::rules::representation_specialization_contract(
            b"omega.psi-rule.case-membership-specialization.v1",
        ),
    );

    psi_rule_impl!(CaseMembershipSpecializationRule, |unit, analyses| {
        required_analysis!(analyses, AnalysisKind::StronglyConnectedComponents => Some(AnalysisProduct::StronglyConnectedComponents(scc)));
        let frozen = scc.cyclic_machines(unit);
        let mut candidates = Vec::new();
        for function in &unit.functions {
            if frozen.contains(&function.machine) {
                continue;
            }
            for declaration in &function.structural_places {
                let Some(patch) =
                    representation_specialization::propose::plan(unit, function, declaration.id)
                else {
                    continue;
                };
                if patch.memberships.is_empty() {
                    continue;
                }
                let Some(provenance) =
                    representation_specialization::accounting::provenance_rows(function, &patch)
                else {
                    continue;
                };
                let mut affected_blocks = patch
                    .memberships
                    .iter()
                    .map(|row| row.site.block)
                    .collect::<Vec<_>>();
                affected_blocks.sort_unstable();
                affected_blocks.dedup();
                let predicted_cost_delta =
                    -i64::try_from(patch.memberships.len()).unwrap_or(i64::MAX);
                candidates.push(
                    PsiRewriteCandidate::new_case_membership_specialization(
                        unit.identity,
                        Self::contract(),
                        affected_blocks,
                        provenance,
                        predicted_cost_delta,
                        patch,
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
        Ok(candidates)
    });

    pub(crate) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
        vec![BuiltInRuleRegistration::new(
            0,
            CaseMembershipSpecializationRule,
        )]
    }
}
pub(crate) use crate::control_flow_cleanup;
pub(crate) mod copy_propagation {
    pub(crate) mod redundant_block_parameter {
        use std::collections::BTreeSet;

        use optimization_core::{
            AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationRuleContract,
            OptimizationSafetyClass,
        };
        use optimization_unit::{
            BlockParameterIncomingBinding, NodeLocation, PsiOptimizationUnit, PsiRealizationSite,
            PsiRewriteCandidate, RedundantBlockParameterRewrite, RedundantBlockParameterWitness,
        };

        use crate::rules::COPY_PROPAGATION_PASS_NAME;
        use crate::rules::control_flow_cleanup::provenance_realized_at;
        use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

        psi_rule_contract!(
            RedundantBlockParameterRule,
            crate::rules::standard_rule_contract(
                b"omega.psi-rule.redundant-block-parameter.v1",
                COPY_PROPAGATION_PASS_NAME,
                1,
                AnalysisSet::new([
                    AnalysisKind::ControlFlowGraph,
                    AnalysisKind::Dominators,
                    AnalysisKind::UseDefinition,
                ]),
                AnalysisInvalidationSet::new([
                    AnalysisKind::UseDefinition,
                    AnalysisKind::EffectSummaries,
                ]),
                OptimizationSafetyClass::StructuralIdentity
            ),
            propose = propose,
        );

        pub(crate) fn propose(
            unit: &PsiOptimizationUnit,
            analyses: RuleAnalysisView<'_>,
            contract: OptimizationRuleContract,
        ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
            required_analysis!(analyses, AnalysisKind::ControlFlowGraph => Some(AnalysisProduct::ControlFlowGraph(_)));
            required_analysis!(analyses, AnalysisKind::Dominators => Some(AnalysisProduct::Dominators(dominators)));
            required_analysis!(analyses, AnalysisKind::UseDefinition => Some(AnalysisProduct::UseDefinition(use_definitions)));

            let mut candidates = Vec::new();
            for function in &unit.functions {
                let machine_dominators = dominators.rows(function.machine);
                for block in function
                    .blocks
                    .iter()
                    .filter(|block| block.id != function.entry)
                {
                    for (position, parameter) in block.parameters.iter().enumerate() {
                        let mut incoming = Vec::new();
                        for source in &function.blocks {
                            for node in &source.nodes {
                                for edge in &node.successors {
                                    if edge.target != block.id {
                                        continue;
                                    }
                                    let Some(binding) = edge.bindings.get(position) else {
                                        continue;
                                    };
                                    incoming.push(BlockParameterIncomingBinding {
                                        source: source.id,
                                        edge: edge.psi_edge,
                                        argument: binding.argument,
                                    });
                                }
                            }
                        }
                        incoming.sort_by_key(|row| (row.edge, row.source));
                        let Some(replacement) = incoming.first().map(|row| row.argument) else {
                            continue;
                        };
                        if replacement == parameter.value
                            || incoming.iter().any(|row| row.argument != replacement)
                            || !use_definitions.replacement_dominates_parameter_uses(
                                function.machine,
                                replacement,
                                parameter.value,
                                machine_dominators,
                            )
                        {
                            continue;
                        }

                        let mut affected_blocks = BTreeSet::from([block.id]);
                        let mut provenance = Vec::new();
                        for source in &function.blocks {
                            for (node_index, node) in source.nodes.iter().enumerate() {
                                let changes_use = node
                                    .uses
                                    .iter()
                                    .any(|use_site| use_site.value == parameter.value);
                                for edge in node
                                    .successors
                                    .iter()
                                    .filter(|edge| edge.target == block.id)
                                {
                                    affected_blocks.insert(source.id);
                                    let site = PsiRealizationSite::Edge {
                                        machine: function.machine,
                                        edge: edge.psi_edge,
                                    };
                                    provenance.push(provenance_realized_at(
                                        site,
                                        site,
                                        &edge.provenance,
                                        &edge.work,
                                    ));
                                }
                                if changes_use {
                                    affected_blocks.insert(source.id);
                                    if !node.provenance.is_empty() {
                                        let site = PsiRealizationSite::Node(NodeLocation {
                                            machine: function.machine,
                                            block: source.id,
                                            node: u32::try_from(node_index)
                                                .expect("unit node index fits u32"),
                                        });
                                        provenance.push(provenance_realized_at(
                                            site,
                                            site,
                                            &node.provenance,
                                            &node.work,
                                        ));
                                    }
                                }
                            }
                        }
                        provenance
                            .sort_by_key(optimization_unit::ProvenanceRewrite::canonical_sort_key);
                        candidates.push(
                            PsiRewriteCandidate::new_redundant_block_parameter(
                                unit.identity,
                                contract,
                                affected_blocks.into_iter().collect(),
                                provenance,
                                RedundantBlockParameterWitness { incoming },
                                -1,
                                RedundantBlockParameterRewrite {
                                    machine: function.machine,
                                    block: block.id,
                                    position: u32::try_from(position)
                                        .expect("unit parameter position fits u32"),
                                    parameter: parameter.value,
                                    replacement,
                                    scalar_type: parameter.scalar_type,
                                },
                            )
                            .map_err(RuleProposalError::InvalidCandidate)?,
                        );
                    }
                }
            }
            Ok(candidates)
        }
    }

    pub(crate) use redundant_block_parameter::RedundantBlockParameterRule;

    use crate::rules::catalog::BuiltInRuleRegistration;

    pub(crate) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
        vec![BuiltInRuleRegistration::new(0, RedundantBlockParameterRule)]
    }
}
pub(crate) mod dead_scalar_elimination {
    use crate::rules::catalog::BuiltInRuleRegistration;

    pub(crate) mod dead_scalar_literal_elimination {
        use abstract_operations::AbstractOperation as O;
        use optimization_core::{
            AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationRuleContract,
            OptimizationSafetyClass,
        };
        use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};
        use semantic_vocabulary::ScalarType;

        use crate::rules::DEAD_PURE_SCALAR_PASS_NAME;
        use crate::rules::node_elision::{DeadScalarShape, propose_unproved_dead_scalar_nodes};
        use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

        psi_rule_contract!(
            DeadScalarLiteralEliminationRule,
            crate::rules::standard_rule_contract(
                b"omega.psi-rule.dead-unused-scalar-literal-elimination.v1",
                DEAD_PURE_SCALAR_PASS_NAME,
                1,
                AnalysisSet::new([AnalysisKind::ValueLiveness, AnalysisKind::EffectSummaries]),
                AnalysisInvalidationSet::new([
                    AnalysisKind::UseDefinition,
                    AnalysisKind::EffectSummaries,
                ]),
                OptimizationSafetyClass::ExactOperationSemantics
            ),
            propose = propose_unproved_dead_scalar_nodes,
            classify,
        );

        pub(crate) fn classify(operation: &O) -> Option<DeadScalarShape> {
            match operation {
                O::IntegerConstant {
                    psi_operation,
                    result,
                    scalar_type,
                    ..
                } => Some(DeadScalarShape {
                    source_operation: *psi_operation,
                    result: *result,
                    scalar_type: *scalar_type,
                }),
                O::BooleanConstant {
                    psi_operation,
                    result,
                    ..
                } => Some(DeadScalarShape {
                    source_operation: *psi_operation,
                    result: *result,
                    scalar_type: ScalarType::Boolean,
                }),
                _ => None,
            }
        }
    }
    pub(crate) mod dead_unconditionally_total_scalar_elimination {
        use abstract_operations::{
            AbstractOperation as O, IntegerShiftPolicy, bitwise_not_and_pair_pattern,
            bitwise_or_xor_pair_pattern, boolean_not_or_equal_pair_pattern,
            integer_equal_less_pair_pattern,
        };
        use optimization_core::{
            AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationRuleContract,
            OptimizationSafetyClass,
        };
        use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};
        use semantic_vocabulary::ScalarType;

        use crate::rules::DEAD_PURE_SCALAR_PASS_NAME;
        use crate::rules::node_elision::{DeadScalarShape, propose_unproved_dead_scalar_nodes};
        use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

        psi_rule_contract!(
            DeadUnconditionallyTotalScalarEliminationRule,
            crate::rules::standard_rule_contract(
                b"omega.psi-rule.dead-unused-unconditionally-total-scalar-elimination.v1",
                DEAD_PURE_SCALAR_PASS_NAME,
                1,
                AnalysisSet::new([AnalysisKind::ValueLiveness, AnalysisKind::EffectSummaries]),
                AnalysisInvalidationSet::new([
                    AnalysisKind::UseDefinition,
                    AnalysisKind::EffectSummaries,
                ]),
                OptimizationSafetyClass::ExactOperationSemantics
            ),
            propose = propose_unproved_dead_scalar_nodes,
            classify,
        );

        pub(crate) fn classify(operation: &O) -> Option<DeadScalarShape> {
            let (source_operation, result, scalar_type) = match operation {
                boolean_not_or_equal_pair_pattern!(psi_operation, result, ..)
                | integer_equal_less_pair_pattern!(psi_operation, result, ..)
                | O::IntegerLessOrEqual {
                    psi_operation,
                    result,
                    ..
                } => (*psi_operation, *result, ScalarType::Boolean),
                bitwise_not_and_pair_pattern!(psi_operation, result, scalar_type, ..)
                | bitwise_or_xor_pair_pattern!(psi_operation, result, scalar_type, ..) => {
                    (*psi_operation, *result, ScalarType::Integer(*scalar_type))
                }
                O::IntegerWiden {
                    psi_operation,
                    result,
                    target_type,
                    ..
                } => (*psi_operation, *result, ScalarType::Integer(*target_type)),
                O::IntegerArithmetic {
                    psi_operation,
                    result,
                    scalar_type,
                    arithmetic,
                    ..
                } if arithmetic.obligation().is_none() => {
                    (*psi_operation, *result, ScalarType::Integer(*scalar_type))
                }
                O::IntegerShift {
                    psi_operation,
                    result,
                    value_type,
                    policy: IntegerShiftPolicy::Wrapping,
                    ..
                } => (*psi_operation, *result, ScalarType::Integer(*value_type)),
                _ => return None,
            };
            Some(DeadScalarShape {
                source_operation,
                result,
                scalar_type,
            })
        }
    }

    pub(crate) use dead_scalar_literal_elimination::DeadScalarLiteralEliminationRule;
    pub(crate) use dead_unconditionally_total_scalar_elimination::DeadUnconditionallyTotalScalarEliminationRule;

    pub(crate) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
        vec![
            BuiltInRuleRegistration::new(0, DeadScalarLiteralEliminationRule),
            BuiltInRuleRegistration::new(1, DeadUnconditionallyTotalScalarEliminationRule),
        ]
    }
}
pub(crate) mod field_value_specialization {
    use optimization_core::{AnalysisKind, OptimizationRuleContract};
    use optimization_unit::{
        FieldValueResolution, PsiOptimizationUnit, PsiRewriteCandidate, ScalarSubstitution,
    };

    use crate::rules::catalog::BuiltInRuleRegistration;
    use crate::{
        AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError,
        field_value_specialization,
    };

    psi_rule_contract!(
        FieldValueSpecializationRule,
        crate::rules::representation_specialization_contract(
            b"omega.psi-rule.field-value-specialization.v1",
        ),
    );

    psi_rule_impl!(FieldValueSpecializationRule, |unit, analyses| {
        required_analysis!(analyses, AnalysisKind::StronglyConnectedComponents => Some(AnalysisProduct::StronglyConnectedComponents(scc)));
        let frozen = scc.cyclic_machines(unit);
        let mut candidates = Vec::new();
        for function in &unit.functions {
            if frozen.contains(&function.machine) {
                continue;
            }
            for declaration in &function.structural_places {
                let Some(patch) =
                    field_value_specialization::propose::plan(unit, function, declaration.id)
                else {
                    continue;
                };
                if patch.reads.is_empty() {
                    continue;
                }
                let mut substitutions = patch
                    .reads
                    .iter()
                    .filter_map(|row| match &row.resolution {
                        FieldValueResolution::Forward(forwarded) => Some(ScalarSubstitution {
                            from: row.result,
                            to: forwarded.initializer,
                            scalar_type: forwarded.scalar_type,
                        }),
                        FieldValueResolution::Constant(_) => None,
                    })
                    .collect::<Vec<_>>();
                substitutions.sort();
                let Some((affected_blocks, provenance)) =
                    optimization_unit::field_value_accounting(function, &patch.reads)
                else {
                    continue;
                };
                let predicted_cost_delta = -i64::try_from(patch.reads.len()).unwrap_or(i64::MAX);
                candidates.push(
                    PsiRewriteCandidate::new_field_value_specialization(
                        unit.identity,
                        Self::contract(),
                        affected_blocks,
                        substitutions,
                        provenance,
                        predicted_cost_delta,
                        patch,
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
        Ok(candidates)
    });

    pub(crate) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
        vec![BuiltInRuleRegistration::new(
            1,
            FieldValueSpecializationRule,
        )]
    }
}
pub(crate) use crate::global_value_numbering;
pub(crate) use crate::proof_check_elision;
pub(crate) use crate::sccp as sparse_conditional_constant_propagation;
pub(crate) mod state_specialization {
    use optimization_core::{
        AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationRuleContract,
        OptimizationSafetyClass,
    };
    use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

    use crate::rules::STATE_SPECIALIZATION_PASS_NAME;
    use crate::rules::catalog::BuiltInRuleRegistration;
    use crate::{
        AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError,
        state_specialization,
    };

    psi_rule_contract!(
        StateArgumentSpecializationRule,
        crate::rules::standard_rule_contract(
            b"omega.psi-rule.state-argument-specialization.v1",
            STATE_SPECIALIZATION_PASS_NAME,
            1,
            AnalysisSet::new([
                AnalysisKind::ScalarConstants,
                AnalysisKind::StronglyConnectedComponents,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::Dominators,
                AnalysisKind::PostDominators,
                AnalysisKind::LoopForest,
                AnalysisKind::StronglyConnectedComponents,
                AnalysisKind::UseDefinition,
                AnalysisKind::ExecutableEdges,
                AnalysisKind::ScalarConstants,
                AnalysisKind::ValueRanges,
                AnalysisKind::EffectSummaries,
                AnalysisKind::ValueLiveness,
            ]),
            OptimizationSafetyClass::StructuralIdentity
        ),
    );

    psi_rule_impl!(StateArgumentSpecializationRule, |unit, analyses| {
        required_analysis!(analyses, AnalysisKind::ScalarConstants => Some(AnalysisProduct::ScalarConstants(constants)));
        required_analysis!(analyses, AnalysisKind::StronglyConnectedComponents => Some(AnalysisProduct::StronglyConnectedComponents(scc)));
        let frozen = scc.cyclic_machines(unit);
        let mut candidates = Vec::new();
        for function in &unit.functions {
            if frozen.contains(&function.machine) {
                continue;
            }
            for block in &function.blocks {
                let Some(patch) =
                    state_specialization::propose::plan(unit, function, block.id, constants)
                else {
                    continue;
                };
                if patch.edges.is_empty() {
                    continue;
                }
                let Some(provenance) =
                    state_specialization::accounting::provenance_rows(function, &patch)
                else {
                    continue;
                };
                let mut affected_blocks = vec![patch.dispatch];
                affected_blocks.extend(patch.edges.iter().map(|row| row.predecessor.block));
                affected_blocks.sort_unstable();
                affected_blocks.dedup();
                let predicted_cost_delta = -i64::try_from(patch.edges.len()).unwrap_or(i64::MAX);
                candidates.push(
                    PsiRewriteCandidate::new_state_argument_specialization(
                        unit.identity,
                        Self::contract(),
                        affected_blocks,
                        provenance,
                        predicted_cost_delta,
                        patch,
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
        Ok(candidates)
    });

    pub(crate) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
        vec![BuiltInRuleRegistration::new(
            0,
            StateArgumentSpecializationRule,
        )]
    }
}

fn representation_specialization_rule_registrations() -> Vec<catalog::BuiltInRuleRegistration> {
    let mut registrations = case_membership_specialization::built_in_registrations();
    registrations.extend(field_value_specialization::built_in_registrations());
    registrations
}

use optimization::PsiOptimizationSelections;
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationRuleSetIdentity,
    OptimizationSafetyClass,
};

use crate::{OrderedRuleRegistry, RuleRegistryError};

pub(crate) enum PsiPassStep {
    RewriteRegistry(OrderedRuleRegistry),
    LoopInvariantScalarMotion,
}

impl PsiPassStep {
    pub(crate) fn pass(&self) -> Option<OptimizationPassIdentity> {
        match self {
            Self::RewriteRegistry(registry) => registry.pass(),
            Self::LoopInvariantScalarMotion => Some(
                OptimizationPassIdentity::from_canonical_bytes(
                    crate::ranked_rewrites::loop_invariant_scalar_motion::LOOP_INVARIANT_SCALAR_MOTION_PASS_NAME,
                ),
            )}
    }

    pub(crate) fn contracts(&self) -> Vec<OptimizationRuleContract> {
        match self {
            Self::RewriteRegistry(registry) => registry.contracts().collect(),
            Self::LoopInvariantScalarMotion => {
                vec![crate::ranked_rewrites::loop_invariant_scalar_motion::contract()]
            }
        }
    }

    pub(crate) fn ordered_rules(&self) -> Vec<OptimizationRuleIdentity> {
        self.contracts()
            .iter()
            .map(|contract| contract.identity())
            .collect()
    }

    pub(crate) fn rule_set_identity(&self) -> OptimizationRuleSetIdentity {
        match self {
            Self::RewriteRegistry(registry) => registry.identity(),
            Self::LoopInvariantScalarMotion => OptimizationRuleSetIdentity::from_ordered_rules(&[
                crate::ranked_rewrites::loop_invariant_scalar_motion::rule_identity(),
            ])
            .expect("a single-rule schedule cannot duplicate"),
        }
    }
}
#[cfg(any(test, feature = "test-support"))]
pub use sparse_conditional_constant_propagation::WrappingIntegerAddConstantsRule;

pub(crate) fn built_in_psi_steps_for_selections(
    selections: &PsiOptimizationSelections,
) -> Result<Vec<PsiPassStep>, RuleRegistryError> {
    if let Some(unsupported) = selections.as_slice().iter().find(|optimization| {
        !catalog::PSI_PASS_CATALOG
            .iter()
            .any(|entry| entry.optimization() == **optimization)
    }) {
        return Err(RuleRegistryError::UnsupportedOptimization(*unsupported));
    }
    catalog::PSI_PASS_CATALOG
        .iter()
        .copied()
        .filter(|entry| selections.contains(entry.optimization()))
        .map(catalog::PsiPassCatalogEntry::step)
        .collect()
}
