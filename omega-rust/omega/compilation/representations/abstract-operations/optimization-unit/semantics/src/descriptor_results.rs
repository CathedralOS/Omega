/*
We adapt current abstract and optimization-unit function bodies into cohort
descriptor flow graphs (`DescriptorMachine`) for views, ordinary references,
and borrowed storage. By reconstructing place definitions, scalar payload reads
and writes, call guarantees, and escaped-invocation invalidations directly from
current operations rather than retained source summaries, we let optimization
and target-lowering consumers query returned descriptor origins and access
results after transformations have mutated the program.
*/

use abstract_operations::{AbstractOperation as O, StructuralCopyPath};
use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
use semantic_vocabulary::{MachineId, PlaceId, StructuralPlaceKind};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    BoundaryMachineDeclaration, StructuralAccess, StructuralArgument, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape,
};
use terminal_semantics::borrowed_storage::Projection;
use terminal_semantics::descriptor_lifetimes::{
    DescriptorFlowBinding, DescriptorFlowEdge, DescriptorFlowSource, DescriptorFlowStep,
    DescriptorLocation, descriptor_location,
};
use terminal_semantics::descriptor_results::{
    self as kernel, DescriptorDefinition, DescriptorInvocation, DescriptorMachine, DescriptorPlace,
    writable_descriptor_slots,
};

pub mod access_results {
    use abstract_operations::AbstractOperation;
    use semantic_vocabulary::{MachineId, OperationId, StructuralTypeId};
    use terminal_psi::{
        StructuralArgument, StructuralParameterDeclaration, StructuralPathSegment,
        StructuralTypeShape,
    };
    use terminal_semantics::borrowed_storage::Projection;
    use terminal_semantics::descriptor_lifetimes::descriptor_path;
    use terminal_semantics::descriptor_results::{DescriptorResultSlot, DescriptorStoredOrigin};

    #[derive(Debug)]
    pub enum Source {
        Captured(abstract_operations::AccessReferenceBacking),
        Incoming(StructuralArgument),
    }

    #[derive(Debug)]
    pub struct ResultSlot {
        pub path: Vec<StructuralPathSegment>,
        pub sources: Vec<Option<Source>>,
    }

    pub fn summarize<'operation>(
        types: &abstract_operations::StructuralTypeCatalog,
        boundaries: &[terminal_psi::BoundaryMachineDeclaration],
        parameters: &[StructuralParameterDeclaration],
        result_type: StructuralTypeId,
        slots: &[DescriptorResultSlot],
        formation: impl Fn(MachineId, OperationId) -> Option<&'operation AbstractOperation>,
    ) -> Option<Vec<ResultSlot>> {
        slots
            .iter()
            .map(|slot| {
                let path = descriptor_path(types, result_type, &slot.path)?;
                let StructuralTypeShape::Reference { referent, access } =
                    terminal_semantics::runtime_structural_path_tip(
                        types.iter(),
                        result_type,
                        &path,
                    )?
                    .shape
                else {
                    return None;
                };
                let sources = slot
                    .origins
                    .iter()
                    .map(|origin| match origin {
                        DescriptorStoredOrigin::AccessFormation {
                            machine,
                            operation,
                            place,
                            carrier,
                            projection,
                        } => {
                            if projection != &Projection::new(&[]) || *carrier != referent {
                                return None;
                            }
                            let formation = formation(*machine, *operation)?;
                            if let AbstractOperation::BoundaryCall {
                                psi_operation,
                                boundary,
                                result:
                                    abstract_operations::AbstractBoundaryResult::Structural(result),
                                ..
                            } = formation
                            {
                                let declaration =
                                    boundaries.iter().find(|row| row.id == *boundary)?;
                                let (_, backing) =
                                    crate::access_references::borrowed_view_formation(
                                        types,
                                        declaration,
                                        result,
                                    )?;
                                if psi_operation != operation
                                    || result.place != *place
                                    || backing.carrier() != *carrier
                                {
                                    return None;
                                }
                                return Some(Some(Source::Captured(backing)));
                            }
                            let AbstractOperation::EstablishAccessReference {
                                psi_operation,
                                result,
                                correspondence,
                                ..
                            } = formation
                            else {
                                return None;
                            };
                            if psi_operation != operation
                                || result.place != *place
                                || correspondence.carrier() != *carrier
                                || !terminal_psi::find_structural_type(
                                    types,
                                    result.structural_type,
                                )
                                .is_some_and(|declaration| {
                                    matches!(declaration.shape,
                                    StructuralTypeShape::Reference { referent, .. }
                                        if referent == *carrier)
                                })
                            {
                                return None;
                            }
                            Some(Some(Source::Captured(correspondence.clone())))
                        }
                        DescriptorStoredOrigin::Parameter {
                            position,
                            projection,
                        } => {
                            if projection != &Projection::new(&[]) {
                                return None;
                            }
                            let parameter = parameters.get(*position)?;
                            if parameter.structural_type != referent
                                || !access.admits(parameter.access)
                            {
                                return None;
                            }
                            Some(Some(Source::Incoming(StructuralArgument {
                                place: parameter.place,
                                path: Vec::new(),
                                access,
                            })))
                        }
                        DescriptorStoredOrigin::IncomingSlot { position, path } => {
                            let parameter = parameters.get(*position)?;
                            let mut path = descriptor_path(types, parameter.structural_type, path)?;
                            let StructuralTypeShape::Reference {
                                referent: incoming,
                                access: ceiling,
                            } = terminal_semantics::runtime_structural_path_tip(
                                types.iter(),
                                parameter.structural_type,
                                &path,
                            )?
                            .shape
                            else {
                                return None;
                            };
                            if incoming != referent || !access.admits(ceiling) {
                                return None;
                            }
                            path.push(StructuralPathSegment::Referent);
                            Some(Some(Source::Incoming(StructuralArgument {
                                place: parameter.place,
                                path,
                                access,
                            })))
                        }
                        DescriptorStoredOrigin::Empty => Some(None),
                        _ => None,
                    })
                    .filter_map(|source| match source {
                        Some(None) => None,
                        Some(Some(source)) => Some(Some(source)),
                        None => Some(None),
                    })
                    .collect();
                Some(ResultSlot { path, sources })
            })
            .collect()
    }

    pub fn resolve<Interpretation: PartialEq>(
        slots: &[ResultSlot],
        path: &[StructuralPathSegment],
        incoming: impl Fn(&StructuralArgument) -> Option<Interpretation>,
        captured: impl Fn(&abstract_operations::AccessReferenceBacking) -> Interpretation,
    ) -> Option<Result<Interpretation, ConflictingInterpretations>> {
        let mut matching = slots.iter().filter(|slot| slot.path == path);
        let slot = matching.next()?;
        if matching.next().is_some() {
            return Some(Err(ConflictingInterpretations));
        }
        derive(slot.sources.iter().map(|source| match source {
            Some(Source::Captured(correspondence)) => Some(captured(correspondence)),
            Some(Source::Incoming(source)) => incoming(source),
            None => None,
        }))
    }

    #[derive(Debug, PartialEq, Eq)]
    pub struct ConflictingInterpretations;

    fn derive<Interpretation: PartialEq>(
        alternatives: impl IntoIterator<Item = Option<Interpretation>>,
    ) -> Option<Result<Interpretation, ConflictingInterpretations>> {
        let mut selected = None;
        let mut incomplete = false;
        for candidate in alternatives {
            let Some(candidate) = candidate else {
                incomplete = true;
                continue;
            };
            if selected
                .as_ref()
                .is_some_and(|selected| selected != &candidate)
            {
                return Some(Err(ConflictingInterpretations));
            }
            selected = Some(candidate);
        }
        if incomplete { None } else { selected.map(Ok) }
    }

    pub fn agree<Interpretation: PartialEq>(
        alternatives: impl IntoIterator<Item = Option<Interpretation>>,
    ) -> Option<Interpretation> {
        let mut selected = None;
        for candidate in alternatives {
            let candidate = candidate?;
            if selected
                .as_ref()
                .is_some_and(|selected| selected != &candidate)
            {
                return None;
            }
            selected = Some(candidate);
        }
        selected
    }

    pub fn reconcile<Interpretation: PartialEq>(
        public: Option<Interpretation>,
        private: Option<Result<Interpretation, ConflictingInterpretations>>,
    ) -> Option<Interpretation> {
        match (public, private) {
            (_, Some(Err(ConflictingInterpretations))) => None,
            (Some(public), Some(Ok(private))) => (public == private).then_some(private),
            (Some(interpretation), None) | (None, Some(Ok(interpretation))) => Some(interpretation),
            (None, None) => None,
        }
    }
}
mod scalar_expressions {
    use super::{O, kernel};
    use optimization_unit::{OptimizationEdge, OptimizationNode, PsiOptimizationFunction};
    use semantic_vocabulary::{BlockId, IntegerValue, ScalarTerm as T, ScalarType, ValueId};
    use std::collections::BTreeMap;

    mod case_conditions {
        use super::*;
        use std::collections::BTreeSet;
        use terminal_psi::StructuralTypeDeclaration;
        use terminal_semantics::descriptor_results::{DescriptorCasePredicate, DescriptorMachine};

        #[cfg(test)]
        mod tests {
            use abstract_operations::{
                AbstractBlockEntry, AbstractFunction, AbstractFunctionResult,
                AbstractOperation as Operation, AbstractParameter, AbstractResult,
                AbstractStructuralCallResult, AbstractSuccessor, ValueBinding,
            };
            use semantic_vocabulary::{
                BlockId, EdgeId, MachineId, OperationId, PlaceId, ScalarType, StructuralCaseId,
                StructuralTypeId, ValueId,
            };
            use std::borrow::Cow;
            use terminal_psi::{
                StructuralAccess, StructuralArgument, StructuralCaseDeclaration,
                StructuralMultiplicity, StructuralParameterDeclaration, StructuralStoreDisposition,
                StructuralTypeDeclaration, StructuralTypeShape, TerminalIdentityIndex,
            };
            use terminal_semantics::descriptor_lifetimes::DescriptorLocation;
            use terminal_semantics::descriptor_results::{
                DescriptorCasePredicate, DescriptorMachine,
            };

            fn parameter(
                place: u64,
                position: u32,
                access: StructuralAccess,
            ) -> StructuralParameterDeclaration {
                StructuralParameterDeclaration {
                    place: PlaceId::new(place).unwrap(),
                    position,
                    structural_type: StructuralTypeId::new(1).unwrap(),
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    additional_input_qualifications: Vec::new(),
                }
            }

            fn replacement(destination: u64, source: u64) -> Operation {
                Operation::StoreStructuralField {
                    disposition: StructuralStoreDisposition::ReplaceDiscardable,
                    psi_operation: OperationId::new(2).unwrap(),
                    destination: parameter(destination, 0, StructuralAccess::MutableBorrow).into(),
                    path: Vec::new(),
                    field: None,
                    value: StructuralArgument {
                        place: PlaceId::new(source).unwrap(),
                        path: Vec::new(),
                        access: StructuralAccess::Owned,
                    },
                }
            }

            fn function(
                machine: u64,
                parameters: Vec<StructuralParameterDeclaration>,
                block_entries: Vec<AbstractBlockEntry>,
                operations: Vec<Operation>,
            ) -> AbstractFunction {
                AbstractFunction {
                    machine: MachineId::new(machine).unwrap(),
                    attachment: None,
                    nominal_cleanup_subject: None,
                    entry: block_entries[0].block,
                    parameters: Vec::new(),
                    structural_parameters: parameters,
                    result: AbstractFunctionResult::Unit,
                    entry_claims: Vec::new(),
                    call_formations: Vec::new(),
                    block_entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries,
                    operations,
                }
            }

            fn forwarded_graph(intervening: Vec<Operation>, forwarded: u64) -> DescriptorMachine {
                let block = |number| BlockId::new(number).unwrap();
                let successor = |number| AbstractSuccessor {
                    psi_edge: EdgeId::new(number).unwrap(),
                    target: block(number + 1),
                    bindings: Vec::new(),
                    structural_bindings: Vec::new(),
                    cleanup_actions: Vec::new(),
                };
                let mut operations = vec![Operation::StructuralCaseMembership {
                    psi_operation: OperationId::new(1).unwrap(),
                    result: AbstractResult {
                        value: ValueId::new(1).unwrap(),
                        scalar_type: ScalarType::Boolean,
                    },
                    source: PlaceId::new(1).unwrap(),
                    path: Vec::new(),
                    case: StructuralCaseId::new(1).unwrap(),
                }];
                operations.extend(intervening);
                operations.push(Operation::Jump {
                    psi_edge: EdgeId::new(1).unwrap(),
                    target: block(2),
                    bindings: vec![ValueBinding {
                        parameter: ValueId::new(2).unwrap(),
                        argument: ValueId::new(forwarded).unwrap(),
                        scalar_type: ScalarType::Boolean,
                    }],
                    structural_bindings: Vec::new(),
                    cleanup_actions: Vec::new(),
                });
                let conditional = operations.len();
                operations.push(Operation::Conditional {
                    condition: ValueId::new(2).unwrap(),
                    when_true: successor(2),
                    when_false: successor(3),
                });
                operations.extend([4, 5].map(|number| Operation::ReturnUnit {
                    psi_edge: EdgeId::new(number).unwrap(),
                    cleanup_actions: Vec::new(),
                }));
                let entries = [0, conditional, conditional + 1, conditional + 2]
                    .into_iter()
                    .enumerate()
                    .map(|(index, operation_offset)| AbstractBlockEntry {
                        block: block(index as u64 + 1),
                        structural_parameters: Vec::new(),
                        parameters: if index == 1 {
                            vec![AbstractParameter {
                                value: ValueId::new(2).unwrap(),
                                scalar_type: ScalarType::Boolean,
                            }]
                        } else {
                            Vec::new()
                        },
                        operation_offset,
                    })
                    .collect();
                let caller = function(
                    1,
                    vec![
                        parameter(1, 0, StructuralAccess::MutableBorrow),
                        parameter(2, 1, StructuralAccess::Owned),
                    ],
                    entries,
                    operations,
                );
                let callee = function(
                    2,
                    vec![
                        parameter(3, 0, StructuralAccess::MutableBorrow),
                        parameter(4, 1, StructuralAccess::Owned),
                    ],
                    vec![AbstractBlockEntry {
                        block: block(5),
                        structural_parameters: Vec::new(),
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    vec![
                        replacement(3, 4),
                        Operation::ReturnUnit {
                            psi_edge: EdgeId::new(6).unwrap(),
                            cleanup_actions: Vec::new(),
                        },
                    ],
                );
                let functions = [caller, callee]
                    .iter()
                    .map(optimization_unit::reconstruct_psi_optimization_function_seed)
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap();
                let types = [StructuralTypeDeclaration {
                    requires: Vec::new(),
                    id: StructuralTypeId::new(1).unwrap(),
                    identity: TerminalIdentityIndex(1),
                    shape: StructuralTypeShape::Sum {
                        cases: [1, 2]
                            .map(|number| StructuralCaseDeclaration {
                                id: StructuralCaseId::new(number).unwrap(),
                                identity: TerminalIdentityIndex(number as u32 + 1),
                                fields: Vec::new(),
                            })
                            .to_vec(),
                    },
                    quotient_carrier: None,
                    owns_linear_claim: false,
                    no_code_affine_cleanup: false,
                    copyable: true,
                    descriptor_cleanup: false,
                    scalar_restriction: None,
                }];
                crate::descriptor_results::optimized_reference_catalog_graph(
                    &functions,
                    [],
                    &types,
                    &Default::default(),
                    |_, node| Some(Cow::Borrowed(&node.operation)),
                )
                .unwrap()
                .remove(0)
            }

            fn expected_polarities(truth: bool) -> Vec<(DescriptorCasePredicate, bool)> {
                let predicate = DescriptorCasePredicate {
                    subject: DescriptorLocation::whole(PlaceId::new(1).unwrap()),
                    case: StructuralCaseId::new(1).unwrap(),
                };
                vec![(predicate.clone(), truth), (predicate, !truth)]
            }

            #[test]
            fn forwarded_case_membership_preserves_both_edge_polarities() {
                let graph = forwarded_graph(Vec::new(), 1);
                assert_eq!(
                    graph.case_successors.get(&(BlockId::new(2).unwrap(), 0)),
                    Some(&expected_polarities(true)),
                );
            }

            #[test]
            fn forwarded_negated_case_membership_reverses_edge_polarities() {
                let graph = forwarded_graph(
                    vec![Operation::BooleanNot {
                        psi_operation: OperationId::new(2).unwrap(),
                        result: ValueId::new(3).unwrap(),
                        operand: ValueId::new(1).unwrap(),
                    }],
                    3,
                );
                assert_eq!(
                    graph.case_successors.get(&(BlockId::new(2).unwrap(), 0)),
                    Some(&expected_polarities(false)),
                );
            }

            #[test]
            fn saved_case_membership_does_not_survive_replacement_or_call() {
                let call = Operation::CallStructural {
                    psi_operation: OperationId::new(2).unwrap(),
                    callee: MachineId::new(2).unwrap().into(),
                    arguments: Vec::new(),
                    structural_arguments: vec![
                        StructuralArgument {
                            place: PlaceId::new(1).unwrap(),
                            path: Vec::new(),
                            access: StructuralAccess::MutableBorrow,
                        },
                        StructuralArgument {
                            place: PlaceId::new(2).unwrap(),
                            path: Vec::new(),
                            access: StructuralAccess::Owned,
                        },
                    ],
                    claim_transfers: Vec::new(),
                    result: AbstractStructuralCallResult::Unit,
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                };
                for (operation, intervening) in [("replacement", replacement(1, 2)), ("call", call)]
                {
                    let graph = forwarded_graph(vec![intervening], 1);
                    assert!(
                        !graph
                            .case_successors
                            .contains_key(&(BlockId::new(2).unwrap(), 0)),
                        "saved membership survived an intervening {operation}",
                    );
                }
            }
        }
        type Position = (BlockId, usize);

        #[derive(Clone, PartialEq, Eq)]
        struct CapturedTest {
            observation: Position,
            predicate: DescriptorCasePredicate,
            truth: bool,
        }

        impl ScalarExpressions<'_> {
            pub(in crate::descriptor_results) fn retain_case_conditions(
                &self,
                types: &[StructuralTypeDeclaration],
                graph: &mut DescriptorMachine,
            ) {
                let mut conditions = Vec::new();
                for block in self.blocks.values() {
                    for (index, node) in block.nodes.iter().enumerate() {
                        let O::Conditional {
                            condition,
                            when_true,
                            when_false,
                        } = &node.operation
                        else {
                            continue;
                        };
                        let Some(test) = self.case_test(
                            types,
                            graph,
                            *condition,
                            (block.id, index),
                            &mut BTreeSet::new(),
                        ) else {
                            continue;
                        };
                        let predicates = node
                            .successors
                            .iter()
                            .map(|edge| {
                                let truth = if edge.psi_edge == when_true.psi_edge
                                    && edge.target == when_true.target
                                {
                                    test.truth
                                } else if edge.psi_edge == when_false.psi_edge
                                    && edge.target == when_false.target
                                {
                                    !test.truth
                                } else {
                                    return None;
                                };
                                Some((test.predicate.clone(), truth))
                            })
                            .collect::<Option<Vec<_>>>();
                        if let Some(predicates) =
                            predicates.filter(|predicates| predicates.len() == 2)
                            && let Ok(index) = u32::try_from(index)
                        {
                            conditions.push(((block.id, index), predicates));
                        }
                    }
                }
                graph.case_successors.extend(conditions);
            }

            fn case_test(
                &self,
                types: &[StructuralTypeDeclaration],
                graph: &DescriptorMachine,
                value: ValueId,
                position: Position,
                active: &mut BTreeSet<(ValueId, Position)>,
            ) -> Option<CapturedTest> {
                if !active.insert((value, position)) {
                    return None;
                }
                let result = (|| {
                    if let Some(definition) = self.definitions.get(&value) {
                        let node = definition.node;
                        let (block, index) = definition.position;
                        if block != position.0
                            || index >= position.1
                            || !self.unchanged(graph, position.0, index + 1, position.1)
                        {
                            return None;
                        }
                        return match &node.operation {
                            O::StructuralCaseMembership {
                                source, path, case, ..
                            } => {
                                let subject =
                                    terminal_semantics::descriptor_lifetimes::descriptor_location(
                                        types,
                                        *source,
                                        *graph.place_types.get(source)?,
                                        path,
                                        None,
                                    )?;
                                Some(CapturedTest {
                                    observation: (block, index),
                                    predicate: DescriptorCasePredicate {
                                        subject,
                                        case: *case,
                                    },
                                    truth: true,
                                })
                            }
                            O::BooleanNot { operand, .. } => {
                                let mut test =
                                    self.case_test(types, graph, *operand, (block, index), active)?;
                                test.truth = !test.truth;
                                Some(test)
                            }
                            _ => None,
                        };
                    }
                    if self.owners.get(&value) != Some(&position.0)
                        || !self.unchanged(graph, position.0, 0, position.1)
                    {
                        return None;
                    }
                    let incoming = self.incoming.get(&position.0)?;
                    let mut selected = None;
                    for incoming in incoming {
                        if !incoming.edge.structural_bindings.is_empty() {
                            return None;
                        }
                        let actual = incoming.edge.binding(value)?.argument;
                        let test = self.case_test(types, graph, actual, incoming.source, active)?;
                        if selected.as_ref().is_some_and(|selected| selected != &test) {
                            return None;
                        }
                        selected = Some(test);
                    }
                    selected
                })();
                active.remove(&(value, position));
                result
            }

            fn unchanged(
                &self,
                graph: &DescriptorMachine,
                block: BlockId,
                start: usize,
                end: usize,
            ) -> bool {
                let Some(nodes) = self
                    .blocks
                    .get(&block)
                    .and_then(|block| block.nodes.get(start..end))
                else {
                    return false;
                };
                nodes.iter().enumerate().all(|(offset, node)| {
                    let Ok(index) = u32::try_from(start + offset) else {
                        return false;
                    };
                    let position = (block, index);
                    graph.flow.get(&position).is_some_and(|step| {
                        step.definitions.is_empty()
                            && step.derivations.is_empty()
                            && !step.may_escape
                            && !graph.calls.contains_key(&position)
                            && !graph.contents_transfers.contains_key(&position)
                            && !graph.payload_invalidations.contains_key(&position)
                            && node.ownership.is_empty()
                    })
                })
            }
        }
    }
    struct ScalarDefinition<'function> {
        node: &'function OptimizationNode,
        position: (BlockId, usize),
    }

    struct IncomingEdge<'function> {
        source: (BlockId, usize),
        edge: &'function OptimizationEdge,
    }

    pub(super) struct ScalarExpressions<'function> {
        entry: BlockId,
        types: BTreeMap<ValueId, ScalarType>,
        definitions: BTreeMap<ValueId, ScalarDefinition<'function>>,
        booleans: BTreeMap<ValueId, bool>,
        integers: BTreeMap<ValueId, (ScalarType, IntegerValue)>,
        owners: BTreeMap<ValueId, BlockId>,
        incoming: BTreeMap<BlockId, Vec<IncomingEdge<'function>>>,
        blocks: BTreeMap<BlockId, &'function optimization_unit::OptimizationBlock>,
    }

    impl<'function> ScalarExpressions<'function> {
        pub(super) fn new(function: &'function PsiOptimizationFunction) -> Self {
            let types = function
                .parameters
                .iter()
                .chain(function.blocks.iter().flat_map(|block| &block.parameters))
                .chain(function.block_nodes().flat_map(|node| &node.definitions))
                .map(|value| (value.value, value.scalar_type))
                .collect();
            let mut reader = Self {
                entry: function.entry,
                types,
                definitions: BTreeMap::new(),
                booleans: BTreeMap::new(),
                integers: BTreeMap::new(),
                owners: BTreeMap::new(),
                incoming: BTreeMap::new(),
                blocks: function
                    .blocks
                    .iter()
                    .map(|block| (block.id, block))
                    .collect(),
            };
            for block in &function.blocks {
                for parameter in &block.parameters {
                    reader.owners.entry(parameter.value).or_insert(block.id);
                }
                for (index, node) in block.nodes.iter().enumerate() {
                    for definition in &node.definitions {
                        reader
                            .definitions
                            .entry(definition.value)
                            .or_insert(ScalarDefinition {
                                node,
                                position: (block.id, index),
                            });
                    }
                    match node.operation {
                        O::BooleanConstant { result, value, .. } => {
                            reader.booleans.entry(result).or_insert(value);
                        }
                        O::IntegerConstant {
                            result,
                            scalar_type,
                            value,
                            ..
                        } => {
                            reader
                                .integers
                                .entry(result)
                                .or_insert((scalar_type, value));
                        }
                        _ => {}
                    }
                    for edge in &node.successors {
                        reader
                            .incoming
                            .entry(edge.target)
                            .or_default()
                            .push(IncomingEdge {
                                source: (block.id, index),
                                edge,
                            });
                    }
                }
            }
            reader
        }

        fn literal(&self, value: ValueId) -> Option<T> {
            if let Some(&literal) = self.booleans.get(&value) {
                return Some(T::Boolean(literal));
            }
            let (scalar_type, literal) =
                terminal_semantics::scalar_constant_value(value, |value| {
                    if let Some(&(scalar_type, literal)) = self.integers.get(&value) {
                        return Some(terminal_semantics::ScalarConstantOrigin::Literal(
                            scalar_type,
                            literal,
                        ));
                    }
                    let owner = *self.owners.get(&value)?;
                    if owner == self.entry {
                        return None;
                    }
                    let incoming = self
                        .incoming
                        .get(&owner)
                        .into_iter()
                        .flatten()
                        .map(|incoming| {
                            incoming.edge.binding(value).map(|binding| binding.argument)
                        })
                        .collect::<Option<Vec<_>>>()?;
                    Some(terminal_semantics::ScalarConstantOrigin::Incoming(incoming))
                })?;
            let ScalarType::Integer(integer) = scalar_type else {
                return None;
            };
            Some(T::Integer {
                scalar_type: integer,
                value: literal,
            })
        }

        pub(super) fn expression(&self, value: ValueId) -> Option<T> {
            let types = &self.types;
            kernel::scalar_payload_expression(value, &|value| {
                let scalar_type = *types.get(&value)?;
                if let Some(literal) = self.literal(value) {
                    return Some((scalar_type, Some(literal)));
                }
                let term = |value| Some(Box::new(T::value(value, *types.get(&value)?)));
                let integer = |value| types.get(&value)?.as_integer();
                let denotation = self.definitions.get(&value).and_then(|definition| {
                    let node = definition.node;
                    Some(match &node.operation {
                        O::IntegerArithmetic {
                            arithmetic,
                            scalar_type,
                            left,
                            right,
                            ..
                        } => arithmetic.scalar_term(*scalar_type, *term(*left)?, *term(*right)?),
                        O::BooleanNot { operand, .. } => T::BooleanNot {
                            operand: term(*operand)?,
                        },
                        O::BooleanEqual { left, right, .. } => T::BooleanEqual {
                            left: term(*left)?,
                            right: term(*right)?,
                        },
                        O::IntegerEqual { left, right, .. } => T::IntegerEqual {
                            scalar_type: integer(*left)?,
                            left: term(*left)?,
                            right: term(*right)?,
                        },
                        O::IntegerLessThan { left, right, .. } => T::IntegerLessThan {
                            scalar_type: integer(*left)?,
                            left: term(*left)?,
                            right: term(*right)?,
                        },
                        O::IntegerLessOrEqual { left, right, .. } => T::IntegerLessOrEqual {
                            scalar_type: integer(*left)?,
                            left: term(*left)?,
                            right: term(*right)?,
                        },
                        _ => return None,
                    })
                });
                Some((scalar_type, denotation))
            })
        }
    }
}
use crate::unit_validation::operation_contracts::structural_source_contract;
use abstract_operations::AbstractFunction;
use optimization_unit::OptimizationNode;
use optimization_unit::reconstruct_psi_optimization_function_seed;
use scalar_expressions::ScalarExpressions;
use semantic_vocabulary::BlockId;
use semantic_vocabulary::StructuralTypeId;
use semantic_vocabulary::ValueId;
use terminal_psi::TerminalDynamicDispatchCatalog;
use terminal_semantics::descriptor_lifetimes::canonical_descriptor_location;
use terminal_semantics::runtime_structural_path_tip;

pub fn abstract_catalog_graph<'function, 'boundary>(
    functions: impl IntoIterator<Item = &'function AbstractFunction>,
    boundaries: impl IntoIterator<Item = &'boundary BoundaryMachineDeclaration>,
    types: &[StructuralTypeDeclaration],
    dynamic_dispatch: &TerminalDynamicDispatchCatalog,
) -> Option<Vec<DescriptorMachine>> {
    abstract_graph_with_slots(
        functions,
        boundaries,
        types,
        dynamic_dispatch,
        kernel::DescriptorSlotKind::Views,
    )
}

pub fn abstract_reference_catalog_graph<'function, 'boundary>(
    functions: impl IntoIterator<Item = &'function AbstractFunction>,
    boundaries: impl IntoIterator<Item = &'boundary BoundaryMachineDeclaration>,
    types: &[StructuralTypeDeclaration],
    dynamic_dispatch: &TerminalDynamicDispatchCatalog,
) -> Option<Vec<DescriptorMachine>> {
    abstract_graph_with_slots(
        functions,
        boundaries,
        types,
        dynamic_dispatch,
        kernel::DescriptorSlotKind::References,
    )
}

pub fn abstract_borrowed_storage_catalog_graph<'function, 'boundary>(
    functions: impl IntoIterator<Item = &'function AbstractFunction>,
    boundaries: impl IntoIterator<Item = &'boundary BoundaryMachineDeclaration>,
    types: &[StructuralTypeDeclaration],
    dynamic_dispatch: &TerminalDynamicDispatchCatalog,
) -> Option<Vec<DescriptorMachine>> {
    abstract_graph_with_slots(
        functions,
        boundaries,
        types,
        dynamic_dispatch,
        kernel::DescriptorSlotKind::BorrowedStorage,
    )
}

pub fn optimized_borrowed_storage_catalog_graph<'function, 'boundary>(
    functions: impl IntoIterator<Item = &'function PsiOptimizationFunction>,
    boundaries: impl IntoIterator<Item = &'boundary BoundaryMachineDeclaration>,
    types: &[StructuralTypeDeclaration],
    dynamic_dispatch: &TerminalDynamicDispatchCatalog,
) -> Option<Vec<DescriptorMachine>> {
    catalog_graph(
        functions,
        boundaries,
        types,
        dynamic_dispatch,
        |_, node| Some(Cow::Borrowed(&node.operation)),
        kernel::DescriptorSlotKind::BorrowedStorage,
        |_, _| false,
    )
}

fn abstract_graph_with_slots<'function, 'boundary>(
    functions: impl IntoIterator<Item = &'function AbstractFunction>,
    boundaries: impl IntoIterator<Item = &'boundary BoundaryMachineDeclaration>,
    types: &[StructuralTypeDeclaration],
    dynamic_dispatch: &TerminalDynamicDispatchCatalog,
    slot_kind: kernel::DescriptorSlotKind,
) -> Option<Vec<DescriptorMachine>> {
    let functions = functions
        .into_iter()
        .map(reconstruct_psi_optimization_function_seed)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    catalog_graph(
        functions.iter(),
        boundaries,
        types,
        dynamic_dispatch,
        |_, node| Some(Cow::Borrowed(&node.operation)),
        slot_kind,
        |_, _| false,
    )
}

pub fn abstract_reference_catalog_graph_with_boundary_preservation<'function, 'boundary>(
    functions: impl IntoIterator<Item = &'function AbstractFunction>,
    boundaries: impl IntoIterator<Item = &'boundary BoundaryMachineDeclaration>,
    types: &[StructuralTypeDeclaration],
    dynamic_dispatch: &TerminalDynamicDispatchCatalog,
    operation: impl for<'node> Fn(
        &PsiOptimizationFunction,
        &'node OptimizationNode,
    ) -> Option<Cow<'node, O>>,
    preserves_boundary: impl Fn(&PsiOptimizationFunction, &OptimizationNode) -> bool,
) -> Option<Vec<DescriptorMachine>> {
    let functions = functions
        .into_iter()
        .map(reconstruct_psi_optimization_function_seed)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    optimized_reference_catalog_graph_with_boundary_preservation(
        functions.iter(),
        boundaries,
        types,
        dynamic_dispatch,
        operation,
        preserves_boundary,
    )
}

pub(crate) fn optimized_graph(unit: &PsiOptimizationUnit) -> Option<Vec<DescriptorMachine>> {
    optimized_catalog_graph(
        unit.functions.iter(),
        &unit.boundary_machines,
        &unit.structural_types,
        &unit.dynamic_dispatch,
        |_, node| Some(Cow::Borrowed(&node.operation)),
    )
}

pub fn optimized_catalog_graph<'function, 'boundary>(
    functions: impl IntoIterator<Item = &'function PsiOptimizationFunction>,
    boundaries: impl IntoIterator<Item = &'boundary BoundaryMachineDeclaration>,
    types: &[StructuralTypeDeclaration],
    dynamic_dispatch: &TerminalDynamicDispatchCatalog,
    operation: impl for<'node> Fn(
        &PsiOptimizationFunction,
        &'node OptimizationNode,
    ) -> Option<Cow<'node, O>>,
) -> Option<Vec<DescriptorMachine>> {
    catalog_graph(
        functions,
        boundaries,
        types,
        dynamic_dispatch,
        operation,
        kernel::DescriptorSlotKind::Views,
        |_, _| false,
    )
}

pub fn optimized_reference_catalog_graph<'function, 'boundary>(
    functions: impl IntoIterator<Item = &'function PsiOptimizationFunction>,
    boundaries: impl IntoIterator<Item = &'boundary BoundaryMachineDeclaration>,
    types: &[StructuralTypeDeclaration],
    dynamic_dispatch: &TerminalDynamicDispatchCatalog,
    operation: impl for<'node> Fn(
        &PsiOptimizationFunction,
        &'node OptimizationNode,
    ) -> Option<Cow<'node, O>>,
) -> Option<Vec<DescriptorMachine>> {
    optimized_reference_catalog_graph_with_boundary_preservation(
        functions,
        boundaries,
        types,
        dynamic_dispatch,
        operation,
        |_, _| false,
    )
}

pub fn optimized_reference_catalog_graph_with_boundary_preservation<'function, 'boundary>(
    functions: impl IntoIterator<Item = &'function PsiOptimizationFunction>,
    boundaries: impl IntoIterator<Item = &'boundary BoundaryMachineDeclaration>,
    types: &[StructuralTypeDeclaration],
    dynamic_dispatch: &TerminalDynamicDispatchCatalog,
    operation: impl for<'node> Fn(
        &PsiOptimizationFunction,
        &'node OptimizationNode,
    ) -> Option<Cow<'node, O>>,
    preserves_boundary: impl Fn(&PsiOptimizationFunction, &OptimizationNode) -> bool,
) -> Option<Vec<DescriptorMachine>> {
    catalog_graph(
        functions,
        boundaries,
        types,
        dynamic_dispatch,
        operation,
        kernel::DescriptorSlotKind::References,
        preserves_boundary,
    )
}

fn catalog_graph<'function, 'boundary>(
    functions: impl IntoIterator<Item = &'function PsiOptimizationFunction>,
    boundaries: impl IntoIterator<Item = &'boundary BoundaryMachineDeclaration>,
    types: &[StructuralTypeDeclaration],
    dynamic_dispatch: &TerminalDynamicDispatchCatalog,
    operation: impl for<'node> Fn(
        &PsiOptimizationFunction,
        &'node OptimizationNode,
    ) -> Option<Cow<'node, O>>,
    slot_kind: kernel::DescriptorSlotKind,
    preserves_boundary: impl Fn(&PsiOptimizationFunction, &OptimizationNode) -> bool,
) -> Option<Vec<DescriptorMachine>> {
    let functions = functions.into_iter().collect::<Vec<_>>();
    let boundaries = boundaries.into_iter().collect::<Vec<_>>();
    let build = |selector_graph: Option<&[DescriptorMachine]>| {
        let mut identities = BTreeSet::new();
        functions
            .iter()
            .map(|function| {
                if !identities.insert(function.machine) {
                    return None;
                }
                adapt(
                    types,
                    &boundaries,
                    dynamic_dispatch,
                    &functions,
                    function,
                    &operation,
                    slot_kind,
                    &preserves_boundary,
                    selector_graph,
                )
            })
            .collect::<Option<Vec<_>>>()
    };
    let initial = build(None)?;
    if !functions.iter().any(|function| {
        function.block_nodes().any(|node| {
            operation(function, node).is_some_and(|operation| {
                matches!(
                    operation.as_ref(),
                    O::CallStructural {
                        callee: abstract_operations::AbstractCallTarget::Dynamic { .. },
                        ..
                    }
                )
            })
        })
    }) {
        return Some(initial);
    }
    build(Some(&initial))
}

pub fn call_origins(
    unit: &PsiOptimizationUnit,
    caller: MachineId,
    result: PlaceId,
) -> Option<Vec<kernel::DescriptorCallOrigin>> {
    kernel::reconstruct_call_origins(
        &unit.structural_types,
        &optimized_graph(unit)?,
        caller,
        result,
    )
}

pub(crate) fn owned_aggregate_descriptor_returns_with_graph(
    function: &PsiOptimizationFunction,
    graph: Option<&[DescriptorMachine]>,
    types: &[StructuralTypeDeclaration],
) -> bool {
    let Some(graph) = graph else {
        return false;
    };
    let Some(returns) = kernel::descriptor_machine(graph, function.machine)
        .and_then(|machine| machine.returns.as_ref())
    else {
        return false;
    };
    !returns.is_empty()
        && returns.iter().all(|(position, location)| {
            location.path.is_empty()
                && structural_source_contract(function, location.root)
                    .is_some_and(|source| source.access == StructuralAccess::Owned)
                && kernel::reconstruct_return_slots_external(
                    types,
                    graph,
                    function.machine,
                    *position,
                    location.root,
                )
        })
}

fn record_escaped_invocation(
    place_types: &BTreeMap<PlaceId, StructuralTypeId>,
    unresolved_contents_writes: &mut BTreeSet<(BlockId, u32)>,
    step: &mut DescriptorFlowStep,
    types: &[StructuralTypeDeclaration],
    structural_arguments: &[StructuralArgument],
    slot_kind: kernel::DescriptorSlotKind,
    position: (BlockId, u32),
) -> Option<()> {
    if structural_arguments.iter().any(|actual| {
        place_types.get(&actual.place).is_none_or(|root_type| {
            slot_kind.may_write_indirect_contents(types, *root_type, actual)
        })
    }) {
        unresolved_contents_writes.insert(position);
    }
    for actual in structural_arguments
        .iter()
        .filter(|actual| actual.access != StructuralAccess::SharedBorrow)
    {
        step.definitions.extend(writable_descriptor_slots(
            types,
            place_types,
            actual,
            slot_kind,
        )?);
    }
    Some(())
}

fn adapt(
    types: &[StructuralTypeDeclaration],
    boundaries: &[&BoundaryMachineDeclaration],
    dynamic_dispatch: &TerminalDynamicDispatchCatalog,
    functions: &[&PsiOptimizationFunction],
    function: &PsiOptimizationFunction,
    operation_projection: &impl for<'node> Fn(
        &PsiOptimizationFunction,
        &'node OptimizationNode,
    ) -> Option<Cow<'node, O>>,
    slot_kind: kernel::DescriptorSlotKind,
    preserves_boundary: &impl Fn(&PsiOptimizationFunction, &OptimizationNode) -> bool,
    selector_graph: Option<&[DescriptorMachine]>,
) -> Option<DescriptorMachine> {
    let scalar_expressions = ScalarExpressions::new(function);
    let mut graph = DescriptorMachine {
        id: function.machine,
        entry: (function.entry, 0),
        returns: function
            .result
            .structural()
            .filter(|result| {
                slot_kind.tracks_returned_value(types, result.place, result.structural_type)
            })
            .map(|_| Vec::new()),
        places: Vec::new(),
        place_types: BTreeMap::new(),
        flow: BTreeMap::new(),
        case_constructors: BTreeMap::new(),
        case_successors: BTreeMap::new(),
        originations: BTreeMap::new(),
        projections: BTreeMap::new(),
        contents_queries: BTreeMap::new(),
        contents_transfers: BTreeMap::new(),
        payload_invalidations: BTreeMap::new(),
        payload_producers: BTreeMap::new(),
        payload_entry_predicates: function.verified_contract.as_ref().and_then(|contract| {
            Some((
                semantic_vocabulary::PropositionContext::from_value_types_and_places(
                    function
                        .parameters
                        .iter()
                        .map(|value| (value.value, value.scalar_type))
                        .chain(
                            contract
                                .erased_scalar_formals
                                .iter()
                                .map(|value| (value.id, value.scalar_type)),
                        ),
                    function
                        .structural_places
                        .iter()
                        .map(|place| (place.id, place.kind)),
                )
                .ok()?,
                contract.requires.clone(),
            ))
        }),
        scalar_payload_writes: BTreeMap::new(),
        scalar_payload_reads: BTreeMap::new(),
        payload_scalar_definitions: BTreeMap::new(),
        payload_scalar_postconditions: BTreeMap::new(),
        payload_scalar_parameters: BTreeMap::new(),
        payload_edge_predicates: BTreeMap::new(),
        payload_scalar_bindings: BTreeMap::new(),
        unresolved_contents_writes: BTreeSet::new(),
        storage_derivations: BTreeSet::new(),
        calls: BTreeMap::new(),
        normal_exits: Vec::new(),
    };
    let declaration = |place| {
        let mut matching = function
            .structural_places
            .iter()
            .filter(|row| row.id == place);
        let row = matching.next()?;
        matching.next().is_none().then_some(row)
    };
    for (position, parameter) in function.structural_parameters.iter().enumerate() {
        if declaration(parameter.place)?.kind
            != (StructuralPlaceKind::Parameter {
                position: parameter.position,
            })
        {
            return None;
        }
        if graph
            .place_types
            .insert(parameter.place, parameter.structural_type)
            .is_some()
        {
            return None;
        }
        graph.places.push(DescriptorPlace {
            place: parameter.place,
            definition: DescriptorDefinition::Parameter {
                position,
                access: parameter.access,
                structural_type: parameter.structural_type,
            },
        });
    }
    for block in &function.blocks {
        graph.payload_scalar_parameters.insert(
            block.id,
            block.parameters.iter().map(|value| value.value).collect(),
        );
        for (position, parameter) in block.structural_parameters.iter().enumerate() {
            if declaration(parameter.place)?.kind
                != (StructuralPlaceKind::BlockParameter {
                    block: block.id,
                    position: u32::try_from(position).ok()?,
                })
            {
                return None;
            }
            if graph
                .place_types
                .insert(parameter.place, parameter.structural_type)
                .is_some()
            {
                return None;
            }
            graph.places.push(DescriptorPlace {
                place: parameter.place,
                definition: DescriptorDefinition::Arrival {
                    access: parameter.access,
                },
            });
        }
        for node in &block.nodes {
            if let abstract_operations::byte_literal_establish_pair_pattern!(
                place,
                structural_type,
                ..
            ) = &node.operation
            {
                if declaration(place.id)? != place
                    || graph
                        .place_types
                        .insert(place.id, structural_type.id)
                        .is_some()
                {
                    return None;
                }
                graph.places.push(DescriptorPlace {
                    place: place.id,
                    definition: if matches!(node.operation, O::EstablishByteSequenceLiteral { .. })
                    {
                        DescriptorDefinition::Literal
                    } else {
                        DescriptorDefinition::Local
                    },
                });
            } else if let Some((producer, result)) = node.operation.structural_result() {
                if declaration(result.place)?.kind
                    != (StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type: result.structural_type,
                    })
                    || graph
                        .place_types
                        .insert(result.place, result.structural_type)
                        .is_some()
                {
                    return None;
                }
                graph.places.push(DescriptorPlace {
                    place: result.place,
                    definition: DescriptorDefinition::Local,
                });
            }
        }
    }
    let location = |root, path: &[StructuralPathSegment], field| {
        descriptor_location(types, root, *graph.place_types.get(&root)?, path, field)
    };
    for block in &function.blocks {
        for (index, node) in block.nodes.iter().enumerate() {
            let position = (block.id, u32::try_from(index).ok()?);
            let selected = operation_projection(function, node)?;
            let operation = selected.as_ref();
            if let Some(guarantees) = scalar_call_guarantees(
                operation,
                &|identity| {
                    let mut matching = functions
                        .iter()
                        .copied()
                        .filter(|candidate| candidate.machine == identity);
                    let candidate = matching.next()?;
                    matching.next().is_none().then_some(candidate)
                },
                &|value| {
                    scalar_expressions
                        .expression(value)
                        .map(|term| term.scalar_type())
                },
            ) {
                graph
                    .payload_scalar_postconditions
                    .insert(position, guarantees);
            }
            if let O::StructuralScalarFieldStore {
                destination,
                path,
                field,
                value,
                ..
            } = operation
                && let Some(value) = scalar_expressions.expression(value.value)
                && let Some(destination) = location(destination.place(), path, Some(*field))
            {
                graph
                    .scalar_payload_writes
                    .entry(position)
                    .or_default()
                    .push((destination, value));
            }
            let result = operation.structural_result().map(|(_, result)| result);
            if !node.definitions.is_empty() {
                graph.payload_scalar_definitions.insert(
                    position,
                    node.definitions.iter().map(|value| value.value).collect(),
                );
            }
            if let O::PrimitiveScalarRead {
                source,
                path,
                result,
                ..
            } = operation
                && let Some(location) = location(*source, path, None)
            {
                graph
                    .scalar_payload_reads
                    .entry(position)
                    .or_default()
                    .push((
                        location,
                        semantic_vocabulary::ScalarTerm::value(result.value, result.scalar_type),
                    ));
            }
            if let O::StructuralScalarFieldRead {
                source,
                path,
                field,
                result,
                ..
            } = operation
            {
                let mut path = path.clone();
                path.push(semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                    *field,
                ));
                if let Some(location) = canonical_descriptor_location(
                    types,
                    *source,
                    *graph.place_types.get(source)?,
                    &path,
                ) {
                    graph
                        .scalar_payload_reads
                        .entry(position)
                        .or_default()
                        .push((
                            location,
                            semantic_vocabulary::ScalarTerm::value(
                                result.value,
                                result.scalar_type,
                            ),
                        ));
                }
            }
            let mut step = DescriptorFlowStep {
                inputs: crate::unit_validation::function_structure::operation_place_inputs(
                    operation,
                )
                .into_iter()
                .map(DescriptorLocation::whole)
                .collect(),
                ..Default::default()
            };
            if let Some(result) = result {
                graph.payload_producers.insert(position, result.place);
                let destination = DescriptorLocation::whole(result.place);
                step.definitions.push(destination.clone());
                if !kernel::is_descriptor(types.iter(), result.structural_type)
                    && !slot_kind.is_slot(types, result.structural_type)
                {
                    graph.originations.entry(position).or_default().push((
                        destination,
                        kernel::DescriptorCallOrigin::Argument {
                            source: result.place,
                            projection: Projection::new(&[]),
                        },
                    ));
                }
            }
            let payload_query = |root, path: &[StructuralPathSegment], field| {
                let mut query = kernel::descriptor_contents_query(
                    types,
                    root,
                    *graph.place_types.get(&root)?,
                    path,
                    field,
                )?;
                if field.is_none() {
                    let selected = runtime_structural_path_tip(
                        types.iter(),
                        *graph.place_types.get(&root)?,
                        path,
                    )?;
                    match selected.shape {
                        StructuralTypeShape::ElementView { element, .. } => {
                            query
                                .steps
                                .push(kernel::DescriptorContentsStep::AnyElement { element });
                        }
                        StructuralTypeShape::ByteSequence(_)
                            if kernel::is_descriptor(types.iter(), selected.id) =>
                        {
                            return None;
                        }
                        _ => {}
                    }
                }
                Some(query)
            };
            let mut payload_writes = Vec::new();
            match operation {
                O::WriteOnlyPrimitiveStore {
                    destination, path, ..
                } => {
                    payload_writes.push(payload_query(destination.place(), path, None));
                }
                O::StructuralScalarFieldStore {
                    destination,
                    path,
                    field,
                    ..
                } => {
                    payload_writes.push(payload_query(destination.place(), path, Some(*field)));
                }
                O::StructuralByteSequenceFieldByteStore {
                    destination,
                    path,
                    field,
                    ..
                }
                | O::StructuralByteSequenceFieldStore {
                    destination,
                    path,
                    field,
                    ..
                } => {
                    payload_writes.push(payload_query(*destination, path, Some(*field)));
                }
                O::StructuralByteViewFieldStore {
                    destination,
                    path,
                    field,
                    ..
                } => {
                    payload_writes.push(payload_query(*destination, path, *field));
                }
                O::ByteSequenceWrite { .. } | O::ByteSequenceStore { .. } => {
                    payload_writes.push(None);
                }
                O::ElementViewWrite {
                    destination,
                    index,
                    obligation,
                    path,
                    ..
                } => {
                    let mut selected = destination.path.clone();
                    selected.push(StructuralPathSegment::RuntimeIndex {
                        index: *index,
                        obligation: *obligation,
                    });
                    selected.extend_from_slice(path);
                    payload_writes.push(payload_query(destination.place, &selected, None));
                }
                O::CallStructural {
                    structural_arguments: arguments,
                    ..
                }
                | O::BoundaryCall {
                    structural_arguments: arguments,
                    ..
                } => {
                    for argument in arguments {
                        let selected = runtime_structural_path_tip(
                            types.iter(),
                            *graph.place_types.get(&argument.place)?,
                            &argument.path,
                        );
                        let mutation = selected
                            .map(|selected| {
                                terminal_semantics::shared_mutation(types.iter(), selected.id, &[])
                            })
                            .unwrap_or(terminal_semantics::SharedMutation::Unbounded);
                        if mutation == terminal_semantics::SharedMutation::Unbounded {
                            payload_writes.push(None);
                        } else if argument.access != StructuralAccess::SharedBorrow
                            || mutation != terminal_semantics::SharedMutation::None
                        {
                            payload_writes.push(payload_query(
                                argument.place,
                                &argument.path,
                                None,
                            ));
                        }
                    }
                }
                O::AtomicEvent { event, .. } => {
                    if let Some(location) = event.location() {
                        payload_writes.push(payload_query(location.root, &location.path, None));
                    }
                }
                _ => {}
            }
            if !payload_writes.is_empty() {
                graph.payload_invalidations.insert(position, payload_writes);
            }
            match operation {
                O::ByteSequenceLength { source, .. } | O::ElementViewLength { source, .. } => {
                    let query = kernel::descriptor_contents_query(
                        types,
                        source.place,
                        *graph.place_types.get(&source.place)?,
                        &source.path,
                        None,
                    )?;
                    step.inputs = vec![query.observation()];
                }
                O::EstablishByteSequenceLiteral { place, .. } => {
                    graph.payload_producers.insert(position, place.id);
                    let destination = DescriptorLocation::whole(place.id);
                    step.definitions.push(destination.clone());
                    graph.originations.entry(position).or_default().push((
                        destination,
                        kernel::DescriptorCallOrigin::Literal {
                            machine: function.machine,
                            place: place.id,
                        },
                    ));
                }
                O::EstablishNativeCodeLayout { result, .. } if slot_kind.tracks_views() => {
                    let destination = kernel::native_code_layout_regions(types, result)?;
                    step.definitions.push(destination.clone());
                    graph.originations.entry(position).or_default().push((
                        destination,
                        kernel::DescriptorCallOrigin::Literal {
                            machine: function.machine,
                            place: result.place,
                        },
                    ));
                }
                O::EstablishZeroedStructural { result, .. } => {
                    for slot in slot_kind
                        .inventory(types, result.place, result.structural_type)?
                        .slots
                    {
                        graph
                            .originations
                            .entry(position)
                            .or_default()
                            .push((slot, kernel::DescriptorCallOrigin::Empty));
                    }
                }
                O::EstablishAccessReference {
                    psi_operation,
                    result,
                    correspondence,
                    ..
                } if slot_kind.tracks_references() => {
                    graph.originations.entry(position).or_default().push((
                        DescriptorLocation::whole(result.place),
                        kernel::DescriptorCallOrigin::AccessFormation {
                            machine: function.machine,
                            operation: *psi_operation,
                            place: result.place,
                            carrier: correspondence.carrier(),
                            projection: Projection::new(&[]),
                        },
                    ));
                }
                O::EstablishElementView { result, source, .. }
                | O::EstablishReference { result, source, .. } => {
                    let destination = DescriptorLocation::whole(result.place);
                    let (path, mut projection) = match source.path.split_last() {
                        Some((range @ StructuralPathSegment::FixedRange { .. }, path))
                            if matches!(operation, O::EstablishElementView { .. }) =>
                        {
                            (path, Some(Projection::new(std::slice::from_ref(range))))
                        }
                        _ => (source.path.as_slice(), None),
                    };
                    if let O::EstablishElementView {
                        element, region, ..
                    } = operation
                        && !element_coordinates_preserved(
                            function,
                            types.iter(),
                            source,
                            *element,
                            *region,
                        )
                    {
                        projection
                            .get_or_insert_with(|| Projection::new(&[]))
                            .forget_address();
                    }
                    let alternatives =
                        terminal_semantics::descriptor_lifetimes::fixed_runtime_index_alternatives(
                            types,
                            source.place,
                            *graph.place_types.get(&source.place)?,
                            path,
                        );
                    if let Some(alternatives) = alternatives {
                        for alternative in alternatives {
                            step.derivations.push((alternative, destination.clone()));
                        }
                    } else if let Some(source) = location(source.place, path, None) {
                        step.derivations.push((source.clone(), destination.clone()));
                        if slot_kind.tracks_references()
                            && matches!(operation, O::EstablishReference { .. })
                        {
                            graph.storage_derivations.insert((
                                position,
                                source.clone(),
                                destination.clone(),
                            ));
                        }
                        if let Some(projection) = projection {
                            graph
                                .projections
                                .insert((position, source, destination.clone()), projection);
                        }
                    } else {
                        step.may_escape = true;
                    }
                    if let Some(origins) = graph.originations.get_mut(&position) {
                        origins.retain(|(location, _)| location != &destination);
                    }
                }
                abstract_operations::element_view_or_byte_subslice_pattern!(
                    result,
                    source,
                    start,
                    end,
                    ..
                ) => {
                    let query = kernel::descriptor_contents_query(
                        types,
                        source.place,
                        *graph.place_types.get(&source.place)?,
                        &source.path,
                        None,
                    )?;
                    let source = query.observation();
                    step.inputs = vec![source.clone()];
                    let destination = DescriptorLocation::whole(result.place);
                    step.derivations.push((source.clone(), destination.clone()));
                    graph
                        .contents_queries
                        .insert((position, source.clone(), destination.clone()), query);
                    graph.projections.insert(
                        (position, source, destination),
                        Projection::window(*start, *end, position),
                    );
                }
                O::StructuralLeafCopy { .. }
                | O::MoveStructuralValue { .. }
                | O::TransformContent { .. } => {
                    let owned_path;
                    let (result, source, path) = match &node.operation {
                        O::StructuralLeafCopy {
                            result,
                            source,
                            path,
                            ..
                        } => (result, source, path),
                        O::MoveStructuralValue { result, source, .. }
                        | O::TransformContent { result, source, .. } => {
                            owned_path = StructuralCopyPath::Ordinary(source.path.clone());
                            (result, &source.place, &owned_path)
                        }
                        _ => return None,
                    };
                    let destination = DescriptorLocation::whole(result.place);
                    let source = match path {
                        StructuralCopyPath::Ordinary(path) => location(*source, path, None),
                        StructuralCopyPath::Case(path) => canonical_descriptor_location(
                            types,
                            *source,
                            *graph.place_types.get(source)?,
                            path,
                        ),
                    };
                    if let Some(source) = source {
                        step.inputs = vec![source.clone()];
                        step.derivations.push((source, destination.clone()));
                    } else {
                        step.may_escape = true;
                    }
                    if (kernel::is_descriptor(types.iter(), result.structural_type)
                        || types.iter().any(|ty| {
                            ty.id == result.structural_type
                                && matches!(ty.shape, StructuralTypeShape::Reference { .. })
                        }))
                        && let Some(origins) = graph.originations.get_mut(&position)
                    {
                        origins.retain(|(location, _)| location != &destination);
                    }
                }
                abstract_operations::byte_len_or_field_read_pattern!(source, path, field, ..) => {
                    if let Some(source) = location(*source, path, Some(*field)) {
                        step.inputs = vec![source];
                    } else {
                        step.may_escape = true;
                    }
                }
                O::MoveStructuralField {
                    source,
                    path,
                    field,
                    ..
                } => {
                    let result = result?;
                    graph.contents_transfers.insert(
                        position,
                        kernel::descriptor_contents_transfers(
                            types,
                            slot_kind,
                            kernel::DescriptorContentsOperand {
                                root: source.place(),
                                root_type: *graph.place_types.get(&source.place())?,
                                path,
                                field: *field,
                            },
                            kernel::DescriptorContentsOperand {
                                root: result.place,
                                root_type: result.structural_type,
                                path: &[],
                                field: None,
                            },
                            result.structural_type,
                            true,
                        )?,
                    );
                    if let Some(source) = location(source.place(), path, *field) {
                        let destination = DescriptorLocation::whole(result.place);
                        step.inputs = vec![source.clone()];
                        step.definitions.push(source.clone());
                        step.derivations.push((source, destination));
                    } else {
                        step.may_escape = true;
                    }
                }
                O::StoreStructuralField {
                    destination,
                    path,
                    field,
                    value,
                    ..
                } => {
                    let value_type = runtime_structural_path_tip(
                        types.iter(),
                        *graph.place_types.get(&value.place)?,
                        &value.path,
                    )?
                    .id;
                    graph.contents_transfers.insert(
                        position,
                        kernel::descriptor_contents_transfers(
                            types,
                            slot_kind,
                            kernel::DescriptorContentsOperand {
                                root: value.place,
                                root_type: *graph.place_types.get(&value.place)?,
                                path: &value.path,
                                field: None,
                            },
                            kernel::DescriptorContentsOperand {
                                root: destination.place(),
                                root_type: *graph.place_types.get(&destination.place())?,
                                path,
                                field: *field,
                            },
                            value_type,
                            false,
                        )?,
                    );
                    if let (Some(destination), Some(source)) = (
                        location(destination.place(), path, *field),
                        location(value.place, &value.path, None),
                    ) {
                        step.definitions.push(destination.clone());
                        step.derivations.push((source, destination.clone()));
                        if destination.path.is_empty()
                            && !kernel::is_descriptor(
                                types.iter(),
                                *graph.place_types.get(&destination.root)?,
                            )
                            && !slot_kind.is_slot(types, *graph.place_types.get(&destination.root)?)
                        {
                            graph.originations.entry(position).or_default().push((
                                destination.clone(),
                                kernel::DescriptorCallOrigin::Argument {
                                    source: destination.root,
                                    projection: Projection::new(&[]),
                                },
                            ));
                        }
                    } else {
                        step.definitions.extend(slot_kind.overwritten_slots(
                            types,
                            destination.place(),
                            *graph.place_types.get(&destination.place())?,
                            path,
                            *field,
                        )?);
                        step.may_escape = true;
                    }
                }
                O::StructuralByteViewFieldStore {
                    destination,
                    path,
                    field,
                    source,
                    ..
                } => {
                    if let Some(destination) = location(*destination, path, *field) {
                        step.definitions.push(destination.clone());
                        step.derivations
                            .push((DescriptorLocation::whole(*source), destination));
                    } else {
                        graph.unresolved_contents_writes.insert(position);
                        step.definitions.extend(slot_kind.overwritten_slots(
                            types,
                            *destination,
                            *graph.place_types.get(destination)?,
                            path,
                            *field,
                        )?);
                        step.may_escape = true;
                    }
                    step.inputs = vec![DescriptorLocation::whole(*source)];
                }
                O::EstablishPrimitiveLocal { result, value, .. } => {
                    if let Some(value) = scalar_expressions.expression(value.value) {
                        graph
                            .scalar_payload_writes
                            .entry(position)
                            .or_default()
                            .push((location(result.place, &[], None)?, value));
                    }
                }
                O::PrimitiveLocalStore {
                    destination, value, ..
                } => {
                    if let Some(value) = scalar_expressions.expression(value.value) {
                        graph
                            .scalar_payload_writes
                            .entry(position)
                            .or_default()
                            .push((location(*destination, &[], None)?, value));
                    }
                }
                O::EstablishScalarArray {
                    result, elements, ..
                } => {
                    for (ordinal, value) in elements.iter().enumerate() {
                        let path = terminal_semantics::scalar_array_leaf_path(
                            types.iter(),
                            result.structural_type,
                            u64::try_from(ordinal).ok()?,
                        )?;
                        if let Some(value) = scalar_expressions.expression(*value) {
                            graph
                                .scalar_payload_writes
                                .entry(position)
                                .or_default()
                                .push((location(result.place, &path, None)?, value));
                        }
                    }
                }
                O::EstablishStructuralArray {
                    result, elements, ..
                } => {
                    for (index, argument) in elements.iter().enumerate() {
                        let destination = location(
                            result.place,
                            &[StructuralPathSegment::FixedIndex(index as u64)],
                            None,
                        )?;
                        if let Some(source) = location(argument.place, &argument.path, None) {
                            step.derivations.push((source, destination));
                        } else {
                            step.may_escape = true;
                        }
                    }
                }
                abstract_operations::establish_record_or_case_pattern!(result, fields, ..) => {
                    if let O::EstablishCase { result_case, .. } = operation {
                        let shape = &types
                            .iter()
                            .find(|row| row.id == result.structural_type)?
                            .shape;
                        let cases = match shape {
                            StructuralTypeShape::Sum { cases }
                            | StructuralTypeShape::Mixed { cases, .. } => cases,
                            _ => return None,
                        };
                        if !cases.iter().any(|case| case.id == *result_case) {
                            return None;
                        }
                        for slot in slot_kind
                            .inventory(types, result.place, result.structural_type)?
                            .slots
                        {
                            if matches!(slot.path.first(), Some(terminal_semantics::descriptor_lifetimes::DescriptorPathSegment::Case(case)) if case != result_case)
                            {
                                graph
                                    .originations
                                    .entry(position)
                                    .or_default()
                                    .push((slot, kernel::DescriptorCallOrigin::Empty));
                            }
                        }
                    }
                    for (field_ordinal, field) in fields.iter().enumerate() {
                        let destination = match operation {
                            O::EstablishCase { result_case, .. } => {
                                let shape = &types
                                    .iter()
                                    .find(|row| row.id == result.structural_type)?
                                    .shape;
                                let path = terminal_psi::structural_case_initializer_path(
                                    shape,
                                    *result_case,
                                    field_ordinal,
                                    field.field,
                                )?;
                                location(result.place, &path, None)?
                            }
                            _ => location(result.place, &[], Some(field.field))?,
                        };
                        if let terminal_psi::RecordFieldValue::Scalar { value, .. } = &field.value
                            && let Some(value) = scalar_expressions.expression(*value)
                        {
                            graph
                                .scalar_payload_writes
                                .entry(position)
                                .or_default()
                                .push((destination.clone(), value));
                        }
                        if let terminal_psi::RecordFieldValue::Structural(source) = &field.value {
                            if let Some(source) = location(source.place, &source.path, None) {
                                step.derivations.push((source, destination));
                            } else {
                                step.may_escape = true;
                            }
                        }
                    }
                }
                O::CallStructural {
                    callee:
                        abstract_operations::AbstractCallTarget::Dynamic {
                            selector,
                            interface,
                            slot,
                        },
                    structural_arguments,
                    ..
                } => {
                    let effects_machine = selected_effects_machine(
                        dynamic_dispatch,
                        functions,
                        function,
                        types,
                        selector_graph,
                        operation_projection,
                        *selector,
                        *interface,
                        *slot,
                    );
                    let realizations =
                        kernel::public_realizations(dynamic_dispatch, *interface, *slot)?;
                    let slot = dynamic_dispatch
                        .interface(*interface)?
                        .slots
                        .get(usize::try_from(*slot).ok()?)?;
                    let mut call = kernel::public_invocation(
                        node.operation.psi_operation()?,
                        types,
                        slot_kind,
                        slot,
                        realizations.clone(),
                        structural_arguments,
                        result,
                        &graph.place_types,
                    )?;
                    let complete_contents = call.argument_contents.iter().all(Option::is_some);
                    let effects_machine = effects_machine.filter(|_| complete_contents);
                    if let kernel::DescriptorInvocationTarget::Public {
                        effects_machine: target,
                        ..
                    } = &mut call.target
                    {
                        *target = effects_machine;
                    }
                    let has_stored_effects = complete_contents
                        && (effects_machine.is_some() || !realizations.is_empty());
                    if !has_stored_effects {
                        for actual in structural_arguments
                            .iter()
                            .filter(|actual| actual.access != StructuralAccess::SharedBorrow)
                        {
                            step.definitions.extend(writable_descriptor_slots(
                                types,
                                &graph.place_types,
                                actual,
                                slot_kind,
                            )?);
                        }
                    }
                    if matches!(
                        call.target,
                        kernel::DescriptorInvocationTarget::Public {
                            invalidates_contents: true,
                            ..
                        }
                    ) && !has_stored_effects
                    {
                        graph.unresolved_contents_writes.insert(position);
                        step.may_escape = true;
                    }
                    graph.calls.insert(position, call);
                }
                O::CallStructural {
                    callee,
                    structural_arguments,
                    claim_transfers,
                    result: call_result,
                    ..
                } => {
                    let mut matching = functions
                        .iter()
                        .filter(|function| Some(function.machine) == callee.machine());
                    if let Some(callee) = matching.next().copied() {
                        if matching.next().is_some()
                            || callee.structural_parameters.len() != structural_arguments.len()
                        {
                            return None;
                        }
                        let mut arguments = Vec::new();
                        let mut argument_contents = Vec::new();
                        let mut exact_arguments = true;
                        for (actual, formal) in structural_arguments
                            .iter()
                            .zip(&callee.structural_parameters)
                        {
                            if actual.access != formal.access {
                                return None;
                            }
                            argument_contents.push(graph.place_types.get(&actual.place).and_then(
                                |root_type| {
                                    kernel::descriptor_contents_query(
                                        types,
                                        actual.place,
                                        *root_type,
                                        &actual.path,
                                        None,
                                    )
                                },
                            ));
                            let actual = location(actual.place, &actual.path, None);
                            exact_arguments &= actual.is_some();
                            arguments.push(actual);
                        }
                        let descriptor_result = result.filter(|result| {
                            slot_kind.tracks_returned_value(
                                types,
                                result.place,
                                result.structural_type,
                            )
                        });
                        if let Some(result) = descriptor_result {
                            let expected = callee.result.structural()?;
                            if result.structural_type != expected.structural_type
                                || result.multiplicity != expected.multiplicity
                                || result.qualifications != expected.qualifications
                                || !expected
                                    .projected_qualifications
                                    .iter()
                                    .all(|row| result.projected_qualifications.contains(row))
                            {
                                return None;
                            }
                        }
                        graph.calls.insert(
                            position,
                            DescriptorInvocation {
                                operation: node.operation.psi_operation()?,
                                target: kernel::DescriptorInvocationTarget::Machine(callee.machine),
                                arguments,
                                argument_contents,
                                result: descriptor_result
                                    .map(|result| DescriptorLocation::whole(result.place)),
                            },
                        );
                        if !exact_arguments {
                            step.may_escape = true;
                            record_escaped_invocation(
                                &graph.place_types,
                                &mut graph.unresolved_contents_writes,
                                &mut step,
                                types,
                                structural_arguments,
                                slot_kind,
                                position,
                            )?;
                        }
                    } else {
                        let declarations = types.iter().map(|ty| (ty.id, ty)).collect();
                        let mut closed_storage = BTreeMap::new();
                        let closed = matches!(
                            call_result,
                            abstract_operations::AbstractStructuralCallResult::Unit
                                | abstract_operations::AbstractStructuralCallResult::Scalar(_)
                        ) && claim_transfers.is_empty()
                            && structural_arguments.iter().all(|actual| {
                                actual.access == StructuralAccess::SharedBorrow
                                    || graph.place_types.get(&actual.place).is_some_and(|ty| {
                                        crate::current_ownership::argument_storage_is_closed(
                                            &declarations,
                                            *ty,
                                            &actual.path,
                                            actual.access,
                                            &mut closed_storage,
                                        )
                                    })
                            });
                        step.may_escape |= !closed;
                        record_escaped_invocation(
                            &graph.place_types,
                            &mut graph.unresolved_contents_writes,
                            &mut step,
                            types,
                            structural_arguments,
                            slot_kind,
                            position,
                        )?;
                    }
                }
                O::BoundaryCall {
                    boundary,
                    result,
                    structural_arguments,
                    completion_receipts,
                    ..
                } => {
                    if slot_kind == kernel::DescriptorSlotKind::References
                        && let abstract_operations::AbstractBoundaryResult::Structural(result) =
                            result
                        && let Some(declaration) = boundaries.iter().find(|row| row.id == *boundary)
                        && declaration.declaration.as_ref().is_some_and(|head| {
                            head.execution.placement_operation()
                                == Some(terminal_psi::TerminalAccessLifecycleOperation::View)
                        })
                        && let Some((path, carrier)) =
                            terminal_semantics::access_view_reference_slot(
                                types,
                                result.structural_type,
                            )
                    {
                        graph.originations.entry(position).or_default().push((
                            descriptor_location(
                                types,
                                result.place,
                                result.structural_type,
                                &path,
                                None,
                            )?,
                            kernel::DescriptorCallOrigin::AccessFormation {
                                machine: function.machine,
                                operation: operation.psi_operation()?,
                                place: result.place,
                                carrier,
                                projection: Projection::new(&[]),
                            },
                        ));
                    }
                    let mut declarations = boundaries
                        .iter()
                        .filter(|declaration| declaration.id == *boundary);
                    let preserved = match (declarations.next(), declarations.next()) {
                        (Some(declaration), None) => {
                            kernel::boundary_completion_preserves_descriptor_storage(
                                types.iter(),
                                declaration,
                                match result {
                                    abstract_operations::AbstractBoundaryResult::Unit => {
                                        kernel::BoundaryCompletionResult::Unit
                                    }
                                    abstract_operations::AbstractBoundaryResult::Scalar(result) => {
                                        kernel::BoundaryCompletionResult::Scalar(result.scalar_type)
                                    }
                                    abstract_operations::AbstractBoundaryResult::Structural(
                                        result,
                                    ) => kernel::BoundaryCompletionResult::Structural(
                                        result.structural_type,
                                    ),
                                },
                                !completion_receipts.is_empty(),
                                structural_arguments,
                                |place| graph.place_types.get(&place).copied(),
                            )
                        }
                        _ => false,
                    };
                    let preserved = preserved
                        || (slot_kind.tracks_references() && preserves_boundary(function, node));
                    if !preserved {
                        step.may_escape = true;
                        record_escaped_invocation(
                            &graph.place_types,
                            &mut graph.unresolved_contents_writes,
                            &mut step,
                            types,
                            structural_arguments,
                            slot_kind,
                            position,
                        )?;
                    }
                    let mut declarations = boundaries
                        .iter()
                        .filter(|declaration| declaration.id == *boundary);
                    if let (Some(declaration), None) = (declarations.next(), declarations.next())
                        && (!declaration.reference_outputs.is_empty()
                            || !declaration.result_reference_regions.is_empty())
                        && slot_kind != kernel::DescriptorSlotKind::AccessOrigins
                        && !(slot_kind == kernel::DescriptorSlotKind::References
                            && declaration.declaration.as_ref().is_some_and(|head| {
                                head.execution.placement_operation()
                                    == Some(terminal_psi::TerminalAccessLifecycleOperation::View)
                            }))
                    {
                        graph.calls.insert(
                            position,
                            kernel::provider_invocation(
                                node.operation.psi_operation()?,
                                types,
                                slot_kind,
                                declaration,
                                structural_arguments,
                                match result {
                                    abstract_operations::AbstractBoundaryResult::Structural(
                                        value,
                                    ) => Some(value),
                                    _ => None,
                                },
                                &graph.place_types,
                            )?,
                        );
                    }
                }
                O::InvokeNoNormal {
                    structural_arguments,
                    ..
                } => {
                    step.inputs.clear();
                    for argument in structural_arguments {
                        step.inputs.push(
                            location(argument.place, &argument.path, None)
                                .unwrap_or_else(|| DescriptorLocation::whole(argument.place)),
                        );
                    }
                    step.may_escape = true;
                }
                O::ReturnStructural { source, .. } => {
                    if let Some(returns) = &mut graph.returns {
                        if graph.place_types.get(source)
                            != Some(&function.result.structural()?.structural_type)
                        {
                            return None;
                        }
                        returns.push((position, DescriptorLocation::whole(*source)));
                    }
                    graph.normal_exits.push(position);
                    step.may_escape = true;
                }
                abstract_operations::return_or_return_unit_pattern!() => {
                    graph.normal_exits.push(position);
                }
                _ => {}
            }
            if let O::EstablishZeroedStructural { result, .. } = operation {
                graph.case_constructors.insert(
                    position,
                    kernel::zeroed_case_predicates(types, result.place, result.structural_type),
                );
            }
            if let O::EstablishCase {
                result,
                result_case,
                ..
            } = operation
            {
                graph.case_constructors.insert(
                    position,
                    vec![kernel::DescriptorCasePredicate {
                        subject: DescriptorLocation::whole(result.place),
                        case: *result_case,
                    }],
                );
            }
            if let O::StructuralCase {
                source,
                path,
                cases,
            } = operation
            {
                let subject = location(*source, path, None)?;
                let predicates = node
                    .successors
                    .iter()
                    .map(|edge| {
                        let mut matching = cases.iter().filter(|case| {
                            case.psi_edge == edge.psi_edge && case.target == edge.target
                        });
                        let case = matching.next()?;
                        matching.next().is_none().then_some((
                            kernel::DescriptorCasePredicate {
                                subject: subject.clone(),
                                case: case.case,
                            },
                            true,
                        ))
                    })
                    .collect::<Option<Vec<_>>>()?;
                graph.case_successors.insert(position, predicates);
            }
            step.authority_derivations = step.derivations.clone();
            if graph.normal_exits.contains(&position) {
                for parameter in &function.structural_parameters {
                    if matches!(
                        parameter.access,
                        terminal_psi::exclusive_borrow_pair_pattern!()
                    ) {
                        step.inputs.extend(slot_kind.slots(
                            types,
                            parameter.place,
                            parameter.structural_type,
                        )?);
                    }
                }
            }
            if node.successors.is_empty() {
                if index + 1 < block.nodes.len() {
                    step.successors.push(DescriptorFlowEdge {
                        target: (block.id, position.1 + 1),
                        parameters: Vec::new(),
                        bindings: Vec::new(),
                    });
                }
            } else {
                for edge in &node.successors {
                    let mut matching = function
                        .blocks
                        .iter()
                        .filter(|block| block.id == edge.target);
                    let target = matching.next()?;
                    if matching.next().is_some()
                        || target.structural_parameters.len() != edge.structural_bindings.len()
                    {
                        return None;
                    }
                    let scalar_bindings = if let O::StructuralCase {
                        source,
                        path,
                        cases,
                        ..
                    } = operation
                    {
                        let mut matching = cases.iter().filter(|case| {
                            case.psi_edge == edge.psi_edge && case.target == edge.target
                        });
                        let case = matching.next()?;
                        if matching.next().is_some()
                            || !edge.bindings.is_empty()
                            || case.payloads.len() != target.parameters.len()
                        {
                            return None;
                        }
                        let fields = target
                            .parameters
                            .iter()
                            .map(|parameter| {
                                let mut matching = case
                                    .payloads
                                    .iter()
                                    .filter(|payload| payload.parameter == parameter.value);
                                let payload = matching.next()?;
                                (matching.next().is_none()
                                    && payload.scalar_type == parameter.scalar_type)
                                    .then_some(payload.field)
                            })
                            .collect::<Option<Vec<_>>>()?;
                        let captured = kernel::case_payload_scalar_types(
                            types,
                            *graph.place_types.get(source)?,
                            path,
                            case.case,
                            &fields,
                        )?;
                        if captured
                            .iter()
                            .zip(&target.parameters)
                            .any(|(actual, parameter)| *actual != parameter.scalar_type)
                        {
                            return None;
                        }
                        Vec::new()
                    } else {
                        target
                            .parameters
                            .iter()
                            .map(|parameter| {
                                let binding = edge.binding(parameter.value)?;
                                let value = scalar_expressions.expression(binding.argument)?;
                                (value.scalar_type() == parameter.scalar_type)
                                    .then_some((parameter.value, value))
                            })
                            .collect::<Option<Vec<_>>>()?
                    };
                    graph
                        .payload_scalar_bindings
                        .entry(position)
                        .or_default()
                        .push(scalar_bindings);
                    let parameters = target
                        .structural_parameters
                        .iter()
                        .map(|parameter| DescriptorLocation::whole(parameter.place))
                        .collect::<Vec<_>>();
                    let mut bindings = Vec::new();
                    for parameter in &target.structural_parameters {
                        let mut matching = edge
                            .structural_bindings
                            .iter()
                            .filter(|binding| binding.parameter == parameter.place);
                        let binding = matching.next()?;
                        if matching.next().is_some() || binding.argument.access != parameter.access
                        {
                            return None;
                        }
                        let source = if let Some(source) =
                            location(binding.argument.place, &binding.argument.path, None)
                        {
                            DescriptorFlowSource::Location(source)
                        } else {
                            step.may_escape = true;
                            DescriptorFlowSource::from_query(
                                binding.argument.place,
                                kernel::descriptor_contents_query(
                                    types,
                                    binding.argument.place,
                                    *graph.place_types.get(&binding.argument.place)?,
                                    &binding.argument.path,
                                    None,
                                ),
                            )
                        };
                        step.inputs.push(source.observation());
                        bindings.push(DescriptorFlowBinding::new(
                            source,
                            DescriptorLocation::whole(parameter.place),
                            binding.argument.access,
                            parameter.access,
                        )?);
                    }
                    step.successors.push(DescriptorFlowEdge {
                        target: (edge.target, 0),
                        parameters,
                        bindings,
                    });
                }
            }
            if let O::Conditional { condition, .. } = operation
                && let Some(term) = scalar_expressions.expression(*condition)
            {
                graph.payload_edge_predicates.insert(
                    position,
                    [true, false]
                        .into_iter()
                        .map(|polarity| {
                            Some(semantic_vocabulary::Proposition::Equal(
                                semantic_vocabulary::ScalarTerm::Boolean(polarity),
                                term.clone(),
                            ))
                        })
                        .collect(),
                );
            }
            if graph.flow.insert(position, step).is_some() {
                return None;
            }
        }
    }
    scalar_expressions.retain_case_conditions(types, &mut graph);
    Some(graph)
}

pub(crate) fn element_coordinates_preserved<'a>(
    function: &PsiOptimizationFunction,
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    source: &StructuralArgument,
    element: StructuralTypeId,
    region: terminal_psi::ElementViewRegion,
) -> bool {
    let Some(root) = structural_source_contract(function, source.place) else {
        return false;
    };
    let path = match source.path.split_last() {
        Some((StructuralPathSegment::FixedRange { .. }, backing)) => {
            if !types.clone().any(|declaration| {
                declaration.id == element
                    && matches!(declaration.shape,
                        StructuralTypeShape::PrimitiveScalar(semantic_vocabulary::ScalarType::Integer(integer))
                            if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned
                                && integer.bits() == 8 && !integer.is_address())
            }) {
                return false;
            }
            backing
        }
        _ => source.path.as_slice(),
    };
    let Some(collection) = runtime_structural_path_tip(types.clone(), root.structural_type, path)
    else {
        return false;
    };
    terminal_semantics::collection_view_preserves_coordinates(types, collection.id, element, region)
}

pub fn captured_scalar_ordering(
    function: &PsiOptimizationFunction,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
) -> terminal_semantics::scalar_ordering::ScalarOrdering {
    crate::current_ownership::address_joins::range_relations::RangeRelations::new(
        function, functions,
    )
    .into_ordering()
}

pub(crate) fn scalar_call_guarantees<'function>(
    operation: &O,
    callee: &impl Fn(MachineId) -> Option<&'function PsiOptimizationFunction>,
    scalar_type: &impl Fn(ValueId) -> Option<semantic_vocabulary::ScalarType>,
) -> Option<Vec<semantic_vocabulary::Proposition>> {
    let (identity, arguments, result) = match operation {
        O::Call {
            callee,
            arguments,
            result,
            scalar_type,
            ..
        } => (
            *callee,
            arguments,
            abstract_operations::AbstractResult {
                value: *result,
                scalar_type: *scalar_type,
            },
        ),
        O::CallStructural {
            callee,
            arguments,
            result,
            ..
        } => (callee.machine()?, arguments, result.scalar()?),
        _ => return None,
    };
    let callee = callee(identity)?;
    let contract = callee.verified_contract.as_ref()?;
    let signature = callee.result.scalar()?;
    if arguments.contains(&result.value)
        || signature.scalar_type != result.scalar_type
        || arguments.len() != callee.parameters.len()
        || !contract.erased_scalar_formals.is_empty()
        || !contract.erased_proof_formals.is_empty()
    {
        return None;
    }
    let mut bindings = Vec::new();
    for (formal, actual) in callee.parameters.iter().zip(arguments) {
        if scalar_type(*actual) != Some(formal.scalar_type) {
            return None;
        }
        bindings.push((
            formal.value,
            semantic_vocabulary::ScalarTerm::value(*actual, formal.scalar_type),
        ));
    }
    bindings.push((
        signature.value,
        semantic_vocabulary::ScalarTerm::value(result.value, result.scalar_type),
    ));
    Some(kernel::instantiate_payload_scalar_guarantees(
        contract, &bindings,
    ))
}

pub(crate) fn scalar_expression(
    function: &PsiOptimizationFunction,
    value: ValueId,
) -> Option<semantic_vocabulary::ScalarTerm> {
    ScalarExpressions::new(function).expression(value)
}

fn selected_effects_machine(
    catalog: &TerminalDynamicDispatchCatalog,
    functions: &[&PsiOptimizationFunction],
    function: &PsiOptimizationFunction,
    types: &[StructuralTypeDeclaration],
    selector_graph: Option<&[DescriptorMachine]>,
    operation: &impl for<'node> Fn(
        &PsiOptimizationFunction,
        &'node OptimizationNode,
    ) -> Option<Cow<'node, O>>,
    selector: ValueId,
    interface: u32,
    slot: u32,
) -> Option<MachineId> {
    #[derive(Clone, PartialEq, Eq)]
    struct SelectedTable(u32);
    let table = terminal_semantics::unchanged_scalar_identity(selector, |value| {
        let mut definitions = function.block_nodes().filter(|node| {
            node.definitions
                .iter()
                .any(|definition| definition.value == value)
        });
        if let Some(node) = definitions.next() {
            if definitions.next().is_some() {
                return None;
            }
            let projected = operation(function, node)?;
            if let O::SelectImplementation { table, .. } = projected.as_ref() {
                return Some(terminal_semantics::UnchangedScalarOrigin::Known(
                    SelectedTable(*table),
                ));
            }
            let sources = kernel::unchanged_stored_scalar_sources(
                types,
                selector_graph?,
                function.machine,
                value,
                |(block, index)| {
                    let Some(node) = function
                        .blocks
                        .iter()
                        .find(|row| row.id == block)
                        .and_then(|block| block.nodes.get(index as usize))
                    else {
                        return Vec::new();
                    };
                    let Some(graph) = selector_graph
                        .and_then(|graph| graph.iter().find(|row| row.id == function.machine))
                    else {
                        return Vec::new();
                    };
                    let Some(projected) = operation(function, node) else {
                        return Vec::new();
                    };
                    let mut writes = Vec::new();
                    let mut write = |root, path: &[StructuralPathSegment], field, actual| {
                        if let Some(location) = graph
                            .place_types
                            .get(&root)
                            .and_then(|ty| descriptor_location(types, root, *ty, path, field))
                        {
                            writes.push((location, actual));
                        }
                    };
                    match projected.as_ref() {
                        O::EstablishPrimitiveLocal { result, value, .. } => {
                            write(result.place, &[], None, value.value)
                        }
                        O::PrimitiveLocalStore {
                            destination, value, ..
                        } => write(*destination, &[], None, value.value),
                        O::WriteOnlyPrimitiveStore {
                            destination,
                            path,
                            value,
                            ..
                        } => write(destination.place(), path, None, value.value),
                        O::StructuralScalarFieldStore {
                            destination,
                            path,
                            field,
                            value,
                            ..
                        } => write(destination.place(), path, Some(*field), value.value),
                        O::EstablishRecord { result, fields, .. } => {
                            for field in fields {
                                if let terminal_psi::RecordFieldValue::Scalar { value, .. } =
                                    &field.value
                                {
                                    write(result.place, &[], Some(field.field), *value);
                                }
                            }
                        }
                        _ => {}
                    }
                    writes
                },
            )?;
            return Some(terminal_semantics::UnchangedScalarOrigin::Incoming(sources));
        }
        let mut owners = function.blocks.iter().filter(|block| {
            block
                .parameters
                .iter()
                .any(|parameter| parameter.value == value)
        });
        let owner = owners.next()?;
        if owners.next().is_some() || owner.id == function.entry {
            return None;
        }
        let incoming = function
            .block_nodes()
            .flat_map(|node| &node.successors)
            .filter(|edge| edge.target == owner.id)
            .map(|edge| edge.binding(value).map(|binding| binding.argument))
            .collect::<Option<Vec<_>>>()?;
        Some(terminal_semantics::UnchangedScalarOrigin::Incoming(
            incoming,
        ))
    })?
    .0;
    let target = kernel::selected_stored_effects_row(catalog, table, interface, slot)?;
    let signature = catalog
        .interface(interface)?
        .slots
        .get(usize::try_from(slot).ok()?)?;
    let mut bodies = functions
        .iter()
        .copied()
        .filter(|body| body.machine == target);
    let body = bodies.next()?;
    let result_matches = match &signature.result {
        terminal_psi::BoundaryMachineResult::Unit => matches!(
            body.result,
            abstract_operations::AbstractFunctionResult::Unit
        ),
        terminal_psi::BoundaryMachineResult::Scalar(expected) => {
            body.result.scalar().is_some_and(|result| {
                expected.qualifications == semantic_vocabulary::ScalarQualificationSetId::ZERO
                    && result.scalar_type == expected.scalar_type
            })
        }
        terminal_psi::BoundaryMachineResult::Structural(_) => false,
    };
    if bodies.next().is_some()
        || !result_matches
        || body.parameters.len() != signature.scalar_parameters.len()
        || !body
            .parameters
            .iter()
            .zip(&signature.scalar_parameters)
            .all(|(actual, expected)| actual.scalar_type == *expected)
        || body.structural_parameters.len() != signature.structural_parameters.len()
        || !body
            .structural_parameters
            .iter()
            .zip(&signature.structural_parameters)
            .all(|(actual, expected)| {
                actual.position == expected.position
                    && actual.structural_type == expected.structural_type
                    && actual.access == expected.access
                    && actual.multiplicity == expected.multiplicity
            })
    {
        return None;
    }
    Some(target)
}
