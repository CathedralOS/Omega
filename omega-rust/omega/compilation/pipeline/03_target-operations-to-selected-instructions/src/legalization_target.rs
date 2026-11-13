/*
We match target-operation function, unit, expression, control-flow, byte-view,
element-view, hosted-scalar, and normalized-foreign rows against their
abstract-operation and optimization-unit sources during target-plan auditing.
*/

use crate::LegalizationError;
use crate::legalization::scalar_graph_input::instruction;
use crate::legalization::scalar_graph_input::{
    AbstractFunction, AbstractOperation, AbstractOperationPlan, PsiOptimizationFunction,
    PsiOptimizationUnit, TargetFunction, TargetOperationPlan, ValueId, ValueLocation,
    ValuePlacement,
};
use semantic_vocabulary::BlockId;
use target_operations::{
    ScalarParameterLocation, TargetBooleanExpression as Boolean,
    TargetIntegerExpression as Expression, TargetScalarExpression, TargetUnitOperation,
};
pub(crate) mod byte_view {
    use super::Checker;
    use crate::legalization::scalar_graph_input::byte_views::{
        descriptor_result, owned_byte_result, writable_bytes,
    };
    use abstract_operations::AbstractOperation;
    use semantic_vocabulary::{PlaceId, ValueId};
    use target_operations::TargetByteView;

    impl Checker<'_> {
        pub(crate) fn byte_view(
            &self,
            view: &TargetByteView,
            source: PlaceId,
            aliases: &[(ValueId, ValueId)],
        ) -> bool {
            match view {
                TargetByteView::Result {
                    psi_operation,
                    place,
                    structural_type,
                } => {
                    *place == source
                        && descriptor_result(self.optimized, source, self.types)
                            == Some((*psi_operation, *structural_type))
                }
                TargetByteView::Owned {
                    psi_operation,
                    place,
                    structural_type,
                } => {
                    *place == source
                        && owned_byte_result(self.optimized, source, self.types)
                            == Some((*psi_operation, *structural_type))
                        && self.types.iter().any(|declaration| {
                            declaration.id == *structural_type
                                && declaration.shape.is_bounded_owned_byte_sequence()
                        })
                }
                TargetByteView::Literal {
                    psi_operation,
                    place,
                    structural_type,
                } => {
                    *place == source
                        && (crate::legalization::scalar_graph_input::literals::declaration_producer(self.optimized, source)
                            == Some((*psi_operation, *structural_type))
                            || self.established_byte_view(*psi_operation, source, *structural_type))
                }
                TargetByteView::BlockParameter {
                    block,
                    place,
                    structural_type,
                } => {
                    *place == source
                        && self.optimized.blocks.iter().any(|owner| {
                            owner.id == *block
                                && owner.id != self.optimized.entry
                                && owner.structural_parameters.iter().any(|parameter| {
                                    parameter.place == *place
                                        && parameter.structural_type == *structural_type
                                        && self
                                            .types
                                            .iter()
                                            .find(|row| row.id == parameter.structural_type)
                                            .is_some_and(|row| {
                                                match row.shape {
                                            terminal_psi::StructuralTypeShape::ByteSequence(
                                                terminal_psi::ByteSequenceCarrier::BoundedOwned {
                                                    ..
                                                },
                                            ) => parameter.multiplicity
                                                == terminal_psi::StructuralMultiplicity::Unrestricted,
                                            terminal_psi::StructuralTypeShape::ByteSequence(
                                                terminal_psi::ByteSequenceCarrier::BorrowedView {
                                                    ..
                                                }
                                                | terminal_psi::ByteSequenceCarrier::BoundedView {
                                                    ..
                                                },
                                            ) => terminal_semantics::descriptor_parameter_access(
                                                parameter,
                                                self.types.iter(),
                                            )
                                            .is_some(),
                                            _ => false,
                                        }
                                            })
                                        && self.optimized.arrival_parameter(source) == Some(parameter)
                                        && parameter.projected_qualifications.is_empty()
                                })
                        })
                }
                TargetByteView::Parameter { place, placement } => {
                    *place == source
                        && crate::legalization::scalar_graph_input::structural_parameters(self.function).is_some_and(
                            |parameters| {
                                parameters.iter().any(|parameter| {
                                    parameter.place == source
                                        && self.optimized.arrival_parameter(source).is_some_and(
                                            |semantic| {
                                                semantic.structural_type == parameter.structural_type
                                                    && semantic.access == parameter.access
                                                    && semantic.multiplicity == parameter.multiplicity
                                                    && semantic.projected_qualifications
                                                        == parameter.projected_qualifications
                                            },
                                        )
                                        && &parameter.placement == placement
                                })
                            },
                        )
                }
                TargetByteView::Subslice {
                    psi_operation,
                    place,
                    source: original,
                    indices,
                    start,
                    end,
                    length,
                    obligation,
                } => {
                    *place == source
                        && self.optimized.block_nodes().any(|node| {
                            let AbstractOperation::ByteSequenceSubslice {
                                psi_operation: operation,
                                result,
                                source: expected_source,
                                start: expected_start,
                                end: expected_end,
                                length: expected_length,
                                obligation: expected_obligation,
                            } = &node.operation
                            else {
                                return false;
                            };
                            operation == psi_operation
                                && result.place == source
                                && length == expected_length
                                && obligation == expected_obligation
                                && original == expected_source
                                && crate::legalization::scalar_graph_input::element_views::descriptor_geometry(
                                    self.optimized,
                                    original,
                                    self.types,
                                )
                                .is_some_and(|geometry| {
                                    geometry.readable
                                        && super::unit::runtime_indices_rejoin(
                                            indices,
                                            &geometry.elements,
                                            self.available.unwrap_or(&crate::legalization_target::control_flow::sources::EMPTY),
                                        )
                                })
                                && self.integer_source(start, *expected_start, aliases)
                                && self.integer_source(end, *expected_end, aliases)
                        })
                }
            }
        }

        pub(crate) fn mutable_byte_view(&self, view: &TargetByteView, source: PlaceId) -> bool {
            writable_bytes(self.optimized, source, self.types) && self.byte_view(view, source, &[])
        }

        fn established_byte_view(
            &self,
            psi_operation: semantic_vocabulary::OperationId,
            source: PlaceId,
            structural_type: semantic_vocabulary::StructuralTypeId,
        ) -> bool {
            self.optimized.block_nodes().any(|node| {
                let AbstractOperation::EstablishElementView {
                    psi_operation: operation,
                    result,
                    destination,
                    element,
                    ..
                } = &node.operation
                else {
                    return false;
                };
                operation == &psi_operation
                    && *destination == source
                    && result.structural_type == structural_type
                    && optimization_unit_semantics::structural_place_access(self.optimized, source)
                        .is_some_and(|access| {
                            matches!(
                                access,
                                terminal_psi::StructuralAccess::SharedBorrow
                                    | terminal_psi::StructuralAccess::MutableBorrow
                            )
                        })
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty()
                    && terminal_psi::find_structural_type(self.types, structural_type)
                        .is_some_and(|declaration| declaration.shape.is_borrowed_byte_carrier())
                    && terminal_psi::find_structural_type(self.types, *element).is_some_and(
                        |declaration| {
                            matches!(
                                declaration.shape,
                                terminal_psi::StructuralTypeShape::PrimitiveScalar(
                                    semantic_vocabulary::ScalarType::Integer(integer)
                                ) if Ok(integer)
                                    == semantic_vocabulary::IntegerType::new(
                                        semantic_vocabulary::IntegerSign::Unsigned,
                                        8,
                                    )
                            )
                        },
                    )
            })
        }
    }
}
pub(crate) mod control_flow {
    use super::{
        AbstractFunction, AbstractOperation, AbstractOperationPlan, PsiOptimizationFunction,
        TargetFunction, TargetScalarExpression, TargetUnitOperation,
    };
    use crate::LegalizationError;
    use crate::legalization::scalar_graph_input::reference_custody::{
        apply, apply_terminator, block_entry_states,
    };
    use crate::legalization::scalar_graph_input::target::Checker;
    use crate::legalization::scalar_graph_input::{AbstractFunctionResult, ScalarType};
    use target_operations::{TargetControlGraph, TargetControlSuccessor, TargetControlTerminator};
    pub(crate) mod no_normal {
        use crate::LegalizationError;
        use crate::legalization::scalar_graph_input::{self as input, reference_custody};
        use abstract_operations::{AbstractOperation, AbstractOperationPlan};
        use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
        use semantic_vocabulary::InvocationSite;
        use target_operations::{TargetControlTerminator, TargetOperationPlan};

        pub(crate) fn validate(
            target: &TargetControlTerminator,
            source: &AbstractOperation,
            available: &crate::legalization_target::control_flow::sources::AvailableSources<'_>,
            custody: &reference_custody::Custody,
            caller: &PsiOptimizationFunction,
            native: &TargetOperationPlan,
            plan: &AbstractOperationPlan,
            unit: &PsiOptimizationUnit,
        ) -> Result<(), LegalizationError> {
            let TargetControlTerminator::InvokeNoNormal {
                psi_edge,
                callee,
                call_plan,
                scalar_arguments,
                arguments,
                access_bindings,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
                declared_result,
            } = target
            else {
                return Err(LegalizationError::custody());
            };
            let Some(invocation) = source.module_invocation() else {
                return Err(LegalizationError::custody());
            };
            let expected = input::callee_plan(*callee, native, plan, unit)?;
            let signature = input::call_signature::signature(*callee, plan)?;
            let declaration = unit.function(*callee).ok_or(LegalizationError::custody())?;
            let site = InvocationSite::InvokeNoNormal(*psi_edge);
            if invocation.site != site
                || invocation.callee != *callee
                || call_plan != &expected
                || declared_result != &declaration.result
                || claim_transfers != invocation.claim_transfers
                || requirement_obligations != invocation.requirement_obligations
                || crash_continuations != invocation.crash_continuations
                || arguments.len() != invocation.structural_arguments.len()
                || arguments.len() != signature.structural_parameter_count()
                || scalar_arguments.len() != invocation.arguments.len()
                || scalar_arguments.len() != signature.scalar_parameter_count()
                || expected.parameters.len() != scalar_arguments.len() + arguments.len()
                || scalar_arguments
                    .iter()
                    .zip(invocation.arguments)
                    .zip(&expected.parameters)
                    .enumerate()
                    .any(|(position, ((argument, value), placement))| {
                        argument.parameter_index != position as u32
                            || argument.placement != *placement
                            || Some(argument.source.scalar_type())
                                != signature.scalar_parameter(position)
                            || !available.matching(*value).any(|(identity, source)| {
                                identity == value && source == &argument.source
                            })
                    })
            {
                return Err(LegalizationError::custody());
            }
            let expected_bindings = reference_custody::access_call_custody(
                caller,
                declaration,
                &plan.structural_types,
                invocation.structural_arguments,
                custody,
            )?;
            if access_bindings != &expected_bindings {
                return Err(LegalizationError::custody());
            }
            for (position, (argument, semantic)) in arguments
                .iter()
                .zip(invocation.structural_arguments)
                .enumerate()
            {
                let expected = input::structural_call::argument_at_invocation(
                    semantic, position, site, caller, signature, &expected, native, plan, custody,
                )?;
                if argument != &expected {
                    return Err(LegalizationError::custody());
                }
            }
            Ok(())
        }
    }
    pub(crate) mod sources {
        use super::super::{BlockId, ValueId};
        use super::{PsiOptimizationFunction, TargetControlGraph, TargetUnitOperation};
        use target_operations::TargetUnitScalarArgumentSource as Source;

        pub(crate) fn call_indices(
            native: &target_operations::TargetOperationPlan,
            caller: &PsiOptimizationFunction,
            owner: target_operations::CallSiteOwner,
            elements: &[crate::structural_inputs::structural_reference_input::RuntimeElement],
        ) -> Result<Vec<target_operations::TargetStructuralRuntimeIndex>, crate::LegalizationError>
        {
            if elements.is_empty() {
                return Ok(Vec::new());
            }
            let function = native
                .function(caller.machine)
                .ok_or(crate::LegalizationError::custody())?;
            let (call_block, call_position) = function
                .graph
                .blocks
                .iter()
                .find_map(|block| {
                    block
                        .operations
                        .iter()
                        .position(|operation| match operation {
                            TargetUnitOperation::Call { psi_operation, .. }
                            | TargetUnitOperation::NormalizedForeignCall {
                                psi_operation, ..
                            } => {
                                owner == target_operations::CallSiteOwner::Operation(*psi_operation)
                            }
                            TargetUnitOperation::CleanupCall {
                                owner: candidate, ..
                            } => *candidate == owner,
                            _ => false,
                        })
                        .map(|position| (block, position))
                })
                .ok_or(crate::LegalizationError::custody())?;
            let inputs = SourceInputs::new(&function.graph, caller);
            let mut sources = inputs.available(call_block.block);
            sources.extend(
                call_block.operations[..call_position]
                    .iter()
                    .filter_map(definition),
            );
            elements
                .iter()
                .map(|element| {
                    let (_, operand) = sources
                        .matching(element.index)
                        .find(|(value, _)| *value == element.index)
                        .ok_or(crate::LegalizationError::custody())?;
                    Ok(target_operations::TargetStructuralRuntimeIndex {
                        operand: *operand,
                        stride: element.stride,
                    })
                })
                .collect()
        }

        type SourceRow = (ValueId, Source);

        #[derive(Clone, Copy)]
        enum SourceScope {
            Function,
            Block {
                block: BlockId,
                interval: Option<(usize, usize)>,
                operation: bool,
            },
        }

        struct InputRow {
            row: SourceRow,
            scope: SourceScope,
        }

        // Keep every inherited source once. Per-block views select these rows
        // without copying them and append only the results checked in that block.
        pub(crate) struct SourceInputs {
            rows: Vec<InputRow>,
            by_value: Vec<usize>,
            dominators: SourceDominators,
        }

        struct InputScope<'inputs> {
            inputs: &'inputs SourceInputs,
            block: BlockId,
            interval: Option<(usize, usize)>,
        }

        impl InputScope<'_> {
            fn admits(&self, row: &InputRow) -> bool {
                match row.scope {
                    SourceScope::Function => true,
                    SourceScope::Block {
                        block,
                        interval,
                        operation,
                    } => {
                        (!operation || block != self.block)
                            && (block == self.block
                                || self.interval.is_none()
                                || interval.is_some_and(|(start, end)| {
                                    self.interval
                                        .is_some_and(|(entry, _)| start <= entry && entry < end)
                                }))
                    }
                }
            }
        }

        pub(crate) struct AvailableSources<'inputs> {
            inherited: Option<InputScope<'inputs>>,
            local: Vec<SourceRow>,
        }

        pub(crate) static EMPTY: AvailableSources<'static> = AvailableSources {
            inherited: None,
            local: Vec::new(),
        };

        impl AvailableSources<'_> {
            pub(crate) fn matching(
                &self,
                value: ValueId,
            ) -> impl DoubleEndedIterator<Item = &SourceRow> {
                self.inherited
                    .iter()
                    .flat_map(move |input| {
                        let positions = &input.inputs.by_value;
                        let first = positions
                            .partition_point(|position| input.inputs.rows[*position].row.0 < value);
                        let end = positions.partition_point(|position| {
                            input.inputs.rows[*position].row.0 <= value
                        });
                        positions[first..end]
                            .iter()
                            .map(move |position| &input.inputs.rows[*position])
                            .filter(move |row| input.admits(row))
                            .map(|row| &row.row)
                    })
                    .chain(self.local.iter().filter(move |row| row.0 == value))
            }

            pub(crate) fn push(&mut self, row: SourceRow) {
                self.local.push(row);
            }

            pub(crate) fn extend(&mut self, rows: impl IntoIterator<Item = SourceRow>) {
                self.local.extend(rows);
            }
        }

        impl SourceInputs {
            pub(crate) fn new(
                graph: &TargetControlGraph,
                optimized: &PsiOptimizationFunction,
            ) -> Self {
                let dominators = SourceDominators::new(optimized);
                let mut rows: Vec<_> = optimized
                    .parameters
                    .iter()
                    .enumerate()
                    .map(|(index, parameter)| InputRow {
                        row: (
                            parameter.value,
                            Source::Parameter {
                                parameter_index: index as u32,
                                source_value: parameter.value,
                                scalar_type: parameter.scalar_type,
                            },
                        ),
                        scope: SourceScope::Function,
                    })
                    .collect();
                for candidate in &graph.blocks {
                    let interval = dominators.interval(candidate.block);
                    rows.extend(candidate.parameters.iter().map(|parameter| InputRow {
                        row: (
                            parameter.value,
                            Source::BlockParameter(target_operations::TargetScalarBlockValue {
                                block: candidate.block,
                                value: parameter.value,
                                scalar_type: parameter.scalar_type,
                            }),
                        ),
                        scope: SourceScope::Block {
                            block: candidate.block,
                            interval,
                            operation: false,
                        },
                    }));
                    rows.extend(
                        candidate
                            .operations
                            .iter()
                            .filter_map(definition)
                            .map(|row| InputRow {
                                row,
                                scope: SourceScope::Block {
                                    block: candidate.block,
                                    interval,
                                    operation: true,
                                },
                            }),
                    );
                }
                let mut by_value: Vec<_> = (0..rows.len()).collect();
                by_value.sort_by_key(|position| rows[*position].row.0);
                Self {
                    rows,
                    by_value,
                    dominators,
                }
            }

            pub(crate) fn available(&self, block: BlockId) -> AvailableSources<'_> {
                AvailableSources {
                    inherited: Some(InputScope {
                        inputs: self,
                        block,
                        interval: self.dominators.interval(block),
                    }),
                    local: Vec::new(),
                }
            }
        }

        pub(crate) fn definition(operation: &TargetUnitOperation) -> Option<(ValueId, Source)> {
            match operation {
                TargetUnitOperation::IeeeFloatConstant {
                    psi_operation,
                    result,
                    value,
                } => Some((
                    *result,
                    Source::IeeeFloatImmediate {
                        defining_operation: *psi_operation,
                        source_value: *result,
                        value: *value,
                    },
                )),
                TargetUnitOperation::PrimitiveScalarRead {
                    psi_operation,
                    result,
                    ..
                }
                | TargetUnitOperation::StructuralScalarFieldRead {
                    psi_operation,
                    result,
                    ..
                }
                | TargetUnitOperation::StructuralByteSequenceFieldLength {
                    psi_operation,
                    result,
                    ..
                }
                | TargetUnitOperation::StructuralByteSequenceFieldRead {
                    psi_operation,
                    result,
                    ..
                }
                | TargetUnitOperation::StructuralCaseMembership {
                    psi_operation,
                    result,
                    ..
                } => Some((
                    result.value,
                    Source::Home(target_operations::TargetUnitScalarHomeRequirement {
                        defining_operation: *psi_operation,
                        source_value: result.value,
                        scalar_type: result.scalar_type,
                        shape: crate::legalization::scalar_graph_input::scalar_shape(
                            result.scalar_type,
                        )?,
                    }),
                )),
                TargetUnitOperation::BooleanConstant {
                    psi_operation,
                    result,
                    value,
                } => Some((
                    *result,
                    Source::BooleanImmediate {
                        defining_operation: *psi_operation,
                        source_value: *result,
                        value: *value,
                    },
                )),
                TargetUnitOperation::IntegerConstant {
                    psi_operation,
                    result,
                    scalar_type,
                    value,
                } => Some((
                    *result,
                    Source::IntegerImmediate {
                        defining_operation: *psi_operation,
                        source_value: *result,
                        scalar_type: *scalar_type,
                        value: *value,
                    },
                )),
                TargetUnitOperation::SelectImplementation { result_home, .. }
                | TargetUnitOperation::SelectMachine { result_home, .. }
                | TargetUnitOperation::AtomicAccess {
                    result_home: Some(result_home),
                    ..
                }
                | TargetUnitOperation::AccessorScalarAccess {
                    result_home: Some(result_home),
                    ..
                }
                | TargetUnitOperation::IeeeFloatCompare { result_home, .. }
                | TargetUnitOperation::ScalarRepresentationView { result_home, .. }
                | TargetUnitOperation::FloatResultPolicy { result_home, .. }
                | TargetUnitOperation::ProgramImageAddress { result_home, .. }
                | TargetUnitOperation::NearestIeeeFloatArithmetic { result_home, .. }
                | TargetUnitOperation::ScalarDefinition { result_home, .. }
                | TargetUnitOperation::Call {
                    result: target_operations::TargetCallResult::Scalar(result_home),
                    ..
                }
                | TargetUnitOperation::NormalizedForeignCall {
                    result: target_operations::TargetBoundaryResult::Scalar(result_home),
                    ..
                } => Some((result_home.source_value, Source::Home(*result_home))),
                _ => None,
            }
        }

        pub(crate) fn dominates(
            function: &PsiOptimizationFunction,
            candidate: BlockId,
            block: BlockId,
        ) -> bool {
            SourceDominators::new(function).dominates(candidate, block)
        }

        pub(crate) struct SourceDominators {
            order: std::collections::BTreeMap<BlockId, usize>,
            intervals: Vec<(usize, usize)>,
        }

        // The completed immediate-dominator tree has one parent for every
        // reachable nonroot vertex. Number its subtrees once instead of walking
        // the parent chain again for every candidate source.
        fn tree_intervals(parents: &[usize]) -> Vec<(usize, usize)> {
            let mut children = vec![Vec::new(); parents.len()];
            for (position, parent) in parents.iter().enumerate().skip(1) {
                children[*parent].push(position);
            }
            let mut intervals = vec![(0, 0); parents.len()];
            let mut next = 0;
            let mut stack = Vec::new();
            if !parents.is_empty() {
                stack.push((0, false));
            }
            while let Some((position, finished)) = stack.pop() {
                if finished {
                    intervals[position].1 = next;
                } else {
                    intervals[position].0 = next;
                    next += 1;
                    stack.push((position, true));
                    stack.extend(children[position].iter().rev().map(|child| (*child, false)));
                }
            }
            intervals
        }

        impl SourceDominators {
            pub(crate) fn new(function: &PsiOptimizationFunction) -> Self {
                let blocks = function
                    .blocks
                    .iter()
                    .enumerate()
                    .map(|(index, block)| (block.id, index))
                    .collect::<std::collections::BTreeMap<_, _>>();
                let successors = |block: BlockId| {
                    blocks.get(&block).into_iter().flat_map(|index| {
                        function.blocks[*index].successors().map(|edge| edge.target)
                    })
                };
                let mut postorder = Vec::new();
                let mut visited = std::collections::BTreeSet::from([function.entry]);
                let mut stack = vec![(
                    function.entry,
                    successors(function.entry).collect::<Vec<_>>(),
                )];
                while let Some((block, pending)) = stack.last_mut() {
                    if let Some(next) = pending.pop() {
                        if visited.insert(next) {
                            let next_successors = successors(next).collect();
                            stack.push((next, next_successors));
                        }
                    } else {
                        postorder.push(*block);
                        stack.pop();
                    }
                }
                postorder.reverse();
                let order = postorder
                    .iter()
                    .enumerate()
                    .map(|(position, block)| (*block, position))
                    .collect::<std::collections::BTreeMap<_, _>>();
                let mut predecessors = vec![Vec::new(); postorder.len()];
                for (position, block) in postorder.iter().enumerate() {
                    for successor in successors(*block) {
                        if let Some(target) = order.get(&successor) {
                            predecessors[*target].push(position);
                        }
                    }
                }
                const UNDEFINED: usize = usize::MAX;
                let mut immediate = vec![UNDEFINED; postorder.len()];
                if !immediate.is_empty() {
                    immediate[0] = 0;
                }
                let intersect = |immediate: &[usize], mut left: usize, mut right: usize| {
                    while left != right {
                        while left > right {
                            left = immediate[left];
                        }
                        while right > left {
                            right = immediate[right];
                        }
                    }
                    left
                };
                let mut changed = true;
                while changed {
                    changed = false;
                    for position in 1..postorder.len() {
                        let mut processed = predecessors[position]
                            .iter()
                            .copied()
                            .filter(|predecessor| immediate[*predecessor] != UNDEFINED);
                        let Some(first) = processed.next() else {
                            continue;
                        };
                        let candidate = processed.fold(first, |current, predecessor| {
                            intersect(&immediate, current, predecessor)
                        });
                        if immediate[position] != candidate {
                            immediate[position] = candidate;
                            changed = true;
                        }
                    }
                }
                let intervals = tree_intervals(&immediate);
                Self { order, intervals }
            }

            fn interval(&self, block: BlockId) -> Option<(usize, usize)> {
                self.order
                    .get(&block)
                    .map(|position| self.intervals[*position])
            }

            pub(crate) fn dominates(&self, candidate: BlockId, block: BlockId) -> bool {
                if candidate == block {
                    return true;
                }
                let Some(position) = self.order.get(&block).copied() else {
                    return true;
                };
                let Some(candidate) = self.order.get(&candidate).copied() else {
                    return false;
                };
                let (start, end) = self.intervals[candidate];
                start <= self.intervals[position].0 && self.intervals[position].0 < end
            }
        }
        #[cfg(test)]
        mod tree_tests {
            use super::tree_intervals;

            #[test]
            fn subtree_intervals_match_parent_walk() {
                assert!(tree_intervals(&[]).is_empty());
                let mut trees = vec![vec![0]];
                for count in 1..=8 {
                    for parents in &trees {
                        let intervals = tree_intervals(parents);
                        for (candidate, &(start, end)) in intervals.iter().enumerate() {
                            for (descendant, &(entry, _)) in intervals.iter().enumerate() {
                                let mut position = descendant;
                                let expected = loop {
                                    if position == candidate {
                                        break true;
                                    }
                                    if position == 0 {
                                        break false;
                                    }
                                    position = parents[position];
                                };
                                assert_eq!(
                                    start <= entry && entry < end,
                                    expected,
                                    "parents={parents:?}, candidate={candidate}, descendant={descendant}"
                                );
                            }
                        }
                    }
                    trees = trees
                        .into_iter()
                        .flat_map(|parents| {
                            (0..count).map(move |parent| {
                                let mut next = parents.clone();
                                next.push(parent);
                                next
                            })
                        })
                        .collect();
                }
                let parents: Vec<_> = (0_usize..8192)
                    .map(|position| position.saturating_sub(1))
                    .collect();
                let intervals = tree_intervals(&parents);
                for (position, interval) in intervals.iter().enumerate() {
                    assert_eq!(*interval, (position, 8192));
                }
            }
        }
    }
    pub(crate) mod structural_cases {
        use super::super::BlockId;
        use super::{AbstractOperationPlan, PsiOptimizationFunction, TargetControlGraph};
        use crate::legalization::scalar_graph_input::aggregate_results::{
            block_home_layout, result_home, type_home_layout,
        };
        use crate::legalization::scalar_graph_input::scalar_shape;
        use crate::legalization::scalar_graph_input::structural_case::dispatch_projection;
        use crate::legalization::scalar_graph_input::target::control_flow::sources;
        use semantic_vocabulary::PlaceId;
        use target_operations::{
            TargetControlCaseSuccessor, TargetStructuralCaseSource,
            TargetStructuralHomeRequirement, TargetStructuralParameter,
        };

        pub(crate) fn home_available(
            graph: &TargetControlGraph,
            optimized: &PsiOptimizationFunction,
            plan: &AbstractOperationPlan,
            block: BlockId,
            source: &TargetStructuralHomeRequirement,
            expected_source: PlaceId,
        ) -> bool {
            if source.place() != expected_source {
                return false;
            }
            match &source.origin {
                target_operations::TargetStructuralHomeOrigin::OperationResult { .. } => {
                    let mut producers =
                        graph.blocks.iter().flat_map(|candidate| {
                            candidate.operations.iter().filter_map(move |operation| {
                                let (producer, result) = operation.structural_result()?;
                                if result.place != expected_source {
                                    return None;
                                }
                                let expected =
                                    result_home(optimized, expected_source, plan).ok()?;
                                if !matches!(&expected.origin,
                                target_operations::TargetStructuralHomeOrigin::OperationResult {
                                    operation, result: declaration,
                                } if *operation == producer && declaration == result)
                                {
                                    return None;
                                }
                                if operation.structural_result_home().is_some_and(
                                    |(identity, home)| identity != producer || home != &expected,
                                ) {
                                    return None;
                                }
                                Some((candidate.block, expected))
                            })
                        });
                    let Some((producer_block, home)) = producers.next() else {
                        return false;
                    };
                    if producers.next().is_some()
                        || source != &home
                        || (producer_block != block
                            && !sources::dominates(optimized, producer_block, block))
                    {
                        return false;
                    }
                }
                target_operations::TargetStructuralHomeOrigin::BlockParameter {
                    block: owner,
                    declaration,
                } => {
                    let Some(original) = optimized
                        .block(*owner)
                        .and_then(|candidate| candidate.structural_parameter(expected_source))
                    else {
                        return false;
                    };
                    let Some(target_parameter) = graph
                        .blocks
                        .iter()
                        .find(|candidate| candidate.block == *owner)
                        .and_then(|candidate| candidate.structural_parameter(expected_source))
                    else {
                        return false;
                    };
                    if declaration != original
                        || target_parameter != original
                        || declaration.access != terminal_psi::StructuralAccess::Owned
                        || (*owner != block && !sources::dominates(optimized, *owner, block))
                    {
                        return false;
                    }
                    if block_home_layout(declaration, plan).ok().as_ref() != Some(&source.layout) {
                        return false;
                    }
                }
            }
            true
        }

        fn parameter_available(
            graph: &TargetControlGraph,
            optimized: &PsiOptimizationFunction,
            plan: &AbstractOperationPlan,
            parameter: &TargetStructuralParameter,
            layout: &target_operations::TargetStructuralHomeLayout,
            expected_source: PlaceId,
        ) -> bool {
            let Some(declaration) = optimized.structural_parameter(expected_source) else {
                return false;
            };
            parameter_corresponds(graph, parameter, declaration, expected_source)
                && block_home_layout(declaration, plan).ok().as_ref() == Some(layout)
                && layout.shape() == parameter.shape
        }

        fn borrowed_parameter_available(
            graph: &TargetControlGraph,
            optimized: &PsiOptimizationFunction,
            plan: &AbstractOperationPlan,
            parameter: &TargetStructuralParameter,
            layout: &target_operations::TargetStructuralHomeLayout,
            expected_source: PlaceId,
        ) -> bool {
            let Some(declaration) = optimized.structural_parameter(expected_source) else {
                return false;
            };
            matches!(
                declaration.access,
                terminal_psi::StructuralAccess::SharedBorrow
                    | terminal_psi::StructuralAccess::MutableBorrow
            ) && declaration.multiplicity != terminal_psi::StructuralMultiplicity::Linear
                && parameter_corresponds(graph, parameter, declaration, expected_source)
                && parameter.shape.class == calling_conventions::ValueClass::BorrowedReference
                && type_home_layout(parameter.structural_type, plan)
                    .ok()
                    .as_ref()
                    == Some(layout)
        }

        pub(crate) fn matches(
            graph: &TargetControlGraph,
            optimized: &PsiOptimizationFunction,
            plan: &AbstractOperationPlan,
            block: BlockId,
            source: &TargetStructuralCaseSource,
            path: &[terminal_psi::StructuralPathSegment],
            byte_offset: u32,
            layout: &calling_conventions::ConventionalSumLayout,
            cases: &[TargetControlCaseSuccessor],
            expected_source: PlaceId,
            expected_cases: &[abstract_operations::AbstractStructuralCaseSuccessor],
        ) -> bool {
            let available = match source {
                TargetStructuralCaseSource::Home(home) => {
                    home_available(graph, optimized, plan, block, home, expected_source)
                }
                TargetStructuralCaseSource::Parameter { parameter, layout } => {
                    parameter_available(graph, optimized, plan, parameter, layout, expected_source)
                }
                TargetStructuralCaseSource::BorrowedParameter { parameter, layout } => {
                    borrowed_parameter_available(
                        graph,
                        optimized,
                        plan,
                        parameter,
                        layout,
                        expected_source,
                    )
                }
                TargetStructuralCaseSource::BorrowedBlockParameter { parameter, layout } => {
                    let Some(declaration) = optimized
                        .block_structural_parameters()
                        .find(|declaration| declaration.place == expected_source)
                    else {
                        return false;
                    };
                    matches!(
                        declaration.access,
                        terminal_psi::StructuralAccess::SharedBorrow
                            | terminal_psi::StructuralAccess::MutableBorrow
                    ) && declaration.multiplicity != terminal_psi::StructuralMultiplicity::Linear
                        && parameter.place == expected_source
                        && parameter.structural_type == declaration.structural_type
                        && parameter.access == declaration.access
                        && parameter.multiplicity == declaration.multiplicity
                        && parameter.projected_qualifications
                            == declaration.projected_qualifications
                        && type_home_layout(parameter.structural_type, plan)
                            .ok()
                            .as_ref()
                            == Some(layout)
                }
            };
            if !available {
                return false;
            }
            let Ok((selected, expected_offset, _, expected_layout)) =
                dispatch_projection(optimized, expected_source, path, plan)
            else {
                return false;
            };
            if expected_offset != byte_offset || expected_layout != *layout {
                return false;
            }
            let Some(declaration) =
                terminal_psi::find_structural_type(&graph.structural_types, selected)
            else {
                return false;
            };
            let declared_cases: &[terminal_psi::StructuralCaseDeclaration] =
                match &declaration.shape {
                    terminal_psi::StructuralTypeShape::Sum { cases }
                    | terminal_psi::StructuralTypeShape::Mixed { cases, .. } => cases,
                    _ => return false,
                };
            if cases.len() != expected_cases.len()
                || cases.len() != declared_cases.len()
                || cases.len() != layout.cases.len()
            {
                return false;
            }
            cases
                .iter()
                .zip(expected_cases)
                .zip(declared_cases)
                .enumerate()
                .all(|(case_position, ((actual, expected), declared))| {
                    let Some(target) = graph
                        .blocks
                        .iter()
                        .find(|block| block.block == expected.target)
                    else {
                        return false;
                    };
                    if actual.psi_edge != expected.psi_edge
                        || actual.target != expected.target
                        || actual.case != expected.case
                        || actual.case != declared.id
                        || i32::try_from(case_position).ok() != Some(actual.case_tag)
                        || actual.cleanup_actions != expected.cleanup_actions
                        || actual.payloads.len() != expected.payloads.len()
                        || actual.payloads.len() != target.parameters.len()
                        || actual.structural_bindings != expected.structural_bindings
                        || actual.structural_bindings.len() != target.structural_parameters.len()
                    {
                        return false;
                    }
                    let fields = declared
                        .fields
                        .iter()
                        .filter(|field| !field.relevance.is_erased())
                        .collect::<Vec<_>>();
                    if fields.len() != layout.cases[case_position].fields.len() {
                        return false;
                    }
                    actual
                        .payloads
                        .iter()
                        .zip(&expected.payloads)
                        .zip(&target.parameters)
                        .all(|((actual, expected), parameter)| {
                            let Some(field_position) =
                                fields.iter().position(|field| field.id == expected.field)
                            else {
                                return false;
                            };
                            let field_layout = layout.cases[case_position].fields[field_position];
                            actual.field == expected.field
                                && actual.parameter.block == target.block
                                && actual.parameter.value == expected.parameter
                                && actual.parameter.value == parameter.value
                                && actual.parameter.scalar_type == expected.scalar_type
                                && actual.parameter.scalar_type == parameter.scalar_type
                                && fields[field_position].field_type.scalar_type()
                                    == Some(expected.scalar_type)
                                && scalar_shape(expected.scalar_type) == Some(field_layout.shape)
                                && byte_offset.checked_add(field_layout.byte_offset)
                                    == Some(actual.field_byte_offset)
                        })
                })
        }

        fn parameter_corresponds(
            graph: &TargetControlGraph,
            parameter: &TargetStructuralParameter,
            declaration: &terminal_psi::StructuralParameterDeclaration,
            expected_source: PlaceId,
        ) -> bool {
            parameter.place == expected_source
                && graph
                    .parameters
                    .iter()
                    .filter(|candidate| candidate.place == expected_source)
                    .eq(std::iter::once(parameter))
                && parameter.structural_type == declaration.structural_type
                && parameter.access == declaration.access
                && parameter.multiplicity == declaration.multiplicity
                && parameter.projected_qualifications == declaration.projected_qualifications
        }
    }

    pub(crate) fn validate(
        function: &TargetFunction,
        graph: &TargetControlGraph,
        abstracted: &AbstractFunction,
        function_inputs: &crate::legalization_function_inputs::FunctionInputs<'_>,
        descriptor_inputs: &crate::legalization_descriptor_inputs::DescriptorInputs<'_>,
    ) -> Result<crate::legalization::scalar_graph_input::BlockEntryCustody, LegalizationError> {
        let native = descriptor_inputs.native();
        let plan = descriptor_inputs.plan();
        let unit = descriptor_inputs.unit();
        let optimized = function_inputs.function();
        if (function.attachment != optimized.attachment
            || function.nominal_cleanup_subject != optimized.nominal_cleanup_subject)
            || graph.structural_types != plan.structural_types
            || graph.structural_types != unit.structural_types
            || graph.entry != optimized.entry
            || graph.blocks.len() != optimized.blocks.len()
        {
            return Err(LegalizationError::custody());
        }
        match optimized.result {
            _ if crate::legalization::scalar_graph_input::aggregate_results::uses(optimized) => {
                crate::legalization::scalar_graph_input::aggregate_results::header(
                    function,
                    abstracted,
                    optimized,
                    native.target,
                    plan,
                )?;
            }
            AbstractFunctionResult::Unit => {}
            AbstractFunctionResult::Scalar(_) if function.mixed_structural_scalar_abi.is_some() => {
                let expected = crate::legalization::scalar_graph_input::byte_views::validate(
                    function,
                    abstracted,
                    optimized,
                    native.target,
                    plan,
                )?;
                let abi = function
                    .mixed_structural_scalar_abi
                    .as_ref()
                    .ok_or(LegalizationError::custody())?;
                if graph.call_plan != expected
                    || graph.scalar_parameters != abi.scalar_parameters
                    || graph.parameters != abi.structural_parameters
                {
                    return Err(LegalizationError::custody());
                }
            }
            AbstractFunctionResult::Scalar(_) => {
                let expected = crate::legalization::scalar_graph_input::header::function_abi(
                    native.target,
                    function,
                    abstracted,
                    optimized,
                )?;
                let Some(abi) = &function.scalar_abi else {
                    return Err(LegalizationError::custody());
                };
                if graph.call_plan != expected || graph.scalar_parameters != abi.parameters {
                    return Err(LegalizationError::custody());
                }
                if !graph.parameters.is_empty()
                    || graph
                        .blocks
                        .iter()
                        .any(|block| !block.structural_parameters.is_empty())
                {
                    return Err(LegalizationError::custody());
                }
            }
            _ => return Err(LegalizationError::custody()),
        }
        let block_entries = block_entry_states(optimized, descriptor_inputs)?;
        let source_inputs = sources::SourceInputs::new(graph, optimized);
        for (block, source) in graph.blocks.iter().zip(&optimized.blocks) {
            let body_len = block
                .operations
                .iter()
                .position(|row| matches!(row, TargetUnitOperation::CleanupCall { .. }))
                .unwrap_or(block.operations.len());
            let (body, cleanup_calls) = block.operations.split_at(body_len);

            if block.block != source.id
                || block.structural_parameters != source.structural_parameters
                || block.parameters.len() != source.parameters.len()
                || block
                    .parameters
                    .iter()
                    .zip(&source.parameters)
                    .any(|(actual, expected)| {
                        actual.value != expected.value || actual.scalar_type != expected.scalar_type
                    })
                || (source.id == optimized.entry && !source.parameters.is_empty())
                || source.nodes.len() != body.len() + 1
            {
                return Err(LegalizationError::custody());
            }
            let source_nodes: Vec<_> = source.nodes.iter().collect();
            let mut available = source_inputs.available(block.block);
            let mut custody = block_entries
                .get(&block.block)
                .cloned()
                .ok_or(LegalizationError::custody())?;
            for (node_index, (operation, node)) in body.iter().zip(&source_nodes).enumerate() {
                custody.set_position(block.block, node_index)?;
                if matches!(operation, TargetUnitOperation::Return { .. }) {
                    return Err(LegalizationError::custody());
                }
                super::unit::validate_operation(
                    function,
                    operation,
                    &node.operation,
                    &graph.scalar_parameters,
                    &graph.parameters,
                    &mut available,
                    &custody,
                    function_inputs,
                    native,
                    plan,
                    unit,
                )?;
                apply(&mut custody, node, optimized, native, plan, unit)?;
            }
            custody.set_position(block.block, body.len())?;
            let source_terminator = &source
                .nodes
                .last()
                .ok_or(LegalizationError::custody())?
                .operation;
            crate::legalization::scalar_graph_input::cleanup_calls::validate_target(
                cleanup_calls,
                source_terminator,
                function_inputs,
                native,
                plan,
                unit,
            )?;
            apply_terminator(&mut custody, source_terminator, optimized, plan)?;
            let matches = match (&block.terminator, source_terminator) {
                (
                    TargetControlTerminator::InvokeNoNormal { .. },
                    AbstractOperation::InvokeNoNormal { .. },
                ) => {
                    no_normal::validate(
                        &block.terminator,
                        source_terminator,
                        &available,
                        &custody,
                        optimized,
                        native,
                        plan,
                        unit,
                    )?;
                    true
                }

                (
                    TargetControlTerminator::ReturnStructural {
                        psi_edge,
                        source,
                        cleanup_actions,
                    },
                    AbstractOperation::ReturnStructural {
                        psi_edge: expected_edge,
                        source: expected_source,
                        trivial_affine_locals,
                        cleanup_actions: expected_cleanup,
                        ..
                    },
                ) => {
                    psi_edge == expected_edge
                        && cleanup_actions == expected_cleanup
                        && trivial_affine_locals.is_empty()
                        && match source {
                            target_operations::TargetStructuralReturnSource::SharedDescriptor { place } => {
                                place == expected_source
                                    && crate::legalization::scalar_graph_input::byte_views::descriptor_return(
                                        optimized, *place, &plan.structural_types,
                                    )
                                    && graph.call_plan.result.as_ref().is_some_and(|result|
                                        result.shape == calling_conventions::ValueShape::integer(16, 8))
                                    && (optimized.structural_parameter(*place).is_some()
                                        || optimized.blocks.iter().any(|producer| {
                                            (producer.id == block.block || sources::dominates(
                                                optimized, producer.id, block.block))
                                                && (producer.structural_parameter(*place).is_some()
                                                    || producer.nodes.iter().any(|node|
                                                        node.operation.structural_result().is_some_and(
                                                            |(_, result)| result.place == *place)
                                                        || matches!(&node.operation,
                                                            AbstractOperation::EstablishByteSequenceLiteral {
                                                                place: declaration, ..
                                                            } if declaration.id == *place)))
                                        }))
                            }
                            target_operations::TargetStructuralReturnSource::Parameter(parameter) => {
                                optimized.structural_parameters.iter().any(|semantic| {
                                    semantic.place == *expected_source
                                && semantic.access == terminal_psi::StructuralAccess::Owned
                                && optimized.result.structural().is_some_and(|result| {
                                    result.structural_type == semantic.structural_type
                                        && result.multiplicity == semantic.multiplicity
                                })
                                }) && parameter.place == *expected_source
                                    && graph
                                        .parameters
                                        .iter()
                                        .any(|expected| expected == parameter)
                                    && graph
                                        .call_plan
                                        .result
                                        .as_ref()
                                        .is_some_and(|result| {
                                            result.shape == parameter.shape
                                        })
                            }
                            target_operations::TargetStructuralReturnSource::Home(source) => {
                                crate::legalization::scalar_graph_input::aggregate_results::result_home(
                                    optimized,
                                    *expected_source,
                                    plan,
                                )
                                .is_ok_and(|expected| expected == *source)
                                    && structural_cases::home_available(
                                        graph,
                                        optimized,
                                        plan,
                                        block.block,
                                        source,
                                        *expected_source,
                                    )
                            }
                        }
                }
                (
                    TargetControlTerminator::ExternalCompletion {
                        psi_edge,
                        boundary,
                        kind,
                        arguments,
                        binding,
                        structural_arguments,
                        requirement_obligations,
                        frontier_lower_bound,
                    },
                    AbstractOperation::ExternalCompletion {
                        psi_edge: expected_edge,
                        boundary: expected_boundary,
                        kind: expected_kind,
                        arguments: expected_arguments,
                        structural_arguments: expected_structural,
                        requirement_obligations: expected_obligations,
                        frontier_lower_bound: expected_frontier,
                    },
                ) => {
                    let declaration = plan
                        .boundary_machine(*boundary)
                        .ok_or(LegalizationError::custody())?;
                    let parameters = declaration
                        .scalar_parameters
                        .iter()
                        .map(|scalar_type| {
                            crate::legalization::scalar_graph_input::scalar_shape(*scalar_type).ok_or(LegalizationError::custody())
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let signature = calling_conventions::CallSignature {
                        parameters,
                        result: None,
                    };
                    let expected_plan = match binding {
                        target_operations::ExternalCompletionBinding::Boundary(
                            target_operations::BoundarySettlementBinding {
                                realization:
                                    target_operations::BoundarySettlementRealization::NormalizedForeignCall(
                                        foreign,
                                    ),
                                ..
                            },
                        ) => calling_conventions::validate_boundary_entry_plan(
                            foreign.boundary_entry_plan.clone(),
                            &signature,
                        )
                        .map_err(|_| LegalizationError::custody())?
                        .plan()
                        .call
                        .clone(),
                        _ => calling_conventions::evaluate_call_plan(
                            calling_conventions::CallingPolicy::native_for_target(native.target),
                            &signature,
                        )
                        .map_err(|_| LegalizationError::custody())?,
                    };
                    psi_edge == expected_edge
                        && boundary == expected_boundary
                        && kind == expected_kind
                        && structural_arguments == expected_structural
                        && requirement_obligations == expected_obligations
                        && frontier_lower_bound == expected_frontier
                        && match binding {
                            target_operations::ExternalCompletionBinding::Boundary(binding) => {
                                binding.boundary == *boundary
                            }
                            target_operations::ExternalCompletionBinding::Checked {
                                callee,
                                origin,
                                call_plan,
                            } => {
                                *call_plan == expected_plan
                                    && plan.function(*callee).is_some_and(|candidate| {
                                        candidate.result
                                            == abstract_operations::AbstractFunctionResult::Unit
                                            && candidate
                                                .parameters
                                                .iter()
                                                .map(|row| row.scalar_type)
                                                .eq(declaration.scalar_parameters.iter().copied())
                                            && candidate.structural_parameters
                                                == declaration.structural_parameters
                                    })
                                    && match origin {
                                        target_operations::NativeCallOrigin::InstalledProvider {
                                            boundary: required,
                                            provider,
                                            ..
                                        } => {
                                            required == boundary
                                                && provider.candidate == *callee
                                                && plan.provider_candidates.contains(provider)
                                        }
                                        target_operations::NativeCallOrigin::TargetFamily {
                                            boundary: required,
                                            member,
                                            ..
                                        } => required == boundary && member == callee,
                                        target_operations::NativeCallOrigin::Authored
                                        | target_operations::NativeCallOrigin::ProgramEntry {
                                            ..
                                        } => false,
                                    }
                            }
                        }
                        && declaration.completion
                            == semantic_vocabulary::BoundaryCompletion::External(*kind)
                        && declaration.result.is_unit()
                        && arguments.len() == expected_arguments.len()
                        && arguments.len() == expected_plan.parameters.len()
                        && structural_arguments.len() == declaration.structural_parameters.len()
                        && arguments
                            .iter()
                            .zip(expected_arguments)
                            .zip(&expected_plan.parameters)
                            .zip(&declaration.scalar_parameters)
                            .enumerate()
                            .all(
                                |(position, (((argument, value), placement), scalar_type))| {
                                    argument.parameter_index as usize == position
                                        && argument.placement == *placement
                                        && argument.source.source_value() == *value
                                        && argument.source.scalar_type() == *scalar_type
                                        && available
                                            .matching(*value)
                                            .rev()
                                            .find(|(candidate, _)| candidate == value)
                                            .is_some_and(|(_, source)| source == &argument.source)
                                },
                            )
                }
                (
                    TargetControlTerminator::Crash {
                        psi_edge,
                        cause,
                        site_guard,
                        frontier_lower_bound,
                    },
                    AbstractOperation::Crash {
                        psi_edge: expected_edge,
                        cause: expected_cause,
                        site_guard: expected_guard,
                        frontier_lower_bound: expected_frontier,
                    },
                ) => {
                    psi_edge == expected_edge
                        && cause == expected_cause
                        && site_guard == expected_guard
                        && frontier_lower_bound == expected_frontier
                }
                (
                    TargetControlTerminator::ReturnScalar {
                        psi_edge,
                        source_value,
                        expression,
                        cleanup_actions,
                    },
                    AbstractOperation::Return {
                        psi_edge: expected_edge,
                        result,
                        value,
                        scalar_type: expected_type,
                        cleanup_actions: expected_cleanup,
                    },
                ) => {
                    matches!(optimized.result, AbstractFunctionResult::Scalar(declaration)
                        if declaration.value == *result && declaration.scalar_type == *expected_type)
                        && psi_edge == expected_edge
                        && source_value == value
                        && cleanup_actions == expected_cleanup
                        && (cleanup_actions.is_empty()
                            || crate::legalization::scalar_graph_input::aggregate_results::cleanup(
                                function_inputs,
                                cleanup_actions,
                                &plan.structural_types,
                            ))
                        && available.matching(*value).any(|(identity, source)| {
                            identity == value && source.scalar_type() == *expected_type
                        })
                        && {
                            let checker = Checker {
                                function,
                                available: Some(&available),
                                optimized: function_inputs.function(),
                                function_inputs,
                                types: unit.structural_types.as_slice(),
                            };
                            match expression {
                                TargetScalarExpression::IeeeFloatElementViewRead { .. } => false,
                                TargetScalarExpression::IeeeFloat(source) => {
                                    matches!(expected_type, ScalarType::IeeeFloat(_))
                                        && source.scalar_type() == *expected_type
                                        && source.source_value() == *value
                                        && available
                                            .matching(*value)
                                            .map(|(_, candidate)| candidate)
                                            .eq(std::iter::once(source))
                                }
                                TargetScalarExpression::Integer {
                                    scalar_type,
                                    expression,
                                } => {
                                    *expected_type == ScalarType::Integer(*scalar_type)
                                        && checker.integer_source(expression, *value, &[])
                                }
                                TargetScalarExpression::Boolean(expression) => {
                                    *expected_type == ScalarType::Boolean
                                        && checker.boolean_source(expression, *value, &[])
                                }
                            }
                        }
                }
                (
                    TargetControlTerminator::StructuralCase {
                        source,
                        path,
                        byte_offset,
                        layout,
                        cases,
                    },
                    AbstractOperation::StructuralCase {
                        source: expected_source,
                        path: expected_path,
                        cases: expected_cases,
                    },
                ) => {
                    path == expected_path
                        && structural_cases::matches(
                            graph,
                            optimized,
                            plan,
                            block.block,
                            source,
                            path,
                            *byte_offset,
                            layout,
                            cases,
                            *expected_source,
                            expected_cases,
                        )
                }
                (
                    TargetControlTerminator::Return {
                        psi_edge,
                        cleanup_actions,
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: expected,
                        cleanup_actions: cleanup,
                    },
                ) => {
                    optimized.result == AbstractFunctionResult::Unit
                        && psi_edge == expected
                        && cleanup_actions == cleanup
                        && (cleanup.is_empty()
                            || crate::legalization::scalar_graph_input::read_byte::cleanup(optimized, cleanup)
                            || crate::legalization::scalar_graph_input::aggregate_results::cleanup(
                                function_inputs,
                                cleanup,
                                &plan.structural_types,
                            ))
                }
                (
                    TargetControlTerminator::Jump { successor },
                    AbstractOperation::Jump {
                        psi_edge,
                        target,
                        bindings,
                        structural_bindings,
                        cleanup_actions,
                    },
                ) => {
                    successor.psi_edge == *psi_edge
                        && successor.target == *target
                        && successor.bindings == *bindings
                        && successor.structural_bindings == *structural_bindings
                        && crate::legalization::scalar_graph_input::address_joins::edge_bindings(
                            optimized,
                            block.block,
                            *target,
                            structural_bindings,
                            plan,
                        )
                        && edge_cleanup_matches(
                            function_inputs,
                            &successor.cleanup_actions,
                            cleanup_actions,
                            &plan.structural_types,
                        )
                }
                (
                    TargetControlTerminator::Conditional {
                        condition_source,
                        condition,
                        when_true,
                        when_false,
                    },
                    AbstractOperation::Conditional {
                        condition: expected,
                        when_true: expected_true,
                        when_false: expected_false,
                    },
                ) => {
                    condition_source == expected
                        && (Checker {
                            function,
                            available: Some(&available),
                            optimized: function_inputs.function(),
                            function_inputs,
                            types: unit.structural_types.as_slice(),
                        })
                        .boolean_source(condition, *expected, &[])
                        && successor_matches(function_inputs, block.block, plan, when_true, expected_true)
                        && successor_matches(function_inputs, block.block, plan, when_false, expected_false)
                }
                _ => false,
            };
            if !matches {
                return Err(LegalizationError::custody());
            }
        }
        Ok(block_entries)
    }

    fn successor_matches(
        function_inputs: &crate::legalization_function_inputs::FunctionInputs<'_>,
        block: semantic_vocabulary::BlockId,
        plan: &AbstractOperationPlan,
        target: &TargetControlSuccessor,
        source: &abstract_operations::AbstractSuccessor,
    ) -> bool {
        let function = function_inputs.function();
        target.psi_edge == source.psi_edge
            && target.target == source.target
            && target.bindings == source.bindings
            && target.structural_bindings == source.structural_bindings
            && crate::legalization::scalar_graph_input::address_joins::edge_bindings(
                function,
                block,
                source.target,
                &source.structural_bindings,
                plan,
            )
            && edge_cleanup_matches(
                function_inputs,
                &target.cleanup_actions,
                &source.cleanup_actions,
                &plan.structural_types,
            )
    }

    fn edge_cleanup_matches(
        function_inputs: &crate::legalization_function_inputs::FunctionInputs<'_>,
        actions: &[terminal_psi::TerminalAffineCleanupAction],
        expected: &[terminal_psi::TerminalAffineCleanupAction],
        structural_types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> bool {
        actions == expected
            && crate::legalization::scalar_graph_input::aggregate_results::cleanup(
                function_inputs,
                actions,
                structural_types,
            )
    }
}
pub(crate) mod element_view {
    use super::Checker;
    use crate::legalization::scalar_graph_input::byte_views::descriptor_result;
    use crate::structural_inputs::structural_reference_input::{
        element_leaf_geometry, projected_collection, typed_collection_layout_matches,
    };
    use abstract_operations::AbstractOperation;
    use abstract_operations::control_flow::address_joins::is_address_join_in;
    use semantic_vocabulary::{PlaceId, ValueId};
    use target_operations::{TargetElementView, TargetStructuralArgumentSource};
    use terminal_psi::StructuralTypeShape;

    impl Checker<'_> {
        pub(crate) fn element_view(
            &self,
            view: &TargetElementView,
            source: PlaceId,
            aliases: &[(ValueId, ValueId)],
        ) -> bool {
            self.element_view_custody(view, source, aliases).is_ok()
        }

        pub(crate) fn element_view_custody(
            &self,
            view: &TargetElementView,
            source: PlaceId,
            aliases: &[(ValueId, ValueId)],
        ) -> Result<(), &'static str> {
            self.element_view_access(
                view,
                source,
                aliases,
                terminal_psi::StructuralAccess::SharedBorrow,
            )
        }

        fn element_view_access(
            &self,
            view: &TargetElementView,
            source: PlaceId,
            aliases: &[(ValueId, ValueId)],
            requested: terminal_psi::StructuralAccess,
        ) -> Result<(), &'static str> {
            use terminal_psi::StructuralAccess as Access;
            let Some(actual) = optimization_unit_semantics::structural_place_backing_access(
                self.optimized,
                source,
                self.types.iter(),
            ) else {
                return Err("view destination has no admitted backing access");
            };
            if !matches!(
                (actual, requested),
                (
                    Access::SharedBorrow | Access::MutableBorrow,
                    Access::SharedBorrow
                ) | (Access::MutableBorrow, Access::MutableBorrow)
            ) {
                return Err("view destination backing cannot supply requested access");
            }
            let access = |access| access == actual;
            let valid = match view {
                TargetElementView::Result {
                    psi_operation,
                    place,
                    structural_type,
                } => {
                    *place == source
                        && descriptor_result(self.optimized, source, self.types)
                            == Some((*psi_operation, *structural_type))
                }
                TargetElementView::BlockParameter {
                    block,
                    place,
                    structural_type,
                } => {
                    *place == source
                        && self.optimized.blocks.iter().any(|owner| {
                            owner.id == *block
                                && owner.id != self.optimized.entry
                                && owner.structural_parameters.iter().any(|parameter| {
                                    parameter.place == *place
                                        && parameter.structural_type == *structural_type
                                        && terminal_semantics::descriptor_parameter_access(
                                            parameter,
                                            self.types.iter(),
                                        )
                                        .is_some_and(access)
                                })
                        })
                }
                TargetElementView::Parameter { place, placement } => {
                    *place == source
                        && crate::legalization::scalar_graph_input::structural_parameters(self.function).is_some_and(
                            |parameters| {
                                parameters.iter().any(|parameter| {
                                    parameter.place == source
                                        && self.optimized.arrival_parameter(source).is_some_and(
                                            |semantic| {
                                                semantic.structural_type == parameter.structural_type
                                                    && semantic.access == parameter.access
                                                    && semantic.multiplicity == parameter.multiplicity
                                                    && semantic.projected_qualifications
                                                        == parameter.projected_qualifications
                                                    && terminal_semantics::descriptor_parameter_access(
                                                        semantic,
                                                        self.types.iter(),
                                                    )
                                                    .is_some_and(access)
                                            },
                                        )
                                        && &parameter.placement == placement
                                })
                            },
                        )
                }
                TargetElementView::Established {
                    psi_operation,
                    place,
                    structural_type,
                    element,
                    root_structural_type,
                    source_byte_offset,
                    indices,
                    extent,
                    region,
                    element_stride,
                    source: established_source,
                } => {
                    let Some((argument, expected_region)) =
                        self.optimized.block_nodes().find_map(|node| {
                            let AbstractOperation::EstablishElementView {
                                psi_operation: operation,
                                result,
                                destination,
                                source: argument,
                                element: expected_element,
                                region: expected_region,
                            } = &node.operation
                            else {
                                return None;
                            };
                            (operation == psi_operation
                                && result.structural_type == *structural_type
                                && matches!(result.multiplicity,
                                    terminal_psi::StructuralMultiplicity::Unrestricted
                                        | terminal_psi::StructuralMultiplicity::Affine)
                                && result.qualifications.is_empty()
                                && result.projected_qualifications.iter().all(|row| row.is_element_membership())
                                && result.claims.is_empty()
                                && *destination == *place
                                && *expected_element == *element)
                                .then_some((argument, *expected_region))
                        })
                    else {
                        return Err("view result does not match an exact declared establishment");
                    };
                    let backing = match established_source {
                        TargetStructuralArgumentSource::Placement(placement) => {
                            self.optimized
                                .structural_parameters
                                .iter()
                                .any(|parameter| {
                                    parameter.place == argument.place
                                        && parameter.structural_type == *root_structural_type
                                })
                                && crate::legalization::scalar_graph_input::structural_parameters(self.function).is_some_and(
                                    |parameters| {
                                        parameters.iter().any(|parameter| {
                                            parameter.place == argument.place
                                                && &parameter.placement == placement
                                        })
                                    },
                                )
                        }
                        TargetStructuralArgumentSource::StructuralHome {
                            psi_operation: producer,
                        } => self.local_home_precedes(
                            *producer,
                            *psi_operation,
                            argument.place,
                            *root_structural_type,
                        ),
                        TargetStructuralArgumentSource::BlockParameter { block, place } => {
                            *place == argument.place
                                && self.block_backing_precedes(
                                    *block,
                                    *place,
                                    *psi_operation,
                                    *root_structural_type,
                                    argument.access,
                                )
                        }
                        _ => false,
                    };
                    if *place != source {
                        return Err("view custody names another destination");
                    }
                    if !access(argument.access) {
                        return Err("view backing access differs from its source loan");
                    }
                    if !backing {
                        return Err("view backing does not match a preceding producer or parameter");
                    }
                    if !optimization_unit_semantics::descriptor_argument_accessible(
                        self.optimized,
                        argument,
                        self.types,
                        argument.access,
                    ) {
                        return Err("view source projection has no admitted descriptor access");
                    }
                    let matching_region = match (region, expected_region) {
                        (
                            target_operations::TargetElementViewRegion::WholeCollection,
                            terminal_psi::ElementViewRegion::WholeCollection,
                        ) => true,
                        (
                            target_operations::TargetElementViewRegion::WholeRepresentation {
                                source_length,
                                obligation,
                            },
                            terminal_psi::ElementViewRegion::WholeRepresentation {
                                source_length: expected_length,
                                obligation: expected_obligation,
                            },
                        ) => *source_length == expected_length && *obligation == expected_obligation,
                        (
                            target_operations::TargetElementViewRegion::RepresentationBytes {
                                start,
                                end,
                                obligation,
                            },
                            terminal_psi::ElementViewRegion::RepresentationBytes {
                                start: expected_start,
                                end: expected_end,
                                obligation: expected_obligation,
                            },
                        ) => {
                            *obligation == expected_obligation
                                && self.integer_source(start, expected_start, aliases)
                                && self.integer_source(end, expected_end, aliases)
                        }
                        _ => false,
                    };
                    if !matching_region {
                        return Err("view region differs from its establishment");
                    }
                    self.established_element_window(
                        *root_structural_type,
                        &argument.path,
                        *element,
                        *source_byte_offset,
                        indices,
                        *extent,
                        *element_stride,
                        expected_region,
                        argument.access,
                    )?;
                    true
                }
                TargetElementView::Subslice {
                    psi_operation,
                    place,
                    source: original,
                    indices,
                    start,
                    end,
                    length,
                    obligation,
                } => {
                    *place == source
                        && self.optimized.block_nodes().any(|node| {
                            let AbstractOperation::ElementViewSubslice {
                                psi_operation: operation,
                                result,
                                source: expected_source,
                                start: expected_start,
                                end: expected_end,
                                length: expected_length,
                                obligation: expected_obligation,
                            } = &node.operation
                            else {
                                return false;
                            };
                            operation == psi_operation
                            && result.place == source
                            && matches!(
                                (actual, result.multiplicity),
                                (
                                    Access::SharedBorrow,
                                    terminal_psi::StructuralMultiplicity::Unrestricted
                                ) | (
                                    Access::MutableBorrow,
                                    terminal_psi::StructuralMultiplicity::Affine
                                )
                            )
                            && result.qualifications.is_empty()
                            && result
                                .projected_qualifications
                                .iter()
                                .all(|row| row.is_element_membership())
                            && result.claims.is_empty()
                            && self.types.iter().any(|declaration| {
                                declaration.id == result.structural_type
                                    && matches!(
                                        declaration.shape,
                                        StructuralTypeShape::ElementView { .. }
                                            | StructuralTypeShape::ByteSequence(
                                                terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                                            )
                                    )
                            })
                            && length == expected_length
                            && obligation == expected_obligation
                            && original == expected_source
                            && crate::legalization::scalar_graph_input::element_views::descriptor_geometry(
                                self.optimized,
                                original,
                                self.types,
                            )
                            .is_some_and(|geometry| {
                                geometry.readable
                                    && super::unit::runtime_indices_rejoin(
                                        indices,
                                        &geometry.elements,
                                        self.available.unwrap_or(&crate::legalization_target::control_flow::sources::EMPTY),
                                    )
                            })
                            && self.integer_source(start, *expected_start, aliases)
                            && self.integer_source(end, *expected_end, aliases)
                        })
                }
            };
            valid
                .then_some(())
                .ok_or("view descriptor differs from its declared source")
        }

        fn block_backing_precedes(
            &self,
            block: semantic_vocabulary::BlockId,
            place: PlaceId,
            consumer: semantic_vocabulary::OperationId,
            root_structural_type: semantic_vocabulary::StructuralTypeId,
            requested: terminal_psi::StructuralAccess,
        ) -> bool {
            let Some(parameter) = self
                .optimized
                .block(block)
                .and_then(|entry| entry.structural_parameter(place))
            else {
                return false;
            };
            let Some((consumer_block, _)) =
                crate::legalization::scalar_graph_input::aggregate_results::node_site(
                    self.optimized,
                    |node| node.psi_operation() == Some(consumer),
                )
            else {
                return false;
            };
            let descriptor_carrier =
                terminal_semantics::view_descriptor_parameter(parameter, self.types.iter());
            let backing = parameter.access == terminal_psi::StructuralAccess::Owned
                || (block != self.optimized.entry
                    && (is_address_join_in(parameter, self.types) || descriptor_carrier)
                    && matches!(
                        (parameter.access, requested),
                        (
                            terminal_psi::StructuralAccess::SharedBorrow,
                            terminal_psi::StructuralAccess::SharedBorrow,
                        ) | (
                            terminal_psi::StructuralAccess::MutableBorrow,
                            terminal_psi::StructuralAccess::SharedBorrow
                                | terminal_psi::StructuralAccess::MutableBorrow,
                        )
                    ));
            backing
                && parameter.structural_type == root_structural_type
                && parameter.multiplicity != terminal_psi::StructuralMultiplicity::Linear
                && self.optimized.structural_places.iter().any(|declaration| {
                    declaration.id == place
                        && declaration.kind
                            == semantic_vocabulary::StructuralPlaceKind::BlockParameter {
                                block,
                                position: parameter.position,
                            }
                })
                && super::control_flow::sources::dominates(self.optimized, block, consumer_block)
        }

        fn local_home_precedes(
            &self,
            producer: semantic_vocabulary::OperationId,
            consumer: semantic_vocabulary::OperationId,
            place: PlaceId,
            root_structural_type: semantic_vocabulary::StructuralTypeId,
        ) -> bool {
            let Ok((actual, result)) =
                crate::legalization::scalar_graph_input::structural_case::source_result(
                    self.optimized,
                    place,
                )
            else {
                return false;
            };
            let site = |operation: semantic_vocabulary::OperationId| {
                crate::legalization::scalar_graph_input::aggregate_results::node_site(
                    self.optimized,
                    |node| node.psi_operation() == Some(operation),
                )
            };
            let (Some(before), Some(after)) = (site(producer), site(consumer)) else {
                return false;
            };
            actual == producer
                && result.structural_type == root_structural_type
                && matches!(
                    result.multiplicity,
                    terminal_psi::StructuralMultiplicity::Unrestricted
                        | terminal_psi::StructuralMultiplicity::Affine
                )
                && result.claims.is_empty()
                && self.optimized.structural_places.iter().any(|declaration| {
                    declaration.id == place
                        && declaration.kind
                            == semantic_vocabulary::StructuralPlaceKind::OperationResult {
                                producer,
                                structural_type: result.structural_type,
                            }
                })
                && if before.0 == after.0 {
                    before.1 < after.1
                } else {
                    super::control_flow::sources::dominates(self.optimized, before.0, after.0)
                }
        }

        fn established_element_window(
            &self,
            root_structural_type: semantic_vocabulary::StructuralTypeId,
            path: &[terminal_psi::StructuralPathSegment],
            element: semantic_vocabulary::StructuralTypeId,
            source_byte_offset: u32,
            indices: &[target_operations::TargetStructuralRuntimeIndex],
            extent: target_operations::TargetViewExtent,
            element_stride: u32,
            region: terminal_psi::ElementViewRegion,
            access: terminal_psi::StructuralAccess,
        ) -> Result<(), &'static str> {
            let (collection_path, byte_window) = match path.last() {
                Some(terminal_psi::StructuralPathSegment::FixedRange { start, end }) => {
                    (&path[..path.len() - 1], Some((*start, *end)))
                }
                _ => (path, None),
            };
            let Some(projected) =
                projected_collection(root_structural_type, collection_path, self.types)
            else {
                return Err("view source collection projection is unresolved");
            };
            let collection = projected.shape;
            let offset = projected.offset;
            let elements = projected.elements;
            if !super::unit::runtime_indices_rejoin(
                indices,
                &elements,
                self.available
                    .unwrap_or(&crate::legalization_target::control_flow::sources::EMPTY),
            ) {
                return Err("view runtime selectors do not rejoin available sources");
            }
            let Some(geometry) = terminal_semantics::element_view_shape_region_geometry(
                self.types.iter(),
                &collection,
                element,
                region,
                access,
            ) else {
                return Err("view source has no matching element-region geometry");
            };
            if region == terminal_psi::ElementViewRegion::WholeCollection
                && !typed_collection_layout_matches(&collection, element, access, self.types)
            {
                return Err("view collection and element layouts disagree");
            }
            let backing = match geometry {
                terminal_semantics::ElementViewRegionGeometry::WholeCollection(backing) => backing,
                terminal_semantics::ElementViewRegionGeometry::WholeRepresentation {
                    backing,
                    source: source_layout,
                    target: target_layout,
                } => {
                    if byte_window.is_some()
                        || access != terminal_psi::StructuralAccess::SharedBorrow
                    {
                        return Err(
                            "representation view has an unsupported source window or access",
                        );
                    }
                    let Some(length_scale) = crate::structural_inputs::structural_reference_input::whole_representation_length_scale(
                        &collection,
                        &source_layout,
                        element,
                        &target_layout,
                        self.types,
                    ) else {
                        return Err("representation view has no matching source and target length scale");
                    };
                    match backing.extent {
                        terminal_semantics::CollectionExtent::Fixed(count) => {
                            if count.checked_mul(length_scale).is_none() {
                                return Err("representation view length multiplication overflows");
                            }
                        }
                        terminal_semantics::CollectionExtent::StoredLength => {}
                        terminal_semantics::CollectionExtent::LiveLength { .. } => {
                            return Err(
                                "representation view source has an unsupported live extent",
                            );
                        }
                    }
                    backing
                }
                terminal_semantics::ElementViewRegionGeometry::RepresentationBytes {
                    byte_extent,
                    element: layout,
                } => {
                    if byte_window.is_some()
                        || layout.size != u64::from(element_stride)
                        || crate::structural_inputs::shape(element, self.types).is_none_or(
                            |shape| {
                                u64::from(shape.byte_size) != layout.size
                                    || u64::from(shape.alignment) != layout.alignment
                            },
                        )
                        || layout.leaves.iter().any(|leaf| {
                            element_leaf_geometry(element, &leaf.path, leaf.scalar_type, self.types)
                                .is_none_or(|(offset, _)| u64::from(offset) != leaf.offset)
                        })
                    {
                        return Err(
                            "representation byte view has a mismatched element layout or source window",
                        );
                    }
                    return (extent == target_operations::TargetViewExtent::Fixed(byte_extent)
                        && offset == source_byte_offset)
                        .then_some(())
                        .ok_or(
                            "representation byte view extent or offset differs from its source",
                        );
                }
            };
            if backing.storage == terminal_semantics::CollectionStorage::Borrowed
                && byte_window.is_some()
            {
                return Err("borrowed collection cannot supply a static inline window");
            }
            match (byte_window, backing.extent) {
                (Some((start, end)), terminal_semantics::CollectionExtent::Fixed(length)) => {
                    if !(start <= end && end <= length)
                        || extent != target_operations::TargetViewExtent::Fixed(end - start)
                    {
                        return Err("view static window has invalid bounds or extent");
                    }
                }
                (Some(_), _) => return Err("view static window has no fixed source extent"),
                (None, terminal_semantics::CollectionExtent::Fixed(length)) => {
                    if extent != target_operations::TargetViewExtent::Fixed(length) {
                        return Err("view fixed extent differs from its source collection");
                    }
                }
                (None, terminal_semantics::CollectionExtent::LiveLength { .. }) => {
                    if extent != target_operations::TargetViewExtent::LiveLength {
                        return Err("view live extent differs from its source collection");
                    }
                }
                (None, terminal_semantics::CollectionExtent::StoredLength) => {
                    if extent != target_operations::TargetViewExtent::StoredLength {
                        return Err("view stored extent differs from its source descriptor");
                    }
                }
            }
            let Some(stride) =
                crate::structural_inputs::structural_reference_input::shape(element, self.types)
                    .and_then(|shape| {
                        crate::structural_inputs::structural_reference_input::align(
                            shape.byte_size,
                            shape.alignment,
                        )
                    })
            else {
                return Err("view element stride cannot be reconstructed");
            };
            let window_start = byte_window.map_or(0, |(start, _)| start);
            let leading: u64 = match backing.extent {
                terminal_semantics::CollectionExtent::LiveLength { .. } => 8,
                terminal_semantics::CollectionExtent::Fixed(_)
                | terminal_semantics::CollectionExtent::StoredLength => 0,
            };
            (stride != 0
                && stride == element_stride
                && window_start
                    .checked_mul(u64::from(stride))
                    .and_then(|window_offset| {
                        u64::from(offset)
                            .checked_add(leading)
                            .and_then(|bytes| bytes.checked_add(window_offset))
                    })
                    == Some(u64::from(source_byte_offset)))
            .then_some(())
            .ok_or("view element stride or selected byte offset differs from its source")
        }
    }
}
pub(crate) mod expressions {
    use super::{AbstractOperation, ValueId};
    use crate::legalization::scalar_graph_input::ScalarType;
    use crate::legalization::scalar_graph_input::target::Boolean;
    use crate::legalization::scalar_graph_input::target::Checker;
    use crate::legalization::scalar_graph_input::target::Expression;
    use crate::legalization::scalar_graph_input::target::location_matches;
    use crate::legalization::scalar_graph_input::target::resolve;
    use crate::legalization::scalar_graph_input::{
        saturating_carrier, supports_wrapping_division, trapping_form,
    };
    use abstract_operations::{
        IntegerArithmetic, IntegerArithmeticOperator, IntegerOverflowPolicy,
    };
    use semantic_vocabulary::IntegerType;
    impl Checker<'_> {
        pub(crate) fn integer_source(
            &self,
            expression: &Expression,
            value: ValueId,
            aliases: &[(ValueId, ValueId)],
        ) -> bool {
            matches!(
                expression,
                Expression::Immediate { .. }
                    | Expression::Parameter { .. }
                    | Expression::ScalarHome(_)
                    | Expression::BlockParameter(_)
            ) && self.expression(expression, value, aliases)
        }

        pub(crate) fn boolean_source(
            &self,
            expression: &Boolean,
            value: ValueId,
            aliases: &[(ValueId, ValueId)],
        ) -> bool {
            matches!(
                expression,
                Boolean::Immediate { .. }
                    | Boolean::Parameter { .. }
                    | Boolean::ScalarHome(_)
                    | Boolean::BlockParameter(_)
            ) && self.boolean(expression, value, aliases)
        }

        fn available_block_value(
            &self,
            parameter: &target_operations::TargetScalarBlockValue,
        ) -> bool {
            let Some(sources) = self.available else {
                return false;
            };
            let mut definitions = sources.matching(parameter.value);
            matches!(definitions.next(),
                Some((_, target_operations::TargetUnitScalarArgumentSource::BlockParameter(expected)))
                    if expected == parameter)
                && definitions.next().is_none()
        }

        pub(crate) fn scalar_parameters(&self) -> &[target_operations::ScalarAbiValue] {
            if let Some(abi) = &self.function.scalar_abi {
                &abi.parameters
            } else if let Some(abi) = &self.function.mixed_structural_scalar_abi {
                &abi.scalar_parameters
            } else {
                &self.function.graph.scalar_parameters
            }
        }

        fn available_home(
            &self,
            home: &target_operations::TargetUnitScalarHomeRequirement,
        ) -> bool {
            let Some(sources) = self.available else {
                return false;
            };
            let mut definitions = sources.matching(home.source_value);
            let Some((_, target_operations::TargetUnitScalarArgumentSource::Home(expected))) =
                definitions.next()
            else {
                return false;
            };
            home == expected && definitions.next().is_none()
        }

        pub(crate) fn float_element_read(
            &self,
            expression: &target_operations::TargetScalarExpression,
            value: ValueId,
        ) -> bool {
            let target_operations::TargetScalarExpression::IeeeFloatElementViewRead {
                format,
                psi_operation,
                source_value,
                source,
                view,
                index,
                length,
                obligation,
                path,
                element_byte_offset,
            } = expression
            else {
                return false;
            };
            *source_value == value
                && self.element_view(view, *source, &[])
                && self.optimized.block_nodes().any(|node| matches!(&node.operation,
                    AbstractOperation::ElementViewRead {
                        psi_operation: operation, result, source: expected,
                        index: expected_index, length: expected_length,
                        obligation: expected_obligation, path: expected_path,
                    } if operation == psi_operation && result.value == value
                        && result.scalar_type == ScalarType::IeeeFloat(*format)
                        && expected == source && path == expected_path
                        && length == expected_length && obligation == expected_obligation
                        && self.integer_source(index, *expected_index, &[])
                        && crate::legalization::scalar_graph_input::byte_views::element_type(self.optimized, *source, self.types)
                            .and_then(|element| crate::structural_inputs::structural_reference_input::element_leaf_projection(
                                element, path, result.scalar_type, self.types))
                            .is_some_and(|(offset, _, _)| offset == *element_byte_offset)))
        }

        pub(crate) fn expression(
            &self,
            expression: &Expression,
            value: ValueId,
            aliases: &[(ValueId, ValueId)],
        ) -> bool {
            let resolved = resolve(value, aliases);
            match expression {
                Expression::BlockParameter(parameter) => parameter.value == resolved
                    && matches!(parameter.scalar_type, ScalarType::Integer(_))
                    && self.available_block_value(parameter),
                Expression::ScalarHome(home) => home.source_value == resolved
                    && matches!(home.scalar_type, ScalarType::Integer(_))
                    && self.available_home(home),
                Expression::ByteSequenceRead { psi_operation, source_value, source, view, index, length, obligation } => {
                    *source_value == resolved
                        && self.byte_view(view, *source, aliases)
                        && self.optimized.block_nodes().any(|node| matches!(&node.operation,
                            AbstractOperation::ByteSequenceRead { psi_operation: operation, result, source: expected, index: expected_index, length: expected_length, obligation: expected_obligation }
                            if operation == psi_operation && result.value == resolved && expected == source
                            && length == expected_length && obligation == expected_obligation && self.integer_source(index, *expected_index, aliases)))
                }
                Expression::ByteSequenceLength { psi_operation, source_value, source, indices, length_byte_offset } => {
                    *source_value == resolved
                        && crate::legalization::scalar_graph_input::element_views::byte_length_geometry(self.optimized, source, self.types)
                            .is_some_and(|(geometry, carrier)| {
                                *length_byte_offset == if matches!(carrier, terminal_psi::ByteSequenceCarrier::BorrowedView { .. }) { 8 } else { 0 }
                                && super::unit::runtime_indices_rejoin(indices, &geometry.elements, self.available.unwrap_or(&crate::legalization_target::control_flow::sources::EMPTY))
                            })
                        && self.optimized.block_nodes().any(|node| matches!(&node.operation,
                            AbstractOperation::ByteSequenceLength { psi_operation: operation, result, source: expected }
                            if operation == psi_operation && result.value == resolved && expected == source))
                }
                Expression::ElementViewRead { psi_operation, source_value, source, view, index, length, obligation, path, element_byte_offset } => {
                    *source_value == resolved
                        && self.element_view(view, *source, aliases)
                        && self.optimized.block_nodes().any(|node| matches!(&node.operation,
                            AbstractOperation::ElementViewRead { psi_operation: operation, result, source: expected, index: expected_index, length: expected_length, obligation: expected_obligation, path: expected_path }
                            if operation == psi_operation && result.value == resolved && expected == source
                            && path == expected_path
                            && crate::legalization::scalar_graph_input::byte_views::element_type(self.optimized, *source, self.types)
                                .and_then(|element| crate::structural_inputs::structural_reference_input::element_leaf_projection(
                                    element, path, result.scalar_type, self.types))
                                .is_some_and(|(offset, _, _)| offset == *element_byte_offset)
                            && length == expected_length && obligation == expected_obligation && self.integer_source(index, *expected_index, aliases)))
                }
                Expression::ElementViewLength { psi_operation, source_value, source, indices, length_byte_offset } => {
                    *length_byte_offset == 8 && *source_value == resolved
                        && crate::legalization::scalar_graph_input::element_views::length_geometry(self.optimized, source, self.types)
                            .is_some_and(|geometry| super::unit::runtime_indices_rejoin(
                                indices, &geometry.elements, self.available.unwrap_or(&crate::legalization_target::control_flow::sources::EMPTY)))
                        && self.optimized.block_nodes().any(|node| matches!(&node.operation,
                            AbstractOperation::ElementViewLength { psi_operation: operation, result, source: expected }
                            if operation == psi_operation && result.value == resolved && expected == source))
                }
                Expression::Immediate {source_value,value:literal} => *source_value == value && self.optimized.block_nodes().any(|node|
                    match &node.operation {
                        AbstractOperation::IntegerConstant {result,value:actual,..} => *result == resolved && actual == literal,
                        AbstractOperation::ByteSequenceLength {result,source,..} => result.value == resolved
                            && (crate::legalization::scalar_graph_input::element_views::byte_length_geometry(self.optimized, source, self.types)
                                .is_some_and(|(_, carrier)| matches!(carrier, terminal_psi::ByteSequenceCarrier::BoundedView { capacity, .. } if *literal == semantic_vocabulary::IntegerValue::Unsigned(u128::from(capacity))))
                                || crate::legalization::scalar_graph_input::element_views::fixed_byte_length(self.optimized, source, self.types).is_some_and(|length| *literal == semantic_vocabulary::IntegerValue::Unsigned(u128::from(length)))),
                        _ => false,
                    }),
                Expression::Parameter {source_value,parameter_index,location} => {
                    let Some(parameter) = self.scalar_parameters().get(*parameter_index) else {return false;};
                    *source_value == value && parameter.value == resolved && location_matches(*location,&parameter.placement)
                }
                Expression::BitwiseAnd { psi_operation, left, right } => {
                    self.optimized.block_nodes().any(|node| matches!(&node.operation,
                        AbstractOperation::IntegerBitwiseAnd { psi_operation: operation, result, left: source_left, right: source_right, .. }
                        if operation == psi_operation && *result == resolved
                            && self.integer_source(left, *source_left, aliases)
                            && self.integer_source(right, *source_right, aliases)))
                }
                Expression::BitwiseOr { psi_operation, left, right } => {
                    self.optimized.block_nodes().any(|node| matches!(&node.operation,
                        AbstractOperation::IntegerBitwiseOr { psi_operation: operation, result, left: source_left, right: source_right, .. }
                        if operation == psi_operation && *result == resolved
                            && self.integer_source(left, *source_left, aliases)
                            && self.integer_source(right, *source_right, aliases)))
                }
                Expression::BitwiseNot { psi_operation, operand } => {
                    self.optimized.block_nodes().any(|node| matches!(&node.operation,
                        AbstractOperation::IntegerBitwiseNot { psi_operation: operation, result, operand: source, .. }
                        if operation == psi_operation && *result == resolved
                            && self.integer_source(operand, *source, aliases)))
                }
                Expression::BitwiseXor { psi_operation, left, right } => {
                    self.optimized.block_nodes().any(|node| matches!(&node.operation,
                        AbstractOperation::IntegerBitwiseXor { psi_operation: operation, result, left: source_left, right: source_right, .. }
                        if operation == psi_operation && *result == resolved
                            && self.integer_source(left, *source_left, aliases)
                            && self.integer_source(right, *source_right, aliases)))
                }
                Expression::Arithmetic { psi_operation, arithmetic, left, right } => {
                    if arithmetic.policy() == IntegerOverflowPolicy::Exact {
                        let Some(node) = self.optimized.block_nodes().find(|node| matches!(&node.operation,
                            AbstractOperation::IntegerArithmetic {psi_operation:operation,arithmetic:source,..} if operation == psi_operation && source.policy() == IntegerOverflowPolicy::Exact)) else {return false;};
                        let AbstractOperation::IntegerArithmetic {result,arithmetic:source,left:source_left,right:source_right,..} = &node.operation else {return false;};
                        return source.operator() == arithmetic.operator() && *result == resolved && source.obligation() == arithmetic.obligation()
                            && self.integer_source(left,*source_left,aliases) && self.integer_source(right,*source_right,aliases);
                    }
                    self.optimized.block_nodes().any(|node| matches!(&node.operation,
                        AbstractOperation::IntegerArithmetic { psi_operation: operation, result, scalar_type, arithmetic: source, left: source_left, right: source_right }
                        if operation == psi_operation && source == arithmetic && *result == resolved
                            && arithmetic_carrier_admitted(*arithmetic, *scalar_type)
                            && self.integer_source(left, *source_left, aliases)
                            && self.integer_source(right, *source_right, aliases)))
                }
                Expression::Shift { psi_operation, policy, direction, count_type, value, count } => {
                    self.optimized.block_nodes().any(|node| matches!(&node.operation,
                        AbstractOperation::IntegerShift { psi_operation: operation, result, count_type: expected_count_type, policy: source_policy, direction: source_direction, value: source_value, count: source_count, .. }
                        if operation == psi_operation && *result == resolved
                            && source_policy == policy && source_direction == direction
                            && expected_count_type == count_type
                            && self.integer_source(value, *source_value, aliases)
                            && self.integer_source(count, *source_count, aliases)))
                }
                Expression::Trapping { psi_operation, primitive, operand_type, left, right } => {
                    self.optimized.block_nodes().any(|node| matches!(&node.operation,
                        AbstractOperation::TrappingInteger { psi_operation: operation, result, scalar_type, operand_type: expected_operand_type, operation: source }
                        if operation == psi_operation && *result == resolved
                            && source.primitive() == *primitive
                            && expected_operand_type == operand_type
                            && trapping_form(*scalar_type, *operand_type, *primitive).is_some()
                            && match (source.operands().as_slice(), right) {
                                ([source_left], None) => self.integer_source(left, *source_left, aliases),
                                ([source_left, source_right], Some(right)) => {
                                    self.integer_source(left, *source_left, aliases)
                                        && self.integer_source(right, *source_right, aliases)
                                }
                                _ => false,
                            }))
                }
                Expression::IntegerWiden { psi_operation, source_type, operand } => {
                    self.optimized.block_nodes().any(|node| matches!(&node.operation,
                        AbstractOperation::IntegerWiden { psi_operation: operation, result, source_type: actual_type, operand: source, .. }
                        if operation == psi_operation && *result == resolved && actual_type == source_type && self.integer_source(operand, *source, aliases)))
                }
                Expression::IntegerExactCast { psi_operation, obligation, source_type, operand } => {
                    self.optimized.block_nodes().any(|node| matches!(&node.operation,
                        AbstractOperation::IntegerExactCast { psi_operation: operation, obligation: expected_obligation, result, source_type: actual_type, operand: source, .. }
                        if operation == psi_operation && expected_obligation == obligation && *result == resolved && actual_type == source_type && self.integer_source(operand, *source, aliases)))
                }
                _ => false,
            }
        }
        pub(crate) fn boolean(
            &self,
            expression: &Boolean,
            value: ValueId,
            aliases: &[(ValueId, ValueId)],
        ) -> bool {
            if let Boolean::ElementViewRead {
                psi_operation,
                source_value,
                source,
                view,
                index,
                length,
                obligation,
                path,
                element_byte_offset,
            } = expression
            {
                let resolved = resolve(value, aliases);
                return *source_value == resolved
                    && self.element_view(view, *source, aliases)
                    && self.optimized.block_nodes().any(|node| matches!(&node.operation,
                        AbstractOperation::ElementViewRead { psi_operation: operation, result, source: expected, index: expected_index, length: expected_length, obligation: expected_obligation, path: expected_path }
                        if operation == psi_operation && result.value == resolved && expected == source
                        && result.scalar_type == ScalarType::Boolean
                        && path == expected_path
                        && crate::legalization::scalar_graph_input::byte_views::element_type(self.optimized, *source, self.types)
                            .and_then(|element| crate::structural_inputs::structural_reference_input::element_leaf_projection(
                                element, path, result.scalar_type, self.types))
                            .is_some_and(|(offset, _, _)| offset == *element_byte_offset)
                        && length == expected_length && obligation == expected_obligation && self.integer_source(index, *expected_index, aliases)));
            }
            if let Boolean::BlockParameter(parameter) = expression {
                return parameter.value == resolve(value, aliases)
                    && parameter.scalar_type == ScalarType::Boolean
                    && self.available_block_value(parameter);
            }
            if let Boolean::Equal {
                psi_operation,
                left,
                right,
            } = expression
            {
                return self.optimized.block_nodes().any(|node| {
                    matches!(&node.operation,
                        AbstractOperation::BooleanEqual { psi_operation: operation, result, left: source_left, right: source_right }
                        if operation == psi_operation && *result == resolve(value, aliases)
                        && self.boolean_source(left, *source_left, aliases)
                        && self.boolean_source(right, *source_right, aliases))
                });
            }
            if let Boolean::ScalarHome(home) = expression {
                return home.source_value == resolve(value, aliases)
                    && home.scalar_type == ScalarType::Boolean
                    && self.available_home(home);
            }
            if let Boolean::Immediate {
                source_value,
                value: literal,
            } = expression
            {
                return *source_value == value
                    && self.optimized.block_nodes().any(|node|
                        matches!(&node.operation, AbstractOperation::BooleanConstant { result, value: actual, .. }
                            if *result == resolve(value, aliases) && actual == literal))
                    && self.available.is_none_or(|sources| sources.matching(value).any(|(source, definition)|
                        *source == value && matches!(definition,
                            target_operations::TargetUnitScalarArgumentSource::BooleanImmediate { value: actual, .. } if actual == literal)));
            }
            if let Boolean::Parameter {
                source_value,
                parameter_index,
                location,
            } = expression
            {
                return *source_value == value
                    && self
                        .scalar_parameters()
                        .get(*parameter_index)
                        .is_some_and(|parameter| {
                            parameter.value == resolve(value, aliases)
                                && parameter.scalar_type == ScalarType::Boolean
                                && location_matches(*location, &parameter.placement)
                        });
            }
            if let Boolean::Not {
                psi_operation,
                operand,
            } = expression
            {
                return self.optimized.block_nodes().any(|node| matches!(&node.operation,
                    AbstractOperation::BooleanNot { psi_operation: operation, result, operand: source }
                    if operation == psi_operation && *result == resolve(value, aliases) && self.boolean_source(operand, *source, aliases)));
            }
            if let Boolean::ByteSequencePredicate {
                psi_operation,
                source_value,
                operand,
                predicate,
            } = expression
            {
                return *source_value == resolve(value, aliases)
                    && self.optimized.block_nodes().any(|node| {
                        let AbstractOperation::ByteSequencePredicate {
                            psi_operation: operation,
                            result,
                            operand: expected_operand,
                            predicate: expected_predicate,
                        } = &node.operation
                        else {
                            return false;
                        };
                        operation == psi_operation
                            && result.value == resolve(value, aliases)
                            && expected_predicate == predicate
                            && operands_agree(operand, expected_operand, aliases)
                    });
            }
            if let Boolean::ByteSequenceEqual {
                psi_operation,
                source_value,
                left,
                right,
            } = expression
            {
                return *source_value == resolve(value, aliases)
                    && self.optimized.block_nodes().any(|node| {
                        let AbstractOperation::ByteSequenceEqual {
                            psi_operation: operation,
                            result,
                            left: expected_left,
                            right: expected_right,
                        } = &node.operation
                        else {
                            return false;
                        };
                        operation == psi_operation
                            && result.value == resolve(value, aliases)
                            && operands_agree(left, expected_left, aliases)
                            && operands_agree(right, expected_right, aliases)
                    });
            }
            let Some(node) = self.optimized.block_nodes().find(|node| {
                node.definitions
                    .iter()
                    .any(|definition| definition.value == value)
            }) else {
                return false;
            };
            let (
                operation,
                source_left,
                source_right,
                scalar_type,
                right_scalar_type,
                left,
                right,
                target_operation,
            ) = match (&node.operation, expression) {
                (
                    AbstractOperation::IntegerEqual {
                        psi_operation,
                        left: source_left,
                        right: source_right,
                        ..
                    },
                    Boolean::IntegerEqual {
                        psi_operation: target_operation,
                        scalar_type,
                        right_scalar_type,
                        left,
                        right,
                    },
                )
                | (
                    AbstractOperation::IntegerLessThan {
                        psi_operation,
                        left: source_left,
                        right: source_right,
                        ..
                    },
                    Boolean::IntegerLessThan {
                        psi_operation: target_operation,
                        scalar_type,
                        right_scalar_type,
                        left,
                        right,
                    },
                )
                | (
                    AbstractOperation::IntegerLessOrEqual {
                        psi_operation,
                        left: source_left,
                        right: source_right,
                        ..
                    },
                    Boolean::IntegerLessOrEqual {
                        psi_operation: target_operation,
                        scalar_type,
                        right_scalar_type,
                        left,
                        right,
                    },
                ) => (
                    *psi_operation,
                    *source_left,
                    *source_right,
                    *scalar_type,
                    *right_scalar_type,
                    left,
                    right,
                    *target_operation,
                ),
                _ => return false,
            };
            operation == target_operation
                && self.function_inputs.value_type(source_left)
                    == Some(ScalarType::Integer(scalar_type))
                && self.function_inputs.value_type(source_right)
                    == Some(ScalarType::Integer(right_scalar_type))
                && self.integer_source(left, source_left, aliases)
                && self.integer_source(right, source_right, aliases)
        }
    }

    fn operands_agree(
        expressed: &terminal_psi::control_flow::ByteSequenceOperand,
        expected: &terminal_psi::control_flow::ByteSequenceOperand,
        aliases: &[(ValueId, ValueId)],
    ) -> bool {
        use terminal_psi::control_flow::ByteSequenceOperand as Operand;
        match (expressed, expected) {
            (
                Operand::Whole {
                    source: expressed_source,
                    length: expressed_length,
                },
                Operand::Whole {
                    source: expected_source,
                    length: expected_length,
                },
            ) => {
                expressed_source == expected_source
                    && resolve(*expressed_length, aliases) == *expected_length
            }
            (
                Operand::Field {
                    source: expressed_source,
                    field: expressed_field,
                    length: expressed_length,
                },
                Operand::Field {
                    source: expected_source,
                    field: expected_field,
                    length: expected_length,
                },
            ) => {
                expressed_source == expected_source
                    && expressed_field == expected_field
                    && resolve(*expressed_length, aliases) == *expected_length
            }
            _ => false,
        }
    }

    fn arithmetic_carrier_admitted(
        arithmetic: IntegerArithmetic,
        scalar_type: IntegerType,
    ) -> bool {
        use IntegerArithmeticOperator as Operator;
        use IntegerOverflowPolicy as Policy;
        match (arithmetic.policy(), arithmetic.operator()) {
            (Policy::Saturating, Operator::Multiply | Operator::Divide | Operator::Remainder) => {
                saturating_carrier(scalar_type).is_some()
            }
            (Policy::Wrapping, Operator::Divide | Operator::Remainder) => {
                supports_wrapping_division(scalar_type)
            }
            _ => true,
        }
    }
}
pub(crate) mod hosted_scalar {
    use super::{
        AbstractOperation, AbstractOperationPlan, PsiOptimizationUnit, TargetUnitOperation,
    };
    use crate::LegalizationError;
    use crate::legalization::scalar_graph_input::i32_type;
    use crate::legalization::scalar_graph_input::{
        CallSignature, CallingPolicy, ScalarType, ValueShape, evaluate_call_plan,
    };
    use target_operations::{
        BoundaryExecutionBinding, BoundaryRealization, CompilerBuiltinExecution,
        TargetBoundaryResult,
    };

    pub(crate) fn validate(
        target: &TargetUnitOperation,
        source: &AbstractOperation,
        native: ::target::NativeTarget,
        plan: &AbstractOperationPlan,
        unit: &PsiOptimizationUnit,
        values: &crate::legalization_target::control_flow::sources::AvailableSources<'_>,
    ) -> Result<(), LegalizationError> {
        let (
            TargetUnitOperation::BoundarySettlement {
                psi_operation,
                boundary,
                result: TargetBoundaryResult::Unit,
                execution,
                realization,
                scalar_arguments,
                runtime_scalar_arguments,
                arguments: structural,
                byte_sequence_arguments,
                completion_claim_sources,
                completion_receipts,
            },
            AbstractOperation::BoundaryCall {
                psi_operation: expected_operation,
                boundary: expected_boundary,
                result: abstract_operations::AbstractBoundaryResult::Unit,
                arguments,
                structural_arguments,
                completion_claim_sources: expected_claims,
                completion_receipts: expected_receipts,
            },
        ) = (target, source)
        else {
            return Err(LegalizationError::custody());
        };
        let ([value], [argument]) = (arguments.as_slice(), runtime_scalar_arguments.as_slice())
        else {
            return Err(LegalizationError::custody());
        };
        let supported = match (execution, realization) {
            (
                BoundaryExecutionBinding::AdmittedProvider(_),
                BoundaryRealization::HostedWriteByteI32(_),
            ) => target_operations::HostedWriteByteI32Realization::supports_target(native),
            (
                BoundaryExecutionBinding::CompilerBuiltin(
                    CompilerBuiltinExecution::HostedWriteByteI32,
                ),
                BoundaryRealization::HostedWriteByteI32(_),
            ) => target_operations::HostedWriteByteI32Realization::supports_target(native),
            (
                BoundaryExecutionBinding::CompilerBuiltin(
                    CompilerBuiltinExecution::HostedExitProcessI32,
                ),
                BoundaryRealization::HostedExitProcessI32(_),
            ) => target_operations::HostedExitProcessI32Realization::supports_target(native),
            _ => false,
        };
        let mut declarations = plan
            .boundary_machines
            .iter()
            .filter(|row| row.id == *boundary);
        let declaration = declarations.next().ok_or(LegalizationError::custody())?;
        if unit.boundary_machine(*boundary) != Some(declaration) {
            return Err(LegalizationError::custody());
        }
        let expected_type = ScalarType::Integer(i32_type());
        let expected = evaluate_call_plan(
            CallingPolicy::native_for_target(native),
            &CallSignature {
                parameters: vec![ValueShape::integer(4, 4)],
                result: None,
            },
        )
        .map_err(|_| LegalizationError::custody())?;
        if declarations.next().is_some()
            || !supported
            || psi_operation != expected_operation
            || boundary != expected_boundary
            || declaration.scalar_parameters != [expected_type]
            || !declaration.structural_parameters.is_empty()
            || !declaration.result.is_unit()
            || !declaration.requires.is_empty()
            || !declaration.program_local_root_introductions.is_empty()
            || !declaration.content_guarantees.is_empty()
            || !scalar_arguments.is_empty()
            || !structural.is_empty()
            || !structural_arguments.is_empty()
            || !byte_sequence_arguments.is_empty()
            || !completion_claim_sources.is_empty()
            || !completion_receipts.is_empty()
            || !expected_claims.is_empty()
            || !expected_receipts.is_empty()
            || argument.parameter_index != 0
            || expected.parameters.as_slice() != [argument.placement.clone()]
            || !values
                .matching(*value)
                .any(|(identity, expected)| identity == value && *expected == argument.source)
        {
            return Err(LegalizationError::custody());
        }
        Ok(())
    }
}
pub(crate) mod normalized_foreign {
    use super::{
        AbstractOperationPlan, PsiOptimizationFunction, PsiOptimizationUnit, TargetUnitOperation,
    };
    use crate::LegalizationError;
    use crate::legalization::scalar_graph_input::normalized_foreign::{
        native_callbacks_at, result_at, structural_argument_at,
    };
    use crate::legalization::scalar_graph_input::scalar_shape;
    use calling_conventions::CallSignature;
    use target_operations::{
        TargetBoundaryResult, TargetOperationPlan, TargetStructuralParameter,
        TargetUnitScalarArgumentSource as Source,
    };

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn validate(
        target: &TargetUnitOperation,
        source: &abstract_operations::AbstractOperation,
        parameters: &[TargetStructuralParameter],
        native: &TargetOperationPlan,
        optimized: &PsiOptimizationFunction,
        plan: &AbstractOperationPlan,
        unit: &PsiOptimizationUnit,
        values: &mut crate::legalization_target::control_flow::sources::AvailableSources<'_>,
    ) -> Result<(), LegalizationError> {
        let (
            TargetUnitOperation::NormalizedForeignCall {
                psi_operation,
                boundary,
                provider_execution,
                binding,
                scalar_arguments,
                structural_arguments,
                result: actual_result,
                descriptor_input,
            },
            abstract_operations::AbstractOperation::BoundaryCall {
                psi_operation: expected_operation,
                boundary: expected_boundary,
                result,
                arguments,
                structural_arguments: expected_structural,
                ..
            },
        ) = (target, source)
        else {
            return Err(LegalizationError::custody());
        };
        let required = target_operations::descriptor_input_guard_for_call(
            binding,
            &plan.structural_types,
            scalar_arguments,
            structural_arguments,
            actual_result,
            |buffer| {
                native
                    .borrowed_storage_layouts
                    .iter()
                    .find(|layout| layout.structural_type == buffer)
            },
        )
        .map_err(|_| LegalizationError::custody())?;
        if required != *descriptor_input {
            return Err(LegalizationError::custody());
        }
        let mut declarations = plan
            .boundary_machines
            .iter()
            .filter(|row| row.id == *boundary);
        let declaration = declarations.next().ok_or(LegalizationError::custody())?;
        if unit.boundary_machine(*boundary) != Some(declaration) {
            return Err(LegalizationError::custody());
        }
        let scalar_shapes = declaration
            .scalar_parameters
            .iter()
            .map(|parameter| scalar_shape(*parameter).ok_or(LegalizationError::custody()))
            .collect::<Result<Vec<_>, LegalizationError>>()?;
        if !declaration.has_valid_parameter_order()
            || expected_structural.len() != declaration.structural_parameters.len()
            || expected_structural.len() != structural_arguments.len()
        {
            return Err(LegalizationError::custody());
        }
        let callbacks = native_callbacks_at(native, *psi_operation, &binding.boundary_entry_plan)?;
        let operands = binding.operand_placements();
        let mut private_ordinals = Vec::new();
        for callback in &callbacks {
            if matches!(
                callback.destination,
                calling_conventions::NativePlace::Parameter(_)
            ) {
                private_ordinals.push(
                    usize::try_from(callback.application.native_ordinal)
                        .map_err(|_| LegalizationError::custody())?
                        + binding.dispatch_operand_count(),
                );
            }
        }
        private_ordinals.sort_unstable();
        let structural = expected_structural
            .iter()
            .zip(&declaration.structural_parameters)
            .enumerate()
            .zip(declaration.parameter_positions(terminal_psi::BoundaryParameterKind::Structural))
            .map(
                |((index, (semantic, declaration_parameter)), semantic_position)| {
                    let native_position = semantic_position
                        + private_ordinals
                            .iter()
                            .filter(|ordinal| **ordinal <= semantic_position)
                            .count();
                    structural_argument_at(
                        semantic,
                        index,
                        declaration_parameter,
                        operands.get(native_position),
                        parameters,
                        optimized,
                        native.target,
                        binding.boundary_entry_plan.call.policy,
                        plan,
                        *expected_operation,
                        native,
                        unit,
                    )
                },
            )
            .collect::<Result<Vec<_>, LegalizationError>>()?;
        let (expected_result, result_shape) = result_at(
            result,
            &declaration.result,
            *psi_operation,
            optimized,
            plan,
            binding.boundary_entry_plan.call.policy,
        )?;
        let mut scalars = scalar_shapes.iter().copied();
        let mut structures = structural.iter().map(|argument| argument.destination.shape);
        let mut parameters = declaration
            .parameter_order
            .iter()
            .map(|kind| {
                match kind {
                    terminal_psi::BoundaryParameterKind::Scalar => scalars.next(),
                    terminal_psi::BoundaryParameterKind::Structural => structures.next(),
                }
                .ok_or(LegalizationError::custody())
            })
            .collect::<Result<Vec<_>, _>>()?;
        for (callback, ordinal) in callbacks
            .iter()
            .filter(|callback| {
                matches!(
                    callback.destination,
                    calling_conventions::NativePlace::Parameter(_)
                )
            })
            .zip(&private_ordinals)
        {
            if *ordinal > parameters.len() {
                return Err(LegalizationError::custody());
            }
            parameters.insert(*ordinal, callback.application.shape);
        }
        if callbacks
            .iter()
            .any(|callback| callback.registrar_context != callbacks[0].registrar_context)
        {
            return Err(LegalizationError::custody());
        }
        let signature = CallSignature {
            parameters: parameters
                .into_iter()
                .skip(binding.dispatch_operand_count())
                .collect(),
            result: result_shape,
        };
        let validated = match callbacks.first() {
            Some(callback) => {
                calling_conventions::validate_boundary_entry_plan_with_callback_materializations(
                    binding.boundary_entry_plan.clone(),
                    &signature,
                    &callback.registrar_context,
                )
            }
            None => calling_conventions::validate_boundary_entry_plan(
                binding.boundary_entry_plan.clone(),
                &signature,
            ),
        }
        .map_err(|_| LegalizationError::custody())?;
        if declarations.next().is_some()
            || psi_operation != expected_operation
            || boundary != expected_boundary
            || binding.native_target() != Some(native.target)
            || binding.provider_plan_report_identity()
                != provider_execution.provider_plan_report_identity().get()
            || !binding.has_valid_entry_mechanism()
            || validated.plan() != &binding.boundary_entry_plan
            || operands.len() != scalar_shapes.len() + structural.len() + private_ordinals.len()
            || scalar_arguments.len() != arguments.len()
            || arguments.len() != declaration.scalar_parameters.len()
            || structural_arguments.as_slice() != structural.as_slice()
            || scalar_arguments
                .iter()
                .zip(arguments)
                .zip(&declaration.scalar_parameters)
                .zip(&scalar_shapes)
                .zip(declaration.parameter_positions(terminal_psi::BoundaryParameterKind::Scalar))
                .any(|((((argument, value), parameter), shape), index)| {
                    let Some(access_byte_size) = calling_conventions::byte_size_at_offset_zero(
                        argument.placement.locations.as_slice(),
                    ) else {
                        return true;
                    };
                    let native_position = index
                        + private_ordinals
                            .iter()
                            .filter(|ordinal| **ordinal <= index)
                            .count();
                    usize::try_from(argument.parameter_index).ok() != Some(native_position)
                        || operands.get(native_position) != Some(&argument.placement)
                        || argument.placement.shape != *shape
                        || shape.byte_size != access_byte_size
                        || argument.source.scalar_type() != *parameter
                        || match &argument.source {
                            Source::IntegerImmediate {
                                scalar_type, value, ..
                            } => semantic_vocabulary::ScalarTerm::integer(*scalar_type, *value)
                                .is_err(),
                            Source::Home(home) => home.shape != *shape,
                            _ => false,
                        }
                        || !values.matching(*value).any(|(identity, expected)| {
                            identity == value && *expected == argument.source
                        })
                })
            || expected_result != *actual_result
            || match (&binding.boundary_entry_plan.call.result, result_shape) {
                (None, None) => false,
                (Some(placement), Some(shape)) => {
                    placement.shape != shape
                        || matches!(expected_result, TargetBoundaryResult::Scalar(_))
                            && !placement.is_complete_scalar_result()
                }
                _ => true,
            }
        {
            return Err(LegalizationError::custody());
        }
        if let Some(home) = expected_result.scalar_home() {
            values.push((home.source_value, Source::Home(*home)));
        }
        Ok(())
    }
}
pub(crate) mod scalar_definitions {
    use super::{AbstractOperation, TargetScalarExpression, TargetUnitOperation, ValueId};
    use crate::LegalizationError;
    use crate::legalization::scalar_graph_input::target::Checker;
    use crate::legalization::scalar_graph_input::target::Expression;
    use crate::legalization::scalar_graph_input::target::location_matches;
    use crate::legalization::scalar_graph_input::{ScalarType, ValueShape, scalar_shape};
    use crate::legalization::scalar_graph_input::{
        saturating_carrier, supports_wrapping_division, trapping_form,
    };
    use abstract_operations::{IntegerArithmeticOperator, IntegerOverflowPolicy};
    use target_operations::{ScalarAbiValue, TargetUnitScalarArgumentSource as Source};

    pub(crate) fn validate(
        target: &TargetUnitOperation,
        abstracted: &AbstractOperation,
        scalar_parameters: &[ScalarAbiValue],
        sources: &crate::legalization_target::control_flow::sources::AvailableSources<'_>,
    ) -> Result<(), LegalizationError> {
        let (
            TargetUnitOperation::ScalarDefinition {
                result_home,
                expression:
                    TargetScalarExpression::Integer {
                        scalar_type,
                        expression:
                            Expression::IntegerWiden {
                                psi_operation,
                                source_type,
                                operand,
                            },
                    },
            },
            AbstractOperation::IntegerWiden {
                psi_operation: expected_operation,
                result,
                source_type: expected_source_type,
                target_type,
                operand: source_value,
            },
        ) = (target, abstracted)
        else {
            return Err(LegalizationError::custody());
        };
        if psi_operation != expected_operation
            || source_type != expected_source_type
            || scalar_type != target_type
            || !source_type.can_widen_to(*target_type)
            || !matches!(target_type.bits(), 16 | 32 | 64)
            || result_home.defining_operation != *expected_operation
            || result_home.source_value != *result
            || result_home.scalar_type != ScalarType::Integer(*target_type)
            || result_home.shape
                != ValueShape::integer(u32::from(target_type.bits() / 8), target_type.bits() / 8)
        {
            return Err(LegalizationError::custody());
        }
        let mut definitions = sources.matching(*source_value);
        let definition = &definitions.next().ok_or(LegalizationError::custody())?.1;
        if definitions.next().is_some() {
            return Err(LegalizationError::custody());
        }
        let matches = match (operand.as_ref(), definition) {
            (
                Expression::Parameter {
                    source_value: value,
                    parameter_index,
                    location,
                },
                Source::Parameter {
                    source_value: expected_value,
                    parameter_index: expected_index,
                    scalar_type,
                },
            ) => {
                value == source_value
                    && value == expected_value
                    && usize::try_from(*expected_index).ok() == Some(*parameter_index)
                    && *scalar_type == ScalarType::Integer(*source_type)
                    && scalar_parameters
                        .get(*parameter_index)
                        .is_some_and(|parameter| {
                            parameter.value == *value
                                && parameter.scalar_type == *scalar_type
                                && location_matches(*location, &parameter.placement)
                        })
            }
            (
                Expression::Immediate {
                    source_value: value,
                    value: literal,
                },
                Source::IntegerImmediate {
                    source_value: expected_value,
                    scalar_type,
                    value: expected_literal,
                    ..
                },
            ) => {
                value == source_value
                    && value == expected_value
                    && scalar_type == source_type
                    && literal == expected_literal
                    && source_type.admits(*literal)
            }
            (Expression::BlockParameter(parameter), Source::BlockParameter(expected)) => {
                parameter == expected
                    && parameter.value == *source_value
                    && parameter.scalar_type == ScalarType::Integer(*source_type)
            }
            (Expression::ScalarHome(home), Source::Home(expected)) => {
                home == expected
                    && home.source_value == *source_value
                    && home.scalar_type == ScalarType::Integer(*source_type)
            }
            _ => false,
        };
        if !matches {
            return Err(LegalizationError::custody());
        }
        Ok(())
    }

    pub(crate) fn observed_family(abstracted: &AbstractOperation) -> bool {
        matches!(
            abstracted,
            AbstractOperation::ByteSequenceLength { .. }
                | abstract_operations::byte_read_predicate_equal_pattern!(..)
                | AbstractOperation::ElementViewLength { .. }
                | AbstractOperation::ElementViewRead { .. }
                | abstract_operations::integer_equal_less_pair_pattern!(..)
                | AbstractOperation::IntegerLessOrEqual { .. }
                | abstract_operations::boolean_not_or_equal_pair_pattern!(..)
                | abstract_operations::integer_dispatch_triple_pattern!(..)
                | AbstractOperation::IntegerBitwiseAnd { .. }
                | abstract_operations::bitwise_or_xor_pair_pattern!(..)
                | AbstractOperation::IntegerBitwiseNot { .. }
                | AbstractOperation::IntegerExactCast { .. }
        )
    }

    pub(crate) fn observation(
        target: &TargetUnitOperation,
        abstracted: &AbstractOperation,
        checker: &Checker<'_>,
    ) -> Result<(), LegalizationError> {
        let TargetUnitOperation::ScalarDefinition {
            result_home,
            expression,
        } = target
        else {
            return Err(LegalizationError::custody());
        };
        let (operation, value, scalar_type) = match abstracted {
            AbstractOperation::IntegerArithmetic {
                psi_operation,
                result,
                scalar_type,
                arithmetic,
                left,
                right,
            } => {
                let carrier_admitted = match (arithmetic.policy(), arithmetic.operator()) {
                    (
                        IntegerOverflowPolicy::Saturating,
                        IntegerArithmeticOperator::Divide | IntegerArithmeticOperator::Remainder,
                    ) => saturating_carrier(*scalar_type).is_some(),
                    (
                        IntegerOverflowPolicy::Wrapping,
                        IntegerArithmeticOperator::Divide | IntegerArithmeticOperator::Remainder,
                    ) => supports_wrapping_division(*scalar_type),
                    _ => true,
                };
                if !carrier_admitted
                    || checker.available.is_none_or(|sources| {
                        if arithmetic.policy() == IntegerOverflowPolicy::Exact
                            && matches!(
                                arithmetic.operator(),
                                IntegerArithmeticOperator::Add
                                    | IntegerArithmeticOperator::Subtract
                            )
                        {
                            let source_type = |identity| {
                                sources
                                    .matching(identity)
                                    .find(|(value, _)| *value == identity)
                                    .map(|(_, source)| source.scalar_type())
                            };
                            return !matches!((source_type(*left), source_type(*right)),
                            (Some(ScalarType::Integer(left)), Some(ScalarType::Integer(right)))
                                if if arithmetic.operator() == IntegerArithmeticOperator::Add {
                                    scalar_type.admits_exact_add_operands(left, right)
                                } else {
                                    scalar_type.admits_exact_subtract_operands(left, right)
                                });
                        }
                        [left, right].iter().any(|operand| {
                            !sources.matching(**operand).any(|(value, source)| {
                                value == *operand
                                    && source.scalar_type() == ScalarType::Integer(*scalar_type)
                            })
                        })
                    })
                {
                    return Err(LegalizationError::custody());
                }
                (*psi_operation, *result, ScalarType::Integer(*scalar_type))
            }
            AbstractOperation::TrappingInteger {
                psi_operation,
                result,
                scalar_type,
                operand_type,
                operation,
            } => {
                use terminal_psi::TrappingIntegerOperation as T;
                let typed = |operand: ValueId, expected| {
                    checker.available.is_some_and(|sources| {
                        sources.matching(operand).any(|(value, source)| {
                            *value == operand
                                && source.scalar_type() == ScalarType::Integer(expected)
                        })
                    })
                };
                let operands_available = match *operation {
                    terminal_psi::trapping_arith_set_pattern!(left, right, ..) => {
                        typed(left, *scalar_type) && typed(right, *scalar_type)
                    }
                    T::ShiftLeft { value, count } | T::ShiftRight { value, count } => {
                        typed(value, *scalar_type) && typed(count, *operand_type)
                    }
                    T::Convert { operand } => typed(operand, *operand_type),
                };
                if trapping_form(*scalar_type, *operand_type, operation.primitive()).is_none()
                    || !operands_available
                {
                    return Err(LegalizationError::custody());
                }
                (*psi_operation, *result, ScalarType::Integer(*scalar_type))
            }
            AbstractOperation::IntegerExactCast {
                psi_operation,
                result,
                source_type,
                target_type,
                operand,
                ..
            } => {
                if !source_type.can_exact_cast_to(*target_type)
                    || checker.available.is_none_or(|sources| {
                        !sources.matching(*operand).any(|(value, source)| {
                            value == operand
                                && source.scalar_type() == ScalarType::Integer(*source_type)
                        })
                    })
                {
                    return Err(LegalizationError::custody());
                }
                (*psi_operation, *result, ScalarType::Integer(*target_type))
            }
            AbstractOperation::IntegerBitwiseNot {
                psi_operation,
                result,
                scalar_type,
                operand,
            } => {
                if checker.available.is_none_or(|sources| {
                    !sources.matching(*operand).any(|(value, source)| {
                        value == operand
                            && source.scalar_type() == ScalarType::Integer(*scalar_type)
                    })
                }) {
                    return Err(LegalizationError::custody());
                }
                (*psi_operation, *result, ScalarType::Integer(*scalar_type))
            }
            AbstractOperation::IntegerBitwiseAnd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            }
            | abstract_operations::bitwise_or_xor_pair_pattern!(
                psi_operation,
                result,
                scalar_type,
                left,
                right,
                ..
            ) => {
                if checker.available.is_none_or(|sources| {
                    [left, right].iter().any(|operand| {
                        !sources.matching(**operand).any(|(value, source)| {
                            value == *operand
                                && source.scalar_type() == ScalarType::Integer(*scalar_type)
                        })
                    })
                }) {
                    return Err(LegalizationError::custody());
                }
                (*psi_operation, *result, ScalarType::Integer(*scalar_type))
            }
            AbstractOperation::IntegerShift {
                psi_operation,
                result,
                value_type,
                count_type,
                value,
                count,
                ..
            } => {
                if scalar_shape(ScalarType::Integer(*value_type)).is_none()
                    || scalar_shape(ScalarType::Integer(*count_type)).is_none()
                    || checker.available.is_none_or(|sources| {
                        !sources.matching(*value).any(|(identity, source)| {
                            identity == value
                                && source.scalar_type() == ScalarType::Integer(*value_type)
                        }) || !sources.matching(*count).any(|(identity, source)| {
                            identity == count
                                && source.scalar_type() == ScalarType::Integer(*count_type)
                        })
                    })
                {
                    return Err(LegalizationError::custody());
                }
                (*psi_operation, *result, ScalarType::Integer(*value_type))
            }
            AbstractOperation::ByteSequenceLength {
                psi_operation,
                result,
                ..
            }
            | abstract_operations::byte_read_predicate_equal_pattern!(psi_operation, result, ..)
            | AbstractOperation::ElementViewLength {
                psi_operation,
                result,
                ..
            }
            | AbstractOperation::ElementViewRead {
                psi_operation,
                result,
                ..
            } => (*psi_operation, result.value, result.scalar_type),
            abstract_operations::integer_equal_less_pair_pattern!(psi_operation, result, ..)
            | AbstractOperation::IntegerLessOrEqual {
                psi_operation,
                result,
                ..
            }
            | abstract_operations::boolean_not_or_equal_pair_pattern!(psi_operation, result, ..) => {
                (*psi_operation, *result, ScalarType::Boolean)
            }
            _ => return Err(LegalizationError::custody()),
        };
        if result_home.defining_operation != operation
            || result_home.source_value != value
            || result_home.scalar_type != scalar_type
            || Some(result_home.shape) != scalar_shape(scalar_type)
        {
            return Err(LegalizationError::custody());
        }
        let matches = match (expression, scalar_type) {
            (
                TargetScalarExpression::IeeeFloatElementViewRead { format, .. },
                ScalarType::IeeeFloat(expected),
            ) => *format == expected && checker.float_element_read(expression, value),
            (
                TargetScalarExpression::Integer {
                    scalar_type: actual,
                    expression,
                },
                ScalarType::Integer(expected),
            ) => *actual == expected && checker.expression(expression, value, &[]),
            (TargetScalarExpression::Boolean(expression), ScalarType::Boolean) => {
                checker.boolean(expression, value, &[])
            }
            _ => false,
        };
        if !matches {
            return Err(LegalizationError::custody());
        }
        Ok(())
    }
}
pub(crate) mod unit {
    use super::{
        AbstractOperation, AbstractOperationPlan, PsiOptimizationFunction, PsiOptimizationUnit,
        TargetFunction, TargetOperationPlan, TargetUnitOperation, ValueId,
    };
    use crate::LegalizationError;
    use crate::legalization::scalar_graph_input::ScalarType;
    use crate::legalization::scalar_graph_input::callee_plan;
    use crate::legalization::scalar_graph_input::structural_case::{
        membership_layout, source_owner, source_result,
    };
    use crate::legalization::scalar_graph_input::structural_fields::{
        byte_read, replacement, scalar_store_destination, view_replacement, view_slot_geometry,
    };
    use crate::legalization::scalar_graph_input::target::Checker;
    use crate::structural_inputs::structural_reference_input::{
        RuntimeElement, primitive_read_geometry, storage_address_geometry,
    };
    use target_operations::{
        ScalarAbiValue, TargetStructuralParameter, TargetUnitScalarArgumentSource as Source,
    };
    pub(crate) mod aggregate_results {
        use super::{
            AbstractOperation, AbstractOperationPlan, PsiOptimizationFunction, PsiOptimizationUnit,
            TargetUnitOperation,
        };
        use crate::LegalizationError;
        use crate::legalization::scalar_graph_input;
        use crate::legalization::{accepted_fact, references_obligation};

        fn same_result_home(
            result_home: &target_operations::TargetStructuralHomeRequirement,
            optimized: &PsiOptimizationFunction,
            place: semantic_vocabulary::PlaceId,
            plan: &AbstractOperationPlan,
        ) -> Result<bool, LegalizationError> {
            Ok(*result_home
                == scalar_graph_input::aggregate_results::result_home(optimized, place, plan)?)
        }

        pub(crate) fn validate(
            target: &TargetUnitOperation,
            abstracted: &AbstractOperation,
            sources: &crate::legalization_target::control_flow::sources::AvailableSources<'_>,
            optimized: &PsiOptimizationFunction,
            plan: &AbstractOperationPlan,
            unit: &PsiOptimizationUnit,
        ) -> Result<(), LegalizationError> {
            match (target, abstracted) {
                (
                    TargetUnitOperation::EstablishActivationClaim {
                        psi_operation,
                        result_home,
                        bound,
                        committed,
                        backing_type,
                        established_case,
                        rejected_case,
                    },
                    AbstractOperation::EstablishActivationClaim {
                        psi_operation: expected,
                        result,
                        bound: eb,
                        committed: ec,
                        backing_type: et,
                        established_case: ee,
                        rejected_case: er,
                    },
                ) => {
                    if psi_operation != expected
                        || bound != eb
                        || backing_type != et
                        || established_case != ee
                        || rejected_case != er
                        || !same_result_home(result_home, optimized, result.place, plan)?
                        || !sources
                            .matching(*ec)
                            .any(|(value, source)| value == ec && source == committed)
                    {
                        return Err(LegalizationError::custody());
                    }
                }
                (
                    TargetUnitOperation::ReleaseActivationClaim {
                        psi_operation,
                        source,
                        claim,
                    },
                    AbstractOperation::ReleaseActivationClaim {
                        psi_operation: expected,
                        source: es,
                        claim: ec,
                    },
                ) => {
                    if psi_operation != expected || source != es || claim != ec {
                        return Err(LegalizationError::custody());
                    }
                }
                (
                    TargetUnitOperation::SplitActivationClaim {
                        psi_operation,
                        result_home,
                        source,
                        claim,
                        bound_at,
                        committed,
                        covered_case,
                        rejected_case,
                    },
                    AbstractOperation::SplitActivationClaim {
                        psi_operation: expected,
                        result,
                        source: expected_source,
                        claim: expected_claim,
                        bound_at: expected_bound,
                        committed_at,
                        covered_case: expected_covered,
                        rejected_case: expected_rejected,
                    },
                ) => {
                    if psi_operation != expected
                        || source != expected_source
                        || claim != expected_claim
                        || bound_at != expected_bound
                        || covered_case != expected_covered
                        || rejected_case != expected_rejected
                        || !same_result_home(result_home, optimized, result.place, plan)?
                        || !sources
                            .matching(*committed_at)
                            .any(|(value, source)| value == committed_at && source == committed)
                    {
                        return Err(LegalizationError::custody());
                    }
                }
                (
                    TargetUnitOperation::MergeActivationClaim {
                        psi_operation,
                        result_home,
                        lower,
                        upper,
                        lower_claim,
                        upper_claim,
                        covered_case,
                        rejected_case,
                    },
                    AbstractOperation::MergeActivationClaim {
                        psi_operation: expected,
                        result,
                        lower: expected_lower,
                        upper: expected_upper,
                        lower_claim: expected_lower_claim,
                        upper_claim: expected_upper_claim,
                        covered_case: expected_covered,
                        rejected_case: expected_rejected,
                    },
                ) => {
                    if psi_operation != expected
                        || lower != expected_lower
                        || upper != expected_upper
                        || lower_claim != expected_lower_claim
                        || upper_claim != expected_upper_claim
                        || covered_case != expected_covered
                        || rejected_case != expected_rejected
                        || !same_result_home(result_home, optimized, result.place, plan)?
                    {
                        return Err(LegalizationError::custody());
                    }
                }

                (
                    TargetUnitOperation::EstablishNativeCodeLayout {
                        psi_operation,
                        result_home,
                        machine,
                    },
                    AbstractOperation::EstablishNativeCodeLayout {
                        psi_operation: expected_operation,
                        result,
                        machine: expected_machine,
                    },
                ) => {
                    if psi_operation != expected_operation
                        || machine != expected_machine
                        || !same_result_home(result_home, optimized, result.place, plan)?
                        || !plan
                            .functions
                            .iter()
                            .any(|function| function.machine == *machine)
                    {
                        return Err(LegalizationError::custody());
                    }
                }

                (
                    TargetUnitOperation::EstablishStructuralArray {
                        psi_operation,
                        result_home,
                        elements,
                    },
                    AbstractOperation::EstablishStructuralArray {
                        psi_operation: expected_operation,
                        result,
                        elements: expected_elements,
                    },
                ) => {
                    let declaration = plan
                        .structural_type(result.structural_type)
                        .ok_or(LegalizationError::custody())?;
                    let terminal_psi::StructuralTypeShape::FixedArray {
                        element,
                        length: terminal_psi::ArrayExtent::Literal(length),
                    } = declaration.shape
                    else {
                        return Err(LegalizationError::custody());
                    };
                    if psi_operation != expected_operation
                        || elements != expected_elements
                        || u64::try_from(elements.len()).ok() != Some(length)
                        || !same_result_home(result_home, optimized, result.place, plan)?
                    {
                        return Err(LegalizationError::custody());
                    }
                    for argument in elements {
                        let actual = owned_projected_field_source(
                            optimized,
                            argument,
                            &unit.structural_types,
                        )?;
                        if argument.access != terminal_psi::StructuralAccess::Owned
                            || argument.place == result.place
                            || terminal_semantics::borrowed_referent_presentation(
                                unit.structural_types.iter(),
                                actual,
                                element,
                            ) != Some(
                                terminal_semantics::BorrowedReferentPresentation::Unchanged,
                            )
                        {
                            return Err(LegalizationError::custody());
                        }
                    }
                }

                (
                    TargetUnitOperation::EstablishRecord {
                        psi_operation,
                        result_home,
                        fields,
                    },
                    AbstractOperation::EstablishRecord {
                        psi_operation: expected_operation,
                        result,
                        fields: expected_fields,
                    },
                ) => {
                    let declaration = plan
                        .structural_type(result.structural_type)
                        .ok_or(LegalizationError::custody())?;
                    let terminal_psi::StructuralTypeShape::Record {
                        fields: declarations,
                        ..
                    } = &declaration.shape
                    else {
                        return Err(LegalizationError::custody());
                    };
                    let declarations = declarations
                        .iter()
                        .filter(|field| !field.relevance.is_erased());
                    if psi_operation != expected_operation
                        || fields != expected_fields
                        || fields.len() != declarations.clone().count()
                        || !same_result_home(result_home, optimized, result.place, plan)?
                        || fields.iter().zip(declarations).any(|(field, declaration)| {
                            field.field != declaration.id
                                || declaration.relevance.is_erased()
                                || match &field.value {
                                    terminal_psi::RecordFieldValue::Scalar {
                                        value: operand,
                                        range_obligation,
                                    } => {
                                        terminal_semantics::scalar_field_requires_evidence(
                                            &declaration.field_type,
                                        ) != range_obligation.is_some()
                                            || !sources.matching(*operand).any(|(value, source)| {
                                                *value == *operand
                                                    && declaration.field_type.scalar_type()
                                                        == Some(source.scalar_type())
                                            })
                                    }
                                    terminal_psi::RecordFieldValue::Structural(argument) => {
                                        validate_owned_field_operand(
                                            optimized,
                                            result.place,
                                            argument,
                                            declaration,
                                            &unit.structural_types,
                                        )
                                        .is_err()
                                    }
                                }
                        })
                    {
                        return Err(LegalizationError::custody());
                    }
                }
                (
                    TargetUnitOperation::EstablishScalarArray {
                        psi_operation,
                        result_home,
                        elements,
                    },
                    AbstractOperation::EstablishScalarArray {
                        psi_operation: expected_operation,
                        result,
                        elements: expected_elements,
                    },
                ) => {
                    let (scalar_type, count, _) =
                        scalar_graph_input::scalar_arrays::shape(result, plan)?;
                    if psi_operation != expected_operation
                        || elements != expected_elements
                        || u64::try_from(elements.len()).ok() != Some(count)
                        || !same_result_home(result_home, optimized, result.place, plan)?
                        || elements.iter().any(|element| {
                            !sources.matching(*element).any(|(value, source)| {
                                value == element && source.scalar_type() == scalar_type
                            })
                        })
                    {
                        return Err(LegalizationError::custody());
                    }
                }
                (
                    TargetUnitOperation::StructuralLeafCopy {
                        psi_operation,
                        result_home,
                        source,
                        path,
                        byte_offset,
                        indices,
                    },
                    abstracted @ (AbstractOperation::StructuralLeafCopy { .. }
                    | AbstractOperation::MoveStructuralValue { .. }
                    | AbstractOperation::TransformContent { .. }
                    | AbstractOperation::BoundaryCall { .. }),
                ) => {
                    let owned_path;
                    let (expected_operation, result, expected_source, expected_path) =
                        match abstracted {
                            AbstractOperation::StructuralLeafCopy {
                                psi_operation,
                                result,
                                source,
                                path,
                            } => (psi_operation, result, source, path),
                            AbstractOperation::MoveStructuralValue {
                                psi_operation,
                                result,
                                source,
                            }
                            | AbstractOperation::TransformContent {
                                psi_operation,
                                result,
                                source,
                                ..
                            } => {
                                owned_path = source.path.clone().into();
                                (psi_operation, result, &source.place, &owned_path)
                            }
                            AbstractOperation::BoundaryCall {
                                psi_operation,
                                boundary,
                                arguments,
                                structural_arguments,
                                result:
                                    abstract_operations::AbstractBoundaryResult::Structural(result),
                                ..
                            } => {
                                let declaration = plan
                                    .boundary_machines
                                    .iter()
                                    .find(|row| row.id == *boundary)
                                    .ok_or(LegalizationError::custody())?;
                                let contract = terminal_semantics::empty_run_release_contract(
                                    plan.structural_types.as_slice(),
                                    declaration,
                                )
                                .map_err(|_| LegalizationError::custody())?;
                                let [source] = structural_arguments.as_slice() else {
                                    return Err(LegalizationError::custody());
                                };
                                if !arguments.is_empty()
                                    || source.access != terminal_psi::StructuralAccess::Owned
                                    || result.structural_type != contract.result.structural_type
                                {
                                    return Err(LegalizationError::custody());
                                }
                                let mut projection = source.path.clone();
                                projection.push(terminal_psi::StructuralPathSegment::Field(
                                    contract.storage,
                                ));
                                owned_path = projection.into();
                                (psi_operation, result, &source.place, &owned_path)
                            }
                            _ => return Err(LegalizationError::custody()),
                        };
                    let (expected_offset, expected_shape, expected_indices) =
                        scalar_graph_input::structural_case::structural_transport_layout(
                            optimized,
                            *expected_operation,
                            *expected_source,
                            expected_path,
                            result,
                            plan,
                        )?;
                    let expected_indices = expected_indices
                        .into_iter()
                        .map(|element| {
                            let scalar_type = crate::legalization::runtime_indices::defined_type(
                                optimized,
                                element.index,
                            )
                            .filter(|scalar| {
                                matches!(scalar, semantic_vocabulary::ScalarType::Integer(integer)
                                    if integer.bits() <= 64)
                            })
                            .ok_or(LegalizationError::custody())?;
                            Ok::<_, LegalizationError>((element.index, scalar_type, element.stride))
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    if psi_operation != expected_operation
                        || source != expected_source
                        || path != expected_path
                        || *byte_offset != expected_offset
                        || indices.len() != expected_indices.len()
                        || indices.iter().zip(&expected_indices).any(
                            |(index, (value, scalar_type, stride))| {
                                index.stride != *stride
                                    || index.operand.source_value() != *value
                                    || index.operand.scalar_type() != *scalar_type
                            },
                        )
                        || !same_result_home(result_home, optimized, result.place, plan)?
                        || result_home.layout.shape() != expected_shape
                    {
                        return Err(LegalizationError::custody());
                    }
                }
                (
                    TargetUnitOperation::EstablishCase {
                        psi_operation,
                        result_home,
                        result_case,
                        fields,
                    },
                    AbstractOperation::EstablishCase {
                        psi_operation: expected_operation,
                        result,
                        result_case: expected_case,
                        fields: expected_fields,
                    },
                ) => {
                    if psi_operation != expected_operation
                        || result_case != expected_case
                        || fields != expected_fields
                        || !same_result_home(result_home, optimized, result.place, plan)?
                    {
                        return Err(LegalizationError::custody());
                    }
                    let declaration = plan
                        .structural_type(result.structural_type)
                        .ok_or(LegalizationError::custody())?;
                    let (common, cases) = match &declaration.shape {
                        terminal_psi::StructuralTypeShape::Sum { cases } => (&[][..], cases),
                        terminal_psi::StructuralTypeShape::Mixed { fields, cases } => {
                            (fields.as_slice(), cases)
                        }
                        _ => return Err(LegalizationError::custody()),
                    };
                    let case = cases
                        .iter()
                        .find(|case| case.id == *result_case)
                        .ok_or(LegalizationError::custody())?;
                    let roster = common
                        .iter()
                        .chain(&case.fields)
                        .filter(|field| !field.relevance.is_erased())
                        .collect::<Vec<_>>();
                    if fields.len() != roster.len() {
                        return Err(LegalizationError::custody());
                    }
                    for (field, declared) in fields.iter().zip(roster) {
                        if field.field != declared.id || declared.relevance.is_erased() {
                            return Err(LegalizationError::custody());
                        }
                        match &field.value {
                            terminal_psi::RecordFieldValue::Scalar {
                                value,
                                range_obligation,
                            } => {
                                if !sources.matching(*value).any(|(source_value, source)| {
                                    *source_value == *value
                                        && declared.field_type.scalar_type()
                                            == Some(source.scalar_type())
                                }) || terminal_semantics::scalar_field_requires_evidence(
                                    &declared.field_type,
                                ) != range_obligation.is_some()
                                    || range_obligation.is_some_and(|obligation| {
                                        accepted_fact(unit, optimized, *psi_operation, obligation)
                                            .is_err()
                                            || !references_obligation(
                                                optimized,
                                                *psi_operation,
                                                obligation,
                                            )
                                    })
                                {
                                    return Err(LegalizationError::custody());
                                }
                            }
                            terminal_psi::RecordFieldValue::Structural(argument) => {
                                validate_owned_field_operand(
                                    optimized,
                                    result.place,
                                    argument,
                                    declared,
                                    &unit.structural_types,
                                )?;
                            }
                        }
                    }
                }
                _ => return Err(LegalizationError::custody()),
            }
            Ok(())
        }

        fn validate_owned_field_operand(
            function: &PsiOptimizationFunction,
            destination: semantic_vocabulary::PlaceId,
            argument: &terminal_psi::StructuralArgument,
            declaration: &terminal_psi::StructuralFieldDeclaration,
            types: &[terminal_psi::StructuralTypeDeclaration],
        ) -> Result<(), LegalizationError> {
            let field = &declaration.field_type;
            if argument.access != terminal_psi::StructuralAccess::Owned
                || argument.place == destination
            {
                return Err(LegalizationError::custody());
            }
            if let Some(expected) = field.as_structural() {
                let actual = owned_projected_field_source(function, argument, types)?;
                return if terminal_semantics::borrowed_referent_presentation(
                    types.iter(),
                    actual,
                    expected,
                ) == Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
                {
                    Ok(())
                } else {
                    Err(LegalizationError::custody())
                };
            }
            if !argument.path.is_empty() {
                return Err(LegalizationError::custody());
            }
            if let Some(carrier) = field.byte_sequence_carrier()
                && carrier.is_borrowed()
            {
                let expected =
                    terminal_semantics::inline_carrier_declaration(types.iter(), declaration)
                        .ok_or(LegalizationError::custody())?;
                return if terminal_semantics::borrowed_referent_presentation(
                    types.iter(),
                    owned_projected_field_source(function, argument, types)?,
                    expected.id,
                ) == Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
                {
                    Ok(())
                } else {
                    Err(LegalizationError::custody())
                };
            }
            let declaration = function
                .structural_place(argument.place)
                .ok_or(LegalizationError::custody())?;
            let actual = optimization_unit::declared_structural_type(function, declaration)
                .ok_or(LegalizationError::custody())?;
            let access =
                optimization_unit_semantics::structural_place_access(function, argument.place)
                    .ok_or(LegalizationError::custody())?;
            let actual = &terminal_psi::find_structural_type(types, actual)
                .ok_or(LegalizationError::custody())?
                .shape;
            let compatible = match field.byte_sequence_carrier() {
                Some(expected @ terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }) => {
                    access == terminal_psi::StructuralAccess::Owned
                        && *actual == terminal_psi::StructuralTypeShape::ByteSequence(expected)
                }
                _ => false,
            };
            if !compatible {
                return Err(LegalizationError::custody());
            }
            Ok(())
        }

        fn owned_field_source(
            function: &PsiOptimizationFunction,
            place: semantic_vocabulary::PlaceId,
        ) -> Result<semantic_vocabulary::StructuralTypeId, LegalizationError> {
            let parameters = function
                .structural_parameters
                .iter()
                .chain(function.block_structural_parameters())
                .filter(|parameter| parameter.place == place)
                .map(|parameter| {
                    (
                        parameter.structural_type,
                        parameter.access == terminal_psi::StructuralAccess::Owned,
                    )
                });
            let result = scalar_graph_input::structural_case::source_result(function, place)
                .ok()
                .map(|(_, result)| (result.structural_type, true));
            let mut sources = parameters.chain(result);
            let (structural_type, owned) = sources.next().ok_or(LegalizationError::custody())?;
            if sources.next().is_some() || !owned {
                return Err(LegalizationError::custody());
            }
            Ok(structural_type)
        }

        pub(crate) fn owned_projected_field_source(
            function: &PsiOptimizationFunction,
            argument: &terminal_psi::StructuralArgument,
            types: &[terminal_psi::StructuralTypeDeclaration],
        ) -> Result<semantic_vocabulary::StructuralTypeId, LegalizationError> {
            if argument.path.is_empty()
                && argument.access == terminal_psi::StructuralAccess::Owned
                && optimization_unit_semantics::structural_place_access(function, argument.place)
                    .is_some()
                && let Some(declaration) = function.structural_place(argument.place)
                && let Some(actual) =
                    optimization_unit::declared_structural_type(function, declaration)
                && terminal_semantics::descriptor_results::is_descriptor(types.iter(), actual)
            {
                return Ok(actual);
            }
            let root = owned_field_source(function, argument.place)?;
            if argument.path.is_empty() {
                return Ok(root);
            }
            if argument.access != terminal_psi::StructuralAccess::Owned {
                return Err(LegalizationError::custody());
            }
            let (_, selected) =
                terminal_semantics::canonical_static_projection(types.iter(), root, &argument.path)
                    .ok_or(LegalizationError::custody())?;
            Ok(selected.id)
        }
    }
    pub(crate) mod borrowed_windows {
        use super::aggregate_results::owned_projected_field_source;
        use super::{
            AbstractOperation, AbstractOperationPlan, PsiOptimizationFunction, TargetUnitOperation,
        };
        use crate::LegalizationError;
        use crate::legalization::scalar_graph_input;
        use scalar_graph_input::borrowed_windows::moved;
        use terminal_psi::StructuralAccess;

        pub(crate) fn validate(
            target: &TargetUnitOperation,
            abstracted: &AbstractOperation,
            optimized: &PsiOptimizationFunction,
            sources: &crate::legalization_target::control_flow::sources::AvailableSources<'_>,
            plan: &AbstractOperationPlan,
            custody: &scalar_graph_input::reference_custody::Custody,
        ) -> Result<(), LegalizationError> {
            match (target, abstracted) {
                (
                    TargetUnitOperation::StructuralLeafCopy {
                        psi_operation,
                        result_home,
                        source,
                        path,
                        byte_offset,
                        indices,
                    },
                    AbstractOperation::MoveStructuralField {
                        psi_operation: expected_operation,
                        result,
                        source: root,
                        path: spelled,
                        field,
                    },
                ) => {
                    let extent = moved(optimized, root, spelled, *field, result, plan, custody)?;
                    if psi_operation != expected_operation
                        || *source != root.place()
                        || *path != abstract_operations::StructuralCopyPath::Ordinary(extent.path)
                        || *byte_offset != extent.byte_offset
                        || !super::runtime_indices_rejoin(indices, &extent.elements, sources)
                        || *result_home
                            != scalar_graph_input::aggregate_results::result_home(
                                optimized,
                                result.place,
                                plan,
                            )?
                        || result_home.layout.shape() != extent.shape
                    {
                        return Err(LegalizationError::custody());
                    }
                }
                (
                    TargetUnitOperation::StoreStructuralField {
                        psi_operation,
                        destination,
                        path,
                        field,
                        value: retained_value,
                        byte_offset,
                        indices,
                    },
                    AbstractOperation::StoreStructuralField {
                        disposition,
                        psi_operation: expected_operation,
                        destination: expected_destination,
                        path: expected_path,
                        field: expected_field,
                        value,
                    },
                ) => {
                    let extent = scalar_graph_input::borrowed_windows::extent(
                        optimized,
                        expected_destination,
                        expected_path,
                        *expected_field,
                        plan,
                        custody,
                        disposition.required_access(),
                    )?;
                    if psi_operation != expected_operation
                        || destination != expected_destination
                        || path != expected_path
                        || field != expected_field
                        || *byte_offset != extent.byte_offset
                        || !super::runtime_indices_rejoin(indices, &extent.elements, sources)
                        || value.access != StructuralAccess::Owned
                        || !value.path.is_empty()
                        || *retained_value != value.place
                    {
                        return Err(LegalizationError::custody());
                    }
                    if owned_projected_field_source(optimized, value, &plan.structural_types)?
                        != extent.field_type
                    {
                        return Err(LegalizationError::custody());
                    }
                }
                _ => return Err(LegalizationError::custody()),
            }
            Ok(())
        }
    }
    pub(crate) mod direct_calls {
        use super::{
            AbstractOperation, AbstractOperationPlan, LegalizationError, PsiOptimizationFunction,
            PsiOptimizationUnit, Source, TargetOperationPlan, TargetUnitOperation, callee_plan,
        };
        use target_operations::{NativeCallOrigin, TargetCallResult};

        pub(crate) mod results {
            use abstract_operations::{
                AbstractOperation, AbstractOperationPlan, AbstractResult,
                AbstractStructuralCallResult,
            };
            use calling_conventions::CallPlan;
            use optimization_unit::PsiOptimizationFunction;
            use target_operations::{TargetCallResult, TargetUnitScalarHomeRequirement};

            use crate::LegalizationError;
            use crate::legalization::scalar_graph_input::{
                aggregate_results, reference_custody, scalar_shape,
            };

            pub(crate) fn validate(
                retained: &TargetCallResult,
                source: &AbstractOperation,
                callee: Option<&PsiOptimizationFunction>,
                call_plan: &CallPlan,
                custody: &reference_custody::Custody,
                caller: &PsiOptimizationFunction,
                plan: &AbstractOperationPlan,
            ) -> Result<(), LegalizationError> {
                match (retained, source) {
                    (
                        TargetCallResult::Unit,
                        AbstractOperation::CallStructural {
                            result: AbstractStructuralCallResult::Unit,
                            ..
                        },
                    ) => {
                        if call_plan.result.is_some() {
                            return Err(LegalizationError::custody());
                        }
                    }
                    (
                        TargetCallResult::Scalar(home),
                        AbstractOperation::Call {
                            psi_operation,
                            result,
                            scalar_type,
                            ..
                        },
                    ) => scalar(
                        *home,
                        *psi_operation,
                        AbstractResult {
                            value: *result,
                            scalar_type: *scalar_type,
                        },
                        call_plan,
                    )?,
                    (
                        TargetCallResult::Scalar(home),
                        AbstractOperation::CallStructural {
                            psi_operation,
                            result: AbstractStructuralCallResult::Scalar(result),
                            ..
                        },
                    ) => scalar(*home, *psi_operation, *result, call_plan)?,
                    (
                        TargetCallResult::Structural {
                            result,
                            callee_result,
                            result_home,
                            reference_results,
                        },
                        AbstractOperation::CallStructural {
                            psi_operation,
                            callee: target,
                            result:
                                AbstractStructuralCallResult::Structural {
                                    result: expected_result,
                                    ..
                                },
                            structural_arguments,
                            ..
                        },
                    ) => {
                        let expected_home = Some(aggregate_results::result_home(
                            caller,
                            expected_result.place,
                            plan,
                        )?);
                        let expected_references = match (callee_result, callee, target) {
                            (
                                target_operations::TargetStructuralResultContract::Machine(
                                    declaration,
                                ),
                                Some(callee),
                                abstract_operations::AbstractCallTarget::Machine(machine),
                            ) => {
                                if callee.machine != *machine
                                    || callee.result.structural() != Some(declaration)
                                    || result.structural_type != declaration.structural_type
                                    || result.multiplicity != declaration.multiplicity
                                {
                                    return Err(LegalizationError::custody());
                                }
                                reference_custody::reference_results(
                                    caller,
                                    Some(callee),
                                    terminal_semantics::ReferenceCallContract::Machine {
                                        parameters: &callee.structural_parameters,
                                        sources: &declaration.reference_sources,
                                    },
                                    structural_arguments,
                                    expected_result,
                                    *psi_operation,
                                    custody,
                                    &plan.structural_types,
                                )
                                .map_err(|error| {
                                    error.attributed(caller.machine, Some(*psi_operation))
                                })?
                            }
                            (
                                target_operations::TargetStructuralResultContract::Callable(declaration),
                                None,
                                abstract_operations::AbstractCallTarget::Callable { contract, .. },
                            ) => {
                                let contract = plan.callables.contract(*contract).ok_or(LegalizationError::custody())?;
                                if contract.result != terminal_psi::BoundaryMachineResult::Structural(declaration.clone())
                                    || !contract.result.matches_operation_result(
                                        &terminal_psi::OperationResult::Structural(expected_result.clone()))
                                {
                                    return Err(LegalizationError::custody());
                                }
                                reference_custody::reference_results(
                                    caller, None, terminal_semantics::ReferenceCallContract::Callable(contract),
                                    structural_arguments, expected_result, *psi_operation, custody, &plan.structural_types,
                                ).map_err(|error| error.attributed(caller.machine, Some(*psi_operation)))?
                            }
                            (
                                target_operations::TargetStructuralResultContract::Requirement(
                                    declaration,
                                ),
                                None,
                                abstract_operations::AbstractCallTarget::Dynamic {
                                    interface,
                                    slot,
                                    ..
                                },
                            ) => {
                                let requirement = plan
                                    .dynamic_dispatch
                                    .interface(*interface)
                                    .and_then(|interface| interface.slots.get(*slot as usize))
                                    .ok_or(LegalizationError::custody())?;
                                if requirement.result
                                    != terminal_psi::BoundaryMachineResult::Structural(
                                        declaration.clone(),
                                    )
                                    || !requirement.matches_operation_result(
                                        &terminal_psi::OperationResult::Structural(
                                            expected_result.clone(),
                                        ),
                                    )
                                {
                                    return Err(LegalizationError::custody());
                                }
                                reference_custody::reference_results(
                                    caller,
                                    None,
                                    terminal_semantics::ReferenceCallContract::Requirement(
                                        requirement,
                                    ),
                                    structural_arguments,
                                    expected_result,
                                    *psi_operation,
                                    custody,
                                    &plan.structural_types,
                                )
                                .map_err(|error| {
                                    error.attributed(caller.machine, Some(*psi_operation))
                                })?
                            }
                            _ => return Err(LegalizationError::custody()),
                        };
                        if result != expected_result
                            || *result_home != expected_home
                            || *reference_results != expected_references
                        {
                            if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                                eprintln!(
                                    "CHECKSITE call-reference-result machine={:?} operation={:?} same_result={} same_home={} retained={:?} reconstructed={:?}",
                                    caller.machine,
                                    psi_operation,
                                    result == expected_result,
                                    *result_home == expected_home,
                                    reference_results,
                                    expected_references
                                );
                            }
                            return Err(LegalizationError::custody());
                        }
                    }
                    _ => return Err(LegalizationError::custody()),
                }
                Ok(())
            }

            fn scalar(
                retained: TargetUnitScalarHomeRequirement,
                operation: semantic_vocabulary::OperationId,
                result: AbstractResult,
                call_plan: &CallPlan,
            ) -> Result<(), LegalizationError> {
                let shape =
                    scalar_shape(result.scalar_type).ok_or_else(|| LegalizationError::custody())?;
                let expected = TargetUnitScalarHomeRequirement {
                    defining_operation: operation,
                    source_value: result.value,
                    scalar_type: result.scalar_type,
                    shape,
                };
                if retained != expected
                    || call_plan.result.as_ref().map(|placement| placement.shape) != Some(shape)
                {
                    return Err(LegalizationError::custody());
                }
                Ok(())
            }
        }

        pub(crate) fn validate(
            target: &TargetUnitOperation,
            abstracted: &AbstractOperation,
            sources: &mut crate::legalization_target::control_flow::sources::AvailableSources<'_>,
            custody: &crate::legalization::scalar_graph_input::reference_custody::Custody,
            optimized: &PsiOptimizationFunction,
            native: &TargetOperationPlan,
            plan: &AbstractOperationPlan,
            unit: &PsiOptimizationUnit,
        ) -> Result<(), LegalizationError> {
            let TargetUnitOperation::Call {
                origin: NativeCallOrigin::Authored,
                psi_operation,
                target: callee,
                call_plan,
                result,
                scalar_arguments,
                arguments,
                access_bindings,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            } = target
            else {
                return Err(LegalizationError::custody());
            };
            let (actual, called, values, structural_arguments, claims, requirements, crashes) =
                match abstracted {
                    AbstractOperation::CallStructural {
                        psi_operation,
                        callee,
                        arguments,
                        structural_arguments,
                        claim_transfers,
                        requirement_obligations,
                        crash_continuations,
                        ..
                    } => (
                        *psi_operation,
                        *callee,
                        arguments.as_slice(),
                        structural_arguments.as_slice(),
                        claim_transfers.as_slice(),
                        requirement_obligations,
                        crash_continuations,
                    ),
                    AbstractOperation::Call {
                        psi_operation,
                        callee,
                        arguments,
                        requirement_obligations,
                        crash_continuations,
                        ..
                    } => (
                        *psi_operation,
                        abstract_operations::AbstractCallTarget::Machine(*callee),
                        arguments.as_slice(),
                        &[][..],
                        &[][..],
                        requirement_obligations,
                        crash_continuations,
                    ),
                    _ => return Err(LegalizationError::custody()),
                };
            let expected = callee_plan(called, native, plan, unit)?;
            let signature =
                crate::legalization::scalar_graph_input::call_signature::signature(called, plan)?;
            let callee_function = called.machine().and_then(|machine| unit.function(machine));
            if psi_operation != &actual
                || *callee
                    != crate::legalization::scalar_graph_input::call_signature::native_target(
                        called,
                    )
                || call_plan != &expected
                || claim_transfers != claims
                || requirement_obligations != requirements
                || crash_continuations != crashes
                || arguments.len() != structural_arguments.len()
                || signature.structural_parameter_count() != arguments.len()
                || signature.scalar_parameter_count() != values.len()
                || scalar_arguments.len() != values.len()
                || expected.parameters.len() != values.len() + arguments.len()
                || scalar_arguments
                    .iter()
                    .zip(values)
                    .zip(&expected.parameters)
                    .enumerate()
                    .any(|(position, ((argument, value), placement))| {
                        argument.parameter_index != position as u32
                            || argument.placement != *placement
                            || Some(argument.source.scalar_type())
                                != signature.scalar_parameter(position)
                            || !sources.matching(*value).any(|(source, definition)| {
                                source == value && *definition == argument.source
                            })
                    })
            {
                return Err(LegalizationError::custody());
            }
            if callee_function.is_none()
                && (0..signature.structural_parameter_count()).any(|position| {
                    signature
                        .structural_parameter(position)
                        .is_some_and(|formal| {
                            plan.structural_types
                                .access_views()
                                .iter()
                                .any(|plan| plan.carrier == formal.structural_type)
                        })
                })
            {
                return Err(LegalizationError::custody());
            }
            let expected_bindings = if let Some(callee) = callee_function {
                crate::legalization::scalar_graph_input::reference_custody::access_call_custody(
                    optimized,
                    callee,
                    &plan.structural_types,
                    structural_arguments,
                    custody,
                )?
            } else {
                Vec::new()
            };
            if access_bindings != &expected_bindings {
                return Err(LegalizationError::custody());
            }
            results::validate(
                result,
                abstracted,
                callee_function,
                &expected,
                custody,
                optimized,
                plan,
            )?;
            let reconstruct = if matches!(result, TargetCallResult::Structural { .. }) {
                crate::legalization::scalar_graph_input::aggregate_results::call_argument
            } else {
                crate::legalization::scalar_graph_input::structural_call::argument_at
            };
            for (position, (argument, semantic)) in
                arguments.iter().zip(structural_arguments).enumerate()
            {
                if reconstruct(
                    semantic,
                    position,
                    *psi_operation,
                    optimized,
                    signature,
                    &expected,
                    native,
                    plan,
                    custody,
                )? != *argument
                {
                    return Err(LegalizationError::custody());
                }
            }
            if let Some(selector) = called.selector()
                && !sources.matching(selector).any(|(value, source)| {
                    *value == selector
                        && source.scalar_type() == target_operations::selector_carrier_type()
                })
            {
                return Err(LegalizationError::custody());
            }
            if let Some(home) = result.scalar_home() {
                sources.push((home.source_value, Source::Home(*home)));
            }
            Ok(())
        }
    }
    pub(crate) mod ieee_float {
        use super::super::{AbstractOperation, TargetUnitOperation};
        use super::Source;
        use crate::legalization::scalar_graph_input::{LegalizationError, ScalarType, ValueShape};
        use semantic_vocabulary::IeeeFloatFormat;

        pub(crate) fn validate(
            target: &TargetUnitOperation,
            abstracted: &AbstractOperation,
            sources: &mut crate::legalization_target::control_flow::sources::AvailableSources<'_>,
        ) -> Result<(), LegalizationError> {
            match (target, abstracted) {
                (
                    TargetUnitOperation::FloatResultPolicy {
                        psi_operation,
                        result_home,
                        format,
                        policy,
                    },
                    AbstractOperation::FloatResultPolicy {
                        psi_operation: expected_operation,
                        result,
                        format: expected_format,
                        policy: expected_policy,
                    },
                ) => {
                    let scalar_type = ScalarType::IeeeFloat(*expected_format);
                    let shape = ValueShape::float(match expected_format {
                        IeeeFloatFormat::Binary32 => 4,
                        IeeeFloatFormat::Binary64 => 8,
                    });
                    if psi_operation != expected_operation
                        || result_home.defining_operation != *expected_operation
                        || result_home.source_value != *result
                        || result_home.scalar_type != scalar_type
                        || result_home.shape != shape
                        || format != expected_format
                        || policy.semantic_policy() != *expected_policy
                        || !policy
                            .semantic_policy()
                            .accepts_operand_types(*format, |value| {
                                policy
                                    .operands()
                                    .find(|operand| operand.source_value() == value)
                                    .map(|operand| operand.scalar_type())
                            })
                        || policy.operands().any(|operand| {
                            !sources
                                .matching(operand.source_value())
                                .filter(|(value, _)| *value == operand.source_value())
                                .map(|(_, source)| source)
                                .eq(std::iter::once(&operand))
                        })
                    {
                        return Err(LegalizationError::custody());
                    }
                    sources.push((*result, Source::Home(*result_home)));
                }
                (
                    TargetUnitOperation::IeeeFloatCompare {
                        result_home,
                        comparison,
                        format,
                        left,
                        right,
                    },
                    AbstractOperation::IeeeFloatCompare {
                        psi_operation,
                        result,
                        comparison: expected_comparison,
                        format: expected_format,
                        left: expected_left,
                        right: expected_right,
                    },
                ) => {
                    if result_home.defining_operation != *psi_operation
                        || result_home.source_value != *result
                        || result_home.scalar_type != ScalarType::Boolean
                        || result_home.shape != ValueShape::integer(1, 1)
                        || comparison != expected_comparison
                        || format != expected_format
                        || left.source_value() != *expected_left
                        || right.source_value() != *expected_right
                        || left.scalar_type() != ScalarType::IeeeFloat(*format)
                        || right.scalar_type() != ScalarType::IeeeFloat(*format)
                        || !sources
                            .matching(*expected_left)
                            .filter(|(value, _)| value == expected_left)
                            .map(|(_, source)| source)
                            .eq(std::iter::once(left))
                        || !sources
                            .matching(*expected_right)
                            .filter(|(value, _)| value == expected_right)
                            .map(|(_, source)| source)
                            .eq(std::iter::once(right))
                    {
                        return Err(LegalizationError::custody());
                    }
                    sources.push((*result, Source::Home(*result_home)));
                }
                (
                    TargetUnitOperation::ScalarRepresentationView {
                        result_home,
                        operand,
                    },
                    AbstractOperation::ScalarRepresentationView {
                        psi_operation,
                        result,
                        source_type,
                        target_type,
                        operand: expected_operand,
                    },
                ) => {
                    if terminal_semantics::ScalarRepresentationViewPair::classify(
                        *source_type,
                        *target_type,
                    )
                    .is_none()
                        || result_home.defining_operation != *psi_operation
                        || result_home.source_value != *result
                        || result_home.scalar_type != *target_type
                        || Some(result_home.shape)
                            != crate::legalization::scalar_graph_input::scalar_shape(*target_type)
                        || operand.source_value() != *expected_operand
                        || operand.scalar_type() != *source_type
                        || !sources
                            .matching(*expected_operand)
                            .filter(|(value, _)| value == expected_operand)
                            .map(|(_, source)| source)
                            .eq(std::iter::once(operand))
                    {
                        return Err(LegalizationError::custody());
                    }
                    sources.push((*result, Source::Home(*result_home)));
                }
                (
                    TargetUnitOperation::IeeeFloatConstant {
                        psi_operation,
                        result,
                        value,
                    },
                    AbstractOperation::IeeeFloatConstant {
                        psi_operation: expected_operation,
                        result: expected_result,
                        value: expected_value,
                    },
                ) if psi_operation == expected_operation
                    && result == expected_result
                    && value == expected_value =>
                {
                    sources.push((
                        *result,
                        Source::IeeeFloatImmediate {
                            defining_operation: *psi_operation,
                            source_value: *result,
                            value: *value,
                        },
                    ));
                }
                _ => return Err(LegalizationError::custody()),
            }
            Ok(())
        }
    }
    pub(crate) mod primitive_store {
        use crate::LegalizationError;
        use abstract_operations::AbstractOperation;
        use optimization_unit::PsiOptimizationUnit;
        use target_operations::{
            TargetStructuralParameter, TargetUnitOperation,
            TargetUnitScalarArgumentSource as Source,
            TargetUnitWriteOnlyPrimitiveStoreSource as PrimitiveSource,
        };

        pub(crate) fn validate(
            target: &TargetUnitOperation,
            abstracted: &AbstractOperation,
            parameters: &[TargetStructuralParameter],
            sources: &crate::legalization_target::control_flow::sources::AvailableSources<'_>,
            optimized: &optimization_unit::PsiOptimizationFunction,
            unit: &PsiOptimizationUnit,
        ) -> Result<(), LegalizationError> {
            let TargetUnitOperation::WriteOnlyPrimitiveStore {
                psi_operation,
                destination,
                path,
                destination_type,
                destination_storage,
                source,
                indices,
            } = target
            else {
                return Err(LegalizationError::custody());
            };
            let AbstractOperation::WriteOnlyPrimitiveStore {
                psi_operation: expected_operation,
                destination: expected_destination,
                path: expected_path,
                value,
                ..
            } = abstracted
            else {
                return Err(LegalizationError::custody());
            };
            let storage_matches = match (destination, destination_storage) {
                (
                    abstract_operations::StructuralScalarStoreDestination::Parameter(parameter),
                    target_operations::TargetStructuralArgumentSource::Placement(placement),
                ) => {
                    optimized.structural_parameters.contains(parameter)
                        && parameters.iter().any(|parameter| {
                            parameter.place == destination.place()
                                && &parameter.placement == placement
                        })
                }
                (
                    abstract_operations::StructuralScalarStoreDestination::Parameter(parameter),
                    target_operations::TargetStructuralArgumentSource::BlockParameter {
                        block,
                        place,
                    },
                ) => {
                    *place == parameter.place
                        && matches!(
                            parameter.access,
                            terminal_psi::StructuralAccess::MutableBorrow
                                | terminal_psi::StructuralAccess::WriteOnlyBorrow
                        )
                        && optimized.blocks.iter().any(|entry| {
                            entry.id == *block && entry.structural_parameters.contains(parameter)
                        })
                }
                (
                    abstract_operations::StructuralScalarStoreDestination::OwnedResult {
                        producer,
                        result,
                    },
                    target_operations::TargetStructuralArgumentSource::StructuralHome {
                        psi_operation,
                    },
                ) => {
                    psi_operation == producer
                        && crate::legalization::scalar_graph_input::structural_case::source_result(
                            optimized,
                            result.place,
                        )
                        .is_ok_and(|(operation, declared)| {
                            operation == *producer && declared == result
                        })
                }
                (
                    abstract_operations::StructuralScalarStoreDestination::OwnedBlockParameter {
                        block,
                        parameter,
                    },
                    target_operations::TargetStructuralArgumentSource::BlockParameter {
                        block: storage_block,
                        place,
                    },
                ) => {
                    storage_block == block
                        && *place == parameter.place
                        && matches!(crate::legalization::scalar_graph_input::structural_case::source_owner(optimized, parameter.place),
                            Ok(legalized_operations::LegalizedStructuralCaseSource::BlockParameter { block: owner, declaration })
                                if owner == *block && declaration == *parameter)
                }
                _ => false,
            };
            let declared_type = terminal_psi::find_structural_type(
                &unit.structural_types,
                expected_destination.structural_type(),
            )
            .ok_or(LegalizationError::custody())?;
            let geometry = crate::structural_inputs::structural_reference_input::primitive_store(
                expected_destination,
                expected_path,
                value.scalar_type,
                &unit.structural_types,
            )
            .ok_or(LegalizationError::custody())?;
            if psi_operation != expected_operation
                || destination != expected_destination
                || path != expected_path
                || destination_type != declared_type
                || !storage_matches
                || !super::runtime_indices_rejoin(indices, &geometry.elements, sources)
                || source.source_value() != value.value
                || source.scalar_type() != value.scalar_type
                || !(sources.matching(value.value).any(|(identity, expected)| {
                    *identity == value.value && source_is_exact(source, expected)
                }) || preceding_ieee_literal(source, *expected_operation, optimized))
            {
                return Err(LegalizationError::custody());
            }
            Ok(())
        }

        fn preceding_ieee_literal(
            source: &PrimitiveSource,
            store: semantic_vocabulary::OperationId,
            optimized: &optimization_unit::PsiOptimizationFunction,
        ) -> bool {
            let PrimitiveSource::IeeeFloatImmediate {
                defining_operation,
                source_value,
                value,
            } = source
            else {
                return false;
            };
            optimized.blocks.iter().any(|block| {
                let Some(store_position) = block.nodes.iter().position(|node| matches!(node.operation,
                    AbstractOperation::WriteOnlyPrimitiveStore { psi_operation, .. } if psi_operation == store)) else {
                    return false;
                };
                block.nodes[..store_position].iter().any(|node| matches!(node.operation,
                    AbstractOperation::IeeeFloatConstant { psi_operation, result, value: literal }
                        if psi_operation == *defining_operation && result == *source_value && literal == *value))
            })
        }

        fn source_is_exact(source: &PrimitiveSource, expected: &Source) -> bool {
            match (source, expected) {
                (
                    PrimitiveSource::Parameter {
                        parameter_index,
                        source_value,
                        scalar_type,
                    },
                    Source::Parameter {
                        parameter_index: expected_index,
                        source_value: expected_value,
                        scalar_type: expected_type,
                    },
                ) => {
                    parameter_index == expected_index
                        && source_value == expected_value
                        && scalar_type == expected_type
                }
                (
                    PrimitiveSource::IntegerImmediate {
                        defining_operation,
                        source_value,
                        scalar_type,
                        value,
                    },
                    Source::IntegerImmediate {
                        defining_operation: expected_operation,
                        source_value: expected_value,
                        scalar_type: expected_type,
                        value: expected_literal,
                    },
                ) => {
                    defining_operation == expected_operation
                        && source_value == expected_value
                        && scalar_type == expected_type
                        && value == expected_literal
                }
                (
                    PrimitiveSource::BooleanImmediate {
                        defining_operation,
                        source_value,
                        value,
                    },
                    Source::BooleanImmediate {
                        defining_operation: expected_operation,
                        source_value: expected_value,
                        value: expected_literal,
                    },
                ) => {
                    defining_operation == expected_operation
                        && source_value == expected_value
                        && value == expected_literal
                }
                (PrimitiveSource::Home(home), Source::Home(expected)) => home == expected,
                (PrimitiveSource::BlockParameter(value), Source::BlockParameter(expected)) => {
                    value == expected
                }
                _ => false,
            }
        }
    }
    pub(crate) fn validate_operation(
        function: &TargetFunction,
        target: &TargetUnitOperation,
        abstracted: &AbstractOperation,
        scalar_parameters: &[ScalarAbiValue],
        parameters: &[TargetStructuralParameter],
        sources: &mut crate::legalization_target::control_flow::sources::AvailableSources<'_>,
        custody: &crate::legalization::scalar_graph_input::reference_custody::Custody,
        function_inputs: &crate::legalization_function_inputs::FunctionInputs<'_>,
        native: &TargetOperationPlan,
        plan: &AbstractOperationPlan,
        unit: &PsiOptimizationUnit,
    ) -> Result<(), LegalizationError> {
        let optimized = function_inputs.function();
        if matches!(abstracted, AbstractOperation::BoundaryCall { .. }) {
            let node = optimized
                .block_nodes()
                .find(|node| &node.operation == abstracted)
                .ok_or(LegalizationError::custody())?;
            if let Some((operation, _)) =
                crate::legalization::scalar_graph_input::call_origin::selected_boundary_operation(
                    node, optimized, native, plan,
                )?
            {
                let mut ordinary = target.clone();
                match &mut ordinary {
                    TargetUnitOperation::Call { origin, .. } => {
                        *origin = target_operations::NativeCallOrigin::Authored
                    }
                    _ => return Err(LegalizationError::custody()),
                }
                return validate_operation(
                    function,
                    &ordinary,
                    &operation,
                    scalar_parameters,
                    parameters,
                    sources,
                    custody,
                    function_inputs,
                    native,
                    plan,
                    unit,
                );
            }
        }
        let checker = Checker {
            function,
            available: Some(sources),
            optimized: function_inputs.function(),
            function_inputs,
            types: unit.structural_types.as_slice(),
        };
        let projected_bytes = match target {
            TargetUnitOperation::StructuralByteSequenceFieldLength {
                source,
                field,
                indices,
                ..
            }
            | TargetUnitOperation::StructuralByteSequenceFieldRead {
                source,
                field,
                indices,
                ..
            } => Some((source, Some(*field), indices)),
            TargetUnitOperation::StructuralByteViewFieldStore {
                destination,
                field,
                indices,
                ..
            } => Some((destination, *field, indices)),
            _ => None,
        };
        if let Some((source, field, indices)) = projected_bytes {
            let geometry = view_slot_geometry(optimized, source, field, &plan.structural_types)
                .ok_or(LegalizationError::custody())?;
            if !runtime_indices_rejoin(indices, &geometry.elements, sources) {
                return Err(LegalizationError::custody());
            }
        }
        match (target, abstracted) {
            (
                TargetUnitOperation::InstallStaticInterrupt { result_home, .. },
                AbstractOperation::BoundaryCall { .. },
            ) => {
                crate::legalization::scalar_graph_input::interrupt_installation::validate(
                    target, abstracted, optimized, native, plan, unit, custody,
                )?;
                sources.push((result_home.source_value, Source::Home(*result_home)));
            }
            (
                TargetUnitOperation::StaticInterrupt {
                    psi_operation,
                    boundary,
                    operation,
                    token,
                    result_home,
                },
                AbstractOperation::BoundaryCall {
                    psi_operation: expected_operation,
                    boundary: expected_boundary,
                    arguments,
                    structural_arguments,
                    result,
                    ..
                },
            ) => {
                use language_core::static_interrupts::StaticInterruptOperation;
                let declaration = plan
                    .boundary_machines
                    .iter()
                    .find(|row| row.id == *boundary)
                    .ok_or(LegalizationError::custody())?;
                if psi_operation != expected_operation
                    || boundary != expected_boundary
                    || unit.boundary_machine(*boundary) != Some(declaration)
                    || declaration
                        .declaration
                        .as_ref()
                        .and_then(|row| row.execution.static_interrupt_operation())
                        != Some(*operation)
                    || *operation == StaticInterruptOperation::Install
                    || !arguments.is_empty()
                    || structural_arguments.as_slice() != std::slice::from_ref(token)
                    || token.access
                        != if *operation != StaticInterruptOperation::FinishCompletion {
                            terminal_psi::StructuralAccess::SharedBorrow
                        } else {
                            terminal_psi::StructuralAccess::Owned
                        }
                {
                    return Err(LegalizationError::custody());
                }
                match (operation, result, result_home) {
                    (
                        StaticInterruptOperation::FinishCompletion,
                        abstract_operations::AbstractBoundaryResult::Unit,
                        None,
                    ) => {}
                    (
                        StaticInterruptOperation::Context
                        | StaticInterruptOperation::BeginCompletion,
                        abstract_operations::AbstractBoundaryResult::Structural(result),
                        Some(home),
                    ) => {
                        let (_, retained) = home
                            .operation_result()
                            .ok_or(LegalizationError::custody())?;
                        if retained != result
                            || *home
                                != crate::legalization::scalar_graph_input::aggregate_results::result_home(
                                    optimized,
                                    result.place,
                                    plan,
                                )?
                        {
                            return Err(LegalizationError::custody());
                        }
                    }
                    _ => return Err(LegalizationError::custody()),
                }
            }

            (
                TargetUnitOperation::AccessLifecycle {
                    psi_operation,
                    boundary,
                    operands,
                    result_home,
                },
                AbstractOperation::BoundaryCall {
                    psi_operation: expected_operation,
                    boundary: expected_boundary,
                    arguments,
                    structural_arguments,
                    result: abstract_operations::AbstractBoundaryResult::Structural(result),
                    ..
                },
            ) => {
                let lifecycle = unit
                    .structural_types
                    .access_lifecycles()
                    .iter()
                    .find(|lifecycle| lifecycle.boundary == *boundary)
                    .ok_or(LegalizationError::custody())?;
                let expected_operands =
                    abstract_operations::AccessLifecycleOperands::from_arguments(
                        lifecycle.operation,
                        structural_arguments,
                    )
                    .map_err(|_| LegalizationError::custody())?;
                let (_, retained_result) = result_home
                    .operation_result()
                    .ok_or(LegalizationError::custody())?;
                if psi_operation != expected_operation
                    || boundary != expected_boundary
                    || !arguments.is_empty()
                    || operands != &expected_operands
                    || retained_result != result
                    || !matches!(
                        lifecycle.execution,
                        terminal_psi::TerminalAccessLifecycleExecution::CompilerPrimitive { .. }
                    )
                    || *result_home
                        != crate::legalization::scalar_graph_input::aggregate_results::result_home(
                            optimized,
                            result.place,
                            plan,
                        )?
                {
                    return Err(LegalizationError::custody());
                }
            }
            (
                TargetUnitOperation::ProgramImageAddress {
                    psi_operation,
                    boundary,
                    execution,
                    result_home,
                },
                AbstractOperation::BoundaryCall {
                    psi_operation: expected_operation,
                    boundary: expected_boundary,
                    result: abstract_operations::AbstractBoundaryResult::Scalar(result),
                    arguments,
                    structural_arguments,
                    completion_claim_sources,
                    completion_receipts,
                },
            ) => {
                let declaration = plan
                    .boundary_machines
                    .iter()
                    .find(|row| row.id == *boundary)
                    .ok_or(LegalizationError::custody())?;
                let scalar = ScalarType::Integer(
                    semantic_vocabulary::IntegerType::address(64)
                        .map_err(|_| LegalizationError::custody())?,
                );
                if native.target != target::NativeTarget::uefi_x64()
                    || psi_operation != expected_operation || boundary != expected_boundary
                    || *execution != target_operations::BoundaryExecutionBinding::CompilerBuiltin(
                        target_operations::CompilerBuiltinExecution::ProgramImageRetainedSuffixStart)
                    || unit.boundary_machine(*boundary) != Some(declaration)
                    || !declaration.scalar_parameters.is_empty()
                    || !declaration.structural_parameters.is_empty()
                    || declaration.result != terminal_psi::BoundaryMachineResult::Scalar(scalar.into())
                    || !declaration.requires.is_empty()
                    || !declaration.program_local_root_introductions.is_empty()
                    || !declaration.content_guarantees.is_empty()
                    || !arguments.is_empty() || !structural_arguments.is_empty()
                    || !completion_claim_sources.is_empty() || !completion_receipts.is_empty()
                    || result.scalar_type != scalar
                    || result_home.defining_operation != *psi_operation
                    || result_home.source_value != result.value
                    || result_home.scalar_type != scalar
                    || Some(result_home.shape) != crate::legalization::scalar_graph_input::scalar_shape(scalar)
                {
                    return Err(LegalizationError::custody());
                }
                sources.push((result.value, Source::Home(*result_home)));
            }
            (
                TargetUnitOperation::AccessorScalarAccess {
                    psi_operation,
                    boundary,
                    realization,
                    location,
                    operand,
                    result_home,
                },
                AbstractOperation::BoundaryCall {
                    psi_operation: expected_operation,
                    boundary: expected_boundary,
                    arguments,
                    structural_arguments,
                    result,
                    completion_claim_sources: _,
                    completion_receipts,
                },
            ) => {
                let demand = terminal_semantics::accessor_scalar_boundary_demand(
                    unit.structural_types.as_slice(),
                    &plan.identities,
                    unit.structural_types.access_views(),
                    &plan.boundary_machines,
                    *boundary,
                )
                .map_err(|_| LegalizationError::custody())?;
                let [argument] = structural_arguments.as_slice() else {
                    return Err(LegalizationError::custody());
                };
                if argument != location
                    || psi_operation != expected_operation
                    || boundary != expected_boundary
                    || realization.demand != demand
                    || argument.access != demand.access
                    || !completion_receipts.is_empty()
                {
                    return Err(LegalizationError::custody());
                }
                crate::legalization::scalar_graph_input::reference_custody::accessor_scalar_custody(
                    optimized,
                    custody,
                    unit.structural_types.as_slice(),
                    location,
                    demand.accessor,
                )?;
                match (
                    demand.operation,
                    arguments.as_slice(),
                    operand,
                    result,
                    result_home,
                ) {
                    (
                        terminal_psi::TerminalAccessorKind::Read,
                        [],
                        None,
                        abstract_operations::AbstractBoundaryResult::Scalar(result),
                        Some(home),
                    ) if result.scalar_type == demand.scalar
                        && home.scalar_type == demand.scalar
                        && home.defining_operation == *psi_operation
                        && home.source_value == result.value
                        && Some(home.shape)
                            == crate::legalization::scalar_graph_input::scalar_shape(
                                demand.scalar,
                            ) =>
                    {
                        sources.push((result.value, Source::Home(*home)));
                    }
                    (
                        terminal_psi::TerminalAccessorKind::Write,
                        [value],
                        Some(operand),
                        abstract_operations::AbstractBoundaryResult::Unit,
                        None,
                    ) if operand.scalar_type() == demand.scalar
                        && sources
                            .matching(*value)
                            .any(|(identity, source)| identity == value && source == operand) => {}
                    _ => return Err(LegalizationError::custody()),
                }
            }
            (
                TargetUnitOperation::SelectMachine { result_home, descriptor },
                AbstractOperation::SelectMachine { psi_operation, result, descriptor: expected_descriptor },
            ) => {
                let selected = plan.callables.descriptor(*descriptor)
                    .ok_or(LegalizationError::custody())?;
                let expected = target_operations::TargetUnitScalarHomeRequirement {
                    defining_operation: *psi_operation,
                    source_value: result.value,
                    scalar_type: result.scalar_type,
                    shape: crate::legalization::scalar_graph_input::scalar_shape(result.scalar_type)
                        .ok_or(LegalizationError::custody())?,
                };
                if result.scalar_type != semantic_vocabulary::ScalarType::Callable(selected.contract)
                    || result_home != &expected || descriptor != expected_descriptor
                {
                    return Err(LegalizationError::custody());
                }
                sources.push((result.value, Source::Home(*result_home)));
            }
            (
                TargetUnitOperation::SelectImplementation { result_home, table },
                AbstractOperation::SelectImplementation {
                    psi_operation,
                    result,
                    table: expected_table,
                },
            ) => {
                let expected = target_operations::TargetUnitScalarHomeRequirement {
                    defining_operation: *psi_operation,
                    source_value: result.value,
                    scalar_type: result.scalar_type,
                    shape: crate::legalization::scalar_graph_input::scalar_shape(
                        result.scalar_type,
                    )
                    .ok_or(LegalizationError::custody())?,
                };
                if result.scalar_type != target_operations::selector_carrier_type()
                    || result_home != &expected
                    || table != expected_table
                    || plan.dynamic_dispatch.table(*table).is_none()
                {
                    return Err(LegalizationError::custody());
                }
                sources.push((result.value, Source::Home(*result_home)));
            }
            (
                TargetUnitOperation::ActivateTaskRegister {
                    psi_operation,
                    service,
                    entries,
                    gdt,
                    task_state,
                    selector,
                    scratch,
                    result_home,
                },
                AbstractOperation::ActivateTaskRegister {
                    psi_operation: expected_operation,
                    service: expected_service,
                    entries: expected_entries,
                    gdt: expected_gdt,
                    task_state: expected_task,
                    selector: expected_selector,
                    scratch: expected_scratch,
                    result,
                },
            ) => {
                if psi_operation != expected_operation
                    || service != expected_service
                    || entries != expected_entries
                    || gdt != expected_gdt
                    || task_state != expected_task
                    || scratch != expected_scratch
                    || !sources
                        .matching(*expected_selector)
                        .any(|(identity, source)| {
                            identity == expected_selector && source == selector
                        })
                    || result_home.defining_operation != *psi_operation
                    || result_home.source_value != result.value
                    || result_home.scalar_type != result.scalar_type
                    || Some(result_home.shape)
                        != crate::legalization::scalar_graph_input::scalar_shape(result.scalar_type)
                {
                    return Err(LegalizationError::custody());
                }
                sources.push((result.value, Source::Home(*result_home)));
            }
            (
                TargetUnitOperation::ReadTaskRegister {
                    psi_operation,
                    service,
                    result_home,
                },
                AbstractOperation::ReadTaskRegister {
                    psi_operation: expected_operation,
                    service: expected_service,
                    result,
                },
            ) => {
                if psi_operation != expected_operation
                    || service != expected_service
                    || result_home.defining_operation != *psi_operation
                    || result_home.source_value != result.value
                    || result_home.scalar_type != result.scalar_type
                    || Some(result_home.shape)
                        != crate::legalization::scalar_graph_input::scalar_shape(result.scalar_type)
                {
                    return Err(LegalizationError::custody());
                }
                sources.push((result.value, Source::Home(*result_home)));
            }
            (
                TargetUnitOperation::ReloadSegmentDescriptors {
                    psi_operation,
                    service,
                    entries,
                    gdt,
                    kind,
                    selector,
                    scratch,
                    result_home,
                },
                AbstractOperation::ReloadSegmentDescriptors {
                    psi_operation: expected_operation,
                    service: expected_service,
                    entries: expected_entries,
                    gdt: expected_gdt,
                    kind: expected_kind,
                    selector: expected_selector,
                    scratch: expected_scratch,
                    result,
                },
            ) => {
                if psi_operation != expected_operation
                    || service != expected_service
                    || entries != expected_entries
                    || gdt != expected_gdt
                    || kind != expected_kind
                    || scratch != expected_scratch
                    || !sources
                        .matching(*expected_selector)
                        .any(|(identity, source)| {
                            identity == expected_selector && source == selector
                        })
                    || result_home.defining_operation != *psi_operation
                    || result_home.source_value != result.value
                    || result_home.scalar_type != result.scalar_type
                    || Some(result_home.shape)
                        != crate::legalization::scalar_graph_input::scalar_shape(result.scalar_type)
                {
                    return Err(LegalizationError::custody());
                }
                sources.push((result.value, Source::Home(*result_home)));
            }
            (
                TargetUnitOperation::ReadSegmentSelector {
                    register,
                    psi_operation,
                    service,
                    result_home,
                },
                AbstractOperation::ReadSegmentSelector {
                    register: expected_register,
                    psi_operation: expected_operation,
                    service: expected_service,
                    result,
                },
            ) => {
                if psi_operation != expected_operation
                    || service != expected_service
                    || register != expected_register
                    || result_home.defining_operation != *psi_operation
                    || result_home.source_value != result.value
                    || result_home.scalar_type != result.scalar_type
                    || Some(result_home.shape)
                        != crate::legalization::scalar_graph_input::scalar_shape(result.scalar_type)
                {
                    return Err(LegalizationError::custody());
                }
                sources.push((result.value, Source::Home(*result_home)));
            }
            (
                TargetUnitOperation::RegisterAccess {
                    psi_operation,
                    service,
                    event,
                    result_home,
                },
                AbstractOperation::RegisterAccess {
                    psi_operation: expected_operation,
                    service: expected_service,
                    event: expected_event,
                    result,
                },
            ) => {
                if psi_operation != expected_operation
                    || service != expected_service
                    || event.operation() != expected_event.operation()
                {
                    return Err(LegalizationError::custody());
                }
                if event.inputs().count() != expected_event.inputs().count()
                    || event
                        .inputs()
                        .zip(expected_event.inputs())
                        .any(|(operand, value)| {
                            !sources
                                .matching(*value)
                                .any(|(identity, source)| identity == value && source == operand)
                        })
                {
                    return Err(LegalizationError::custody());
                }
                match (result_home, result) {
                    (None, None) => {}
                    (Some(home), Some(result))
                        if home.defining_operation == *psi_operation
                            && home.source_value == result.value
                            && home.scalar_type == result.scalar_type
                            && Some(home.shape)
                                == crate::legalization::scalar_graph_input::scalar_shape(
                                    result.scalar_type,
                                ) =>
                    {
                        sources.push((result.value, Source::Home(*home)));
                    }
                    _ => return Err(LegalizationError::custody()),
                }
            }
            (
                TargetUnitOperation::PortWrite {
                    psi_operation,
                    service,
                    port,
                    value,
                },
                AbstractOperation::PortWrite {
                    psi_operation: expected_operation,
                    service: expected_service,
                    port: expected_port,
                    value: expected_value,
                },
            ) if psi_operation == expected_operation
                && service == expected_service
                && port == expected_port
                && value == expected_value => {}
            (
                TargetUnitOperation::AtomicFence {
                    psi_operation,
                    ordering,
                },
                AbstractOperation::AtomicEvent {
                    psi_operation: expected_operation,
                    event:
                        abstract_operations::AbstractAtomicEvent::Fence {
                            ordering: expected_ordering,
                        },
                    reads_from: None,
                    modification_after: None,
                    resident_obligation: None,
                },
            ) if psi_operation == expected_operation && ordering == expected_ordering => {}
            (
                TargetUnitOperation::AtomicAccess {
                    psi_operation,
                    location,
                    scalar_type,
                    field_byte_offset,
                    event,
                    operands,
                    result_home,
                    outcome_home,
                },
                AbstractOperation::AtomicEvent {
                    psi_operation: expected_operation,
                    event: source,
                    ..
                },
            ) => {
                let expected = crate::legalization::scalar_graph_input::atomic_access::access(
                    source,
                    optimized,
                    &unit.structural_types,
                )
                .ok_or(LegalizationError::custody())?;
                if psi_operation != expected_operation
                    || location != &expected.location
                    || *scalar_type != expected.scalar_type
                    || *field_byte_offset != expected.field_byte_offset
                    || *event != expected.event
                    || operands.len() != event.operands().len()
                    || !event
                        .operands()
                        .iter()
                        .zip(operands)
                        .all(|(value, operand)| {
                            sources
                                .matching(*value)
                                .any(|(identity, source)| identity == value && source == operand)
                        })
                {
                    return Err(LegalizationError::custody());
                }
                match (result_home, expected.result) {
                    (None, None) => {}
                    (Some(home), Some(result))
                        if home.defining_operation == *psi_operation
                            && home.source_value == result.value
                            && home.scalar_type == result.scalar_type
                            && Some(home.shape)
                                == crate::legalization::scalar_graph_input::scalar_shape(
                                    result.scalar_type,
                                ) =>
                    {
                        sources.push((result.value, Source::Home(*home)));
                    }
                    _ => return Err(LegalizationError::custody()),
                }
                match (outcome_home, &expected.outcome) {
                    (None, None) => {}
                    (Some(home), Some((result, layout)))
                        if home.operation_result() == Some((*psi_operation, result))
                            && home.layout.sum() == Some(layout) => {}
                    _ => return Err(LegalizationError::custody()),
                }
            }
            (
                TargetUnitOperation::StructuralCaseMembership {
                    psi_operation,
                    result,
                    source,
                    path,
                    case,
                    case_tag,
                    tag_byte_offset,
                    indices,
                },
                AbstractOperation::StructuralCaseMembership {
                    psi_operation: expected_operation,
                    result: expected_result,
                    source: expected_source,
                    path: expected_path,
                    case: expected_case,
                },
            ) if psi_operation == expected_operation
                && result == expected_result
                && source == expected_source
                && path == expected_path
                && case == expected_case
                && result.scalar_type == ScalarType::Boolean =>
            {
                let (expected_tag, expected_offset, elements) = membership_layout(
                    optimized,
                    *expected_source,
                    expected_path,
                    *expected_case,
                    plan,
                    function_inputs.source_result(*expected_source),
                )?;
                if (*case_tag, *tag_byte_offset) != (expected_tag, expected_offset)
                    || !runtime_indices_rejoin(indices, &elements, sources)
                {
                    return Err(LegalizationError::custody());
                }
                sources.push((
                    result.value,
                    Source::Home(target_operations::TargetUnitScalarHomeRequirement {
                        defining_operation: *psi_operation,
                        source_value: result.value,
                        scalar_type: result.scalar_type,
                        shape: calling_conventions::ValueShape::integer(1, 1),
                    }),
                ));
            }
            (
                _,
                AbstractOperation::IeeeFloatCompare { .. }
                | AbstractOperation::FloatResultPolicy { .. }
                | AbstractOperation::IeeeFloatConstant { .. }
                | AbstractOperation::ScalarRepresentationView { .. },
            ) => {
                ieee_float::validate(target, abstracted, sources)?;
            }
            (
                TargetUnitOperation::EstablishAccessReference {
                    psi_operation,
                    result_home,
                    source,
                    correspondence,
                },
                AbstractOperation::EstablishAccessReference {
                    psi_operation: expected_operation,
                    result,
                    source: expected_source,
                    correspondence: expected_correspondence,
                },
            ) if psi_operation == expected_operation
                && source == expected_source
                && correspondence == expected_correspondence
                && *result_home
                    == crate::legalization::scalar_graph_input::aggregate_results::result_home(
                        optimized,
                        result.place,
                        plan,
                    )? => {}
            (
                TargetUnitOperation::EstablishReference {
                    psi_operation,
                    result_home,
                    source,
                    storage,
                },
                AbstractOperation::EstablishReference {
                    psi_operation: expected_operation,
                    result,
                    source: expected_source,
                    storage: expected_storage,
                },
            ) if psi_operation == expected_operation
                && source == expected_source
                && storage == expected_storage
                && *result_home
                    == crate::legalization::scalar_graph_input::aggregate_results::result_home(
                        optimized,
                        result.place,
                        plan,
                    )? => {}
            (
                TargetUnitOperation::ReleaseReference {
                    psi_operation,
                    source,
                    path,
                },
                AbstractOperation::ReleaseReference {
                    psi_operation: expected_operation,
                    source: expected_source,
                    path: expected_path,
                },
            ) if psi_operation == expected_operation
                && source == expected_source
                && path == expected_path => {}
            (
                TargetUnitOperation::StructuralLeafCopy { .. },
                AbstractOperation::BoundaryCall { .. },
            ) => {
                aggregate_results::validate(target, abstracted, sources, optimized, plan, unit)?;
            }
            (
                _,
                AbstractOperation::EstablishStructuralArray { .. }
                | AbstractOperation::EstablishScalarArray { .. }
                | AbstractOperation::EstablishActivationClaim { .. }
                | AbstractOperation::SplitActivationClaim { .. }
                | AbstractOperation::MergeActivationClaim { .. }
                | AbstractOperation::ReleaseActivationClaim { .. }
                | AbstractOperation::EstablishNativeCodeLayout { .. }
                | AbstractOperation::EstablishRecord { .. }
                | AbstractOperation::StructuralLeafCopy { .. }
                | AbstractOperation::MoveStructuralValue { .. }
                | AbstractOperation::TransformContent { .. }
                | AbstractOperation::EstablishCase { .. },
            ) => {
                aggregate_results::validate(target, abstracted, sources, optimized, plan, unit)?;
            }
            (
                TargetUnitOperation::EstablishZeroedStructural {
                    psi_operation,
                    result_home,
                },
                AbstractOperation::EstablishZeroedStructural {
                    psi_operation: expected_operation,
                    result,
                },
            ) if psi_operation == expected_operation
                && *result_home
                    == crate::legalization::scalar_graph_input::aggregate_results::result_home(
                        optimized,
                        result.place,
                        plan,
                    )?
                && terminal_semantics::zeroed_structural_shape(
                    plan.structural_types.iter(),
                    result.structural_type,
                ) => {}
            (
                TargetUnitOperation::EstablishStructuralLocal {
                    psi_operation,
                    result_home,
                },
                AbstractOperation::EstablishStructuralLocal {
                    psi_operation: expected_operation,
                    result,
                },
            ) if psi_operation == expected_operation
                && *result_home
                    == crate::legalization::scalar_graph_input::aggregate_results::result_home(
                        optimized,
                        result.place,
                        plan,
                    )?
                && crate::legalization::scalar_graph_input::aggregate_results::home_layout(
                    result, plan,
                )
                .is_ok() => {}
            (_, abstract_operations::move_or_store_structural_field_pattern!()) => {
                borrowed_windows::validate(target, abstracted, optimized, sources, plan, custody)?;
            }
            (
                TargetUnitOperation::EstablishTrivialAffineLocal {
                    psi_operation,
                    place,
                    structural_type,
                },
                AbstractOperation::EstablishTrivialAffineLocal {
                    psi_operation: expected_operation,
                    place: expected_place,
                    structural_type: expected_type,
                },
            ) if psi_operation == expected_operation
                && place == expected_place
                && structural_type == expected_type
                && plan.structural_types.contains(structural_type)
                && matches!(
                    &structural_type.shape,
                    terminal_psi::StructuralTypeShape::Record { fields, .. } if fields.is_empty()
                )
                && matches!(
                    place.kind,
                    semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                        structural_type: declared,
                        construction: None,
                        ..
                    } if declared == structural_type.id
                ) => {}
            (
                TargetUnitOperation::StructuralByteSequenceFieldByteStore {
                    psi_operation,
                    destination,
                    field,
                    index,
                    value,
                    length,
                    obligation,
                },
                AbstractOperation::StructuralByteSequenceFieldByteStore {
                    psi_operation: expected_operation,
                    field: expected_field,
                    index: expected_index,
                    value: expected_value,
                    length: expected_length,
                    obligation: expected_obligation,
                    ..
                },
            ) if psi_operation == expected_operation
                && field == expected_field
                && length == expected_length
                && obligation == expected_obligation
                && Some(destination.clone())
                    == replacement(
                        optimized,
                        abstracted,
                        &plan.structural_types,
                        &unit.structural_domains,
                        &unit.identities,
                        custody,
                    )
                && sources
                    .matching(*expected_index)
                    .any(|(identity, actual)| identity == expected_index && actual == index)
                && sources
                    .matching(*expected_value)
                    .any(|(identity, actual)| identity == expected_value && actual == value) => {}
            (
                TargetUnitOperation::StructuralByteSequenceFieldRead {
                    psi_operation,
                    result,
                    source,
                    field,
                    index,
                    length,
                    obligation,
                    ..
                },
                AbstractOperation::StructuralByteSequenceFieldRead {
                    psi_operation: expected_operation,
                    result: expected_result,
                    field: expected_field,
                    index: expected_index,
                    length: expected_length,
                    obligation: expected_obligation,
                    ..
                },
            ) => {
                if psi_operation != expected_operation
                    || result != expected_result
                    || field != expected_field
                    || length != expected_length
                    || obligation != expected_obligation
                    || Some(source.clone())
                        != byte_read(optimized, abstracted, &plan.structural_types)
                    || !sources
                        .matching(*expected_index)
                        .any(|(identity, actual)| identity == expected_index && actual == index)
                {
                    return Err(LegalizationError::custody());
                }
                sources.push(scalar_home(psi_operation, result)?);
            }
            (
                TargetUnitOperation::EstablishByteSequenceConcatenation {
                    psi_operation,
                    result,
                    left,
                    right,
                    capacity_obligation,
                },
                AbstractOperation::EstablishByteSequenceConcatenation {
                    psi_operation: expected_operation,
                    result: expected_result,
                    left: expected_left,
                    right: expected_right,
                    capacity_obligation: expected_obligation,
                },
            ) if psi_operation == expected_operation
                && result == expected_result
                && left == expected_left
                && right == expected_right
                && capacity_obligation == expected_obligation => {}
            (
                TargetUnitOperation::ByteSequenceStore {
                    psi_operation,
                    destination,
                    source,
                    length,
                    obligation,
                },
                AbstractOperation::ByteSequenceStore {
                    psi_operation: expected_operation,
                    destination: expected_destination,
                    source: expected_source,
                    length: expected_length,
                    obligation: expected_obligation,
                },
            ) if psi_operation == expected_operation
                && destination == expected_destination
                && source == expected_source
                && obligation == expected_obligation
                && replacement(
                    optimized,
                    abstracted,
                    &plan.structural_types,
                    &unit.structural_domains,
                    &unit.identities,
                    custody,
                )
                .is_some()
                && sources
                    .matching(*expected_length)
                    .any(|(value, actual)| value == expected_length && actual == length) => {}
            (
                TargetUnitOperation::StructuralByteViewFieldStore {
                    psi_operation,
                    destination,
                    field,
                    source,
                    ..
                },
                AbstractOperation::StructuralByteViewFieldStore {
                    psi_operation: expected_operation,
                    field: expected_field,
                    source: expected_source,
                    ..
                },
            ) if psi_operation == expected_operation
                && field == expected_field
                && source == expected_source
                && Some(destination.clone())
                    == view_replacement(
                        optimized,
                        abstracted,
                        &plan.structural_types,
                        &unit.structural_domains,
                        &unit.identities,
                        custody,
                    ) => {}
            (
                TargetUnitOperation::StructuralByteSequenceFieldStore {
                    psi_operation,
                    destination,
                    field,
                    source,
                    length,
                    obligation,
                },
                AbstractOperation::StructuralByteSequenceFieldStore {
                    psi_operation: expected_operation,
                    field: expected_field,
                    source: expected_source,
                    length: expected_length,
                    obligation: expected_obligation,
                    ..
                },
            ) if psi_operation == expected_operation
                && field == expected_field
                && source == expected_source
                && obligation == expected_obligation
                && Some(destination.clone())
                    == replacement(
                        optimized,
                        abstracted,
                        &plan.structural_types,
                        &unit.structural_domains,
                        &unit.identities,
                        custody,
                    )
                && sources
                    .matching(*expected_length)
                    .any(|(value, actual)| value == expected_length && actual == length) => {}
            (
                TargetUnitOperation::ByteSequenceWrite {
                    psi_operation,
                    destination,
                    view,
                    index,
                    value,
                    length,
                    obligation,
                },
                AbstractOperation::ByteSequenceWrite {
                    psi_operation: expected_operation,
                    destination: expected_destination,
                    index: expected_index,
                    value: expected_value,
                    length: expected_length,
                    obligation: expected_obligation,
                },
            ) if psi_operation == expected_operation
                && destination == expected_destination
                && length == expected_length
                && obligation == expected_obligation
                && checker.mutable_byte_view(view, *expected_destination)
                && sources
                    .matching(*expected_index)
                    .any(|(identity, source)| identity == expected_index && source == index)
                && sources
                    .matching(*expected_value)
                    .any(|(identity, source)| identity == expected_value && source == value) => {}
            (
                TargetUnitOperation::ElementViewWrite {
                    path,
                    psi_operation,
                    destination,
                    indices,
                    index,
                    value,
                    length,
                    obligation,
                },
                AbstractOperation::ElementViewWrite {
                    path: expected_path,
                    psi_operation: expected_operation,
                    destination: expected_destination,
                    index: expected_index,
                    value: expected_value,
                    length: expected_length,
                    obligation: expected_obligation,
                },
            ) if path == expected_path
                && psi_operation == expected_operation
                && destination == expected_destination
                && length == expected_length
                && obligation == expected_obligation
                && crate::legalization::scalar_graph_input::element_views::descriptor_geometry(
                    optimized,
                    expected_destination,
                    &unit.structural_types,
                )
                .is_some_and(|geometry| {
                    geometry.readable
                        && runtime_indices_rejoin(indices, &geometry.elements, sources)
                })
                && sources
                    .matching(*expected_index)
                    .any(|(identity, source)| identity == expected_index && source == index)
                && sources
                    .matching(*expected_value)
                    .any(|(identity, source)| identity == expected_value && source == value) => {}
            (
                TargetUnitOperation::EstablishPrimitiveLocal {
                    psi_operation,
                    result,
                    value,
                    shape,
                },
                AbstractOperation::EstablishPrimitiveLocal {
                    psi_operation: expected_operation,
                    result: expected,
                    value: expected_value,
                    ..
                },
            ) if psi_operation == expected_operation
                && result == expected
                && value == expected_value
                && crate::legalization::scalar_graph_input::scalar_shape(value.scalar_type)
                    == Some(*shape)
                && sources.matching(value.value).any(|(identity, source)| {
                    *identity == value.value && source.scalar_type() == value.scalar_type
                }) => {}
            (
                TargetUnitOperation::PrimitiveLocalStore {
                    psi_operation,
                    destination,
                    value,
                },
                AbstractOperation::PrimitiveLocalStore {
                    psi_operation: expected_operation,
                    destination: expected,
                    value: expected_value,
                },
            ) if psi_operation == expected_operation
                && destination == expected
                && value == expected_value
                && sources.matching(value.value).any(|(identity, source)| {
                    *identity == value.value && source.scalar_type() == value.scalar_type
                }) => {}
            (
                TargetUnitOperation::GlobalDescriptorTable {
                    psi_operation,
                    service,
                    operation,
                    source,
                },
                AbstractOperation::GlobalDescriptorTable {
                    psi_operation: expected_operation,
                    service: expected_service,
                    operation: expected_kind,
                    source: expected_source,
                },
            ) if psi_operation == expected_operation
                && service == expected_service
                && operation == expected_kind
                && source == expected_source => {}
            (
                TargetUnitOperation::StorageAddress {
                    psi_operation,
                    result,
                    source,
                    path,
                    indices,
                },
                AbstractOperation::StorageAddress {
                    psi_operation: expected_operation,
                    result: expected,
                    source: expected_source,
                    path: expected_path,
                },
            ) if psi_operation == expected_operation
                && result == expected
                && source == expected_source
                && path == expected_path =>
            {
                let root =
                    crate::legalization::scalar_graph_input::primitive_locals::read_root_type(
                        optimized, *source,
                    )
                    .ok_or(LegalizationError::custody())?;
                let geometry = storage_address_geometry(root, path, &unit.structural_types)
                    .ok_or(LegalizationError::custody())?;
                if !matches!(result.scalar_type, ScalarType::Integer(integer) if integer.is_address())
                    || !runtime_indices_rejoin(indices, &geometry.elements, sources)
                {
                    return Err(LegalizationError::custody());
                }
                sources.push(scalar_home(psi_operation, result)?);
            }
            (
                TargetUnitOperation::PrimitiveScalarRead {
                    psi_operation,
                    result,
                    source,
                    path,
                    indices,
                },
                AbstractOperation::PrimitiveScalarRead {
                    psi_operation: expected_operation,
                    result: expected,
                    source: expected_source,
                    path: expected_path,
                },
            ) if psi_operation == expected_operation
                && result == expected
                && source == expected_source
                && path == expected_path =>
            {
                let elements = if terminal_psi::is_static_structural_path(expected_path) {
                    Vec::new()
                } else {
                    let root_type =
                        crate::legalization::scalar_graph_input::primitive_locals::read_root_type(
                            optimized,
                            *expected_source,
                        )
                        .ok_or(LegalizationError::custody())?;
                    primitive_read_geometry(
                        root_type,
                        expected_path,
                        expected.scalar_type,
                        &unit.structural_types,
                    )
                    .ok_or(LegalizationError::custody())?
                    .elements
                };
                if !runtime_indices_rejoin(indices, &elements, sources) {
                    return Err(LegalizationError::custody());
                }
                sources.push(scalar_home(psi_operation, result)?);
            }
            (
                TargetUnitOperation::StructuralScalarFieldRead {
                    psi_operation,
                    result,
                    source,
                    field,
                    ..
                },
                AbstractOperation::StructuralScalarFieldRead { .. },
            )
            | (
                TargetUnitOperation::StructuralByteSequenceFieldLength {
                    psi_operation,
                    result,
                    source,
                    field,
                    ..
                },
                AbstractOperation::StructuralByteSequenceFieldLength { .. },
            ) => {
                let case = match target {
                    TargetUnitOperation::StructuralScalarFieldRead { case, .. } => *case,
                    _ => None,
                };
                let expected = crate::legalization::scalar_graph_input::structural_fields::read(
                    optimized,
                    abstracted,
                    &plan.structural_types,
                    custody,
                )
                .ok_or(LegalizationError::custody())?;
                if expected != (*psi_operation, *result, source.clone(), case, *field) {
                    return Err(LegalizationError::custody());
                }
                sources.push(scalar_home(psi_operation, result)?);
            }
            (
                TargetUnitOperation::EstablishByteSequenceLiteral {
                    psi_operation,
                    place,
                    structural_type,
                    bytes,
                    qualifications,
                },
                AbstractOperation::EstablishByteSequenceLiteral {
                    psi_operation: expected_operation,
                    place: expected_place,
                    structural_type: expected_type,
                    bytes: expected_bytes,
                    qualifications: expected_qualifications,
                },
            ) if psi_operation == expected_operation
                && place == expected_place
                && structural_type == expected_type
                && bytes == expected_bytes
                && qualifications == expected_qualifications => {}
            (
                TargetUnitOperation::WriteOnlyPrimitiveStore { .. },
                AbstractOperation::WriteOnlyPrimitiveStore { .. },
            ) => {
                primitive_store::validate(
                    target, abstracted, parameters, sources, optimized, unit,
                )?;
            }
            (TargetUnitOperation::ScalarDefinition { result_home, .. }, abstracted)
                if super::scalar_definitions::observed_family(abstracted) =>
            {
                super::scalar_definitions::observation(target, abstracted, &checker)?;
                sources.push((result_home.source_value, Source::Home(*result_home)));
            }
            (
                TargetUnitOperation::ByteSequenceSubslice { result, view },
                AbstractOperation::ByteSequenceSubslice {
                    result: expected,
                    psi_operation,
                    ..
                },
            ) => {
                if result != expected
                    || !matches!(view, target_operations::TargetByteView::Subslice { psi_operation: operation, .. } if operation == psi_operation)
                    || !checker.byte_view(view, result.place, &[])
                {
                    return Err(LegalizationError::custody());
                }
            }
            (
                TargetUnitOperation::ElementViewSubslice { result, view },
                AbstractOperation::ElementViewSubslice {
                    result: expected,
                    psi_operation,
                    ..
                },
            ) => {
                if result != expected
                    || !matches!(view, target_operations::TargetElementView::Subslice { psi_operation: operation, .. } if operation == psi_operation)
                    || !checker.element_view(view, result.place, &[])
                {
                    return Err(LegalizationError::custody());
                }
            }
            (
                TargetUnitOperation::EstablishElementView { result, view },
                AbstractOperation::EstablishElementView {
                    result: expected,
                    destination,
                    psi_operation,
                    ..
                },
            ) => {
                if result != expected
                    || !matches!(view, target_operations::TargetElementView::Established { psi_operation: operation, .. } if operation == psi_operation)
                {
                    return Err(LegalizationError::custody());
                }
                checker
                    .element_view_custody(view, *destination, &[])
                    .map_err(|reason| LegalizationError::ElementViewCustody {
                        machine: optimized.machine,
                        operation: *psi_operation,
                        place: *destination,
                        reason,
                    })?;
            }
            (
                TargetUnitOperation::ScalarDefinition { result_home, .. },
                AbstractOperation::IntegerWiden { .. },
            ) => {
                super::scalar_definitions::validate(
                    target,
                    abstracted,
                    scalar_parameters,
                    sources,
                )?;
                sources.push((result_home.source_value, Source::Home(*result_home)));
            }
            (
                TargetUnitOperation::NormalizedForeignCall { .. },
                AbstractOperation::BoundaryCall { .. },
            ) => {
                super::normalized_foreign::validate(
                    target, abstracted, parameters, native, optimized, plan, unit, sources,
                )?;
            }
            (
                TargetUnitOperation::NearestIeeeFloatArithmetic { .. }
                | TargetUnitOperation::IeeeFloatClassify { .. },
                AbstractOperation::BoundaryCall { .. },
            ) => {
                crate::legalization::scalar_graph_input::ieee_float_arithmetic::validate(
                    target,
                    abstracted,
                    native.target,
                    plan,
                    unit,
                    sources,
                )?;
            }
            (
                TargetUnitOperation::BoundarySettlement { .. },
                AbstractOperation::BoundaryCall {
                    result: abstract_operations::AbstractBoundaryResult::Structural(_),
                    ..
                },
            ) => {
                crate::legalization::scalar_graph_input::read_byte::validate(
                    target,
                    abstracted,
                    native.target,
                    plan,
                    unit,
                )?;
            }
            (
                TargetUnitOperation::BoundarySettlement { .. },
                AbstractOperation::BoundaryCall { .. },
            ) => {
                super::hosted_scalar::validate(
                    target,
                    abstracted,
                    native.target,
                    plan,
                    unit,
                    sources,
                )?;
            }
            (
                TargetUnitOperation::StructuralScalarFieldStore {
                    psi_operation,
                    destination,
                    path,
                    field,
                    destination_storage,
                    field_byte_offset,
                    source,
                    indices,
                },
                AbstractOperation::StructuralScalarFieldStore {
                    psi_operation: expected_operation,
                    destination: expected_destination,
                    path: expected_path,
                    field: expected_field,
                    value,
                    ..
                },
            ) => {
                let (expected_destination, expected_path) = scalar_store_destination(
                    optimized,
                    expected_destination,
                    expected_path,
                    *expected_field,
                    &unit.structural_types,
                    &unit.structural_domains,
                    &unit.identities,
                    custody,
                )
                .ok_or(LegalizationError::custody())?;
                let storage_matches = match (destination, destination_storage) {
                    (
                        abstract_operations::StructuralScalarStoreDestination::Parameter(parameter),
                        target_operations::TargetStructuralArgumentSource::Placement(placement),
                    ) => {
                        optimized.structural_parameters.contains(parameter)
                            && parameters.iter().any(|declared| {
                                declared.place == parameter.place && &declared.placement == placement
                            })
                    }
                    (
                        abstract_operations::StructuralScalarStoreDestination::Parameter(parameter),
                        target_operations::TargetStructuralArgumentSource::BlockParameter {
                            block,
                            place,
                        },
                    ) => {
                        *place == parameter.place
                            && matches!(
                                parameter.access,
                                terminal_psi::StructuralAccess::Owned
                                    | terminal_psi::exclusive_borrow_pair_pattern!()
                            )
                            && optimized.blocks.iter().any(|entry| {
                                entry.id == *block && entry.structural_parameters.contains(parameter)
                            })
                            && optimized.arrival_parameter(*place) == Some(parameter)
                            && optimized
                                .entry_claim_declarations
                                .iter()
                                .all(|claim| claim.input != *place)
                            && optimized
                                .content_entry_claims
                                .iter()
                                .all(|claim| claim.input.root != *place)
                    }
                    (
                        abstract_operations::StructuralScalarStoreDestination::OwnedResult {
                            producer,
                            result,
                        },
                        target_operations::TargetStructuralArgumentSource::StructuralHome {
                            psi_operation,
                        },
                    ) => {
                        psi_operation == producer
                            && source_result(optimized, result.place).is_ok_and(
                                |(operation, declared)| operation == *producer && declared == result,
                            )
                    }
                    (
                        abstract_operations::StructuralScalarStoreDestination::OwnedBlockParameter {
                            block,
                            parameter,
                        },
                        target_operations::TargetStructuralArgumentSource::BlockParameter {
                            block: storage_block,
                            place,
                        },
                    ) => {
                        storage_block == block
                            && *place == parameter.place
                            && matches!(
                                source_owner(optimized, parameter.place),
                                Ok(legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
                                    block: owner,
                                    declaration,
                                }) if owner == *block && declaration == *parameter
                            )
                    }
                    _ => false,
                };
                let (offset, _, elements) =
                    crate::structural_inputs::structural_reference_input::store(
                        expected_destination.structural_type(),
                        &expected_path,
                        *expected_field,
                        value.scalar_type,
                        &unit.structural_types,
                    )
                    .ok_or(LegalizationError::custody())?;
                if psi_operation != expected_operation
                    || destination != &expected_destination
                    || path != &expected_path
                    || field != expected_field
                    || !storage_matches
                    || *field_byte_offset != offset
                    || !runtime_indices_rejoin(indices, &elements, sources)
                    || !sources
                        .matching(value.value)
                        .any(|(identity, expected)| *identity == value.value && expected == source)
                {
                    return Err(LegalizationError::custody());
                }
            }
            (
                TargetUnitOperation::BooleanConstant {
                    psi_operation,
                    result,
                    value,
                },
                AbstractOperation::BooleanConstant {
                    psi_operation: actual,
                    result: defined,
                    value: literal,
                },
            ) if psi_operation == actual && result == defined && value == literal => {
                sources.push((
                    *result,
                    Source::BooleanImmediate {
                        defining_operation: *psi_operation,
                        source_value: *result,
                        value: *value,
                    },
                ));
            }
            (TargetUnitOperation::Call { .. }, _) => {
                direct_calls::validate(
                    target, abstracted, sources, custody, optimized, native, plan, unit,
                )?;
            }
            (
                TargetUnitOperation::IntegerConstant {
                    psi_operation,
                    result,
                    scalar_type,
                    value,
                },
                AbstractOperation::IntegerConstant {
                    psi_operation: actual,
                    result: defined,
                    scalar_type: integer,
                    value: literal,
                },
            ) if psi_operation == actual
                && result == defined
                && *integer == ScalarType::Integer(*scalar_type)
                && value == literal =>
            {
                sources.push((
                    *result,
                    Source::IntegerImmediate {
                        defining_operation: *psi_operation,
                        source_value: *result,
                        scalar_type: *scalar_type,
                        value: *value,
                    },
                ));
            }
            (
                TargetUnitOperation::Return {
                    psi_edge,
                    cleanup_actions,
                },
                AbstractOperation::ReturnUnit {
                    psi_edge: edge,
                    cleanup_actions: cleanup,
                },
            ) if psi_edge == edge
                && cleanup_actions == cleanup
                && (cleanup.is_empty()
                    || crate::legalization::scalar_graph_input::read_byte::cleanup(
                        optimized, cleanup,
                    )) => {}
            _ => return Err(LegalizationError::custody()),
        }
        Ok(())
    }

    pub(crate) fn runtime_indices_rejoin(
        indices: &[target_operations::TargetStructuralRuntimeIndex],
        elements: &[RuntimeElement],
        sources: &crate::legalization_target::control_flow::sources::AvailableSources<'_>,
    ) -> bool {
        indices.len() == elements.len()
            && indices.iter().zip(elements).all(|(index, element)| {
                index.stride == element.stride
                    && index.operand.source_value() == element.index
                    && sources.matching(element.index).any(|(identity, known)| {
                        *identity == element.index && *known == index.operand
                    })
            })
    }

    fn scalar_home(
        psi_operation: &semantic_vocabulary::OperationId,
        result: &abstract_operations::AbstractResult,
    ) -> Result<(ValueId, Source), LegalizationError> {
        Ok((
            result.value,
            Source::Home(target_operations::TargetUnitScalarHomeRequirement {
                defining_operation: *psi_operation,
                source_value: result.value,
                scalar_type: result.scalar_type,
                shape: crate::legalization::scalar_graph_input::scalar_shape(result.scalar_type)
                    .ok_or(LegalizationError::custody())?,
            }),
        ))
    }
}
pub(crate) fn validate_target(
    target: &TargetFunction,
    abstracted: &AbstractFunction,
    function_inputs: &crate::legalization_function_inputs::FunctionInputs<'_>,
    descriptor_inputs: &crate::legalization_descriptor_inputs::DescriptorInputs<'_>,
) -> Result<crate::legalization::scalar_graph_input::BlockEntryCustody, LegalizationError> {
    let native = descriptor_inputs.native();
    let plan = descriptor_inputs.plan();
    let optimized = function_inputs.function();
    crate::legalization::scalar_graph_input::hosted_scalar::validate_normal_call_completions(
        native, optimized, plan,
    )?;
    let operations = optimized
        .block_nodes()
        .filter_map(|node| instruction(node).map(|row| row.0))
        .collect::<Vec<_>>();
    let mut edges = optimized
        .block_nodes()
        .flat_map(|node| match &node.operation {
            AbstractOperation::Return { psi_edge, .. }
            | AbstractOperation::ReturnStructural { psi_edge, .. }
            | AbstractOperation::ReturnUnit { psi_edge, .. }
            | AbstractOperation::ExternalCompletion { psi_edge, .. }
            | AbstractOperation::InvokeNoNormal { psi_edge, .. }
            | AbstractOperation::Crash { psi_edge, .. }
            | AbstractOperation::Jump { psi_edge, .. } => vec![*psi_edge],
            AbstractOperation::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true.psi_edge, when_false.psi_edge],
            AbstractOperation::StructuralCase { cases, .. } => {
                cases.iter().map(|case| case.psi_edge).collect()
            }
            _ => vec![],
        })
        .collect::<Vec<_>>();
    let mut target_edges = target.provenance.edges.clone();
    edges.sort();
    target_edges.sort();
    if target.provenance.operations != operations || target_edges != edges {
        return Err(LegalizationError::custody());
    }
    control_flow::validate(
        target,
        &target.graph,
        abstracted,
        function_inputs,
        descriptor_inputs,
    )
}

pub(crate) struct Checker<'a> {
    function: &'a TargetFunction,
    available: Option<&'a crate::legalization_target::control_flow::sources::AvailableSources<'a>>,
    optimized: &'a PsiOptimizationFunction,
    function_inputs: &'a crate::legalization_function_inputs::FunctionInputs<'a>,
    types: &'a [terminal_psi::StructuralTypeDeclaration],
}
fn resolve(value: ValueId, aliases: &[(ValueId, ValueId)]) -> ValueId {
    aliases
        .iter()
        .rev()
        .find(|(parameter, _)| *parameter == value)
        .map_or(value, |(_, source)| *source)
}
fn location_matches(location: ScalarParameterLocation, placement: &ValuePlacement) -> bool {
    matches!(placement.locations.as_slice(),[ValueLocation::Register {register,value_byte_offset:0,byte_size}] if *byte_size == placement.shape.byte_size && location == ScalarParameterLocation::Register(*register))
        || (crate::legalization::scalar_graph_input::scalar_stack(placement)
            && matches!(placement.locations.as_slice(),
                [ValueLocation::Stack { stack_byte_offset, .. }]
                    if location == ScalarParameterLocation::IncomingStack { byte_offset: *stack_byte_offset }))
}
