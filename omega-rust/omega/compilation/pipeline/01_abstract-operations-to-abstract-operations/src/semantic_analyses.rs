/*
Target-neutral semantic analyses over `PsiOptimizationUnit`: interprocedural
effect summaries (`effect_summaries`), ownership frontiers
(`ownership_frontiers`), per-function place-access bounds
(`place_access_summaries`), declared-place aliasing (`place_aliases`), sparse
conditional constants and executable edges (`sparse_conditional_constants`),
scalar use-definition chains (`use_definitions`), and backward value liveness
(`value_liveness`).
*/

pub(crate) mod effect_summaries {
    use std::collections::{BTreeMap, BTreeSet};

    use abstract_operations::AbstractOperation as O;
    use optimization_core::OptimizationUnitIdentity;
    use optimization_unit::{PsiOptimizationUnit, PsiProvenance};
    use semantic_vocabulary::{BlockId, BoundaryMachineId, MachineId, ServiceId};

    use super::semantically_reachable_blocks;

    pub(crate) type EffectKnowledge = optimization_unit::ObservationKnowledge;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub(crate) enum EffectClass {
        PureScalar,
        StructuralState,
        InternalCall,
        BoundaryCall,
        Service,
        Control,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct NodeEffectSummary {
        pub machine: MachineId,
        pub block: BlockId,
        pub node: u32,
        pub class: EffectClass,
        pub observable: EffectKnowledge,
        pub structural_state: EffectKnowledge,
        pub crash: EffectKnowledge,
        pub suspension: EffectKnowledge,
        pub support: Vec<PsiProvenance>,
        pub revision: OptimizationUnitIdentity,
    }

    impl NodeEffectSummary {
        pub(crate) fn is_pure_scalar_baseline(&self, revision: OptimizationUnitIdentity) -> bool {
            self.revision == revision
                && self.class == EffectClass::PureScalar
                && self.observable == EffectKnowledge::No
                && self.structural_state == EffectKnowledge::No
                && self.crash == EffectKnowledge::No
                && self.suspension == EffectKnowledge::No
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct EffectSummaryAnalysis {
        pub nodes: Vec<NodeEffectSummary>,
        pub functions: Vec<FunctionEffectSummary>,
    }

    impl EffectSummaryAnalysis {
        pub fn node(
            &self,
            machine: MachineId,
            block: BlockId,
            node: u32,
        ) -> Option<&NodeEffectSummary> {
            self.nodes
                .iter()
                .find(|row| row.machine == machine && row.block == block && row.node == node)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct FunctionEffectSummary {
        pub machine: MachineId,
        pub observable: EffectKnowledge,
        pub structural_state: EffectKnowledge,
        pub crash: EffectKnowledge,
        pub suspension: EffectKnowledge,
        pub services: Vec<ServiceId>,
        pub boundaries: Vec<BoundaryMachineId>,
        pub support: Vec<PsiProvenance>,
        pub revision: OptimizationUnitIdentity,
    }

    pub(crate) fn effect_summaries(
        unit: &PsiOptimizationUnit,
    ) -> Result<EffectSummaryAnalysis, &'static str> {
        let mut nodes = Vec::new();
        for function in &unit.functions {
            for block in &function.blocks {
                for (node_index, node) in block.nodes.iter().enumerate() {
                    let node_index =
                        u32::try_from(node_index).expect("optimization node index is u32");
                    let (class, observable, structural_state, crash, suspension) =
                        operation_effect(&node.operation);
                    nodes.push(NodeEffectSummary {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                        class,
                        observable,
                        structural_state,
                        crash,
                        suspension,
                        support: node
                            .provenance
                            .iter()
                            .chain(node.successors.iter().flat_map(|edge| &edge.provenance))
                            .copied()
                            .collect(),
                        revision: unit.identity,
                    });
                }
            }
        }
        Ok(EffectSummaryAnalysis {
            nodes,
            functions: transitive_function_effects(unit)?,
        })
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct PendingFunctionEffect {
        observable: EffectKnowledge,
        structural_state: EffectKnowledge,
        crash: EffectKnowledge,
        suspension: EffectKnowledge,
        services: BTreeSet<ServiceId>,
        boundaries: BTreeSet<BoundaryMachineId>,
        support: BTreeSet<PsiProvenance>,
        callees: BTreeSet<MachineId>,
    }

    fn transitive_function_effects(
        unit: &PsiOptimizationUnit,
    ) -> Result<Vec<FunctionEffectSummary>, &'static str> {
        let callable_invocations =
            optimization_unit_semantics::callable_values::optimized_callable_invocations(unit)?;
        fn join(left: EffectKnowledge, right: EffectKnowledge) -> EffectKnowledge {
            use optimization_unit::ObservationKnowledge::{May, No, Yes};
            match (left, right) {
                (May, _) | (_, May) => May,
                (Yes, _) | (_, Yes) => Yes,
                (No, No) => No,
            }
        }

        let machines = unit
            .functions
            .iter()
            .map(|function| function.machine)
            .collect::<BTreeSet<_>>();
        let mut summaries = BTreeMap::<MachineId, PendingFunctionEffect>::new();
        for function in &unit.functions {
            let mut summary = PendingFunctionEffect {
                observable: EffectKnowledge::No,
                structural_state: EffectKnowledge::No,
                crash: EffectKnowledge::No,
                suspension: EffectKnowledge::No,
                services: BTreeSet::new(),
                boundaries: BTreeSet::new(),
                support: BTreeSet::new(),
                callees: BTreeSet::new(),
            };
            let reachable = semantically_reachable_blocks(function);
            for node in function
                .blocks
                .iter()
                .filter(|block| reachable.contains(&block.id))
                .flat_map(|block| &block.nodes)
            {
                summary.support.extend(node.provenance.iter().copied());
                summary.support.extend(
                    node.successors
                        .iter()
                        .flat_map(|edge| edge.provenance.iter().copied()),
                );
                match &node.operation {
                    abstract_operations::call_or_call_structural_pattern!(..)
                    | O::InvokeNoNormal { .. } => {
                        let mut callees = Vec::new();
                        let mut unresolved = false;
                        for reference in unit.call_machine_references(&node.operation) {
                            match reference {
                                optimization_unit::CallMachineReference::Declaration(machine) => {
                                    callees.push(machine)
                                }
                                optimization_unit::CallMachineReference::Callable {
                                    contract,
                                    ..
                                } => {
                                    let operation = node.operation.psi_operation()
                                        .ok_or("effect summary runtime invocation lost its operation identity")?;
                                    let row = callable_invocations
                                        .as_ref()
                                        .map(|summary| {
                                            summary.operation(function.machine, operation)
                                        })
                                        .transpose()?
                                        .flatten();
                                    if let Some(row) = row {
                                        callees.extend(row.selected_machines.iter().copied());
                                    }
                                    unresolved |= row.is_none_or(|row| !row.targets_are_resolved());
                                    let contract = unit.callables.contract(contract)
                                        .ok_or("effect summary runtime invocation lost its exact callable contract")?;
                                    summary
                                        .services
                                        .extend(contract.envelope.service_reach.iter().copied());
                                }
                            }
                        }
                        if callees.is_empty()
                            || unresolved
                            || callees.iter().any(|callee| !machines.contains(callee))
                        {
                            summary.observable = EffectKnowledge::May;
                            summary.structural_state = EffectKnowledge::May;
                            summary.crash = EffectKnowledge::May;
                            summary.suspension = EffectKnowledge::May;
                        }
                        summary.callees.extend(callees);
                        if matches!(
                            &node.operation,
                            O::CallStructural {
                                callee: abstract_operations::AbstractCallTarget::Dynamic { .. },
                                ..
                            }
                        ) {
                            summary.observable = EffectKnowledge::May;
                            summary.structural_state = EffectKnowledge::May;
                            summary.crash = EffectKnowledge::May;
                            summary.suspension = EffectKnowledge::May;
                        }
                    }
                    O::ActivateTaskRegister { service, .. }
                    | O::ReadTaskRegister { service, .. }
                    | O::ReloadSegmentDescriptors { service, .. }
                    | O::ReadSegmentSelector { service, .. }
                    | O::GlobalDescriptorTable { service, .. }
                    | O::PortWrite { service, .. }
                    | O::RegisterAccess {
                        service: Some(service),
                        ..
                    } => {
                        summary.services.insert(*service);
                        join_operation_effect(&mut summary, &node.operation, join);
                    }
                    O::BoundaryCall { boundary, .. } => {
                        summary.boundaries.insert(*boundary);
                        join_operation_effect(&mut summary, &node.operation, join);
                    }
                    _ => join_operation_effect(&mut summary, &node.operation, join),
                }
            }
            summaries.insert(function.machine, summary);
        }

        loop {
            let prior = summaries.clone();
            let mut changed = false;
            for summary in summaries.values_mut() {
                for callee in summary.callees.clone() {
                    let Some(callee) = prior.get(&callee) else {
                        continue;
                    };
                    let before = summary.clone();
                    summary.observable = join(summary.observable, callee.observable);
                    summary.structural_state =
                        join(summary.structural_state, callee.structural_state);
                    summary.crash = join(summary.crash, callee.crash);
                    summary.suspension = join(summary.suspension, callee.suspension);
                    summary.services.extend(callee.services.iter().copied());
                    summary.boundaries.extend(callee.boundaries.iter().copied());
                    summary.support.extend(callee.support.iter().copied());
                    changed |= *summary != before;
                }
            }
            if !changed {
                break;
            }
        }

        Ok(summaries
            .into_iter()
            .map(|(machine, summary)| FunctionEffectSummary {
                machine,
                observable: summary.observable,
                structural_state: summary.structural_state,
                crash: summary.crash,
                suspension: summary.suspension,
                services: summary.services.into_iter().collect(),
                boundaries: summary.boundaries.into_iter().collect(),
                support: summary.support.into_iter().collect(),
                revision: unit.identity,
            })
            .collect())
    }

    fn join_operation_effect(
        summary: &mut PendingFunctionEffect,
        operation: &O,
        join: fn(EffectKnowledge, EffectKnowledge) -> EffectKnowledge,
    ) {
        let (_, observable, structural, crash, suspension) = operation_effect(operation);
        summary.observable = join(summary.observable, observable);
        summary.structural_state = join(summary.structural_state, structural);
        summary.crash = join(summary.crash, crash);
        summary.suspension = join(summary.suspension, suspension);
    }

    fn operation_effect(
        operation: &O,
    ) -> (
        EffectClass,
        EffectKnowledge,
        EffectKnowledge,
        EffectKnowledge,
        EffectKnowledge,
    ) {
        use optimization_unit::ObservationKnowledge::{May, No, Yes};
        match operation {
            O::SelectImplementation { .. }
            | O::SelectMachine { .. }
            | O::IntegerConstant { .. }
            | O::IeeeFloatConstant { .. }
            | O::NearestIeeeFloatFusedMultiplyAdd { .. }
            | O::IeeeFloatCompare { .. }
            | O::BooleanConstant { .. }
            | abstract_operations::boolean_not_or_equal_pair_pattern!(..)
            | abstract_operations::integer_equal_less_pair_pattern!(..)
            | abstract_operations::bitwise_not_or_less_pair_pattern!(..)
            | abstract_operations::integer_widen_or_exact_cast_pattern!(..)
            | abstract_operations::bitwise_and_or_view_pair_pattern!(..)
            | abstract_operations::bitwise_or_xor_pair_pattern!(..)
            | O::IntegerShift { .. }
            | O::IntegerArithmetic { .. } => (EffectClass::PureScalar, No, No, No, No),
            O::StorageAddress { .. } => (EffectClass::StructuralState, Yes, Yes, No, No),
            O::EstablishActivationClaim { .. }
            | O::SplitActivationClaim { .. }
            | O::MergeActivationClaim { .. }
            | O::ReleaseActivationClaim { .. }
            | O::EstablishNativeCodeLayout { .. }
            | O::EstablishPrimitiveLocal { .. }
            | O::PrimitiveLocalStore { .. }
            | O::PrimitiveScalarRead { .. }
            | O::StructuralCaseMembership { .. }
            | O::WriteOnlyPrimitiveStore { .. }
            | O::ByteSequenceWrite { .. }
            | abstract_operations::byte_sequence_store_pair_pattern!(..)
            | O::StructuralByteViewFieldStore { .. }
            | O::StructuralByteSequenceFieldByteStore { .. }
            | O::StructuralScalarFieldStore { .. }
            | abstract_operations::move_or_store_structural_field_pattern!()
            | O::StructuralLeafCopy { .. }
            | O::MoveStructuralValue { .. }
            | O::TransformContent { .. }
            | O::EstablishScalarArray { .. }
            | abstract_operations::structural_array_or_case_pattern!(..)
            | abstract_operations::establish_byte_literal_triple_pattern!(..)
            | abstract_operations::structural_establish_triple_pattern!(..)
            | O::EstablishReference { .. }
            | O::EstablishAccessReference { .. }
            | O::ReleaseReference { .. }
            | abstract_operations::byte_read_predicate_equal_pattern!(..)
            | O::ByteSequenceSubslice { .. }
            | abstract_operations::byte_view_length_triple_pattern!(..)
            | abstract_operations::element_view_read_write_pair_pattern!(..)
            | O::ElementViewSubslice { .. }
            | abstract_operations::byte_len_or_field_read_pattern!(..)
            | O::StructuralScalarFieldRead { .. }
            | O::ReturnStructural { .. } => (EffectClass::StructuralState, No, Yes, No, No),
            O::AtomicEvent { event, .. } => (
                EffectClass::StructuralState,
                Yes,
                if event.place().is_some() { Yes } else { No },
                No,
                No,
            ),
            abstract_operations::call_or_call_structural_pattern!(..) => {
                (EffectClass::InternalCall, May, May, May, May)
            }
            O::BoundaryCall { .. } => (EffectClass::BoundaryCall, Yes, May, May, May),
            O::InvokeNoNormal { .. } => (EffectClass::InternalCall, Yes, May, May, May),
            O::ExternalCompletion { .. } => (EffectClass::BoundaryCall, Yes, May, No, No),
            O::PortWrite { .. } => (EffectClass::Service, Yes, No, No, No),
            O::ActivateTaskRegister { .. }
            | O::ReadTaskRegister { .. }
            | O::ReloadSegmentDescriptors { .. }
            | O::ReadSegmentSelector { .. }
            | O::RegisterAccess { .. }
            | O::GlobalDescriptorTable { .. } => (EffectClass::Service, Yes, May, No, No),
            O::Crash { .. } => (EffectClass::Control, Yes, No, Yes, No),
            O::TrappingInteger { .. } => (EffectClass::Control, May, No, May, No),
            O::FloatResultPolicy { policy, .. } => {
                if policy.can_trap() {
                    (EffectClass::Control, May, No, May, No)
                } else {
                    (EffectClass::PureScalar, No, No, No, No)
                }
            }
            O::StructuralCase { .. } => (EffectClass::Control, No, Yes, No, No),
            abstract_operations::jump_or_conditional_pattern!()
            | abstract_operations::return_or_return_unit_pattern!() => {
                (EffectClass::Control, No, May, No, No)
            }
        }
    }
}
pub(crate) mod ownership_frontiers {
    use optimization_core::{OptimizationUnitIdentity, OwnershipFrontierFactIdentity};
    use optimization_unit::{
        OwnershipFrontierSite, OwnershipFrontierSnapshot, PsiOptimizationUnit,
    };
    use semantic_vocabulary::MachineId;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct OwnershipFrontierAnalysisFact {
        pub identity: OwnershipFrontierFactIdentity,
        pub revision: OptimizationUnitIdentity,
        pub machine: MachineId,
        pub site: OwnershipFrontierSite,
        pub snapshot: OwnershipFrontierSnapshot,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct OwnershipFrontierAnalysis {
        pub facts: Vec<OwnershipFrontierAnalysisFact>,
    }

    impl OwnershipFrontierAnalysis {
        pub fn fact(
            &self,
            machine: MachineId,
            site: OwnershipFrontierSite,
        ) -> Option<&OwnershipFrontierAnalysisFact> {
            self.facts
                .binary_search_by_key(&(machine, site), |fact| (fact.machine, fact.site))
                .ok()
                .map(|index| &self.facts[index])
        }
    }

    pub(crate) fn ownership_frontiers(unit: &PsiOptimizationUnit) -> OwnershipFrontierAnalysis {
        OwnershipFrontierAnalysis {
            facts: unit
                .ownership_frontier_facts
                .iter()
                .map(|fact| OwnershipFrontierAnalysisFact {
                    identity: fact.identity,
                    revision: unit.identity,
                    machine: fact.machine,
                    site: fact.site,
                    snapshot: fact.snapshot.clone(),
                })
                .collect(),
        }
    }
}
pub(crate) mod place_access_summaries {
    use std::collections::{BTreeMap, BTreeSet};

    use abstract_operations::{
        AbstractAtomicEvent, AbstractOperation as O, AbstractStructuralBinding, StructuralCopyPath,
    };
    use optimization_core::OptimizationUnitIdentity;
    use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance};
    use semantic_vocabulary::{
        BlockId, CanonicalStructuralPathSegment, MachineId, PlaceId, StructuralFieldId,
        StructuralPlaceKind,
    };
    use terminal_psi::{
        ReferenceOriginProjection, StructuralAccess, StructuralArgument, StructuralPathSegment,
        StructuralReferenceOrigin, TerminalAffineCleanupAction, control_flow::ByteSequenceOperand,
    };

    use super::semantically_reachable_blocks;

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub(crate) enum PlaceAccessStep {
        Runtime(StructuralPathSegment),
        Canonical(CanonicalStructuralPathSegment),
        Element,
    }

    impl PlaceAccessStep {
        fn is_referent(&self) -> bool {
            matches!(
                self,
                Self::Runtime(StructuralPathSegment::Referent)
                    | Self::Canonical(CanonicalStructuralPathSegment::Referent)
            )
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub(crate) struct PlaceAccessView {
        pub owner: MachineId,
        pub root: PlaceId,
        pub path: Vec<PlaceAccessStep>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ParameterAccessSurface {
        pub position: u32,
        pub place: PlaceId,
        pub declared: StructuralAccess,
        pub reads: Vec<Vec<PlaceAccessStep>>,
        pub writes: Vec<Vec<PlaceAccessStep>>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ResolvedResultSource {
        pub result_path: Vec<StructuralPathSegment>,
        pub origins: Vec<ResultSourceResolution>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum ResultSourceResolution {
        Parameter {
            position: u32,
            path: Vec<PlaceAccessStep>,
        },
        External {
            root: PlaceId,
            path: Vec<PlaceAccessStep>,
        },
        Internal,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct FunctionPlaceAccessSummary {
        pub machine: MachineId,
        pub parameters: Vec<ParameterAccessSurface>,
        pub external_reads: Vec<PlaceAccessView>,
        pub external_writes: Vec<PlaceAccessView>,
        pub result_sources: Vec<ResolvedResultSource>,
        pub unsummarized_targets: bool,
        pub support: Vec<PsiProvenance>,
        pub revision: OptimizationUnitIdentity,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct PlaceAccessSummaryAnalysis {
        pub functions: Vec<FunctionPlaceAccessSummary>,
    }

    pub(crate) fn declared_access(access: StructuralAccess) -> (bool, bool) {
        match access {
            StructuralAccess::SharedBorrow => (true, false),
            terminal_psi::mutating_access_pair_pattern!() => (true, true),
            StructuralAccess::WriteOnlyBorrow => (false, true),
        }
    }

    fn runtime_prefix(path: &[StructuralPathSegment]) -> Vec<PlaceAccessStep> {
        path.iter().cloned().map(PlaceAccessStep::Runtime).collect()
    }

    fn canonical_prefix(path: &[CanonicalStructuralPathSegment]) -> Vec<PlaceAccessStep> {
        path.iter()
            .copied()
            .map(|step| match step {
                CanonicalStructuralPathSegment::Referent => {
                    PlaceAccessStep::Runtime(StructuralPathSegment::Referent)
                }
                step => PlaceAccessStep::Canonical(step),
            })
            .collect()
    }

    fn with_field(path: &mut Vec<PlaceAccessStep>, field: StructuralFieldId) {
        path.push(PlaceAccessStep::Canonical(
            CanonicalStructuralPathSegment::Field(field),
        ));
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum AccessKind {
        Read,
        Write,
    }

    enum ResolvedAccess {
        Parameter {
            position: u32,
            path: Vec<PlaceAccessStep>,
        },
        External {
            root: PlaceId,
            path: Vec<PlaceAccessStep>,
        },
        Internal,
    }

    #[derive(Debug, Clone)]
    enum AliasTarget {
        View {
            target: PlaceId,
            prefix: Vec<PlaceAccessStep>,
        },
        Reference {
            target: PlaceId,
            prefix: Vec<PlaceAccessStep>,
        },
    }

    struct FunctionContext<'u> {
        function: &'u PsiOptimizationFunction,
        roots: BTreeMap<PlaceId, Option<StructuralPlaceKind>>,
        aliases: BTreeMap<PlaceId, AliasTarget>,
        bindings: BTreeMap<PlaceId, Vec<(PlaceId, Vec<StructuralPathSegment>)>>,
        reachable: BTreeSet<BlockId>,
    }

    impl<'u> FunctionContext<'u> {
        fn build(function: &'u PsiOptimizationFunction) -> Self {
            let mut roots: BTreeMap<PlaceId, Option<StructuralPlaceKind>> = function
                .declared_places
                .iter()
                .map(|place| (*place, None))
                .collect();
            for declaration in &function.structural_places {
                roots.insert(declaration.id, Some(declaration.kind));
            }
            for parameter in &function.structural_parameters {
                roots.insert(
                    parameter.place,
                    Some(StructuralPlaceKind::Parameter {
                        position: parameter.position,
                    }),
                );
            }
            for block in &function.blocks {
                for (position, parameter) in block.structural_parameters.iter().enumerate() {
                    roots.insert(
                        parameter.place,
                        Some(StructuralPlaceKind::BlockParameter {
                            block: block.id,
                            position: u32::try_from(position)
                                .expect("block structural parameter index is u32"),
                        }),
                    );
                }
            }
            if let Some(result) = function.result.structural() {
                roots.insert(result.place, Some(StructuralPlaceKind::Result));
            }

            let mut aliases = BTreeMap::new();
            let mut bindings: BTreeMap<PlaceId, Vec<(PlaceId, Vec<StructuralPathSegment>)>> =
                BTreeMap::new();
            for block in &function.blocks {
                for node in &block.nodes {
                    match &node.operation {
                        abstract_operations::element_view_or_byte_subslice_pattern!(
                            result,
                            source,
                            ..
                        )
                        | O::EstablishElementView { result, source, .. } => {
                            aliases.insert(
                                result.place,
                                AliasTarget::View {
                                    target: source.place,
                                    prefix: runtime_prefix(&source.path),
                                },
                            );
                        }
                        O::EstablishReference { result, source, .. }
                        | O::EstablishAccessReference { result, source, .. } => {
                            aliases.insert(
                                result.place,
                                AliasTarget::Reference {
                                    target: source.place,
                                    prefix: runtime_prefix(&source.path),
                                },
                            );
                        }
                        O::Jump {
                            structural_bindings,
                            ..
                        } => bind_arguments(&mut bindings, structural_bindings),
                        O::Conditional {
                            when_true,
                            when_false,
                            ..
                        } => {
                            bind_arguments(&mut bindings, &when_true.structural_bindings);
                            bind_arguments(&mut bindings, &when_false.structural_bindings);
                        }
                        _ => {}
                    }
                }
            }
            Self {
                function,
                roots,
                aliases,
                bindings,
                reachable: semantically_reachable_blocks(function),
            }
        }

        fn resolve(&self, place: PlaceId, path: Vec<PlaceAccessStep>) -> Vec<ResolvedAccess> {
            let mut visiting = BTreeSet::new();
            self.resolve_inner(place, path, &mut visiting)
        }

        fn resolve_inner(
            &self,
            place: PlaceId,
            path: Vec<PlaceAccessStep>,
            visiting: &mut BTreeSet<PlaceId>,
        ) -> Vec<ResolvedAccess> {
            if !visiting.insert(place) {
                return vec![ResolvedAccess::External { root: place, path }];
            }
            let resolved = self.resolve_kind(place, path, visiting);
            visiting.remove(&place);
            resolved
        }

        fn resolve_kind(
            &self,
            place: PlaceId,
            path: Vec<PlaceAccessStep>,
            visiting: &mut BTreeSet<PlaceId>,
        ) -> Vec<ResolvedAccess> {
            match self.roots.get(&place).copied().flatten() {
                Some(StructuralPlaceKind::Parameter { position, .. }) => {
                    vec![ResolvedAccess::Parameter { position, path }]
                }
                Some(StructuralPlaceKind::BlockParameter { .. }) => match self.bindings.get(&place)
                {
                    Some(arguments) if !arguments.is_empty() => arguments
                        .iter()
                        .flat_map(|(argument_place, argument_path)| {
                            let mut composed = runtime_prefix(argument_path);
                            composed.extend(path.iter().cloned());
                            self.resolve_inner(*argument_place, composed, visiting)
                        })
                        .collect(),
                    _ => vec![ResolvedAccess::External { root: place, path }],
                },
                Some(StructuralPlaceKind::OperationResult { .. })
                | Some(StructuralPlaceKind::ByteSequenceLiteral { .. })
                | Some(StructuralPlaceKind::TrivialAffineLocal { .. }) => {
                    match self.aliases.get(&place) {
                        Some(AliasTarget::View { target, prefix }) => {
                            let mut composed = prefix.clone();
                            composed.extend(path.iter().cloned());
                            self.resolve_inner(*target, composed, visiting)
                        }
                        Some(AliasTarget::Reference { target, prefix }) => match path.first() {
                            Some(step) if step.is_referent() => {
                                let mut composed = prefix.clone();
                                composed.extend(path[1..].iter().cloned());
                                self.resolve_inner(*target, composed, visiting)
                            }
                            _ => vec![ResolvedAccess::Internal],
                        },
                        None if path.iter().any(PlaceAccessStep::is_referent) => {
                            vec![ResolvedAccess::External { root: place, path }]
                        }
                        None => vec![ResolvedAccess::Internal],
                    }
                }
                Some(StructuralPlaceKind::Result) => {
                    if path.iter().any(PlaceAccessStep::is_referent) {
                        vec![ResolvedAccess::External { root: place, path }]
                    } else {
                        vec![ResolvedAccess::Internal]
                    }
                }
                Some(StructuralPlaceKind::ProviderAttachment { .. }) | None => {
                    vec![ResolvedAccess::External { root: place, path }]
                }
            }
        }
    }

    fn bind_arguments(
        bindings: &mut BTreeMap<PlaceId, Vec<(PlaceId, Vec<StructuralPathSegment>)>>,
        structural_bindings: &[AbstractStructuralBinding],
    ) {
        for binding in structural_bindings {
            bindings
                .entry(binding.parameter)
                .or_default()
                .push((binding.argument.place, binding.argument.path.clone()));
        }
    }

    #[derive(Debug, Default, Clone, PartialEq, Eq)]
    struct PendingSurface {
        parameter_reads: BTreeMap<u32, BTreeSet<Vec<PlaceAccessStep>>>,
        parameter_writes: BTreeMap<u32, BTreeSet<Vec<PlaceAccessStep>>>,
        external_reads: BTreeSet<PlaceAccessView>,
        external_writes: BTreeSet<PlaceAccessView>,
        unsummarized: bool,
        support: BTreeSet<PsiProvenance>,
        contributions: Vec<PendingContribution>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum PendingContribution {
        Call {
            callee: MachineId,
            arguments: Vec<StructuralArgument>,
        },
        Reach {
            callee: MachineId,
        },
    }

    impl PendingSurface {
        fn attribute(
            &mut self,
            context: &FunctionContext<'_>,
            place: PlaceId,
            path: Vec<PlaceAccessStep>,
            kind: AccessKind,
        ) {
            for resolved in context.resolve(place, path) {
                match (resolved, kind) {
                    (ResolvedAccess::Parameter { position, path }, AccessKind::Read) => {
                        self.parameter_reads
                            .entry(position)
                            .or_default()
                            .insert(path);
                    }
                    (ResolvedAccess::Parameter { position, path }, AccessKind::Write) => {
                        self.parameter_writes
                            .entry(position)
                            .or_default()
                            .insert(path);
                    }
                    (ResolvedAccess::External { root, path }, AccessKind::Read) => {
                        self.external_reads.insert(PlaceAccessView {
                            owner: context.function.machine,
                            root,
                            path,
                        });
                    }
                    (ResolvedAccess::External { root, path }, AccessKind::Write) => {
                        self.external_writes.insert(PlaceAccessView {
                            owner: context.function.machine,
                            root,
                            path,
                        });
                    }
                    (ResolvedAccess::Internal, _) => {}
                }
            }
        }

        fn attribute_argument(
            &mut self,
            context: &FunctionContext<'_>,
            argument: &StructuralArgument,
            reads: bool,
            writes: bool,
        ) {
            let path = runtime_prefix(&argument.path);
            if reads {
                self.attribute(context, argument.place, path.clone(), AccessKind::Read);
            }
            if writes {
                self.attribute(context, argument.place, path, AccessKind::Write);
            }
        }

        fn push_call(
            &mut self,
            context: &FunctionContext<'_>,
            machines: &BTreeSet<MachineId>,
            callee: MachineId,
            structural_arguments: &[StructuralArgument],
        ) {
            if machines.contains(&callee) {
                self.contributions.push(PendingContribution::Call {
                    callee,
                    arguments: structural_arguments.to_vec(),
                });
            } else {
                for argument in structural_arguments {
                    let (reads, writes) = declared_access(argument.access);
                    self.attribute_argument(context, argument, reads, writes);
                }
                self.unsummarized = true;
            }
        }

        fn push_reach(&mut self, machines: &BTreeSet<MachineId>, callee: MachineId) {
            if machines.contains(&callee) {
                self.contributions
                    .push(PendingContribution::Reach { callee });
            } else {
                self.unsummarized = true;
            }
        }
    }

    fn scan_operation(
        context: &FunctionContext<'_>,
        machines: &BTreeSet<MachineId>,
        pending: &mut PendingSurface,
        operation: &O,
    ) {
        for (_, actions) in operation.cleanup_rosters() {
            for action in actions {
                match action {
                    TerminalAffineCleanupAction::RetireHome(_) => {}
                    TerminalAffineCleanupAction::DiscardRoot(place) => {
                        pending.attribute(context, *place, Vec::new(), AccessKind::Write);
                    }
                    TerminalAffineCleanupAction::DiscardResidual(discard) => {
                        pending.attribute(
                            context,
                            discard.place,
                            runtime_prefix(&discard.path),
                            AccessKind::Write,
                        );
                    }
                    TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                        pending.attribute(context, cleanup.place, Vec::new(), AccessKind::Write);
                        pending.push_reach(machines, cleanup.cleanup_machine);
                    }
                }
            }
        }
        match operation {
            O::ReleaseActivationClaim { source, .. } => {
                pending.attribute_argument(context, source, true, true)
            }
            O::ActivateTaskRegister {
                gdt,
                task_state,
                scratch,
                ..
            } => {
                pending.attribute_argument(context, gdt, true, true);
                pending.attribute_argument(context, task_state, true, false);
                pending.attribute_argument(context, scratch, false, true);
            }
            O::ReloadSegmentDescriptors { gdt, scratch, .. } => {
                pending.attribute_argument(context, gdt, true, true);
                pending.attribute_argument(context, scratch, false, true);
            }
            O::ReadTaskRegister { .. } | O::ReadSegmentSelector { .. } => {}
            O::GlobalDescriptorTable {
                operation, source, ..
            } => {
                let writes = *operation == terminal_psi::GlobalDescriptorTableOperation::Store;
                pending.attribute_argument(context, source, !writes, writes);
            }
            O::RegisterAccess { .. } => {
                pending.unsummarized = true;
            }
            O::StorageAddress { source, path, .. } => {
                pending.attribute(context, *source, runtime_prefix(path), AccessKind::Read);
            }
            O::PrimitiveScalarRead { source, path, .. } => {
                pending.attribute(context, *source, runtime_prefix(path), AccessKind::Read);
            }
            O::StructuralScalarFieldRead {
                source,
                path,
                field,
                ..
            } => {
                let mut view = canonical_prefix(path);
                with_field(&mut view, *field);
                pending.attribute(context, *source, view, AccessKind::Read);
            }
            O::StructuralCaseMembership { source, path, .. } => {
                pending.attribute(context, *source, runtime_prefix(path), AccessKind::Read);
            }
            O::StructuralCase { source, path, .. } => {
                pending.attribute(context, *source, runtime_prefix(path), AccessKind::Read);
            }
            O::StructuralLeafCopy { source, path, .. } => {
                let view = match path {
                    StructuralCopyPath::Ordinary(path) => runtime_prefix(path),
                    StructuralCopyPath::Case(path) => canonical_prefix(path),
                };
                pending.attribute(context, *source, view, AccessKind::Read);
            }
            O::MoveStructuralValue { source, .. } => {
                pending.attribute_argument(context, source, true, true);
            }
            O::TransformContent {
                source, parents, ..
            } => {
                for argument in std::iter::once(source).chain(parents) {
                    pending.attribute_argument(context, argument, true, true);
                }
            }
            O::ByteSequenceRead { source, .. } => {
                pending.attribute(context, *source, Vec::new(), AccessKind::Read);
            }
            O::ByteSequencePredicate { operand, .. } => match operand {
                ByteSequenceOperand::Whole { source, .. } => {
                    pending.attribute_argument(context, source, true, false);
                }
                ByteSequenceOperand::Field { source, field, .. } => {
                    let mut view = runtime_prefix(&source.path);
                    with_field(&mut view, *field);
                    pending.attribute(context, source.place, view, AccessKind::Read);
                }
            },
            O::ElementViewLength { source, .. }
            | O::ByteSequenceLength { source, .. }
            | O::ByteSequenceSubslice { source, .. }
            | O::ElementViewSubslice { source, .. } => {
                pending.attribute(
                    context,
                    source.place,
                    runtime_prefix(&source.path),
                    AccessKind::Read,
                );
            }
            O::ElementViewRead { source, path, .. } => {
                pending.attribute(context, *source, runtime_prefix(path), AccessKind::Read);
            }
            O::ElementViewWrite { destination, .. } => {
                pending.attribute(
                    context,
                    destination.place,
                    runtime_prefix(&destination.path),
                    AccessKind::Write,
                );
            }
            abstract_operations::byte_len_or_field_read_pattern!(source, path, field, ..) => {
                let mut view = runtime_prefix(path);
                with_field(&mut view, *field);
                pending.attribute(context, *source, view, AccessKind::Read);
            }
            O::PrimitiveLocalStore { destination, .. }
            | O::ByteSequenceWrite { destination, .. } => {
                pending.attribute(context, *destination, Vec::new(), AccessKind::Write);
            }
            O::ByteSequenceStore {
                destination,
                source,
                ..
            } => {
                pending.attribute(context, *destination, Vec::new(), AccessKind::Write);
                pending.attribute(context, *source, Vec::new(), AccessKind::Read);
            }
            O::WriteOnlyPrimitiveStore {
                destination, path, ..
            } => {
                pending.attribute(
                    context,
                    destination.place(),
                    runtime_prefix(path),
                    AccessKind::Write,
                );
            }
            O::StructuralScalarFieldStore {
                destination,
                path,
                field,
                ..
            } => {
                let mut view = runtime_prefix(path);
                with_field(&mut view, *field);
                pending.attribute(context, destination.place(), view, AccessKind::Write);
            }
            O::StructuralByteSequenceFieldStore {
                destination,
                path,
                field,
                source,
                ..
            } => {
                let mut view = runtime_prefix(path);
                with_field(&mut view, *field);
                pending.attribute(context, *destination, view, AccessKind::Write);
                pending.attribute(context, *source, Vec::new(), AccessKind::Read);
            }
            O::StructuralByteViewFieldStore {
                destination,
                path,
                field,
                source,
                ..
            } => {
                let mut view = runtime_prefix(path);
                if let Some(field) = field {
                    with_field(&mut view, *field);
                }
                pending.attribute(context, *destination, view, AccessKind::Write);
                pending.attribute(context, *source, Vec::new(), AccessKind::Read);
            }
            O::StructuralByteSequenceFieldByteStore {
                destination,
                path,
                field,
                ..
            } => {
                let mut view = runtime_prefix(path);
                with_field(&mut view, *field);
                pending.attribute(context, *destination, view, AccessKind::Write);
            }
            O::MoveStructuralField {
                source,
                path,
                field,
                ..
            } => {
                let mut view = runtime_prefix(path);
                if let Some(field) = field {
                    with_field(&mut view, *field);
                }
                pending.attribute(context, source.place(), view, AccessKind::Write);
            }
            O::StoreStructuralField {
                destination,
                path,
                field,
                value,
                ..
            } => {
                let mut view = runtime_prefix(path);
                if let Some(field) = field {
                    with_field(&mut view, *field);
                }
                pending.attribute(context, destination.place(), view, AccessKind::Write);
                let (reads, writes) = declared_access(value.access);
                pending.attribute_argument(context, value, reads, writes);
            }
            O::EstablishByteSequenceConcatenation { left, right, .. }
            | O::ByteSequenceEqual { left, right, .. } => {
                for operand in [left, right] {
                    match operand {
                        ByteSequenceOperand::Whole { source, .. } => {
                            pending.attribute_argument(context, source, true, false);
                        }
                        ByteSequenceOperand::Field { source, field, .. } => {
                            let mut view = runtime_prefix(&source.path);
                            with_field(&mut view, *field);
                            pending.attribute(context, source.place, view, AccessKind::Read);
                        }
                    }
                }
            }
            O::EstablishStructuralArray { elements, .. } => {
                for argument in elements {
                    let (reads, writes) = declared_access(argument.access);
                    pending.attribute_argument(context, argument, reads, writes);
                }
            }
            abstract_operations::establish_record_or_case_pattern!(fields, ..) => {
                for field in fields {
                    if let Some(argument) = field.value.as_structural() {
                        let (reads, writes) = declared_access(argument.access);
                        pending.attribute_argument(context, argument, reads, writes);
                    }
                }
            }
            O::ReleaseReference { source, path, .. } => {
                pending.attribute(context, *source, runtime_prefix(path), AccessKind::Write);
            }
            O::AtomicEvent { event, .. } => {
                if let Some(location) = event.location() {
                    let view = runtime_prefix(&location.path);
                    let accesses: &[AccessKind] = match event {
                        AbstractAtomicEvent::Load { .. } => &[AccessKind::Read],
                        AbstractAtomicEvent::Store { .. } => &[AccessKind::Write],
                        AbstractAtomicEvent::Fence { .. } => &[],
                        _ => &[AccessKind::Read, AccessKind::Write],
                    };
                    for kind in accesses {
                        pending.attribute(context, location.root, view.clone(), *kind);
                    }
                }
                if let AbstractAtomicEvent::CompareExchangeOnce { outcome, .. } = event {
                    pending.attribute(context, outcome.place, Vec::new(), AccessKind::Write);
                }
            }
            O::ReturnStructural { source, .. } => {
                pending.attribute(context, *source, Vec::new(), AccessKind::Read);
            }
            abstract_operations::jump_or_conditional_pattern!()
            | abstract_operations::return_or_return_unit_pattern!() => {}
            O::CallStructural {
                callee,
                structural_arguments,
                ..
            } => {
                if let Some(machine) = callee.machine() {
                    pending.push_call(context, machines, machine, structural_arguments);
                } else {
                    for argument in structural_arguments {
                        let (reads, writes) = declared_access(argument.access);
                        pending.attribute_argument(context, argument, reads, writes);
                    }
                    pending.unsummarized = true;
                }
            }
            O::InvokeNoNormal {
                callee,
                structural_arguments,
                ..
            } => {
                pending.push_call(context, machines, *callee, structural_arguments);
            }
            O::Call { callee, .. } => {
                pending.push_call(context, machines, *callee, &[]);
            }
            O::BoundaryCall {
                structural_arguments,
                ..
            }
            | O::ExternalCompletion {
                structural_arguments,
                ..
            } => {
                for argument in structural_arguments {
                    let (reads, writes) = declared_access(argument.access);
                    pending.attribute_argument(context, argument, reads, writes);
                }
                pending.unsummarized = true;
            }
            abstract_operations::element_view_or_byte_subslice_pattern!(..)
            | O::EstablishElementView { .. }
            | O::EstablishReference { .. }
            | O::EstablishAccessReference { .. }
            | O::EstablishScalarArray { .. }
            | O::EstablishPrimitiveLocal { .. }
            | abstract_operations::byte_literal_establish_pair_pattern!(..)
            | O::EstablishActivationClaim { .. }
            | O::SplitActivationClaim { .. }
            | O::MergeActivationClaim { .. }
            | O::EstablishNativeCodeLayout { .. }
            | O::EstablishZeroedStructural { .. }
            | O::EstablishStructuralLocal { .. }
            | O::PortWrite { .. }
            | O::Crash { .. }
            | O::SelectImplementation { .. }
            | O::SelectMachine { .. }
            | O::IntegerConstant { .. }
            | O::IeeeFloatConstant { .. }
            | O::NearestIeeeFloatFusedMultiplyAdd { .. }
            | O::IeeeFloatCompare { .. }
            | O::FloatResultPolicy { .. }
            | O::BooleanConstant { .. }
            | abstract_operations::boolean_not_or_equal_pair_pattern!(..)
            | abstract_operations::integer_equal_less_pair_pattern!(..)
            | abstract_operations::bitwise_not_or_less_pair_pattern!(..)
            | abstract_operations::integer_widen_or_exact_cast_pattern!(..)
            | abstract_operations::bitwise_and_or_view_pair_pattern!(..)
            | abstract_operations::bitwise_or_xor_pair_pattern!(..)
            | abstract_operations::integer_dispatch_triple_pattern!(..) => {}
        }
    }

    fn scan_function(
        context: &FunctionContext<'_>,
        machines: &BTreeSet<MachineId>,
        function: &PsiOptimizationFunction,
    ) -> PendingSurface {
        let mut pending = PendingSurface::default();
        for block in function
            .blocks
            .iter()
            .filter(|block| context.reachable.contains(&block.id))
        {
            for node in &block.nodes {
                pending.support.extend(node.provenance.iter().copied());
                pending.support.extend(
                    node.successors
                        .iter()
                        .flat_map(|edge| edge.provenance.iter().copied()),
                );
                scan_operation(context, machines, &mut pending, &node.operation);
            }
        }
        pending
    }

    pub(crate) fn place_access_summaries(unit: &PsiOptimizationUnit) -> PlaceAccessSummaryAnalysis {
        let machines: BTreeSet<MachineId> = unit
            .functions
            .iter()
            .map(|function| function.machine)
            .collect();
        let mut contexts = BTreeMap::new();
        let mut pending = BTreeMap::<MachineId, PendingSurface>::new();
        for function in &unit.functions {
            contexts.insert(function.machine, FunctionContext::build(function));
        }
        for function in &unit.functions {
            pending.insert(
                function.machine,
                scan_function(&contexts[&function.machine], &machines, function),
            );
        }

        loop {
            let prior = pending.clone();
            let mut changed = false;
            for (machine, summary) in pending.iter_mut() {
                let context = &contexts[machine];
                for contribution in summary.contributions.clone() {
                    match contribution {
                        PendingContribution::Call { callee, arguments } => {
                            let Some(callee_summary) = prior.get(&callee) else {
                                continue;
                            };
                            let touched: BTreeSet<u32> = callee_summary
                                .parameter_reads
                                .keys()
                                .chain(callee_summary.parameter_writes.keys())
                                .copied()
                                .collect();
                            for position in touched {
                                let index =
                                    usize::try_from(position).expect("parameter position is usize");
                                let Some(argument) = arguments.get(index) else {
                                    continue;
                                };
                                let path = runtime_prefix(&argument.path);
                                if callee_summary
                                    .parameter_reads
                                    .get(&position)
                                    .is_some_and(|reads| !reads.is_empty())
                                {
                                    summary.attribute(
                                        context,
                                        argument.place,
                                        path.clone(),
                                        AccessKind::Read,
                                    );
                                }
                                if callee_summary
                                    .parameter_writes
                                    .get(&position)
                                    .is_some_and(|writes| !writes.is_empty())
                                {
                                    summary.attribute(
                                        context,
                                        argument.place,
                                        path,
                                        AccessKind::Write,
                                    );
                                }
                            }
                            let parameter_count = contexts
                                .get(&callee)
                                .map(|callee_context| {
                                    callee_context.function.structural_parameters.len()
                                })
                                .unwrap_or_default();
                            for argument in arguments.iter().skip(parameter_count) {
                                let (reads, writes) = declared_access(argument.access);
                                summary.attribute_argument(context, argument, reads, writes);
                            }
                            summary
                                .external_reads
                                .extend(callee_summary.external_reads.iter().cloned());
                            summary
                                .external_writes
                                .extend(callee_summary.external_writes.iter().cloned());
                            summary.unsummarized |= callee_summary.unsummarized;
                        }
                        PendingContribution::Reach { callee } => {
                            let Some(callee_summary) = prior.get(&callee) else {
                                continue;
                            };
                            summary
                                .external_reads
                                .extend(callee_summary.external_reads.iter().cloned());
                            summary
                                .external_writes
                                .extend(callee_summary.external_writes.iter().cloned());
                            summary.unsummarized |= callee_summary.unsummarized;
                        }
                    }
                }
                changed |= *summary != *prior.get(machine).expect("pending row persists");
            }
            if !changed {
                break;
            }
        }

        let mut functions = Vec::with_capacity(pending.len());
        for (machine, summary) in pending {
            let context = &contexts[&machine];
            let mut parameters = Vec::with_capacity(context.function.structural_parameters.len());
            for parameter in &context.function.structural_parameters {
                parameters.push(ParameterAccessSurface {
                    position: parameter.position,
                    place: parameter.place,
                    declared: parameter.access,
                    reads: summary
                        .parameter_reads
                        .get(&parameter.position)
                        .map(|paths| paths.iter().cloned().collect())
                        .unwrap_or_default(),
                    writes: summary
                        .parameter_writes
                        .get(&parameter.position)
                        .map(|paths| paths.iter().cloned().collect())
                        .unwrap_or_default(),
                });
            }
            parameters.sort_by_key(|surface| surface.position);
            let result_sources = context
                .function
                .result
                .structural()
                .map(|result| {
                    result
                        .reference_sources
                        .iter()
                        .map(|source| ResolvedResultSource {
                            result_path: source.path.clone(),
                            origins: source
                                .sources
                                .iter()
                                .flat_map(|candidate| {
                                    let StructuralReferenceOrigin::Parameter(candidate) = candidate
                                    else {
                                        return vec![ResolvedAccess::External {
                                            root: result.place,
                                            path: Vec::new(),
                                        }];
                                    };
                                    context.resolve(
                                        candidate.place,
                                        candidate
                                            .path
                                            .iter()
                                            .map(|projection| match projection {
                                                ReferenceOriginProjection::Exact(segment) => {
                                                    PlaceAccessStep::Runtime(segment.clone())
                                                }
                                                ReferenceOriginProjection::Element => {
                                                    PlaceAccessStep::Element
                                                }
                                            })
                                            .collect(),
                                    )
                                })
                                .map(|resolved| match resolved {
                                    ResolvedAccess::Parameter { position, path } => {
                                        ResultSourceResolution::Parameter { position, path }
                                    }
                                    ResolvedAccess::External { root, path } => {
                                        ResultSourceResolution::External { root, path }
                                    }
                                    ResolvedAccess::Internal => ResultSourceResolution::Internal,
                                })
                                .fold(Vec::new(), |mut origins, origin| {
                                    if !origins.contains(&origin) {
                                        origins.push(origin);
                                    }
                                    origins
                                }),
                        })
                        .collect()
                })
                .unwrap_or_default();
            functions.push(FunctionPlaceAccessSummary {
                machine,
                parameters,
                external_reads: summary.external_reads.into_iter().collect(),
                external_writes: summary.external_writes.into_iter().collect(),
                result_sources,
                unsummarized_targets: summary.unsummarized,
                support: summary.support.into_iter().collect(),
                revision: unit.identity,
            });
        }
        functions.sort_by_key(|function| function.machine);
        PlaceAccessSummaryAnalysis { functions }
    }
}
pub(crate) mod place_aliases {
    use std::collections::{BTreeMap, BTreeSet};

    use optimization_unit::{OwnershipFrontierSite, PsiOptimizationUnit};
    use semantic_vocabulary::{MachineId, PlaceId, StructuralPlaceKind};
    use terminal_psi::StructuralPathSegment;

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub(crate) struct PlaceView {
        pub root: PlaceId,
        pub path: Vec<StructuralPathSegment>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct PlaceAliasClaim {
        pub view: PlaceView,
        pub sites: Vec<OwnershipFrontierSite>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct PlaceAliasRoot {
        pub place: PlaceId,
        pub kind: Option<StructuralPlaceKind>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct PlaceAliasFunction {
        pub machine: MachineId,
        pub roots: Vec<PlaceAliasRoot>,
        pub claims: Vec<PlaceAliasClaim>,
        pub unrooted_claims: u64,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct PlaceAliasesAnalysis {
        pub functions: Vec<PlaceAliasFunction>,
    }

    impl PlaceAliasesAnalysis {}

    impl PlaceAliasFunction {}

    pub(crate) fn place_aliases(unit: &PsiOptimizationUnit) -> PlaceAliasesAnalysis {
        let mut claim_views: BTreeMap<
            MachineId,
            BTreeMap<PlaceView, BTreeSet<OwnershipFrontierSite>>,
        > = BTreeMap::new();
        let mut unrooted_claims: BTreeMap<MachineId, u64> = BTreeMap::new();
        for fact in &unit.ownership_frontier_facts {
            for claim in &fact.snapshot.claims {
                match claim.input {
                    Some(root) => {
                        claim_views
                            .entry(fact.machine)
                            .or_default()
                            .entry(PlaceView {
                                root,
                                path: claim.path.clone(),
                            })
                            .or_default()
                            .insert(fact.site);
                    }
                    None => *unrooted_claims.entry(fact.machine).or_default() += 1,
                }
            }
        }
        let mut functions = Vec::with_capacity(unit.functions.len());
        for function in &unit.functions {
            let mut roots: BTreeMap<PlaceId, Option<StructuralPlaceKind>> = function
                .declared_places
                .iter()
                .map(|place| (*place, None))
                .collect();
            for declaration in &function.structural_places {
                roots.insert(declaration.id, Some(declaration.kind));
            }
            let claims = claim_views
                .remove(&function.machine)
                .map(|views| {
                    views
                        .into_iter()
                        .map(|(view, sites)| PlaceAliasClaim {
                            view,
                            sites: sites.into_iter().collect(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            functions.push(PlaceAliasFunction {
                machine: function.machine,
                roots: roots
                    .into_iter()
                    .map(|(place, kind)| PlaceAliasRoot { place, kind })
                    .collect(),
                claims,
                unrooted_claims: unrooted_claims.remove(&function.machine).unwrap_or(0),
            });
        }
        functions.sort_by_key(|function| function.machine);
        PlaceAliasesAnalysis { functions }
    }
}
pub(crate) mod sparse_conditional_constants {
    use std::collections::{BTreeMap, BTreeSet};

    use abstract_operations::AbstractOperation as O;
    use optimization_core::{OptimizationUnitIdentity, ScalarConstantFactIdentity};
    use optimization_unit::{
        OptimizationFact, PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance,
        ScalarConstantValue, SccpBlockRow, SccpEdgeRow, SccpEdgeState, SccpMachineSnapshot,
        SccpValueRow, SccpValueState, derived_sccp_scalar_constant_fact_identity,
        literal_scalar_constant_fact_identity,
    };
    use semantic_vocabulary::{BlockId, EdgeId, IntegerValue, MachineId, OperationId, ValueId};

    use super::scalar_value_definition;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub(crate) struct ValueFactRegion {
        pub revision: OptimizationUnitIdentity,
        pub machine: MachineId,
        pub value: ValueId,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub(crate) struct ScalarConstantFact {
        pub value: ValueId,
        pub constant: ScalarConstantValue,
        pub identity: Option<ScalarConstantFactIdentity>,
        pub support: ScalarConstantSupport,
        pub valid_in: ValueFactRegion,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub(crate) struct ScalarConstantSupport {
        pub operations: Vec<OperationId>,
        pub edges: Vec<EdgeId>,
    }

    impl ScalarConstantSupport {
        fn literal(operation: OperationId) -> Self {
            Self {
                operations: vec![operation],
                edges: Vec::new(),
            }
        }

        pub fn literal_operation(&self) -> Option<OperationId> {
            let [operation] = self.operations.as_slice() else {
                return None;
            };
            self.edges.is_empty().then_some(*operation)
        }

        fn through_edge(mut self, edge: EdgeId) -> Self {
            if let Err(position) = self.edges.binary_search(&edge) {
                self.edges.insert(position, edge);
            }
            self
        }

        fn union_with(&mut self, other: &Self) {
            self.operations.extend_from_slice(&other.operations);
            self.operations.sort_unstable();
            self.operations.dedup();
            self.edges.extend_from_slice(&other.edges);
            self.edges.sort_unstable();
            self.edges.dedup();
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ScalarConstantAnalysis {
        pub facts: Vec<ScalarConstantFact>,
    }

    impl ScalarConstantAnalysis {
        pub(crate) fn literal_integer_constant(
            &self,
            machine: MachineId,
            value: ValueId,
        ) -> Option<(IntegerValue, ScalarConstantFactIdentity)> {
            self.constant_fact(machine, value, true, |constant| match constant {
                ScalarConstantValue::Integer(value) => Some(*value),
                ScalarConstantValue::Boolean(_) => None,
            })
        }

        pub(crate) fn boolean_constant(
            &self,
            machine: MachineId,
            value: ValueId,
        ) -> Option<(bool, ScalarConstantFactIdentity)> {
            self.constant_fact(machine, value, false, |constant| match constant {
                ScalarConstantValue::Boolean(value) => Some(*value),
                ScalarConstantValue::Integer(_) => None,
            })
        }

        fn constant_fact<R>(
            &self,
            machine: MachineId,
            value: ValueId,
            literal_only: bool,
            extract: impl Fn(&ScalarConstantValue) -> Option<R>,
        ) -> Option<(R, ScalarConstantFactIdentity)> {
            self.facts.iter().find_map(|fact| {
                (fact.valid_in.machine == machine
                    && fact.value == value
                    && (!literal_only || fact.support.literal_operation().is_some()))
                .then_some(fact)
                .and_then(|fact| extract(&fact.constant).zip(fact.identity))
            })
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub(crate) enum ExecutableEdgeKnowledge {
        KnownExecutable,
        KnownInexecutable,
        Unknown,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ExecutableEdgeFact {
        pub machine: MachineId,
        pub source: BlockId,
        pub edge: EdgeId,
        pub knowledge: ExecutableEdgeKnowledge,
        pub support: ScalarConstantSupport,
        pub revision: OptimizationUnitIdentity,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ExecutableEdgeAnalysis {
        pub edges: Vec<ExecutableEdgeFact>,
    }

    pub(crate) fn scalar_constants(unit: &PsiOptimizationUnit) -> ScalarConstantAnalysis {
        sparse_conditional_constants(unit).0
    }

    pub(crate) fn executable_edges(unit: &PsiOptimizationUnit) -> ExecutableEdgeAnalysis {
        sparse_conditional_constants(unit).1
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum LatticeValue {
        Unknown,
        Constant(ScalarConstantValue, ScalarConstantSupport),
        Overdefined,
    }

    fn sparse_conditional_constants(
        unit: &PsiOptimizationUnit,
    ) -> (ScalarConstantAnalysis, ExecutableEdgeAnalysis) {
        fn merge(
            target: &mut LatticeValue,
            incoming: &LatticeValue,
            path_support: &ScalarConstantSupport,
        ) -> bool {
            let incoming = match incoming {
                LatticeValue::Unknown => return false,
                LatticeValue::Overdefined => LatticeValue::Overdefined,
                LatticeValue::Constant(constant, support) => {
                    let mut support = support.clone();
                    support.union_with(path_support);
                    LatticeValue::Constant(*constant, support)
                }
            };
            let next = match (&*target, incoming) {
                (LatticeValue::Unknown, incoming) => incoming,
                (_, LatticeValue::Unknown) | (LatticeValue::Overdefined, _) => return false,
                (_, LatticeValue::Overdefined) => LatticeValue::Overdefined,
                (
                    LatticeValue::Constant(current, current_support),
                    LatticeValue::Constant(incoming, incoming_support),
                ) if *current == incoming => {
                    let mut support = current_support.clone();
                    support.union_with(&incoming_support);
                    LatticeValue::Constant(*current, support)
                }
                (LatticeValue::Constant(..), LatticeValue::Constant(..)) => {
                    LatticeValue::Overdefined
                }
            };
            if *target == next {
                false
            } else {
                *target = next;
                true
            }
        }

        let mut facts = Vec::new();
        let mut edge_facts = Vec::new();
        for function in &unit.functions {
            let mut values = BTreeMap::<ValueId, LatticeValue>::new();
            let support_blocks = function
                .blocks
                .iter()
                .flat_map(|block| {
                    block.nodes.iter().flat_map(move |node| {
                        node.provenance
                            .iter()
                            .filter_map(move |source| match source {
                                PsiProvenance::Operation(operation) => Some((*operation, block.id)),
                                PsiProvenance::Edge(_) => None,
                            })
                    })
                })
                .collect::<BTreeMap<_, _>>();
            for parameter in &function.parameters {
                values.insert(parameter.value, LatticeValue::Overdefined);
            }
            for block in &function.blocks {
                for parameter in &block.parameters {
                    values.insert(parameter.value, LatticeValue::Unknown);
                }
                for definition in block.definitions() {
                    values.insert(definition.value, LatticeValue::Overdefined);
                }
            }
            let mut literal_rows = Vec::new();
            for fact in &function.facts {
                let (value, constant, support) = match fact {
                    OptimizationFact::BooleanConstant {
                        value,
                        constant,
                        support,
                    } => (
                        *value,
                        ScalarConstantValue::Boolean(*constant),
                        ScalarConstantSupport::literal(*support),
                    ),
                    OptimizationFact::IntegerConstant {
                        value,
                        constant,
                        support,
                    } => (
                        *value,
                        ScalarConstantValue::Integer(*constant),
                        ScalarConstantSupport::literal(*support),
                    ),
                    OptimizationFact::OperationObligationReference { .. } => continue,
                };
                let block = support_blocks.get(&support.operations[0]).copied();
                literal_rows.push((value, constant, support.clone(), block));
                values.insert(
                    value,
                    if block.is_some() {
                        LatticeValue::Unknown
                    } else {
                        LatticeValue::Constant(constant, support)
                    },
                );
            }

            let mut reachable = BTreeSet::from([function.entry]);
            let mut feasible_edges = BTreeMap::<EdgeId, ScalarConstantSupport>::new();
            let mut reach_support = BTreeMap::from([(
                function.entry,
                ScalarConstantSupport {
                    operations: Vec::new(),
                    edges: Vec::new(),
                },
            )]);
            loop {
                let mut changed = false;
                for block in &function.blocks {
                    if !reachable.contains(&block.id) {
                        continue;
                    }
                    for (value, constant, support, site) in &literal_rows {
                        if *site == Some(block.id)
                            && matches!(values.get(value), Some(LatticeValue::Unknown))
                        {
                            values
                                .insert(*value, LatticeValue::Constant(*constant, support.clone()));
                            changed = true;
                        }
                    }
                    let Some(node) = block.nodes.last() else {
                        continue;
                    };
                    let operation_successors = &node.successors;
                    let successors = match &node.operation {
                        O::Jump { .. } | O::StructuralCase { .. } => operation_successors
                            .iter()
                            .map(|successor| (successor, None))
                            .collect::<Vec<_>>(),
                        O::Conditional { condition, .. } => match values.get(condition) {
                            Some(LatticeValue::Constant(
                                ScalarConstantValue::Boolean(value),
                                condition_support,
                            )) => operation_successors
                                .iter()
                                .filter(|successor| {
                                    matches!(
                                        &node.operation,
                                        O::Conditional {
                                            when_true,
                                            when_false,
                                            ..
                                        } if successor.psi_edge
                                            == if *value {
                                                when_true.psi_edge
                                            } else {
                                                when_false.psi_edge
                                            }
                                    )
                                })
                                .map(|successor| (successor, Some(condition_support.clone())))
                                .collect(),
                            Some(LatticeValue::Overdefined) => operation_successors
                                .iter()
                                .map(|successor| (successor, None))
                                .collect::<Vec<_>>(),
                            Some(LatticeValue::Constant(ScalarConstantValue::Integer(_), _))
                            | Some(LatticeValue::Unknown)
                            | None => Vec::new(),
                        },
                        _ => Vec::new(),
                    };
                    for (successor, condition_support) in successors {
                        let mut path_support = reach_support
                            .get(&block.id)
                            .cloned()
                            .expect("reachable block has support");
                        if let Some(condition_support) = condition_support {
                            path_support.union_with(&condition_support);
                        }
                        path_support = path_support.through_edge(successor.psi_edge);
                        if let O::StructuralCase { cases, .. } = &node.operation {
                            for payload in cases
                                .iter()
                                .filter(|case| case.psi_edge == successor.psi_edge)
                                .flat_map(|case| &case.payloads)
                            {
                                let target = values
                                    .entry(payload.parameter)
                                    .or_insert(LatticeValue::Unknown);
                                changed |= merge(target, &LatticeValue::Overdefined, &path_support);
                            }
                        }
                        changed |=
                            join_support(&mut feasible_edges, successor.psi_edge, &path_support);
                        changed |= reachable.insert(successor.target);
                        changed |=
                            join_support(&mut reach_support, successor.target, &path_support);
                        for binding in &successor.bindings {
                            let incoming = values
                                .get(&binding.argument)
                                .cloned()
                                .unwrap_or(LatticeValue::Overdefined);
                            let target = values
                                .entry(binding.parameter)
                                .or_insert(LatticeValue::Unknown);
                            changed |= merge(target, &incoming, &path_support);
                        }
                    }
                }
                if !changed {
                    break;
                }
            }

            for block in &function.blocks {
                let source_reachable = reachable.contains(&block.id);
                let source_support = reach_support.get(&block.id);
                let Some(node) = block.nodes.last() else {
                    continue;
                };
                for successor in &node.successors {
                    let feasible_support = feasible_edges.get(&successor.psi_edge);
                    let (knowledge, support) = if let Some(support) = feasible_support {
                        (ExecutableEdgeKnowledge::KnownExecutable, support.clone())
                    } else if !source_reachable {
                        (
                            ExecutableEdgeKnowledge::KnownInexecutable,
                            ScalarConstantSupport {
                                operations: Vec::new(),
                                edges: Vec::new(),
                            },
                        )
                    } else if let O::Conditional { condition, .. } = &node.operation {
                        match values.get(condition) {
                            Some(LatticeValue::Constant(
                                ScalarConstantValue::Boolean(_),
                                condition,
                            )) => {
                                let mut support = source_support.cloned().unwrap_or_else(|| {
                                    ScalarConstantSupport {
                                        operations: Vec::new(),
                                        edges: Vec::new(),
                                    }
                                });
                                support.union_with(condition);
                                (ExecutableEdgeKnowledge::KnownInexecutable, support)
                            }
                            _ => (
                                ExecutableEdgeKnowledge::Unknown,
                                ScalarConstantSupport {
                                    operations: Vec::new(),
                                    edges: Vec::new(),
                                },
                            ),
                        }
                    } else {
                        (
                            ExecutableEdgeKnowledge::KnownInexecutable,
                            ScalarConstantSupport {
                                operations: Vec::new(),
                                edges: Vec::new(),
                            },
                        )
                    };
                    edge_facts.push(ExecutableEdgeFact {
                        machine: function.machine,
                        source: block.id,
                        edge: successor.psi_edge,
                        knowledge,
                        support,
                        revision: unit.identity,
                    });
                }
            }

            let snapshot = sccp_machine_snapshot(function, &values, &reachable, &feasible_edges);

            facts.extend(values.into_iter().filter_map(|(value, state)| {
                let LatticeValue::Constant(constant, support) = state else {
                    return None;
                };
                let definition = scalar_value_definition(function, value);
                let constant_value = constant;
                let identity = definition.and_then(|definition| {
                    support
                        .literal_operation()
                        .and_then(|operation| {
                            literal_scalar_constant_fact_identity(
                                unit.identity,
                                function.machine,
                                definition,
                                constant_value,
                                operation,
                            )
                        })
                        .or_else(|| {
                            derived_sccp_scalar_constant_fact_identity(
                                unit.identity,
                                function.machine,
                                definition,
                                constant_value,
                                &snapshot,
                            )
                        })
                });
                Some(ScalarConstantFact {
                    value,
                    constant,
                    identity,
                    support,
                    valid_in: ValueFactRegion {
                        revision: unit.identity,
                        machine: function.machine,
                        value,
                    },
                })
            }));
        }
        facts.sort_by_key(|fact| (fact.valid_in.machine, fact.value));
        (
            ScalarConstantAnalysis { facts },
            ExecutableEdgeAnalysis { edges: edge_facts },
        )
    }

    fn sccp_machine_snapshot(
        function: &PsiOptimizationFunction,
        values: &BTreeMap<ValueId, LatticeValue>,
        reachable: &BTreeSet<BlockId>,
        feasible_edges: &BTreeMap<EdgeId, ScalarConstantSupport>,
    ) -> SccpMachineSnapshot {
        let mut blocks = function
            .blocks
            .iter()
            .map(|block| SccpBlockRow {
                block: block.id,
                executable: reachable.contains(&block.id),
            })
            .collect::<Vec<_>>();
        blocks.sort_by_key(|row| row.block);

        let mut edges = function
            .blocks
            .iter()
            .flat_map(|block| {
                let reachable_source = reachable.contains(&block.id);
                let terminator = block.nodes.last();
                terminator.into_iter().flat_map(move |node| {
                    node.successors.iter().map(move |successor| {
                        let state = if feasible_edges.contains_key(&successor.psi_edge) {
                            SccpEdgeState::Executable
                        } else if !reachable_source {
                            SccpEdgeState::Inexecutable
                        } else if let O::Conditional { condition, .. } = &node.operation {
                            match values.get(condition) {
                                Some(LatticeValue::Constant(
                                    ScalarConstantValue::Boolean(_),
                                    _,
                                )) => SccpEdgeState::Inexecutable,
                                _ => SccpEdgeState::Unknown,
                            }
                        } else {
                            SccpEdgeState::Inexecutable
                        };
                        SccpEdgeRow {
                            source: block.id,
                            edge: successor.psi_edge,
                            target: successor.target,
                            state,
                        }
                    })
                })
            })
            .collect::<Vec<_>>();
        edges.sort_by_key(|row| (row.source, row.edge));

        let mut snapshot_values = values
            .iter()
            .filter_map(|(value, state)| {
                let definition = scalar_value_definition(function, *value)?;
                Some(SccpValueRow {
                    definition,
                    state: match state {
                        LatticeValue::Unknown => SccpValueState::Unknown,
                        LatticeValue::Constant(ScalarConstantValue::Boolean(value), _) => {
                            SccpValueState::Boolean(*value)
                        }
                        LatticeValue::Constant(ScalarConstantValue::Integer(value), _) => {
                            SccpValueState::Integer(*value)
                        }
                        LatticeValue::Overdefined => SccpValueState::Overdefined,
                    },
                })
            })
            .collect::<Vec<_>>();
        snapshot_values.sort_by_key(|row| row.definition.value);
        SccpMachineSnapshot {
            blocks,
            edges,
            values: snapshot_values,
        }
    }

    fn join_support<K: Ord>(
        map: &mut BTreeMap<K, ScalarConstantSupport>,
        key: K,
        support: &ScalarConstantSupport,
    ) -> bool {
        match map.entry(key) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(support.clone());
                true
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let mut joined = entry.get().clone();
                joined.union_with(support);
                if joined != *entry.get() {
                    entry.insert(joined);
                    true
                } else {
                    false
                }
            }
        }
    }
}
pub(crate) mod use_definitions {
    use optimization_unit::{PsiOptimizationUnit, ValueDefinition, ValueDefinitionSite, ValueUse};
    use semantic_vocabulary::{BlockId, MachineId, ValueId};

    use crate::analyses::block_dominates;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct UseDefinitionAnalysis {
        pub definitions: Vec<(MachineId, ValueDefinition)>,
        pub uses: Vec<(MachineId, ValueUse)>,
    }

    impl UseDefinitionAnalysis {
        pub(crate) fn replacement_dominates_parameter_uses(
            &self,
            machine: MachineId,
            replacement: ValueId,
            parameter: ValueId,
            dominators: &[(BlockId, Vec<BlockId>)],
        ) -> bool {
            let Some((_, definition)) = self
                .definitions
                .iter()
                .find(|(owner, definition)| *owner == machine && definition.value == replacement)
            else {
                return false;
            };
            self.uses
                .iter()
                .filter(|(owner, use_site)| *owner == machine && use_site.value == parameter)
                .all(|(_, use_site)| match definition.site {
                    ValueDefinitionSite::FunctionParameter(_) => true,
                    ValueDefinitionSite::BlockParameter {
                        block: defining, ..
                    } => block_dominates(dominators, defining, use_site.block),
                    ValueDefinitionSite::Node {
                        block: defining,
                        node,
                    } if defining == use_site.block => node < use_site.node,
                    ValueDefinitionSite::Node {
                        block: defining, ..
                    } => block_dominates(dominators, defining, use_site.block),
                })
        }
    }

    pub(crate) fn use_definitions(unit: &PsiOptimizationUnit) -> UseDefinitionAnalysis {
        let mut definitions = Vec::new();
        let mut uses = Vec::new();
        for function in &unit.functions {
            definitions.extend(
                function
                    .parameters
                    .iter()
                    .chain(
                        function
                            .blocks
                            .iter()
                            .flat_map(|block| block.parameters.iter().chain(block.definitions())),
                    )
                    .copied()
                    .map(|definition| (function.machine, definition)),
            );
            uses.extend(
                function
                    .node_uses()
                    .copied()
                    .map(|use_site| (function.machine, use_site)),
            );
        }
        UseDefinitionAnalysis { definitions, uses }
    }
}
pub(crate) mod value_liveness {
    use std::collections::{BTreeMap, BTreeSet};

    use abstract_operations::AbstractOperation as O;
    use optimization_unit::PsiOptimizationUnit;
    use semantic_vocabulary::{BlockId, MachineId, ValueId};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct NodeLiveness {
        pub node: u32,
        pub entry: Vec<ValueId>,
        pub exit: Vec<ValueId>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ValueLivenessBlock {
        pub machine: MachineId,
        pub block: BlockId,
        pub entry: Vec<ValueId>,
        pub exit: Vec<ValueId>,
        pub nodes: Vec<NodeLiveness>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ValueLivenessAnalysis {
        pub blocks: Vec<ValueLivenessBlock>,
    }

    pub(crate) fn value_liveness(unit: &PsiOptimizationUnit) -> ValueLivenessAnalysis {
        let mut output = Vec::new();
        for function in &unit.functions {
            let successors = function
                .blocks
                .iter()
                .map(|block| {
                    let targets = block
                        .nodes
                        .last()
                        .map(|node| match &node.operation {
                            O::Jump { target, .. } => vec![*target],
                            O::Conditional {
                                when_true,
                                when_false,
                                ..
                            } => vec![when_true.target, when_false.target],
                            _ => Vec::new(),
                        })
                        .unwrap_or_default();
                    (block.id, targets)
                })
                .collect::<BTreeMap<_, _>>();
            let mut live_entry = function
                .blocks
                .iter()
                .map(|block| (block.id, BTreeSet::new()))
                .collect::<BTreeMap<_, _>>();
            let mut live_exit = live_entry.clone();
            loop {
                let mut changed = false;
                for block in function.blocks.iter().rev() {
                    let next_exit = successors[&block.id]
                        .iter()
                        .filter_map(|successor| live_entry.get(successor))
                        .flat_map(|set| set.iter().copied())
                        .collect::<BTreeSet<_>>();
                    let mut next_entry = next_exit.clone();
                    for node in block.nodes.iter().rev() {
                        for definition in &node.definitions {
                            next_entry.remove(&definition.value);
                        }
                        next_entry.extend(node.uses.iter().map(|use_site| use_site.value));
                    }
                    for parameter in &block.parameters {
                        next_entry.remove(&parameter.value);
                    }
                    if live_exit[&block.id] != next_exit {
                        live_exit.insert(block.id, next_exit);
                        changed = true;
                    }
                    if live_entry[&block.id] != next_entry {
                        live_entry.insert(block.id, next_entry);
                        changed = true;
                    }
                }
                if !changed {
                    break;
                }
            }
            for block in &function.blocks {
                let mut live = live_exit[&block.id].clone();
                let mut rows = Vec::with_capacity(block.nodes.len());
                for (node_index, node) in block.nodes.iter().enumerate().rev() {
                    let exit = live.iter().copied().collect();
                    for definition in &node.definitions {
                        live.remove(&definition.value);
                    }
                    live.extend(node.uses.iter().map(|use_site| use_site.value));
                    rows.push(NodeLiveness {
                        node: u32::try_from(node_index).expect("optimization node index is u32"),
                        entry: live.iter().copied().collect(),
                        exit,
                    });
                }
                rows.reverse();
                output.push(ValueLivenessBlock {
                    machine: function.machine,
                    block: block.id,
                    entry: live_entry[&block.id].iter().copied().collect(),
                    exit: live_exit[&block.id].iter().copied().collect(),
                    nodes: rows,
                });
            }
        }
        ValueLivenessAnalysis { blocks: output }
    }
}
pub(crate) use crate::value_ranges;

pub(crate) use effect_summaries::{EffectClass, EffectKnowledge, EffectSummaryAnalysis};

pub(crate) use ownership_frontiers::OwnershipFrontierAnalysis;

pub(crate) use place_access_summaries::PlaceAccessSummaryAnalysis;

pub(crate) use place_aliases::PlaceAliasesAnalysis;
pub(crate) use sparse_conditional_constants::{ExecutableEdgeAnalysis, ScalarConstantAnalysis};

pub(crate) use use_definitions::UseDefinitionAnalysis;
pub(crate) use value_liveness::ValueLivenessAnalysis;

pub(crate) use value_ranges::ValueRangeAnalysis;

pub(crate) use effect_summaries::effect_summaries;
pub(crate) use ownership_frontiers::ownership_frontiers;
pub(crate) use place_access_summaries::place_access_summaries;
pub(crate) use place_aliases::place_aliases;
pub(crate) use sparse_conditional_constants::{executable_edges, scalar_constants};
pub(crate) use use_definitions::use_definitions;
pub(crate) use value_liveness::value_liveness;
pub(crate) use value_ranges::value_ranges;

use std::collections::{BTreeMap, BTreeSet};

pub(crate) use optimization_unit::{PsiOptimizationFunction, scalar_value_definition};
use semantic_vocabulary::BlockId;

pub(crate) fn semantically_reachable_blocks(
    function: &PsiOptimizationFunction,
) -> BTreeSet<BlockId> {
    let successors = function
        .blocks
        .iter()
        .map(|block| {
            (
                block.id,
                block
                    .nodes
                    .last()
                    .map(|node| node.successors.iter().map(|edge| edge.target).collect())
                    .unwrap_or_default(),
            )
        })
        .collect::<BTreeMap<BlockId, Vec<BlockId>>>();
    let mut reachable = BTreeSet::new();
    let mut pending = vec![function.entry];
    while let Some(block) = pending.pop() {
        if reachable.insert(block) {
            pending.extend(successors.get(&block).into_iter().flatten().copied());
        }
    }
    reachable
}
