/*
Validated countdown-loop induction summaries (`countdown_induction`),
certificate-owned countdown-invariant integer constants
(`countdown_invariant_constants`), and preheader constant-placement analyses
(`countdown_invariant_constant_placement`). Every analysis pairs direct
proposal construction with independent replay and exact candidate comparison
against authenticated Terminal-cycle custody.
*/

pub(crate) use crate::control_flow_analyses::*;

pub(crate) mod countdown_induction {
    use abstract_operations::AbstractOperation as O;
    use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};

    use crate::validation::ValidatedOptimizerCycleComponents;
    use optimization_unit::{
        CycleComponentEdge, OptimizerCycleComponent, OptimizerUnsignedCountdownRankingCertificate,
    };
    use semantic_vocabulary::{IntegerType, MachineId, ScalarType, ValueId};

    use super::{AnalysisRoot, AnalysisRootError, LoopRegion, by_key, validate_analysis_root};

    pub(crate) mod compute {
        use super::{
            AnalysisRoot, CountedLoopAnalysisError, CountedLoopAnalysisSnapshot,
            ExactUnsignedTripCount, LoopRegion, O, OptimizerCycleComponent,
            OptimizerUnsignedCountdownRankingCertificate, PsiOptimizationFunction,
            PsiOptimizationUnit, ScalarType, UnsignedCountdownLoopSummary,
            ValidatedOptimizerCycleComponents, region, shape, validate_analysis_root,
        };
        pub(crate) fn propose(
            unit: &PsiOptimizationUnit,
            custody: &ValidatedOptimizerCycleComponents,
        ) -> Result<CountedLoopAnalysisSnapshot, CountedLoopAnalysisError> {
            validate_roots(unit, custody)?;
            let mut loops = Vec::new();
            for certificate in custody.ranking_certificates().certificates() {
                let component = custody
                    .components()
                    .iter()
                    .find(|component| component.id == certificate.component)
                    .ok_or(CountedLoopAnalysisError::CertificateComponentRosterMismatch)?;
                let function = unit
                    .function(certificate.component.machine)
                    .ok_or_else(|| shape(certificate.component.machine))?;
                let region = region::derive(function.machine, component, certificate)?;
                loops.push(summary(function, component, certificate, &region)?);
            }
            loops.sort_by(|left, right| {
                left.certificate.component.cmp(&right.certificate.component)
            });
            Ok(CountedLoopAnalysisSnapshot {
                root: AnalysisRoot::of(unit),
                loops,
            })
        }

        fn summary(
            function: &PsiOptimizationFunction,
            component: &OptimizerCycleComponent,
            certificate: &OptimizerUnsignedCountdownRankingCertificate,
            region: &LoopRegion,
        ) -> Result<UnsignedCountdownLoopSummary, CountedLoopAnalysisError> {
            let [preheader_edge] = component.entries.as_slice() else {
                return Err(shape(function.machine));
            };
            let [exit_edge] = component.exits.as_slice() else {
                return Err(shape(function.machine));
            };
            let source = function
                .block(preheader_edge.source)
                .and_then(|block| block.nodes.last())
                .ok_or_else(|| shape(function.machine))?;
            let O::Jump {
                psi_edge,
                target,
                bindings,
                ..
            } = &source.operation
            else {
                return Err(shape(function.machine));
            };
            let binding = bindings
                .iter()
                .find(|binding| binding.parameter == certificate.rank_parameter)
                .filter(|binding| binding.scalar_type == ScalarType::Integer(certificate.rank_type))
                .ok_or_else(|| shape(function.machine))?;
            if *psi_edge != preheader_edge.edge
                || *target != certificate.header
                || preheader_edge.target != certificate.header
            {
                return Err(shape(function.machine));
            }
            Ok(UnsignedCountdownLoopSummary {
                certificate: certificate.clone(),
                region: (*region).clone(),
                preheader_edge: *preheader_edge,
                exit_edge: *exit_edge,
                trip_count: ExactUnsignedTripCount {
                    initial_value: binding.argument,
                    scalar_type: certificate.rank_type,
                },
            })
        }

        fn validate_roots(
            unit: &PsiOptimizationUnit,
            custody: &ValidatedOptimizerCycleComponents,
        ) -> Result<(), CountedLoopAnalysisError> {
            validate_analysis_root(unit, custody, &[]).map_err(CountedLoopAnalysisError::Root)?;
            if custody
                .ranking_certificates()
                .certificates()
                .iter()
                .any(|certificate| {
                    !custody
                        .components()
                        .iter()
                        .any(|component| component.id == certificate.component)
                })
            {
                return Err(CountedLoopAnalysisError::CertificateComponentRosterMismatch);
            }
            Ok(())
        }
    }
    pub(crate) mod region {
        use super::{
            CountedLoopAnalysisError, LoopRegion, MachineId, OptimizerCycleComponent,
            OptimizerUnsignedCountdownRankingCertificate, shape,
        };

        pub(crate) fn derive(
            machine: MachineId,
            component: &OptimizerCycleComponent,
            certificate: &OptimizerUnsignedCountdownRankingCertificate,
        ) -> Result<LoopRegion, CountedLoopAnalysisError> {
            let [entry] = component.entries.as_slice() else {
                return Err(shape(machine));
            };
            if component.members.is_empty()
                || !component.members.windows(2).all(|pair| pair[0] < pair[1])
                || !component.members.contains(&certificate.header)
                || entry.target != certificate.header
            {
                return Err(shape(machine));
            }
            Ok(LoopRegion {
                header: Some(certificate.header),
                blocks: component.members.clone(),
                irreducible: false,
            })
        }
    }
    pub(crate) mod replay {
        use super::{
            AnalysisRoot, CountedLoopAnalysisError, CountedLoopAnalysisSnapshot,
            CycleComponentEdge, ExactUnsignedTripCount, MachineId, O, OptimizerCycleComponent,
            OptimizerUnsignedCountdownRankingCertificate, PsiOptimizationFunction,
            PsiOptimizationUnit, ScalarType, UnsignedCountdownLoopSummary,
            ValidatedOptimizerCycleComponents, by_key, region, shape, validate_analysis_root,
        };

        pub(crate) fn reconstruct(
            unit: &PsiOptimizationUnit,
            custody: &ValidatedOptimizerCycleComponents,
        ) -> Result<CountedLoopAnalysisSnapshot, CountedLoopAnalysisError> {
            validate_analysis_root(unit, custody, &[]).map_err(CountedLoopAnalysisError::Root)?;
            let components = by_key(custody.components(), |component| component.id.clone());
            let certificates = by_key(
                custody.ranking_certificates().certificates(),
                |certificate| certificate.component.clone(),
            );
            if components.len() != custody.components().len()
                || certificates.len() != custody.ranking_certificates().certificates().len()
                || certificates
                    .keys()
                    .any(|component| !components.contains_key(component))
            {
                return Err(CountedLoopAnalysisError::CertificateComponentRosterMismatch);
            }
            let functions = by_key(&unit.functions, |function| function.machine);
            let mut loops = Vec::with_capacity(certificates.len());
            for (id, certificate) in certificates {
                let component = components[&id];
                let function = functions
                    .get(&id.machine)
                    .copied()
                    .ok_or_else(|| shape(id.machine))?;
                loops.push(reconstruct_one(function, component, certificate)?);
            }
            Ok(CountedLoopAnalysisSnapshot {
                root: AnalysisRoot::of(unit),
                loops,
            })
        }

        fn reconstruct_one(
            function: &PsiOptimizationFunction,
            component: &OptimizerCycleComponent,
            certificate: &OptimizerUnsignedCountdownRankingCertificate,
        ) -> Result<UnsignedCountdownLoopSummary, CountedLoopAnalysisError> {
            let preheader_edge = single_edge(&component.entries, function.machine)?;
            let exit_edge = single_edge(&component.exits, function.machine)?;
            let blocks = by_key(&function.blocks, |block| block.id);
            let preheader = blocks
                .get(&preheader_edge.source)
                .and_then(|block| block.nodes.last())
                .ok_or_else(|| shape(function.machine))?;
            let O::Jump {
                psi_edge,
                target,
                bindings,
                ..
            } = &preheader.operation
            else {
                return Err(shape(function.machine));
            };
            if (*psi_edge, *target) != (preheader_edge.edge, certificate.header)
                || preheader_edge.target != certificate.header
            {
                return Err(shape(function.machine));
            }
            let rank_bindings = bindings
                .iter()
                .filter(|binding| binding.parameter == certificate.rank_parameter)
                .collect::<Vec<_>>();
            let [binding] = rank_bindings.as_slice() else {
                return Err(shape(function.machine));
            };
            if binding.scalar_type != ScalarType::Integer(certificate.rank_type) {
                return Err(shape(function.machine));
            }
            let region = region::derive(function.machine, component, certificate)?;
            Ok(UnsignedCountdownLoopSummary {
                certificate: certificate.clone(),
                region,
                preheader_edge,
                exit_edge,
                trip_count: ExactUnsignedTripCount {
                    initial_value: binding.argument,
                    scalar_type: certificate.rank_type,
                },
            })
        }

        fn single_edge(
            edges: &[CycleComponentEdge],
            machine: MachineId,
        ) -> Result<CycleComponentEdge, CountedLoopAnalysisError> {
            let [edge] = edges else {
                return Err(shape(machine));
            };
            Ok(*edge)
        }
    }
    pub(crate) mod validate {
        use super::{
            CountedLoopAnalysisError, CountedLoopAnalysisSnapshot, PsiOptimizationUnit,
            ValidatedCountedLoopAnalysis, ValidatedOptimizerCycleComponents, replay,
        };
        pub(crate) fn accept(
            unit: &PsiOptimizationUnit,
            custody: &ValidatedOptimizerCycleComponents,
            candidate: &CountedLoopAnalysisSnapshot,
        ) -> Result<ValidatedCountedLoopAnalysis, CountedLoopAnalysisError> {
            let reconstructed = replay::reconstruct(unit, custody)?;
            if *candidate != reconstructed {
                return Err(CountedLoopAnalysisError::SnapshotMismatch);
            }
            Ok(ValidatedCountedLoopAnalysis::new(reconstructed))
        }
    }

    pub(crate) fn analyze_counted_loops(
        unit: &PsiOptimizationUnit,
        custody: &ValidatedOptimizerCycleComponents,
    ) -> Result<ValidatedCountedLoopAnalysis, CountedLoopAnalysisError> {
        let candidate = compute::propose(unit, custody)?;
        validate::accept(unit, custody, &candidate)
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct ExactUnsignedTripCount {
        pub initial_value: ValueId,
        pub scalar_type: IntegerType,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct UnsignedCountdownLoopSummary {
        pub certificate: OptimizerUnsignedCountdownRankingCertificate,
        pub region: LoopRegion,
        pub preheader_edge: CycleComponentEdge,
        pub exit_edge: CycleComponentEdge,
        pub trip_count: ExactUnsignedTripCount,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct CountedLoopAnalysisSnapshot {
        pub root: AnalysisRoot,
        pub loops: Vec<UnsignedCountdownLoopSummary>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ValidatedCountedLoopAnalysis {
        snapshot: CountedLoopAnalysisSnapshot,
    }

    impl ValidatedCountedLoopAnalysis {
        const fn new(snapshot: CountedLoopAnalysisSnapshot) -> Self {
            Self { snapshot }
        }

        pub fn loops(&self) -> &[UnsignedCountdownLoopSummary] {
            &self.snapshot.loops
        }

        pub const fn snapshot(&self) -> &CountedLoopAnalysisSnapshot {
            &self.snapshot
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum CountedLoopAnalysisError {
        Root(AnalysisRootError),
        CertificateComponentRosterMismatch,
        UnsupportedCountdownShape { machine: MachineId },
        SnapshotMismatch,
    }

    failed_display!(CountedLoopAnalysisError, "counted-loop analysis failure");

    pub(crate) fn shape(machine: MachineId) -> CountedLoopAnalysisError {
        CountedLoopAnalysisError::UnsupportedCountdownShape { machine }
    }
}

pub(crate) mod countdown_invariant_constants {
    use abstract_operations::AbstractOperation as O;
    use optimization_unit::{
        EffectLink, NodeLocation, OptimizationBlock, OptimizationNode, PsiOptimizationFunction,
        PsiOptimizationUnit, PsiProvenance, ValueDefinition, ValueDefinitionSite, WorkSettlement,
    };

    use crate::validation::ValidatedOptimizerCycleComponents;
    use optimization_unit::OptimizerUnsignedCountdownRankingCertificate;
    use semantic_vocabulary::{
        BlockId, IntegerType, IntegerValue, MachineId, OperationId, ScalarType, ValueId,
    };

    use super::{
        AnalysisRoot, AnalysisRootError, CountedLoopAnalysisError, UnsignedCountdownLoopSummary,
        ValidatedCountedLoopAnalysis, by_key, validate_analysis_root,
    };

    pub(crate) mod compute {
        use super::{
            AnalysisRoot, CountdownInvariantConstantAnalysisError,
            CountdownInvariantConstantAnalysisSnapshot, CountdownInvariantConstantRole,
            CountdownInvariantIntegerConstant, O, OptimizerUnsignedCountdownRankingCertificate,
            PsiOptimizationFunction, PsiOptimizationUnit, UnsignedCountdownInvariantConstants,
            ValidatedCountedLoopAnalysis, ValidatedOptimizerCycleComponents, expected_constant,
            invariant_constant_row, validate_analysis_root, validate_locations,
        };
        pub(crate) fn propose(
            unit: &PsiOptimizationUnit,
            custody: &ValidatedOptimizerCycleComponents,
            counted: &ValidatedCountedLoopAnalysis,
        ) -> Result<
            CountdownInvariantConstantAnalysisSnapshot,
            CountdownInvariantConstantAnalysisError,
        > {
            validate_roots(unit, custody, counted)?;
            let mut loops = Vec::with_capacity(counted.loops().len());
            for summary in counted.loops() {
                let certificate = custody
                    .ranking_certificates()
                    .certificates()
                    .iter()
                    .find(|certificate| certificate.component == summary.certificate.component)
                    .filter(|certificate| *certificate == &summary.certificate)
                    .ok_or(CountdownInvariantConstantAnalysisError::ComponentRosterMismatch)?;
                let component = custody
                    .components()
                    .iter()
                    .find(|component| component.id == certificate.component)
                    .ok_or(CountdownInvariantConstantAnalysisError::ComponentRosterMismatch)?;
                let function = unit
                    .function(certificate.component.machine)
                    .ok_or(CountdownInvariantConstantAnalysisError::ComponentRosterMismatch)?;
                if summary.region.blocks != component.members {
                    return Err(CountdownInvariantConstantAnalysisError::ComponentRosterMismatch);
                }
                let constants = [
                    locate(
                        function,
                        certificate,
                        CountdownInvariantConstantRole::PositiveGuardZero,
                    )?,
                    locate(
                        function,
                        certificate,
                        CountdownInvariantConstantRole::BackedgeDecrementOne,
                    )?,
                ];
                validate_locations(function, component, certificate, &constants)?;
                loops.push(UnsignedCountdownInvariantConstants {
                    counted_loop: summary.clone(),
                    prospective_preheader: summary.preheader_edge.source,
                    constants: constants.into(),
                });
            }
            Ok(CountdownInvariantConstantAnalysisSnapshot {
                root: AnalysisRoot::of(unit),
                loops,
            })
        }

        fn locate(
            function: &PsiOptimizationFunction,
            certificate: &OptimizerUnsignedCountdownRankingCertificate,
            role: CountdownInvariantConstantRole,
        ) -> Result<CountdownInvariantIntegerConstant, CountdownInvariantConstantAnalysisError>
        {
            let expected = expected_constant(certificate, role);
            let mut matches = function
                .blocks
                .iter()
                .flat_map(|block| {
                    block
                        .nodes
                        .iter()
                        .enumerate()
                        .map(move |(node, value)| (block.id, node, value))
                })
                .filter(|(_, _, node)| {
                    matches!(
                        node.operation,
                        O::IntegerConstant { psi_operation, .. } if psi_operation == expected.operation
                    )
                });
            let Some((block, node_index, node)) = matches.next() else {
                return Err(unsupported(function.machine, expected.operation));
            };
            if matches.next().is_some() {
                return Err(unsupported(function.machine, expected.operation));
            }
            let node_index = u32::try_from(node_index)
                .map_err(|_| unsupported(function.machine, expected.operation))?;
            invariant_constant_row(function, block, node_index, node, role, expected)
                .ok_or_else(|| unsupported(function.machine, expected.operation))
        }

        fn validate_roots(
            unit: &PsiOptimizationUnit,
            custody: &ValidatedOptimizerCycleComponents,
            counted: &ValidatedCountedLoopAnalysis,
        ) -> Result<(), CountdownInvariantConstantAnalysisError> {
            validate_analysis_root(unit, custody, &[counted.snapshot().root])
                .map_err(CountdownInvariantConstantAnalysisError::Root)?;
            if custody.ranking_certificates().certificates().len() != counted.loops().len() {
                return Err(CountdownInvariantConstantAnalysisError::ComponentRosterMismatch);
            }
            Ok(())
        }

        fn unsupported(
            machine: semantic_vocabulary::MachineId,
            operation: semantic_vocabulary::OperationId,
        ) -> CountdownInvariantConstantAnalysisError {
            CountdownInvariantConstantAnalysisError::UnsupportedInvariantConstant {
                machine,
                operation,
            }
        }
    }
    pub(crate) mod replay {
        use super::{
            AnalysisRoot, CountdownInvariantConstantAnalysisError,
            CountdownInvariantConstantAnalysisSnapshot, CountdownInvariantConstantRole,
            CountdownInvariantIntegerConstant, OptimizerUnsignedCountdownRankingCertificate,
            PsiOptimizationFunction, PsiOptimizationUnit, UnsignedCountdownInvariantConstants,
            ValidatedCountedLoopAnalysis, ValidatedOptimizerCycleComponents, by_key,
            expected_constant, invariant_constant_row, sole_provenance_node, unsupported,
            validate_analysis_root, validate_locations,
        };

        pub(crate) fn reconstruct(
            unit: &PsiOptimizationUnit,
            custody: &ValidatedOptimizerCycleComponents,
            counted: &ValidatedCountedLoopAnalysis,
        ) -> Result<
            CountdownInvariantConstantAnalysisSnapshot,
            CountdownInvariantConstantAnalysisError,
        > {
            validate_roots(unit, custody, counted)?;
            let functions = by_key(&unit.functions, |function| function.machine);
            let components = by_key(custody.components(), |component| component.id.clone());
            let counted_loops = by_key(counted.loops(), |summary| {
                summary.certificate.component.clone()
            });
            if functions.len() != unit.functions.len()
                || components.len() != custody.components().len()
                || counted_loops.len() != counted.loops().len()
            {
                return Err(CountdownInvariantConstantAnalysisError::ComponentRosterMismatch);
            }

            let mut loops = Vec::with_capacity(custody.ranking_certificates().certificates().len());
            for certificate in custody.ranking_certificates().certificates() {
                let component = components
                    .get(&certificate.component)
                    .copied()
                    .ok_or(CountdownInvariantConstantAnalysisError::ComponentRosterMismatch)?;
                let summary = counted_loops
                    .get(&certificate.component)
                    .copied()
                    .filter(|summary| summary.certificate == *certificate)
                    .ok_or(CountdownInvariantConstantAnalysisError::ComponentRosterMismatch)?;
                let function = functions
                    .get(&certificate.component.machine)
                    .copied()
                    .ok_or(CountdownInvariantConstantAnalysisError::ComponentRosterMismatch)?;
                if summary.region.blocks != component.members
                    || summary.preheader_edge.target != certificate.header
                {
                    return Err(CountdownInvariantConstantAnalysisError::ComponentRosterMismatch);
                }
                let constants = [
                    reconstruct_row(
                        function,
                        certificate,
                        CountdownInvariantConstantRole::PositiveGuardZero,
                    )?,
                    reconstruct_row(
                        function,
                        certificate,
                        CountdownInvariantConstantRole::BackedgeDecrementOne,
                    )?,
                ];
                validate_locations(function, component, certificate, &constants)?;
                loops.push(UnsignedCountdownInvariantConstants {
                    counted_loop: summary.clone(),
                    prospective_preheader: summary.preheader_edge.source,
                    constants: constants.into(),
                });
            }
            Ok(CountdownInvariantConstantAnalysisSnapshot {
                root: AnalysisRoot::of(unit),
                loops,
            })
        }

        fn reconstruct_row(
            function: &PsiOptimizationFunction,
            certificate: &OptimizerUnsignedCountdownRankingCertificate,
            role: CountdownInvariantConstantRole,
        ) -> Result<CountdownInvariantIntegerConstant, CountdownInvariantConstantAnalysisError>
        {
            let expected = expected_constant(certificate, role);
            let (block, node_index, node) =
                sole_provenance_node(function.blocks.iter(), expected.operation)
                    .ok_or_else(|| unsupported(function.machine, expected.operation))?;
            invariant_constant_row(function, block, node_index, node, role, expected)
                .ok_or_else(|| unsupported(function.machine, expected.operation))
        }

        fn validate_roots(
            unit: &PsiOptimizationUnit,
            custody: &ValidatedOptimizerCycleComponents,
            counted: &ValidatedCountedLoopAnalysis,
        ) -> Result<(), CountdownInvariantConstantAnalysisError> {
            validate_analysis_root(unit, custody, &[counted.snapshot().root])
                .map_err(CountdownInvariantConstantAnalysisError::Root)?;
            let component_keys = custody
                .components()
                .iter()
                .map(|component| &component.id)
                .collect::<Vec<_>>();
            let certificate_keys = custody
                .ranking_certificates()
                .certificates()
                .iter()
                .map(|certificate| &certificate.component)
                .collect::<Vec<_>>();
            let counted_keys = counted
                .loops()
                .iter()
                .map(|summary| &summary.certificate.component)
                .collect::<Vec<_>>();
            if certificate_keys != counted_keys
                || certificate_keys
                    .iter()
                    .any(|key| !component_keys.contains(key))
            {
                return Err(CountdownInvariantConstantAnalysisError::ComponentRosterMismatch);
            }
            Ok(())
        }
    }
    pub(crate) mod validate {
        use super::{
            CountdownInvariantConstantAnalysisError, CountdownInvariantConstantAnalysisSnapshot,
            PsiOptimizationUnit, ValidatedCountdownInvariantConstantAnalysis,
            ValidatedCountedLoopAnalysis, ValidatedOptimizerCycleComponents, replay,
        };
        pub(crate) fn accept(
            unit: &PsiOptimizationUnit,
            custody: &ValidatedOptimizerCycleComponents,
            counted: &ValidatedCountedLoopAnalysis,
            candidate: &CountdownInvariantConstantAnalysisSnapshot,
        ) -> Result<
            ValidatedCountdownInvariantConstantAnalysis,
            CountdownInvariantConstantAnalysisError,
        > {
            let reconstructed = replay::reconstruct(unit, custody, counted)?;
            if *candidate != reconstructed {
                return Err(CountdownInvariantConstantAnalysisError::SnapshotMismatch);
            }
            Ok(ValidatedCountdownInvariantConstantAnalysis::new(
                reconstructed,
            ))
        }
    }

    pub(crate) fn analyze_countdown_invariant_constants(
        unit: &PsiOptimizationUnit,
        custody: &ValidatedOptimizerCycleComponents,
        counted: &ValidatedCountedLoopAnalysis,
    ) -> Result<ValidatedCountdownInvariantConstantAnalysis, CountdownInvariantConstantAnalysisError>
    {
        let candidate = compute::propose(unit, custody, counted)?;
        validate::accept(unit, custody, counted, &candidate)
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub(crate) enum CountdownInvariantConstantRole {
        PositiveGuardZero,
        BackedgeDecrementOne,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct CountdownInvariantIntegerConstant {
        pub role: CountdownInvariantConstantRole,
        pub location: NodeLocation,
        pub psi_operation: OperationId,
        pub result: ValueId,
        pub scalar_type: IntegerType,
        pub value: IntegerValue,
        pub definition: ValueDefinition,
        pub provenance: Vec<PsiProvenance>,
        pub work: Vec<WorkSettlement>,
        pub effect: EffectLink,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct UnsignedCountdownInvariantConstants {
        pub counted_loop: UnsignedCountdownLoopSummary,
        pub prospective_preheader: BlockId,
        pub constants: Vec<CountdownInvariantIntegerConstant>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct CountdownInvariantConstantAnalysisSnapshot {
        pub root: AnalysisRoot,
        pub loops: Vec<UnsignedCountdownInvariantConstants>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ValidatedCountdownInvariantConstantAnalysis {
        snapshot: CountdownInvariantConstantAnalysisSnapshot,
    }

    impl ValidatedCountdownInvariantConstantAnalysis {
        const fn new(snapshot: CountdownInvariantConstantAnalysisSnapshot) -> Self {
            Self { snapshot }
        }

        pub fn loops(&self) -> &[UnsignedCountdownInvariantConstants] {
            &self.snapshot.loops
        }

        pub const fn snapshot(&self) -> &CountdownInvariantConstantAnalysisSnapshot {
            &self.snapshot
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum CountdownInvariantConstantAnalysisError {
        CountedLoop(CountedLoopAnalysisError),
        Root(AnalysisRootError),
        ComponentRosterMismatch,
        UnsupportedInvariantConstant {
            machine: MachineId,
            operation: OperationId,
        },
        SnapshotMismatch,
    }

    failed_display!(
        CountdownInvariantConstantAnalysisError,
        "countdown invariant-constant analysis failure"
    );
    fn validate_locations(
        function: &PsiOptimizationFunction,
        component: &optimization_unit::OptimizerCycleComponent,
        certificate: &OptimizerUnsignedCountdownRankingCertificate,
        constants: &[CountdownInvariantIntegerConstant; 2],
    ) -> Result<(), CountdownInvariantConstantAnalysisError> {
        validate_moved_constant_locations(function, component, certificate, constants, unsupported)
    }

    pub(crate) fn validate_moved_constant_locations<E>(
        function: &PsiOptimizationFunction,
        component: &optimization_unit::OptimizerCycleComponent,
        certificate: &OptimizerUnsignedCountdownRankingCertificate,
        constants: &[CountdownInvariantIntegerConstant; 2],
        unsupported: impl Fn(MachineId, OperationId) -> E,
    ) -> Result<(), E> {
        let original_blocks = [certificate.header, certificate.descent.backedge.source];
        if constants
            .iter()
            .zip(original_blocks)
            .all(|(constant, original)| constant.location.block == original)
        {
            return Ok(());
        }
        let rejected = || unsupported(function.machine, certificate.guard.zero_operation);
        let [entry] = component.entries.as_slice() else {
            return Err(rejected());
        };
        if component.members.contains(&entry.source) || entry.target != certificate.header {
            return Err(rejected());
        }
        let preheader = function.block(entry.source).ok_or_else(rejected)?;
        let jump_index = preheader.nodes.len().checked_sub(1).ok_or_else(rejected)?;
        let O::Jump {
            psi_edge, target, ..
        } = preheader.nodes[jump_index].operation
        else {
            return Err(rejected());
        };
        if psi_edge != entry.edge || target != certificate.header {
            return Err(rejected());
        }
        let moved = constants
            .iter()
            .zip(original_blocks)
            .filter_map(|(constant, original)| {
                (constant.location.block != original).then_some(constant)
            })
            .collect::<Vec<_>>();
        let suffix_start = jump_index.checked_sub(moved.len()).ok_or_else(rejected)?;
        if moved.iter().enumerate().any(|(offset, constant)| {
            constant.location.machine != function.machine
                || constant.location.block != preheader.id
                || usize::try_from(constant.location.node).ok() != Some(suffix_start + offset)
        }) {
            return Err(rejected());
        }
        Ok(())
    }

    #[derive(Debug, Clone, Copy)]
    pub(crate) struct ExpectedInvariantConstant {
        pub operation: OperationId,
        pub result: ValueId,
        pub scalar_type: IntegerType,
        pub value: IntegerValue,
    }

    pub(crate) fn expected_constant(
        certificate: &OptimizerUnsignedCountdownRankingCertificate,
        role: CountdownInvariantConstantRole,
    ) -> ExpectedInvariantConstant {
        let (operation, result, value) = match role {
            CountdownInvariantConstantRole::PositiveGuardZero => (
                certificate.guard.zero_operation,
                certificate.guard.zero,
                IntegerValue::Unsigned(0),
            ),
            CountdownInvariantConstantRole::BackedgeDecrementOne => (
                certificate.descent.one_operation,
                certificate.descent.one,
                IntegerValue::Unsigned(1),
            ),
        };
        ExpectedInvariantConstant {
            operation,
            result,
            scalar_type: certificate.rank_type,
            value,
        }
    }

    pub(crate) fn sole_provenance_node<'function>(
        blocks: impl Iterator<Item = &'function OptimizationBlock>,
        operation: OperationId,
    ) -> Option<(BlockId, u32, &'function OptimizationNode)> {
        let mut matches = blocks
            .flat_map(|block| {
                block
                    .nodes
                    .iter()
                    .enumerate()
                    .map(move |(node, value)| (block.id, node, value))
            })
            .filter(|(_, _, node)| {
                node.provenance
                    .first()
                    .is_some_and(|source| *source == PsiProvenance::Operation(operation))
            });
        let (block, node_index, node) = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        Some((block, u32::try_from(node_index).ok()?, node))
    }

    pub(crate) fn invariant_constant_row(
        function: &PsiOptimizationFunction,
        block: BlockId,
        node_index: u32,
        node: &OptimizationNode,
        role: CountdownInvariantConstantRole,
        expected: ExpectedInvariantConstant,
    ) -> Option<CountdownInvariantIntegerConstant> {
        let O::IntegerConstant {
            psi_operation,
            result,
            scalar_type: ScalarType::Integer(scalar_type),
            value,
        } = node.operation
        else {
            return None;
        };
        let [definition] = node.definitions.as_slice() else {
            return None;
        };
        if psi_operation != expected.operation
            || result != expected.result
            || scalar_type != expected.scalar_type
            || value != expected.value
            || definition.value != result
            || definition.scalar_type != ScalarType::Integer(scalar_type)
            || definition.site
                != (ValueDefinitionSite::Node {
                    block,
                    node: node_index,
                })
            || !node.uses.is_empty()
            || !node.successors.is_empty()
            || !node.ownership.is_empty()
        {
            return None;
        }
        Some(CountdownInvariantIntegerConstant {
            role,
            location: NodeLocation {
                machine: function.machine,
                block,
                node: node_index,
            },
            psi_operation,
            result,
            scalar_type,
            value,
            definition: *definition,
            provenance: node.provenance.clone(),
            work: node.work.clone(),
            effect: node.effect,
        })
    }

    fn unsupported(
        machine: MachineId,
        operation: OperationId,
    ) -> CountdownInvariantConstantAnalysisError {
        CountdownInvariantConstantAnalysisError::UnsupportedInvariantConstant { machine, operation }
    }
}

pub(crate) mod countdown_invariant_constant_placement {
    use abstract_operations::{
        AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
    };
    use optimization_unit::{
        NodeLocation, OptimizationNode, PsiOptimizationFunction, PsiOptimizationUnit, ValueUse,
    };

    use crate::validation::ValidatedOptimizerCycleComponents;
    use optimization_unit::{
        CycleComponentEdge, CycleComponentId, OptimizerUnsignedCountdownRankingCertificate,
    };
    use semantic_vocabulary::{BlockId, MachineId, OperationId, ScalarType, ValueId};

    use super::{
        AnalysisRoot, AnalysisRootError, CountdownInvariantConstantAnalysisError,
        CountdownInvariantConstantRole, CountdownInvariantIntegerConstant,
        CountedLoopAnalysisError, UnsignedCountdownLoopSummary,
        ValidatedCountdownInvariantConstantAnalysis, ValidatedCountedLoopAnalysis, by_key,
        expected_constant, invariant_constant_row, sole_provenance_node, validate_analysis_root,
        validate_moved_constant_locations,
    };

    pub(crate) mod compute {
        use super::{
            AnalysisRoot, BlockId, CountdownInvariantConstantConsumer,
            CountdownInvariantConstantDestination, CountdownInvariantConstantPlacement,
            CountdownInvariantConstantPlacementAnalysisError,
            CountdownInvariantConstantPlacementAnalysisSnapshot, CountdownInvariantIntegerConstant,
            MachineId, NodeLocation, O, OperationId, OptimizationNode,
            OptimizerUnsignedCountdownRankingCertificate, PsiOptimizationFunction,
            PsiOptimizationUnit, UnsignedCountdownInvariantConstantPlacements,
            UnsignedCountdownLoopSummary, ValidatedCountdownInvariantConstantAnalysis,
            ValidatedCountedLoopAnalysis, ValidatedOptimizerCycleComponents, consumer_matches,
            consumer_operation, validate_analysis_root,
        };
        pub(crate) fn propose(
            unit: &PsiOptimizationUnit,
            custody: &ValidatedOptimizerCycleComponents,
            counted: &ValidatedCountedLoopAnalysis,
            invariants: &ValidatedCountdownInvariantConstantAnalysis,
        ) -> Result<
            CountdownInvariantConstantPlacementAnalysisSnapshot,
            CountdownInvariantConstantPlacementAnalysisError,
        > {
            validate_roots(unit, custody, counted, invariants)?;
            let mut loops = Vec::with_capacity(invariants.loops().len());
            for invariant in invariants.loops() {
                let certificate = &invariant.counted_loop.certificate;
                let component = custody
                    .components()
                    .iter()
                    .find(|component| component.id == certificate.component)
                    .ok_or(
                        CountdownInvariantConstantPlacementAnalysisError::ComponentRosterMismatch,
                    )?;
                let function = unit.function(certificate.component.machine).ok_or(
                    CountdownInvariantConstantPlacementAnalysisError::ComponentRosterMismatch,
                )?;
                let destination = destination(function, component, &invariant.counted_loop)?;
                let placements = invariant
                    .constants
                    .iter()
                    .map(|constant| {
                        Ok(CountdownInvariantConstantPlacement {
                            constant: constant.clone(),
                            destination: destination.clone(),
                            consumer: consumer(function, component, certificate, constant)?,
                        })
                    })
                    .collect::<Result<Vec<_>, CountdownInvariantConstantPlacementAnalysisError>>(
                    )?;
                loops.push(UnsignedCountdownInvariantConstantPlacements {
                    component: component.id.clone(),
                    counted_loop: invariant.counted_loop.clone(),
                    placements,
                });
            }
            Ok(CountdownInvariantConstantPlacementAnalysisSnapshot {
                root: AnalysisRoot::of(unit),
                loops,
            })
        }

        fn destination(
            function: &PsiOptimizationFunction,
            component: &optimization_unit::OptimizerCycleComponent,
            counted: &UnsignedCountdownLoopSummary,
        ) -> Result<
            CountdownInvariantConstantDestination,
            CountdownInvariantConstantPlacementAnalysisError,
        > {
            let rejected =
                || unsupported(function.machine, counted.certificate.guard.zero_operation);
            if component.members.contains(&counted.preheader_edge.source) {
                return Err(rejected());
            }
            let block = function
                .block(counted.preheader_edge.source)
                .ok_or_else(rejected)?;
            let node = block.nodes.last().ok_or_else(rejected)?;
            let O::Jump {
                psi_edge, target, ..
            } = node.operation
            else {
                return Err(rejected());
            };
            if psi_edge != counted.preheader_edge.edge
                || target != counted.certificate.header
                || counted.preheader_edge.target != counted.certificate.header
                || component.entries.as_slice() != [counted.preheader_edge]
            {
                return Err(rejected());
            }
            Ok(CountdownInvariantConstantDestination {
                before: NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: u32::try_from(block.nodes.len() - 1).map_err(|_| rejected())?,
                },
                entry_edge: counted.preheader_edge,
            })
        }

        fn consumer(
            function: &PsiOptimizationFunction,
            component: &optimization_unit::OptimizerCycleComponent,
            certificate: &OptimizerUnsignedCountdownRankingCertificate,
            constant: &CountdownInvariantIntegerConstant,
        ) -> Result<
            CountdownInvariantConstantConsumer,
            CountdownInvariantConstantPlacementAnalysisError,
        > {
            let operation = consumer_operation(certificate, constant.role);
            let uses = function
                .node_uses()
                .filter(|value_use| value_use.value == constant.result)
                .copied()
                .collect::<Vec<_>>();
            let [value_use] = uses.as_slice() else {
                return Err(unsupported(function.machine, constant.psi_operation));
            };
            if !component.members.contains(&value_use.block) {
                return Err(unsupported(function.machine, constant.psi_operation));
            }
            let node = node_at(function, value_use.block, value_use.node)
                .ok_or_else(|| unsupported(function.machine, constant.psi_operation))?;
            if !consumer_matches(node, certificate, constant.role, operation, constant.result) {
                return Err(unsupported(certificate.component.machine, operation));
            }
            Ok(CountdownInvariantConstantConsumer {
                location: NodeLocation {
                    machine: function.machine,
                    block: value_use.block,
                    node: value_use.node,
                },
                psi_operation: operation,
                value_use: *value_use,
            })
        }

        fn node_at(
            function: &PsiOptimizationFunction,
            block: BlockId,
            node: u32,
        ) -> Option<&OptimizationNode> {
            function
                .block(block)?
                .nodes
                .get(usize::try_from(node).ok()?)
        }

        fn validate_roots(
            unit: &PsiOptimizationUnit,
            custody: &ValidatedOptimizerCycleComponents,
            counted: &ValidatedCountedLoopAnalysis,
            invariants: &ValidatedCountdownInvariantConstantAnalysis,
        ) -> Result<(), CountdownInvariantConstantPlacementAnalysisError> {
            validate_analysis_root(
                unit,
                custody,
                &[counted.snapshot().root, invariants.snapshot().root],
            )
            .map_err(CountdownInvariantConstantPlacementAnalysisError::Root)?;
            if custody.ranking_certificates().certificates().len() != counted.loops().len()
                || counted.loops().len() != invariants.loops().len()
                || counted
                    .loops()
                    .iter()
                    .zip(invariants.loops())
                    .any(|(counted, invariant)| counted != &invariant.counted_loop)
            {
                return Err(
                    CountdownInvariantConstantPlacementAnalysisError::ComponentRosterMismatch,
                );
            }
            Ok(())
        }

        pub(crate) fn unsupported(
            machine: MachineId,
            operation: OperationId,
        ) -> CountdownInvariantConstantPlacementAnalysisError {
            CountdownInvariantConstantPlacementAnalysisError::UnsupportedPlacement {
                machine,
                operation,
            }
        }
    }
    pub(crate) mod replay {
        use super::{
            AnalysisRoot, CountdownInvariantConstantConsumer,
            CountdownInvariantConstantDestination, CountdownInvariantConstantPlacement,
            CountdownInvariantConstantPlacementAnalysisError,
            CountdownInvariantConstantPlacementAnalysisSnapshot, CountdownInvariantConstantRole,
            CountdownInvariantIntegerConstant, NodeLocation, O,
            OptimizerUnsignedCountdownRankingCertificate, PsiOptimizationFunction,
            PsiOptimizationUnit, ScalarType, UnsignedCountdownInvariantConstantPlacements,
            UnsignedCountdownLoopSummary, ValidatedCountdownInvariantConstantAnalysis,
            ValidatedCountedLoopAnalysis, ValidatedOptimizerCycleComponents, by_key,
            consumer_matches, consumer_operation, expected_constant, invariant_constant_row,
            sole_provenance_node, validate_analysis_root, validate_moved_constant_locations,
        };

        pub(crate) fn reconstruct(
            unit: &PsiOptimizationUnit,
            custody: &ValidatedOptimizerCycleComponents,
            counted: &ValidatedCountedLoopAnalysis,
            invariants: &ValidatedCountdownInvariantConstantAnalysis,
        ) -> Result<
            CountdownInvariantConstantPlacementAnalysisSnapshot,
            CountdownInvariantConstantPlacementAnalysisError,
        > {
            validate_roots(unit, custody, counted, invariants)?;
            let functions = by_key(&unit.functions, |function| function.machine);
            let components = by_key(custody.components(), |component| component.id.clone());
            let counted_loops = by_key(counted.loops(), |summary| {
                summary.certificate.component.clone()
            });
            let invariant_loops = by_key(invariants.loops(), |summary| {
                summary.counted_loop.certificate.component.clone()
            });
            if functions.len() != unit.functions.len()
                || components.len() != custody.components().len()
                || counted_loops.len() != counted.loops().len()
                || invariant_loops.len() != invariants.loops().len()
                || counted_loops.keys().ne(invariant_loops.keys())
                || counted_loops
                    .keys()
                    .any(|component| !components.contains_key(component))
            {
                return Err(
                    CountdownInvariantConstantPlacementAnalysisError::ComponentRosterMismatch,
                );
            }

            let mut loops = Vec::with_capacity(custody.ranking_certificates().certificates().len());
            for certificate in custody.ranking_certificates().certificates() {
                let component = components.get(&certificate.component).copied().ok_or(
                    CountdownInvariantConstantPlacementAnalysisError::ComponentRosterMismatch,
                )?;
                let counted = counted_loops
                    .get(&certificate.component)
                    .copied()
                    .filter(|summary| summary.certificate == *certificate)
                    .ok_or(
                        CountdownInvariantConstantPlacementAnalysisError::ComponentRosterMismatch,
                    )?;
                let invariants = invariant_loops
                    .get(&certificate.component)
                    .copied()
                    .filter(|summary| summary.counted_loop == *counted)
                    .ok_or(
                        CountdownInvariantConstantPlacementAnalysisError::ComponentRosterMismatch,
                    )?;
                let function = functions
                    .get(&certificate.component.machine)
                    .copied()
                    .ok_or(
                        CountdownInvariantConstantPlacementAnalysisError::ComponentRosterMismatch,
                    )?;
                let destination = reconstruct_destination(function, component, counted)?;
                let constants = [
                    reconstruct_constant(
                        function,
                        certificate,
                        CountdownInvariantConstantRole::PositiveGuardZero,
                    )?,
                    reconstruct_constant(
                        function,
                        certificate,
                        CountdownInvariantConstantRole::BackedgeDecrementOne,
                    )?,
                ];
                validate_constant_locations(function, component, certificate, &constants)?;
                let placements = [
                    CountdownInvariantConstantRole::PositiveGuardZero,
                    CountdownInvariantConstantRole::BackedgeDecrementOne,
                ]
                .into_iter()
                .map(|role| {
                    let constant = constants
                        .iter()
                        .find(|constant| constant.role == role)
                        .cloned()
                        .ok_or(CountdownInvariantConstantPlacementAnalysisError::ComponentRosterMismatch)?;
                    let retained = invariants
                        .constants
                        .iter()
                        .filter(|candidate| candidate.role == role)
                        .collect::<Vec<_>>();
                    let [retained] = retained.as_slice() else {
                        return Err(
                            CountdownInvariantConstantPlacementAnalysisError::ComponentRosterMismatch,
                        );
                    };
                    if **retained != constant {
                        return Err(
                            CountdownInvariantConstantPlacementAnalysisError::ComponentRosterMismatch,
                        );
                    }
                    Ok(CountdownInvariantConstantPlacement {
                        consumer: reconstruct_consumer(function, component, certificate, &constant)?,
                        constant,
                        destination: destination.clone(),
                    })
                })
                .collect::<Result<Vec<_>, CountdownInvariantConstantPlacementAnalysisError>>()?;
                if placements.len() != invariants.constants.len() {
                    return Err(
                        CountdownInvariantConstantPlacementAnalysisError::ComponentRosterMismatch,
                    );
                }
                loops.push(UnsignedCountdownInvariantConstantPlacements {
                    component: component.id.clone(),
                    counted_loop: counted.clone(),
                    placements,
                });
            }
            Ok(CountdownInvariantConstantPlacementAnalysisSnapshot {
                root: AnalysisRoot::of(unit),
                loops,
            })
        }

        fn reconstruct_destination(
            function: &PsiOptimizationFunction,
            component: &optimization_unit::OptimizerCycleComponent,
            counted: &UnsignedCountdownLoopSummary,
        ) -> Result<
            CountdownInvariantConstantDestination,
            CountdownInvariantConstantPlacementAnalysisError,
        > {
            let [entry_edge] = component.entries.as_slice() else {
                return Err(unsupported(
                    function.machine,
                    counted.certificate.guard.zero_operation,
                ));
            };
            if *entry_edge != counted.preheader_edge
                || entry_edge.target != counted.certificate.header
                || component.members.contains(&entry_edge.source)
            {
                return Err(unsupported(
                    function.machine,
                    counted.certificate.guard.zero_operation,
                ));
            }
            let preheader = function.block(entry_edge.source).ok_or_else(|| {
                unsupported(function.machine, counted.certificate.guard.zero_operation)
            })?;
            let Some((node_index, terminator)) = preheader.nodes.iter().enumerate().next_back()
            else {
                return Err(unsupported(
                    function.machine,
                    counted.certificate.guard.zero_operation,
                ));
            };
            let O::Jump {
                psi_edge,
                target,
                bindings,
                ..
            } = &terminator.operation
            else {
                return Err(unsupported(
                    function.machine,
                    counted.certificate.guard.zero_operation,
                ));
            };
            let rank_bindings = bindings
                .iter()
                .filter(|binding| binding.parameter == counted.certificate.rank_parameter)
                .collect::<Vec<_>>();
            let [rank_binding] = rank_bindings.as_slice() else {
                return Err(unsupported(
                    function.machine,
                    counted.certificate.guard.zero_operation,
                ));
            };
            if (*psi_edge, *target) != (entry_edge.edge, counted.certificate.header)
                || rank_binding.argument != counted.trip_count.initial_value
                || rank_binding.scalar_type != ScalarType::Integer(counted.certificate.rank_type)
            {
                return Err(unsupported(
                    function.machine,
                    counted.certificate.guard.zero_operation,
                ));
            }
            Ok(CountdownInvariantConstantDestination {
                before: NodeLocation {
                    machine: function.machine,
                    block: preheader.id,
                    node: u32::try_from(node_index).map_err(|_| {
                        unsupported(function.machine, counted.certificate.guard.zero_operation)
                    })?,
                },
                entry_edge: *entry_edge,
            })
        }

        fn reconstruct_constant(
            function: &PsiOptimizationFunction,
            certificate: &OptimizerUnsignedCountdownRankingCertificate,
            role: CountdownInvariantConstantRole,
        ) -> Result<
            CountdownInvariantIntegerConstant,
            CountdownInvariantConstantPlacementAnalysisError,
        > {
            let expected = expected_constant(certificate, role);
            let (block, node_index, node) =
                sole_provenance_node(function.blocks.iter(), expected.operation)
                    .ok_or_else(|| unsupported(function.machine, expected.operation))?;
            invariant_constant_row(function, block, node_index, node, role, expected)
                .ok_or_else(|| unsupported(function.machine, expected.operation))
        }

        fn validate_constant_locations(
            function: &PsiOptimizationFunction,
            component: &optimization_unit::OptimizerCycleComponent,
            certificate: &OptimizerUnsignedCountdownRankingCertificate,
            constants: &[CountdownInvariantIntegerConstant; 2],
        ) -> Result<(), CountdownInvariantConstantPlacementAnalysisError> {
            validate_moved_constant_locations(
                function,
                component,
                certificate,
                constants,
                unsupported,
            )
        }

        fn reconstruct_consumer(
            function: &PsiOptimizationFunction,
            component: &optimization_unit::OptimizerCycleComponent,
            certificate: &OptimizerUnsignedCountdownRankingCertificate,
            constant: &CountdownInvariantIntegerConstant,
        ) -> Result<
            CountdownInvariantConstantConsumer,
            CountdownInvariantConstantPlacementAnalysisError,
        > {
            let operation = consumer_operation(certificate, constant.role);
            let (block, node_index, node) = sole_provenance_node(
                function
                    .blocks
                    .iter()
                    .filter(|block| component.members.contains(&block.id)),
                operation,
            )
            .ok_or_else(|| unsupported(function.machine, operation))?;
            let value_uses = function
                .node_uses()
                .filter(|value_use| value_use.value == constant.result)
                .copied()
                .collect::<Vec<_>>();
            let [value_use] = value_uses.as_slice() else {
                return Err(unsupported(function.machine, operation));
            };
            if (value_use.block, value_use.node) != (block, node_index) {
                return Err(unsupported(function.machine, operation));
            }
            if !consumer_matches(node, certificate, constant.role, operation, constant.result) {
                return Err(unsupported(function.machine, operation));
            }
            Ok(CountdownInvariantConstantConsumer {
                location: NodeLocation {
                    machine: function.machine,
                    block,
                    node: node_index,
                },
                psi_operation: operation,
                value_use: *value_use,
            })
        }

        fn validate_roots(
            unit: &PsiOptimizationUnit,
            custody: &ValidatedOptimizerCycleComponents,
            counted: &ValidatedCountedLoopAnalysis,
            invariants: &ValidatedCountdownInvariantConstantAnalysis,
        ) -> Result<(), CountdownInvariantConstantPlacementAnalysisError> {
            validate_analysis_root(
                unit,
                custody,
                &[counted.snapshot().root, invariants.snapshot().root],
            )
            .map_err(CountdownInvariantConstantPlacementAnalysisError::Root)?;
            let component_keys = custody
                .components()
                .iter()
                .map(|component| &component.id)
                .collect::<Vec<_>>();
            let certificate_keys = custody
                .ranking_certificates()
                .certificates()
                .iter()
                .map(|certificate| &certificate.component)
                .collect::<Vec<_>>();
            let counted_keys = counted
                .loops()
                .iter()
                .map(|summary| &summary.certificate.component)
                .collect::<Vec<_>>();
            let invariant_keys = invariants
                .loops()
                .iter()
                .map(|summary| &summary.counted_loop.certificate.component)
                .collect::<Vec<_>>();
            if certificate_keys != counted_keys
                || counted_keys != invariant_keys
                || certificate_keys
                    .iter()
                    .any(|key| !component_keys.contains(key))
            {
                return Err(
                    CountdownInvariantConstantPlacementAnalysisError::ComponentRosterMismatch,
                );
            }
            Ok(())
        }

        use super::compute::unsupported;
    }
    pub(crate) mod validate {
        use super::{
            CountdownInvariantConstantPlacementAnalysisError,
            CountdownInvariantConstantPlacementAnalysisSnapshot, PsiOptimizationUnit,
            ValidatedCountdownInvariantConstantAnalysis,
            ValidatedCountdownInvariantConstantPlacementAnalysis, ValidatedCountedLoopAnalysis,
            ValidatedOptimizerCycleComponents, replay, validate_analysis_root,
        };
        pub(crate) fn accept(
            unit: &PsiOptimizationUnit,
            custody: &ValidatedOptimizerCycleComponents,
            counted: &ValidatedCountedLoopAnalysis,
            invariants: &ValidatedCountdownInvariantConstantAnalysis,
            candidate: &CountdownInvariantConstantPlacementAnalysisSnapshot,
        ) -> Result<
            ValidatedCountdownInvariantConstantPlacementAnalysis,
            CountdownInvariantConstantPlacementAnalysisError,
        > {
            validate_analysis_root(unit, custody, &[candidate.root])
                .map_err(CountdownInvariantConstantPlacementAnalysisError::Root)?;
            let reconstructed = replay::reconstruct(unit, custody, counted, invariants)?;
            if *candidate != reconstructed {
                return Err(CountdownInvariantConstantPlacementAnalysisError::SnapshotMismatch);
            }
            Ok(ValidatedCountdownInvariantConstantPlacementAnalysis::new(
                reconstructed,
            ))
        }
    }

    pub(crate) fn analyze_countdown_invariant_constant_placement(
        unit: &PsiOptimizationUnit,
        custody: &ValidatedOptimizerCycleComponents,
        counted: &ValidatedCountedLoopAnalysis,
        invariants: &ValidatedCountdownInvariantConstantAnalysis,
    ) -> Result<
        ValidatedCountdownInvariantConstantPlacementAnalysis,
        CountdownInvariantConstantPlacementAnalysisError,
    > {
        let candidate = compute::propose(unit, custody, counted, invariants)?;
        validate::accept(unit, custody, counted, invariants, &candidate)
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct CountdownInvariantConstantDestination {
        pub before: NodeLocation,
        pub entry_edge: CycleComponentEdge,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct CountdownInvariantConstantConsumer {
        pub location: NodeLocation,
        pub psi_operation: OperationId,
        pub value_use: ValueUse,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct CountdownInvariantConstantPlacement {
        pub constant: CountdownInvariantIntegerConstant,
        pub destination: CountdownInvariantConstantDestination,
        pub consumer: CountdownInvariantConstantConsumer,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct UnsignedCountdownInvariantConstantPlacements {
        pub component: CycleComponentId,
        pub counted_loop: UnsignedCountdownLoopSummary,
        pub placements: Vec<CountdownInvariantConstantPlacement>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct CountdownInvariantConstantPlacementAnalysisSnapshot {
        pub root: AnalysisRoot,
        pub loops: Vec<UnsignedCountdownInvariantConstantPlacements>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ValidatedCountdownInvariantConstantPlacementAnalysis {
        snapshot: CountdownInvariantConstantPlacementAnalysisSnapshot,
    }

    impl ValidatedCountdownInvariantConstantPlacementAnalysis {
        const fn new(snapshot: CountdownInvariantConstantPlacementAnalysisSnapshot) -> Self {
            Self { snapshot }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum CountdownInvariantConstantPlacementAnalysisError {
        CountedLoop(CountedLoopAnalysisError),
        InvariantConstant(CountdownInvariantConstantAnalysisError),
        Root(AnalysisRootError),
        ComponentRosterMismatch,
        UnsupportedPlacement {
            machine: MachineId,
            operation: OperationId,
        },
        SnapshotMismatch,
    }

    failed_display!(
        CountdownInvariantConstantPlacementAnalysisError,
        "countdown invariant-constant placement analysis failure"
    );

    pub(crate) fn consumer_operation(
        certificate: &OptimizerUnsignedCountdownRankingCertificate,
        role: CountdownInvariantConstantRole,
    ) -> OperationId {
        match role {
            CountdownInvariantConstantRole::PositiveGuardZero => {
                certificate.guard.comparison_operation
            }
            CountdownInvariantConstantRole::BackedgeDecrementOne => {
                certificate.descent.subtract_operation
            }
        }
    }

    pub(crate) fn consumer_matches(
        node: &OptimizationNode,
        certificate: &OptimizerUnsignedCountdownRankingCertificate,
        role: CountdownInvariantConstantRole,
        operation: OperationId,
        constant: ValueId,
    ) -> bool {
        match (role, &node.operation) {
            (
                CountdownInvariantConstantRole::PositiveGuardZero,
                O::IntegerLessThan {
                    psi_operation,
                    result,
                    left,
                    right,
                },
            ) => {
                (*psi_operation, *result, *left, *right)
                    == (
                        operation,
                        certificate.guard.condition,
                        constant,
                        certificate.rank_parameter,
                    )
            }
            (
                CountdownInvariantConstantRole::BackedgeDecrementOne,
                O::IntegerArithmetic {
                    psi_operation,
                    result,
                    scalar_type,
                    arithmetic,
                    left,
                    right,
                },
            ) if (arithmetic.policy(), arithmetic.operator())
                == (
                    IntegerOverflowPolicy::Exact,
                    IntegerArithmeticOperator::Subtract,
                ) =>
            {
                (
                    *psi_operation,
                    arithmetic.obligation(),
                    *result,
                    *scalar_type,
                    *left,
                    *right,
                ) == (
                    operation,
                    Some(certificate.descent.subtract_obligation),
                    certificate.descent.argument,
                    certificate.rank_type,
                    certificate.descent.source_parameter,
                    constant,
                )
            }
            _ => false,
        }
    }
}
