/*
Root `TerminalModule` aggregate combining interned identities, structural and scalar domain
declarations, mathematical proof declarations, boundary requirements, executable `TerminalMachine`
bodies, folded constants, quotients, scalar applications, and task-activation descriptors.
*/

pub use crate::types_and_values::{types, values};
pub use crate::{boundary, control_flow, identity, observation, ownership, proof};

pub use boundary::*;
pub(crate) mod folded_constants {

    use super::{
        NominalDeclarationIdentity, TargetMachineCallable, TerminalIdentityIndex, TerminalModule,
    };
    use semantic_vocabulary::{BoundaryMachineId, MachineId};

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct FoldedConstantClosure {
        pub target: Option<String>,
        pub dependencies: Vec<FoldedConstantDependency>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub struct FoldedConstantDependency {
        pub target: String,
        pub requirement: TerminalIdentityIndex,
        pub plan: [u8; 32],
        pub candidate: Option<FoldedConstantCandidate>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub struct FoldedConstantCandidate {
        pub boundary: BoundaryMachineId,
        pub machine: MachineId,
        pub declaration: NominalDeclarationIdentity,
    }

    impl FoldedConstantClosure {
        pub fn validate_plan_inputs(
            &self,
            target: &str,
            inputs: &[Vec<u8>],
        ) -> Result<(), &'static str> {
            if self.target.as_deref() != Some(target) {
                return Err("folded constant closure belongs to another target");
            }
            let expected: std::collections::BTreeSet<_> = self
                .dependencies
                .iter()
                .filter(|dependency| dependency.target == target)
                .map(|dependency| dependency.plan)
                .collect();
            if inputs.len() != expected.len() {
                return Err(
                    "folded constant selected realization closure is incomplete or repeated",
                );
            }
            let supplied: std::collections::BTreeSet<_> = inputs
                .iter()
                .map(|preimage| {
                    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(b"");
                    digest.raw(preimage);
                    digest.finish()
                })
                .collect();
            if supplied != expected || supplied.len() != inputs.len() {
                return Err(
                    "folded constant normalized plan input differs from its retained commitment",
                );
            }
            Ok(())
        }

        pub fn validate(&self, module: &TerminalModule) -> Result<(), &'static str> {
            if self.dependencies.is_empty() {
                return if self.target.is_none() {
                    Ok(())
                } else {
                    Err("folded constant target has no realization dependencies")
                };
            }
            if self.target.as_deref().is_none_or(str::is_empty) {
                return Err("folded constant dependencies have no primary target closure");
            }
            if self.dependencies.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err("folded constant dependencies are repeated or out of canonical order");
            }
            for dependency in &self.dependencies {
                if dependency.target.is_empty()
                    || dependency.plan == [0; 32]
                    || module
                        .identities
                        .text(dependency.requirement)
                        .is_none_or(str::is_empty)
                {
                    return Err("folded constant dependency has incomplete realization identity");
                }
                let Some(association) = &dependency.candidate else {
                    continue;
                };
                let count = module
                    .provider_candidates
                    .iter()
                    .filter(|candidate| {
                        candidate.boundary == association.boundary
                            && candidate.candidate == association.machine
                            && candidate.provider_declaration == association.declaration
                            && candidate.source_requirement_identity == dependency.requirement
                    })
                    .count();
                if count != 1 {
                    return Err(
                        "folded constant dependency lost its exact checked candidate association",
                    );
                }
                let callable = TargetMachineCallable::Machine(association.machine);
                for family in &module.target_machine_families {
                    if family
                        .members
                        .iter()
                        .any(|member| member.callable == callable)
                        && !family.members.iter().any(|member| {
                            member.callable == callable && member.target == dependency.target
                        })
                    {
                        return Err("folded constant candidate belongs to another target scope");
                    }
                }
            }
            Ok(())
        }
    }
}
pub use folded_constants::*;
pub mod proof_application_scope {

    use super::{CheckedMachineDeclaration, TerminalIdentityIndex};
    use arena::Arena;
    use semantic_vocabulary::proof_applications::{
        ProofApplicationPlan, ProofApplications, ProofCallGraph,
    };

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
    pub struct ProofDeclarationId(pub u32);

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub enum ProofScopeDeclaration {
        #[default]
        Missing,
        Nominal(TerminalIdentityIndex),
        Callable(CheckedMachineDeclaration),
        Operator(super::BoundaryOperatorRequirement),
        Unavailable(String),
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub enum ProofCallableAssociation {
        #[default]
        Missing,
        Callable {
            declaration: ProofDeclarationId,
            callable: super::TargetMachineCallable,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ProofCallableHeader {
        pub declaration: ProofDeclarationId,
        pub callable: Result<TerminalIdentityIndex, String>,
        pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
        pub source_target: Option<String>,
        pub product_eligible: bool,
        pub supplies_checked_body: bool,
        pub type_parameter_count: usize,
        pub compiler_evaluation:
            Result<semantic_vocabulary::compiler_evaluation::CompilerEvaluationDeclaration, String>,
        pub synchronous_invocations: Result<Vec<DeclaredSynchronousBoundaryTarget>, String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct DeclaredSynchronousBoundaryTarget {
        pub declaration: TerminalIdentityIndex,
        pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
        pub application: Option<TerminalIdentityIndex>,
    }

    impl Default for ProofCallableHeader {
        fn default() -> Self {
            Self {
                declaration: ProofDeclarationId::default(),
                callable: Err("checked callable header is absent".into()),
                package: None,
                source_target: None,
                product_eligible: false,
                supplies_checked_body: false,
                type_parameter_count: 0,
                compiler_evaluation: Err("checked compiler-evaluation facts are absent".into()),
                synchronous_invocations: Err(
                    "checked synchronous invocation facts are absent".into()
                ),
            }
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct ProofApplicationScope {
        pub declarations: Arena<ProofScopeDeclaration>,
        pub callable_headers: Arena<ProofCallableHeader>,
        pub callables: Arena<ProofCallableAssociation>,
        pub plan: ProofApplicationPlan<ProofDeclarationId>,
        pub calls: ProofCallGraph<ProofDeclarationId>,
        pub recursive_components: Arena<ProofCheckedRecursiveComponent>,
        pub recursive_members: Arena<ProofDeclarationId>,
        pub recursive_calls: Arena<ProofCheckedRecursiveCall>,
        pub pending_evaluation_conditions: Arena<ProofDeclarationId>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct ProofCheckedRecursiveComponent {
        pub members: arena::HandleSpan<ProofDeclarationId>,
        pub calls: arena::HandleSpan<ProofCheckedRecursiveCall>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct ProofCheckedRecursiveCall {
        pub caller: ProofDeclarationId,
        pub call_ordinal: u32,
        pub callee: ProofDeclarationId,
    }

    impl ProofApplicationScope {
        pub fn callable_declaration(
            &self,
            callable: super::TargetMachineCallable,
        ) -> Option<ProofDeclarationId> {
            self.callables.iter().find_map(|(_, row)| match row {
                ProofCallableAssociation::Callable {
                    declaration,
                    callable: emitted,
                } if *emitted == callable => Some(*declaration),
                _ => None,
            })
        }

        pub fn applications(
            &self,
        ) -> ProofApplications<'_, ProofDeclarationId, ProofCallGraph<ProofDeclarationId>> {
            ProofApplications::new(&self.plan, &self.calls)
        }

        pub fn declaration(&self, identity: ProofDeclarationId) -> Option<&ProofScopeDeclaration> {
            let position = usize::try_from(identity.0.checked_sub(1)?).ok()?;
            self.declarations.storage_slice().get(position)
        }
    }
}
pub mod scalar_applications {

    use arena::Arena;
    use semantic_vocabulary::{ScalarApplicationId, ScalarType, StructuralTypeId};

    use crate::TargetMachineCallable;

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct ScalarApplications {
        declarations: Arena<ScalarApplication>,
        pub conditions: super::ScalarConditions,
        pub proof_scope: super::ProofApplicationScope,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub enum ScalarApplication {
        #[default]
        Missing,
        LayoutObservation {
            subject: StructuralTypeId,
            measurement: LayoutMeasurement,
            semantics_version: u32,
            scalar_type: ScalarType,
        },
        Evaluate {
            entry: TargetMachineCallable,
            scalar_type: ScalarType,
        },
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum LayoutMeasurement {
        Size,
        Alignment,
    }

    impl LayoutMeasurement {
        pub const SEMANTICS_VERSION: u32 = 1;
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum ArrayExtent {
        Literal(u64),
        Application(ScalarApplicationId),
    }

    impl ArrayExtent {
        pub const fn literal(self) -> Option<u64> {
            match self {
                Self::Literal(length) => Some(length),
                Self::Application(_) => None,
            }
        }
    }

    impl From<u64> for ArrayExtent {
        fn from(length: u64) -> Self {
            Self::Literal(length)
        }
    }

    impl std::fmt::Display for ArrayExtent {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Literal(length) => write!(formatter, "{length}"),
                Self::Application(application) => {
                    write!(formatter, "pending application #{}", application.get())
                }
            }
        }
    }

    impl ScalarApplication {
        pub const fn scalar_type(self) -> Option<ScalarType> {
            match self {
                Self::Missing => None,
                Self::LayoutObservation { scalar_type, .. }
                | Self::Evaluate { scalar_type, .. } => Some(scalar_type),
            }
        }
    }

    impl ScalarApplications {
        pub fn insert(
            &mut self,
            declaration: ScalarApplication,
        ) -> Result<ScalarApplicationId, &'static str> {
            if declaration == ScalarApplication::Missing {
                return Err("a scalar application cannot publish the arena sentinel");
            }
            let coordinate = u64::try_from(self.declarations.len())
                .ok()
                .and_then(|length| length.checked_add(1))
                .and_then(ScalarApplicationId::new)
                .ok_or("scalar application namespace is exhausted")?;
            self.declarations.insert(declaration);
            Ok(coordinate)
        }

        pub fn declaration(&self, application: ScalarApplicationId) -> Option<&ScalarApplication> {
            let position = usize::try_from(application.get().checked_sub(1)?).ok()?;
            self.declarations.storage_slice().get(position)
        }

        pub fn declarations(&self) -> &[ScalarApplication] {
            self.declarations.storage_slice()
        }

        pub fn is_empty(&self) -> bool {
            self.declarations.is_empty() && self.conditions.is_empty()
        }
    }
}
pub mod scalar_conditions {

    use arena::{Arena, Handle, HandleSpan};
    use numerics::bignum::BigInt;
    use numerics::integer_bounds::IntegerBoundStep;
    use numerics::scalar_predicates::{
        PredicateNode, PredicateValueKind, ScalarValue, evaluate_nodes,
    };
    use semantic_vocabulary::{
        MachineId, ScalarApplicationId, ScalarConditionRole, ScalarType, ValueId,
    };

    use super::ProofDeclarationId;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ScalarConditionSubject {
        Module,
        Declaration(ProofDeclarationId),
        Application(ScalarApplicationId),
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub enum ScalarRangeObservation {
        #[default]
        Missing,
        Literal(BigInt),
        Application(ScalarApplicationId),
        EntryResult {
            application: ScalarApplicationId,
            machine: MachineId,
            result: ValueId,
            scalar_type: ScalarType,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum ScalarRangeRequirement {
        Fixed { minimum: BigInt, maximum: BigInt },
        AddressModel,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub enum ScalarRangeCondition {
        #[default]
        Missing,
        Range {
            subject: ScalarConditionSubject,
            role: ScalarConditionRole,
            requirement: ScalarRangeRequirement,
            steps: HandleSpan<IntegerBoundStep<ScalarRangeObservation>>,
        },
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub enum ScalarCompletionCondition {
        #[default]
        Missing,
        Complete {
            subject: ScalarConditionSubject,
            application: ScalarApplicationId,
        },
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct ScalarConditions {
        ranges: Arena<ScalarRangeCondition>,
        steps: Arena<IntegerBoundStep<ScalarRangeObservation>>,
        predicates: Arena<ScalarPredicateCondition>,
        completions: Arena<ScalarCompletionCondition>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct ScalarPredicateNode(
        pub PredicateNode<Handle<ScalarPredicateNode>, ScalarRangeObservation>,
    );

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ScalarPredicateCondition {
        pub subject: ScalarConditionSubject,
        pub role: ScalarConditionRole,
        pub description: String,
        nodes: Arena<ScalarPredicateNode>,
        root: Handle<ScalarPredicateNode>,
    }

    impl Default for ScalarPredicateCondition {
        fn default() -> Self {
            Self {
                subject: ScalarConditionSubject::Module,
                role: ScalarConditionRole::Prerequisite,
                description: String::new(),
                nodes: Arena::new(),
                root: Handle::invalid(),
            }
        }
    }

    impl ScalarPredicateCondition {
        pub fn nodes(&self) -> &Arena<ScalarPredicateNode> {
            &self.nodes
        }
        pub fn root(&self) -> Handle<ScalarPredicateNode> {
            self.root
        }

        pub fn evaluate(
            &self,
            observe: &mut impl FnMut(ScalarRangeObservation) -> Option<ScalarValue>,
        ) -> Option<ScalarValue> {
            evaluate_nodes(
                self.root,
                &mut |reference| {
                    self.nodes
                        .is_valid(reference)
                        .then(|| self.nodes.get(reference).0.clone())
                },
                &mut |observation| match observation {
                    ScalarRangeObservation::Missing => None,
                    ScalarRangeObservation::Literal(value) => Some(ScalarValue::Integer(value)),
                    observed => match observe(observed)? {
                        ScalarValue::Integer(value) => Some(ScalarValue::Integer(value)),
                        _ => None,
                    },
                },
                &|_, _| true,
            )
        }
    }

    impl ScalarConditions {
        pub fn contains_subject(&self, subject: ScalarConditionSubject) -> bool {
            self.ranges().iter().any(|condition| {
                matches!(condition, ScalarRangeCondition::Range { subject: owner, .. } if *owner == subject)
            }) || self.predicates().iter().any(|condition| condition.subject == subject)
                || self.completions().iter().any(|condition| {
                    matches!(condition, ScalarCompletionCondition::Complete { subject: owner, .. } if *owner == subject)
                })
        }

        pub fn completions(&self) -> &[ScalarCompletionCondition] {
            self.completions.storage_slice()
        }

        pub fn retain_completion(
            &mut self,
            subject: ScalarConditionSubject,
            application: ScalarApplicationId,
        ) {
            self.completions
                .insert(ScalarCompletionCondition::Complete {
                    subject,
                    application,
                });
        }

        pub fn ranges(&self) -> &[ScalarRangeCondition] {
            self.ranges.storage_slice()
        }

        pub fn predicates(&self) -> &[ScalarPredicateCondition] {
            self.predicates.storage_slice()
        }

        pub fn is_empty(&self) -> bool {
            self.ranges.is_empty() && self.predicates.is_empty() && self.completions.is_empty()
        }

        pub fn retain_predicate(
            &mut self,
            subject: ScalarConditionSubject,
            role: ScalarConditionRole,
            description: String,
            nodes: Arena<ScalarPredicateNode>,
            root: Handle<ScalarPredicateNode>,
        ) -> Result<(), &'static str> {
            let mut preceding = Vec::new();
            for (reference, node) in nodes.iter() {
                if matches!(
                    node.0,
                    PredicateNode::Missing
                        | PredicateNode::Value(ScalarValue::Unknown)
                        | PredicateNode::Leaf(ScalarRangeObservation::Missing)
                ) {
                    return Err("scalar predicate contains an absent node or observation");
                }
                let kind = node.0.value_kind(
                    &mut |child| {
                        preceding
                            .iter()
                            .find(|(reference, _)| *reference == child)
                            .map(|(_, kind)| *kind)
                            .ok_or("scalar predicate lost a preceding child")
                    },
                    &mut |_| Ok(PredicateValueKind::Integer),
                )?;
                preceding.push((reference, kind));
            }
            if !nodes.is_valid(root) {
                return Err("scalar predicate lost its root");
            }
            if preceding
                .iter()
                .find(|(reference, _)| *reference == root)
                .map(|(_, kind)| *kind)
                != Some(PredicateValueKind::Boolean)
            {
                return Err("scalar predicate root is not Boolean");
            }
            self.predicates.insert(ScalarPredicateCondition {
                subject,
                role,
                description,
                nodes,
                root,
            });
            Ok(())
        }

        pub fn observations(&self) -> impl Iterator<Item = &ScalarRangeObservation> {
            self.steps
                .storage_slice()
                .iter()
                .filter_map(|step| match step {
                    IntegerBoundStep::Observation(observation) => Some(observation),
                    _ => None,
                })
                .chain(
                    self.predicates
                        .storage_slice()
                        .iter()
                        .flat_map(|condition| {
                            condition.nodes.storage_slice().iter().filter_map(|node| {
                                match &node.0 {
                                    PredicateNode::Leaf(observation) => Some(observation),
                                    _ => None,
                                }
                            })
                        }),
                )
        }

        pub fn steps(
            &self,
            condition: &ScalarRangeCondition,
        ) -> Result<&[IntegerBoundStep<ScalarRangeObservation>], &'static str> {
            let ScalarRangeCondition::Range { steps, .. } = condition else {
                return Err("scalar condition is absent");
            };
            self.steps
                .span(*steps)
                .ok_or("scalar condition lost its captured bounds")
        }

        pub fn retain_range(
            &mut self,
            subject: ScalarConditionSubject,
            role: ScalarConditionRole,
            requirement: ScalarRangeRequirement,
            steps: Vec<IntegerBoundStep<ScalarRangeObservation>>,
        ) -> Result<(), &'static str> {
            if let ScalarRangeRequirement::Fixed { minimum, maximum } = &requirement
                && minimum > maximum
            {
                return Err("scalar condition has inverted required bounds");
            }
            let mut depth = 0usize;
            for step in &steps {
                match step {
                    IntegerBoundStep::Missing => {
                        return Err("scalar condition contains an absent step");
                    }
                    IntegerBoundStep::Bounds { low, high } => {
                        if low > high {
                            return Err("scalar condition has inverted captured bounds");
                        }
                        depth += 1;
                    }
                    IntegerBoundStep::Observation(ScalarRangeObservation::Missing) => {
                        return Err("scalar condition contains an absent observation");
                    }
                    IntegerBoundStep::Observation(_) => depth += 1,
                    IntegerBoundStep::Add
                    | IntegerBoundStep::Subtract
                    | IntegerBoundStep::Multiply
                    | IntegerBoundStep::Union
                    | IntegerBoundStep::Interval
                    | IntegerBoundStep::Intersect => {
                        if depth < 2 {
                            return Err("scalar condition has missing bound operands");
                        }
                        depth -= 1;
                    }
                }
            }
            if depth != 1 {
                return Err("scalar condition does not produce one bound interval");
            }
            let steps = self.steps.insert_many(steps);
            self.ranges.insert(ScalarRangeCondition::Range {
                subject,
                role,
                requirement,
                steps,
            });
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use numerics::scalar_predicates::PredicateOperator;

        #[test]
        fn scalar_predicate_formation_checks_root_and_all_operator_kinds() {
            for operator in [
                PredicateOperator::And,
                PredicateOperator::Or,
                PredicateOperator::Less,
                PredicateOperator::GreaterOrEqual,
            ] {
                let mut nodes = Arena::new();
                let integer = nodes.insert(ScalarPredicateNode(PredicateNode::Value(
                    ScalarValue::Integer(BigInt::from_u64(1)),
                )));
                let boolean = nodes.insert(ScalarPredicateNode(PredicateNode::Value(
                    ScalarValue::Boolean(true),
                )));
                let root = nodes.insert(ScalarPredicateNode(PredicateNode::Binary {
                    operator,
                    left: integer,
                    right: boolean,
                }));
                assert_eq!(
                    ScalarConditions::default().retain_predicate(
                        ScalarConditionSubject::Module,
                        semantic_vocabulary::ScalarConditionRole::Prerequisite,
                        "bad kinds".into(),
                        nodes,
                        root
                    ),
                    Err("scalar predicate operator has incompatible operand kinds")
                );
            }
            let mut nodes = Arena::new();
            let root = nodes.insert(ScalarPredicateNode(PredicateNode::Value(
                ScalarValue::Integer(BigInt::from_u64(1)),
            )));
            assert_eq!(
                ScalarConditions::default().retain_predicate(
                    ScalarConditionSubject::Module,
                    semantic_vocabulary::ScalarConditionRole::Prerequisite,
                    "integer root".into(),
                    nodes,
                    root
                ),
                Err("scalar predicate root is not Boolean")
            );
        }

        #[test]
        fn scalar_predicate_observations_cannot_change_declared_kind_or_literal() {
            let mut nodes = Arena::new();
            let left = nodes.insert(ScalarPredicateNode(PredicateNode::Leaf(
                ScalarRangeObservation::Application(ScalarApplicationId::new(1).unwrap()),
            )));
            let right = nodes.insert(ScalarPredicateNode(PredicateNode::Leaf(
                ScalarRangeObservation::Literal(BigInt::from_u64(24)),
            )));
            let root = nodes.insert(ScalarPredicateNode(PredicateNode::Binary {
                operator: PredicateOperator::Equal,
                left,
                right,
            }));
            let mut conditions = ScalarConditions::default();
            conditions
                .retain_predicate(
                    ScalarConditionSubject::Module,
                    semantic_vocabulary::ScalarConditionRole::Prerequisite,
                    "exact integer observations".into(),
                    nodes,
                    root,
                )
                .unwrap();
            assert_eq!(
                conditions.predicates()[0].evaluate(&mut |_| Some(ScalarValue::Boolean(true))),
                None
            );
            let mut observations = 0;
            assert_eq!(
                conditions.predicates()[0].evaluate(&mut |_| {
                    observations += 1;
                    Some(ScalarValue::Integer(BigInt::from_u64(24)))
                }),
                Some(ScalarValue::Boolean(true))
            );
            assert_eq!(observations, 1);
        }
    }
}
pub use control_flow::{
    AtomicAccessEvent, AtomicReadModifyWrite, Block, CrashCause, ElementViewRegion, FenceOrdering,
    FloatResultPolicy, MemoryOrdering, ModuleInvocation, ModuleInvocationInputs, Operation,
    OperationKind, OperationResult, RecordFieldInitializer, RecordFieldValue, ReferenceStorageView,
    RegisterAccessEvent, ScalarCaseField, StructuralCaseSuccessorEdge, StructuralOperationResult,
    StructuralResultClaimBinding, StructuralStoreDisposition, SuccessorEdge, TargetMachineCallable,
    TargetMachineFamily, TargetMachineFamilyMember, TargetMachineSupply, TerminalBlockNaturalRank,
    TerminalIntegerRankValue, TerminalMachine, TerminalNaturalCycle, TerminalNaturalRankComparison,
    TerminalNaturalRankEdge, TerminalRankedScc, Terminator, TrappingIntegerOperation,
    TrappingIntegerPrimitive, structural_parameter_position,
};
pub use identity::*;
pub use observation::*;
pub use ownership::*;
pub use proof::*;
pub use proof_application_scope::{
    DeclaredSynchronousBoundaryTarget, ProofApplicationScope, ProofCallableAssociation,
    ProofCallableHeader, ProofCheckedRecursiveCall, ProofCheckedRecursiveComponent,
    ProofDeclarationId, ProofScopeDeclaration,
};
pub use scalar_applications::{
    ArrayExtent, LayoutMeasurement, ScalarApplication, ScalarApplications,
};
pub use scalar_conditions::{
    ScalarCompletionCondition, ScalarConditionSubject, ScalarConditions, ScalarPredicateCondition,
    ScalarPredicateNode, ScalarRangeCondition, ScalarRangeObservation, ScalarRangeRequirement,
};
pub mod quotients {
    use language_semantics::quotient_correspondence::{
        QuotientPositionalRelation, QuotientRelationIdentity,
    };
    use semantic_vocabulary::{MachineId, OperationId, PlaceId, StructuralTypeId};

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct TerminalQuotientCatalog {
        pub representations: Vec<TerminalQuotientRepresentation>,
        pub operations: Vec<TerminalQuotientOperation>,
        pub adaptations: Vec<TerminalQuotientAdaptation>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalQuotientRepresentation {
        pub quotient: StructuralTypeId,
        pub carrier: StructuralTypeId,
        pub quotient_storage_identity: crate::TerminalIdentityIndex,
        pub carrier_storage_identity: crate::TerminalIdentityIndex,
        pub formation: QuotientRelationIdentity,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalQuotientOperation {
        pub public_machine: MachineId,
        pub representative_machine: MachineId,
        pub call: OperationId,
        pub input_relations: Vec<QuotientPositionalRelation>,
        pub result_relation: QuotientPositionalRelation,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TerminalQuotientAdaptationLicense {
        Mint,
        Input { operation: u32, position: u32 },
        Result { operation: u32 },
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalQuotientAdaptation {
        pub machine: MachineId,
        pub copy: OperationId,
        pub source: PlaceId,
        pub result: PlaceId,
        pub source_type: StructuralTypeId,
        pub result_type: StructuralTypeId,
        pub representation: u32,
        pub license: TerminalQuotientAdaptationLicense,
    }
}
pub mod task_activation {

    use crate::{
        LayoutApplicationId, StructuralTypeDeclaration, terminal_module::TerminalIdentityIndex,
    };
    use language_semantics::CarryPolicy;
    use semantic_vocabulary::{ScalarType, StructuralTypeId};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TerminalTaskStartOperation {
        Start,
        TryStart,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TerminalTaskValueType {
        Unit,
        Scalar(ScalarType),
        Structural(StructuralTypeId),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalTaskSignatureValue {
        pub value_type: TerminalTaskValueType,
        pub normalized_identity: TerminalIdentityIndex,
        pub is_mutable: bool,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TerminalTaskCarryStorage {
        Persistent,
        Parameter,
        Local,
        CallArgument,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalTaskLiveCarry {
        pub place: u64,
        pub value_type: u64,
        pub storage: TerminalTaskCarryStorage,
        pub claims: Vec<u64>,
        pub effective: CarryPolicy,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalTaskSuspensionCrossing {
        pub identity: u64,
        pub suspension_allowed: bool,
        pub preserve_cpu: bool,
        pub preserve_host_thread: bool,
        pub live_carry: Vec<TerminalTaskLiveCarry>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct TerminalTaskCarryObligations {
        pub preserve_cpu: bool,
        pub preserve_host_thread: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalTaskResidentValue {
        pub value_type: TerminalTaskValueType,
        pub normalized_identity: TerminalIdentityIndex,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalTaskFrameState {
        pub state: TerminalIdentityIndex,
        pub resident: Vec<TerminalTaskResidentValue>,
        pub crossing_arguments: Vec<Vec<TerminalTaskResidentValue>>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalTaskFrameCall {
        pub callee: u64,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TerminalTaskUnresolvedCallKind {
        UnresolvedTarget,
        NonCheckedSupply,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalTaskUnresolvedCall {
        pub state: TerminalIdentityIndex,
        pub statement_index: u32,
        pub call_ordinal: u32,
        pub kind: TerminalTaskUnresolvedCallKind,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalTaskFrame {
        pub identity: u64,
        pub machine_callable_identity: TerminalIdentityIndex,
        pub entry: TerminalIdentityIndex,
        pub states: Vec<TerminalTaskFrameState>,
        pub calls: Vec<TerminalTaskFrameCall>,
        pub unresolved_calls: Vec<TerminalTaskUnresolvedCall>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalTaskActivationDemand {
        pub operation: TerminalTaskStartOperation,
        pub report_fingerprint: u64,
        pub requirement_callable_identity: TerminalIdentityIndex,
        pub requirement_owner_path: TerminalIdentityIndex,
        pub requirement_owner_package: Option<semantic_vocabulary::PackageKeyIdentity>,
        pub requirement_in_scope: bool,
        pub authored_requirement_identity: TerminalIdentityIndex,
        pub machine_callable_identity: TerminalIdentityIndex,
        pub entry: TerminalIdentityIndex,
        pub entry_report_fingerprint: u64,
        pub contract_report_fingerprint: u64,
        pub specialization_commitment: [u8; 32],
        pub specialization_report_fingerprint: u64,
        pub may_suspend: bool,
        pub may_block: bool,
        pub cancellation_required: bool,
        pub signature_arguments: Vec<TerminalTaskSignatureValue>,
        pub signature_outcome: TerminalTaskSignatureValue,
        pub root_frame: u64,
        pub frames: Vec<TerminalTaskFrame>,
        pub crossings: Vec<TerminalTaskSuspensionCrossing>,
        pub carry_obligations: TerminalTaskCarryObligations,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct TerminalTaskActivationDemands {
        pub demands: Vec<TerminalTaskActivationDemand>,
        pub structural_types: Vec<StructuralTypeDeclaration>,
        pub structural_referents: Vec<crate::StructuralReferentIdentity>,
        pub target_layout_applications: Vec<TerminalTaskTargetLayout>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalTaskTargetLayout {
        pub id: LayoutApplicationId,
        pub record: StructuralTypeId,
        pub realizations: Vec<TerminalTaskLayoutRealization>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalTaskLayoutRealization {
        pub target: String,
        pub storage: semantic_vocabulary::AuthoredRecordStorage,
    }
}
pub use task_activation::*;
pub use types::{
    ByteSequenceCarrier, ResultQualificationEstablishment, ScalarDomainDeclaration,
    ScalarDomainEstablishmentRoute, ScalarDomainPredicate, ScalarFieldQualification,
    ScalarFloatRange, ScalarIntegerRange, ScalarQualificationCatalog, ScalarQualificationCoercion,
    ScalarQualificationSet, ScalarReturnOrigin, ScalarReturnQualification, ScalarReturnSite,
    StructuralCaseDeclaration, StructuralContentProjection, StructuralDomainDeclaration,
    StructuralDomainPredicate, StructuralEstablishmentRoute, StructuralFieldDeclaration,
    StructuralFieldType, StructuralPathQualification, StructuralReferentIdentity,
    StructuralTypeDeclaration, StructuralTypeShape, field_by_identity, find_structural_case,
    find_structural_type, live_field, live_field_by_id, live_field_by_identity, live_field_entry,
    structural_case_by_identity, structural_case_initializer_path, structural_case_position,
    structural_children, structural_type_by_identity, structural_type_contains_reference,
    structural_type_contains_reference_by, structural_type_has_borrowed_storage_by,
    unique_structural_declaration,
};
pub use values::*;

use semantic_vocabulary::{MachineId, Proposition, ServiceId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalMathematicalCarrier {
    pub type_identity: TerminalIdentityIndex,
    pub carrier_key: Vec<u8>,
}

/// Source-free formation of an Id family over checked value subjects.
/// These rows grant no identity witness and allocate no runtime value copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalIdentityFormation {
    pub source_owner: TerminalIdentityIndex,
    pub source_carrier: TerminalIdentityIndex,
    pub subject_carriers: Vec<TerminalIdentityIndex>,
    pub subject_origins: Vec<(u32, u32)>,
    pub declaration: u32,
}

/// One shared checked signature avoids repeating it for every Id occurrence.
/// Each row names a transparent type-valued definition in the certificate.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TerminalIdentityFormations {
    pub rows: Vec<TerminalIdentityFormation>,
    pub certificate: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalIndexedTheorem {
    pub source_contract: CheckedMachineDeclaration,
    pub ensure_ordinal: u32,
    pub evidence_source_position: u32,
    pub type_identity: TerminalIdentityIndex,
    pub certificate: Vec<u8>,
    pub assumption_closure: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalModule {
    pub scalar_applications: ScalarApplications,
    pub folded_constants: FoldedConstantClosure,
    pub indexed_theorems: Vec<TerminalIndexedTheorem>,
    pub mathematical_carriers: Vec<TerminalMathematicalCarrier>,
    pub identity_formations: TerminalIdentityFormations,
    pub quotients: quotients::TerminalQuotientCatalog,
    pub vocabulary_marker: VocabularyMarker,
    pub identities: TerminalIdentityTable,
    pub entry: Option<TargetMachineCallable>,
    pub scalar_qualifications: ScalarQualificationCatalog,
    pub structural_types: Vec<StructuralTypeDeclaration>,
    pub structural_referents: Vec<types::StructuralReferentIdentity>,
    pub structural_domains: Vec<StructuralDomainDeclaration>,
    pub services: Vec<ServiceDeclaration>,
    pub root_service_reach: TerminalRootServiceReach,
    pub access_view_inputs: Vec<TerminalAccessViewInput>,
    pub access_view_plans: Vec<TerminalAccessViewPlan>,
    pub access_lifecycle_plans: Vec<TerminalAccessLifecyclePlan>,
    pub reborrow_restored_call_uses: Vec<TerminalReborrowRestoredCallUse>,
    pub boundary_machines: Vec<BoundaryMachineDeclaration>,
    pub calling_signatures: Vec<crate::CallingSignatureDeclaration>,
    pub calling_abi_signatures: Vec<crate::CallingAbiDeclaration>,
    pub calling_policy_requests: Vec<crate::CallingPolicyRequest>,
    pub opaque_representations: Vec<crate::TerminalOpaqueRepresentation>,
    pub unoptimized_behavior: Vec<crate::UnoptimizedMachineBehavior>,
    pub external_providers: Vec<ExternalProviderConformance>,
    pub provider_candidates: Vec<ProviderCandidateConformance>,
    pub provider_instance_requirements: Vec<ProviderInstanceRequirements>,
    pub boundary_application_candidates: Vec<crate::BoundaryApplicationCandidate>,
    pub callback_applications: Vec<crate::TerminalCallbackApplication>,
    pub installed_interrupt_registrations: Vec<crate::TerminalInstalledInterruptRegistration>,
    pub dynamic_dispatch: TerminalDynamicDispatchCatalog,
    pub float_meaning_projections: Vec<crate::FloatMeaningProjection>,
    pub float_meaning_equalities: Vec<crate::FloatMeaningEqualityProposition>,
    pub proposition_declarations: Vec<PropositionDeclaration>,
    pub proposition_applications: Vec<PropositionApplicationIdentity>,
    pub evidence_terms: Vec<EvidenceTermDeclaration>,
    pub evidence_contract_lanes: Vec<EvidenceContractLane>,
    pub proof_output_calls: Vec<ProofOutputCall>,
    pub proof_recursive_components: Vec<TerminalProofRecursiveComponent>,
    pub scalar_block_invariants: Vec<ScalarBlockInvariant>,
    pub operation_crash_contracts: Vec<TerminalOperationCrashContract>,
    pub closed_conformance_applications: Vec<ClosedConformanceApplication>,
    pub suspension_call_plan_count: u32,
    pub suspension_call_sites: Vec<TerminalSuspensionCallSite>,
    pub suspension_call_plans: Vec<TerminalSuspensionCallPlan>,
    /// Real declaration contracts with no executable body, ordered by ContractId.
    pub declaration_contracts: Vec<crate::MachineContract>,
    pub machines: Vec<TerminalMachine>,
    pub target_machine_families: Vec<TargetMachineFamily>,
    pub target_layout_applications: Vec<crate::TargetLayoutApplication>,
    pub native_layouts: crate::NativeLayoutCatalog,
    pub boundary_application_demands: Vec<TerminalBoundaryApplicationDemand>,
    pub task_activation_demands: TerminalTaskActivationDemands,
}

impl TerminalModule {
    pub fn callback_application(&self, index: u32) -> Option<&crate::TerminalCallbackApplication> {
        self.callback_applications
            .binary_search_by_key(&index, |row| row.application_index)
            .ok()
            .map(|ordinal| &self.callback_applications[ordinal])
            .or_else(|| {
                self.installed_interrupt_registrations
                    .iter()
                    .find_map(|registration| {
                        registration
                            .callbacks
                            .binary_search_by_key(&index, |row| row.application_index)
                            .ok()
                            .map(|ordinal| &registration.callbacks[ordinal])
                    })
            })
    }
    pub fn calling_abi_identity(
        &self,
        boundary: semantic_vocabulary::BoundaryMachineId,
    ) -> Result<semantic_vocabulary::CallingAbiIdentity, String> {
        let declaration = self
            .calling_signatures
            .iter()
            .find(|row| row.boundary == boundary)
            .ok_or_else(|| {
                let name = self
                    .boundary_machines
                    .iter()
                    .find(|declaration| declaration.id == boundary)
                    .and_then(|declaration| self.identities.text(declaration.identity))
                    .unwrap_or("<unknown>");
                format!("boundary `{name}` has no Terminal calling declaration")
            })?;
        self.calling_abi_signature(*declaration.application.as_ref().map_err(Clone::clone)?)
            .ok_or("boundary lost its exact declaration ABI")?
            .identity(&self.identities)
    }
    pub fn blocks(&self) -> impl Iterator<Item = &Block> + '_ {
        self.machines.iter().flat_map(|machine| &machine.blocks)
    }

    pub fn operations(&self) -> impl Iterator<Item = &Operation> + '_ {
        self.machines
            .iter()
            .flat_map(|machine| machine.operations())
    }

    pub fn machine(&self, id: MachineId) -> Option<&TerminalMachine> {
        self.machines.iter().find(|machine| machine.id == id)
    }

    pub fn declaration_contract(
        &self,
        id: semantic_vocabulary::ContractId,
    ) -> Option<&crate::MachineContract> {
        self.declaration_contracts
            .binary_search_by_key(&id, |contract| contract.id)
            .ok()
            .map(|position| &self.declaration_contracts[position])
    }

    /// Read the written contract of the exact declaration a call names. A
    /// boundary contributes its interface contract, never a fabricated body
    /// or a body-derived result summary. Provider admission remains separate.
    pub fn callable_requires(
        &self,
        callable: crate::TargetMachineCallable,
    ) -> Option<&[Proposition]> {
        match callable {
            crate::TargetMachineCallable::Machine(machine) => {
                Some(&self.machine(machine)?.contract.requires)
            }
            crate::TargetMachineCallable::Boundary(boundary) => {
                Some(&self.boundary_machine(boundary)?.scalar_requires)
            }
        }
    }

    pub fn callable_ensures(
        &self,
        callable: crate::TargetMachineCallable,
    ) -> Option<impl Iterator<Item = &Proposition>> {
        let (machine, boundary) = match callable {
            crate::TargetMachineCallable::Machine(machine) => (Some(self.machine(machine)?), None),
            crate::TargetMachineCallable::Boundary(boundary) => {
                (None, Some(self.boundary_machine(boundary)?))
            }
        };
        Some(
            machine
                .into_iter()
                .flat_map(|machine| {
                    machine
                        .contract
                        .ensures
                        .iter()
                        .map(|clause| &clause.proposition)
                })
                .chain(
                    boundary
                        .into_iter()
                        .flat_map(|boundary| &boundary.scalar_ensures),
                ),
        )
    }

    pub fn callable_structural_parameters(
        &self,
        callable: crate::TargetMachineCallable,
    ) -> Option<&[StructuralParameterDeclaration]> {
        match callable {
            crate::TargetMachineCallable::Machine(machine) => {
                Some(&self.machine(machine)?.structural_parameters)
            }
            crate::TargetMachineCallable::Boundary(boundary) => {
                Some(&self.boundary_machine(boundary)?.structural_parameters)
            }
        }
    }

    pub fn calling_abi_signature(
        &self,
        id: semantic_vocabulary::CallingSignatureId,
    ) -> Option<&crate::CallingAbiDeclaration> {
        self.calling_abi_signatures
            .binary_search_by_key(&id, |row| row.id)
            .ok()
            .map(|position| &self.calling_abi_signatures[position])
    }

    pub fn unoptimized_machine_behavior(
        &self,
        id: MachineId,
    ) -> Option<&crate::UnoptimizedMachineBehavior> {
        self.unoptimized_behavior
            .iter()
            .find(|body| body.machine == id)
    }

    pub fn service(&self, id: ServiceId) -> Option<&ServiceDeclaration> {
        self.services.iter().find(|service| service.id == id)
    }

    pub fn structural_type(
        &self,
        id: semantic_vocabulary::StructuralTypeId,
    ) -> Option<&StructuralTypeDeclaration> {
        find_structural_type(&self.structural_types, id)
    }

    pub fn foreign_reference_descriptor(
        &self,
        referent: semantic_vocabulary::StructuralTypeId,
    ) -> Option<bool> {
        if let Some(declaration) = self.structural_type(referent) {
            return Some(matches!(
                declaration.shape,
                StructuralTypeShape::ByteSequence(_) | StructuralTypeShape::ElementView { .. }
            ));
        }
        self.structural_referents
            .iter()
            .find(|row| row.id == referent)
            .map(|row| row.foreign_descriptor)
    }

    pub fn structural_domain(
        &self,
        id: semantic_vocabulary::StructuralDomainId,
    ) -> Option<&StructuralDomainDeclaration> {
        self.structural_domains
            .iter()
            .find(|declaration| declaration.id == id)
    }

    pub fn boundary_machine(
        &self,
        id: semantic_vocabulary::BoundaryMachineId,
    ) -> Option<&BoundaryMachineDeclaration> {
        self.boundary_machines
            .iter()
            .find(|declaration| declaration.id == id)
    }

    pub fn for_entry(entry: MachineId) -> Self {
        Self {
            entry: Some(TargetMachineCallable::Machine(entry)),
            ..Self::declarations()
        }
    }

    pub fn declarations() -> Self {
        Self {
            scalar_applications: Default::default(),
            folded_constants: Default::default(),
            indexed_theorems: Vec::new(),
            mathematical_carriers: Vec::new(),
            identity_formations: TerminalIdentityFormations::default(),
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: None,
            identities: Default::default(),
            scalar_qualifications: Default::default(),
            structural_types: Vec::new(),
            structural_referents: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            access_view_inputs: Vec::new(),
            access_view_plans: Vec::new(),
            access_lifecycle_plans: Vec::new(),
            reborrow_restored_call_uses: Vec::new(),
            boundary_machines: Vec::new(),
            calling_signatures: Vec::new(),
            calling_abi_signatures: Vec::new(),
            calling_policy_requests: Vec::new(),
            opaque_representations: Vec::new(),
            unoptimized_behavior: Vec::new(),
            external_providers: Vec::new(),
            provider_candidates: Vec::new(),
            provider_instance_requirements: Vec::new(),
            boundary_application_candidates: Vec::new(),
            callback_applications: Vec::new(),
            installed_interrupt_registrations: Vec::new(),
            dynamic_dispatch: TerminalDynamicDispatchCatalog::default(),
            quotients: Default::default(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            proof_recursive_components: Vec::new(),
            scalar_block_invariants: Vec::new(),
            operation_crash_contracts: Vec::new(),
            closed_conformance_applications: Vec::new(),
            suspension_call_plan_count: 0,
            suspension_call_sites: Vec::new(),
            suspension_call_plans: Vec::new(),
            declaration_contracts: Vec::new(),
            machines: Vec::new(),
            target_machine_families: Vec::new(),
            target_layout_applications: Vec::new(),
            native_layouts: Default::default(),
            boundary_application_demands: Vec::new(),
            task_activation_demands: TerminalTaskActivationDemands::default(),
        }
    }
}
