/*
Proven structural and state-dispatch specialization plans: field-value
specialization (`field_value_specialization`), case-membership representation
specialization (`representation_specialization`), and constant state-argument
dispatch specialization (`state_specialization`). Every specialization family
excludes machines holding authenticated cyclic components and derives explicit
provenance custody for independent validation.
*/

pub(crate) mod field_value_specialization {
    pub(crate) mod propose {
        use optimization_unit::{
            FieldValueSpecializationRewrite, PsiOptimizationFunction, PsiOptimizationUnit,
        };
        use semantic_vocabulary::PlaceId;

        pub(crate) fn plan(
            unit: &PsiOptimizationUnit,
            function: &PsiOptimizationFunction,
            place: PlaceId,
        ) -> Option<FieldValueSpecializationRewrite> {
            let evidence = optimization_unit::field_evidence(function, place)?;
            Some(FieldValueSpecializationRewrite {
                machine: function.machine,
                place,
                producer: evidence.root_producer.operation(),
                reads: optimization_unit::field_value_reads(unit, &evidence, function),
            })
        }
    }
}

pub(crate) mod representation_specialization {
    use optimization_unit::{
        CaseMembershipSpecializationRewrite, FoldedCaseMembershipRow, NodeLocation,
        ProvenanceDisposition, ProvenanceRewrite, PsiOptimizationFunction, PsiOptimizationUnit,
        PsiRealizationSite,
    };
    use semantic_vocabulary::{OperationId, PlaceId, ScalarType};

    use abstract_operations::AbstractOperation as O;

    pub(crate) mod accounting {
        use super::{
            CaseMembershipSpecializationRewrite, ProvenanceDisposition, ProvenanceRewrite,
            PsiOptimizationFunction, PsiRealizationSite,
        };

        pub(crate) fn provenance_rows(
            input_function: &PsiOptimizationFunction,
            plan: &CaseMembershipSpecializationRewrite,
        ) -> Option<Vec<ProvenanceRewrite>> {
            let mut rows = Vec::new();
            for row in &plan.memberships {
                let index = usize::try_from(row.site.node).ok()?;
                let node = input_function
                    .block(row.site.block)
                    .and_then(|block| block.nodes.get(index))?;
                let site = PsiRealizationSite::Node(row.site);
                rows.push(ProvenanceRewrite {
                    input: site,
                    disposition: ProvenanceDisposition::RealizedAt(site),
                    sources: node.provenance.clone(),
                    work: node.work.clone(),
                });
            }
            rows.sort_by_key(ProvenanceRewrite::canonical_sort_key);
            Some(rows)
        }
    }
    pub(crate) mod admission {
        use super::{
            FoldedCaseMembershipRow, NodeLocation, O, OperationId, PlaceId,
            PsiOptimizationFunction, PsiOptimizationUnit, ScalarType,
        };
        use optimization_unit::{declared_structural_type, resolved_sole_case};
        use semantic_vocabulary::{StructuralCaseId, StructuralTypeId};
        use terminal_psi::StructuralPlaceDeclaration;

        pub(crate) struct MembershipEvidence<'a> {
            pub(crate) declaration: &'a StructuralPlaceDeclaration,
            pub(crate) root_type: Option<StructuralTypeId>,
            pub(crate) root_basis: Option<(StructuralCaseId, Option<OperationId>)>,
            pub(crate) function: &'a PsiOptimizationFunction,
        }

        pub(crate) fn membership_evidence<'a>(
            unit: &PsiOptimizationUnit,
            function: &'a PsiOptimizationFunction,
            place: PlaceId,
        ) -> Option<MembershipEvidence<'a>> {
            let declaration = function.structural_place(place)?;
            Some(MembershipEvidence {
                declaration,
                root_type: declared_structural_type(function, declaration),
                root_basis: optimization_unit::proof_basis(unit, function, declaration),
                function,
            })
        }

        pub(crate) fn admit_membership_node(
            unit: &PsiOptimizationUnit,
            evidence: &MembershipEvidence<'_>,
            machine: semantic_vocabulary::MachineId,
            block: semantic_vocabulary::BlockId,
            node_index: usize,
            node: &optimization_unit::OptimizationNode,
        ) -> Option<FoldedCaseMembershipRow> {
            let O::StructuralCaseMembership {
                psi_operation,
                result,
                source,
                path,
                case,
            } = &node.operation
            else {
                return None;
            };
            if *source != evidence.declaration.id || result.scalar_type != ScalarType::Boolean {
                return None;
            }
            let (proven_case, producer) = optimization_unit::established_case_at(
                unit,
                evidence.function,
                evidence.declaration,
                path,
            )
            .map(|(operation, case)| (case, Some(operation)))
            .or_else(|| {
                resolved_sole_case(unit, evidence.root_type?, path).map(|case| (case, None))
            })?;
            Some(FoldedCaseMembershipRow {
                site: NodeLocation {
                    machine,
                    block,
                    node: u32::try_from(node_index).ok()?,
                },
                psi_operation: *psi_operation,
                result: result.value,
                source: evidence.declaration.id,
                producer,
                observed_case: *case,
                proven_case,
                outcome: proven_case == *case,
            })
        }
    }
    pub(crate) mod propose {
        use super::{
            CaseMembershipSpecializationRewrite, PlaceId, PsiOptimizationFunction,
            PsiOptimizationUnit, admission,
        };

        pub(crate) fn plan(
            unit: &PsiOptimizationUnit,
            function: &PsiOptimizationFunction,
            place: PlaceId,
        ) -> Option<CaseMembershipSpecializationRewrite> {
            let evidence = admission::membership_evidence(unit, function, place)?;
            let mut memberships = Vec::new();
            for block in &function.blocks {
                for (node_index, node) in block.nodes.iter().enumerate() {
                    let Some(row) = admission::admit_membership_node(
                        unit,
                        &evidence,
                        function.machine,
                        block.id,
                        node_index,
                        node,
                    ) else {
                        continue;
                    };
                    memberships.push(row);
                }
            }
            memberships.sort_by_key(|row| (row.site.block, row.site.node));
            Some(CaseMembershipSpecializationRewrite {
                machine: function.machine,
                place,
                producer: evidence.root_basis.and_then(|(_, producer)| producer),
                memberships,
            })
        }
    }
}

pub(crate) mod state_specialization {
    use optimization_unit::{
        NodeLocation, OptimizationEdge, OptimizationNode, ProvenanceDisposition, ProvenanceRewrite,
        PsiOptimizationFunction, PsiOptimizationUnit, PsiRealizationSite, SpecializedStateEdgeRow,
        StateArgumentSpecializationRewrite,
    };
    use semantic_vocabulary::BlockId;

    use abstract_operations::AbstractOperation as O;

    use crate::{ScalarConstantAnalysis, ScalarConstantValue};

    pub(crate) mod accounting {
        use super::{
            OptimizationEdge, ProvenanceDisposition, ProvenanceRewrite, PsiOptimizationFunction,
            PsiRealizationSite, StateArgumentSpecializationRewrite,
        };
        use semantic_vocabulary::EdgeId;
        use std::collections::BTreeSet;

        pub(crate) fn provenance_rows(
            input_function: &PsiOptimizationFunction,
            plan: &StateArgumentSpecializationRewrite,
        ) -> Option<Vec<ProvenanceRewrite>> {
            let machine = plan.machine;
            let mut rows = Vec::new();
            let mut resolved_edges = BTreeSet::new();
            for row in &plan.edges {
                let incoming = find_edge(input_function, row.incoming_edge)?;
                let resolved = find_edge(input_function, row.taken_edge)?;
                let incoming_site = PsiRealizationSite::Edge {
                    machine,
                    edge: row.incoming_edge,
                };
                let resolved_site = PsiRealizationSite::Edge {
                    machine,
                    edge: row.taken_edge,
                };
                rows.push(ProvenanceRewrite {
                    input: incoming_site,
                    disposition: ProvenanceDisposition::RealizedAt(incoming_site),
                    sources: incoming.provenance.clone(),
                    work: incoming.work.clone(),
                });
                rows.push(ProvenanceRewrite {
                    input: resolved_site,
                    disposition: ProvenanceDisposition::RealizedAt(incoming_site),
                    sources: resolved.provenance.clone(),
                    work: resolved.work.clone(),
                });
                resolved_edges.insert(row.taken_edge);
            }
            for taken_edge in resolved_edges {
                let resolved = find_edge(input_function, taken_edge)?;
                let resolved_site = PsiRealizationSite::Edge {
                    machine,
                    edge: taken_edge,
                };
                rows.push(ProvenanceRewrite {
                    input: resolved_site,
                    disposition: ProvenanceDisposition::RealizedAt(resolved_site),
                    sources: resolved.provenance.clone(),
                    work: resolved.work.clone(),
                });
            }
            rows.sort_by_key(ProvenanceRewrite::canonical_sort_key);
            Some(rows)
        }

        fn find_edge(
            function: &PsiOptimizationFunction,
            edge: EdgeId,
        ) -> Option<&OptimizationEdge> {
            function
                .block_edges()
                .find(|candidate| candidate.psi_edge == edge)
        }
    }
    pub(crate) mod admission {
        use super::{
            BlockId, NodeLocation, O, OptimizationEdge, OptimizationNode, PsiOptimizationFunction,
            PsiOptimizationUnit, ScalarConstantAnalysis, ScalarConstantValue,
            SpecializedStateEdgeRow,
        };
        use optimization_unit::{DispatchCondition, integer_comparison_condition};
        use semantic_vocabulary::{EdgeId, MachineId, ValueId};

        pub(crate) struct DispatchEvidence<'a> {
            pub(crate) unit: &'a PsiOptimizationUnit,
            pub(crate) function: &'a PsiOptimizationFunction,
            pub(crate) machine: MachineId,
            pub(crate) dispatch: BlockId,
            pub(crate) parameter: ValueId,
            pub(crate) condition: DispatchCondition,
            pub(crate) when_true: &'a OptimizationEdge,
            pub(crate) when_false: &'a OptimizationEdge,
        }

        pub(crate) fn dispatch_evidence<'a>(
            unit: &'a PsiOptimizationUnit,
            function: &'a PsiOptimizationFunction,
            dispatch: BlockId,
            constants: &ScalarConstantAnalysis,
        ) -> Option<DispatchEvidence<'a>> {
            let machine = function.machine;
            let block = function.block(dispatch)?;
            if block.id == function.entry || !block.structural_parameters.is_empty() {
                return None;
            }
            let (node, prefix) = block.nodes.split_last()?;
            let O::Conditional {
                condition,
                when_true,
                when_false,
            } = &node.operation
            else {
                return None;
            };
            let (parameter, condition_kind) = if prefix.is_empty() {
                let parameter = block
                    .parameters
                    .iter()
                    .find(|parameter| parameter.value == *condition)?;
                (parameter.value, DispatchCondition::DirectParameter)
            } else {
                integer_comparison_condition(block, prefix, *condition)?
            };
            if constants.facts.iter().any(|fact| {
                fact.valid_in.machine == machine
                    && fact.valid_in.revision == unit.identity
                    && fact.value == parameter
            }) {
                return None;
            }
            let arm_edge = |edge: EdgeId| node.edge(edge);
            Some(DispatchEvidence {
                unit,
                function,
                machine,
                dispatch,
                parameter,
                condition: condition_kind,
                when_true: arm_edge(when_true.psi_edge)?,
                when_false: arm_edge(when_false.psi_edge)?,
            })
        }

        pub(crate) fn incoming_edges(
            function: &PsiOptimizationFunction,
            dispatch: BlockId,
        ) -> Vec<(BlockId, usize, &OptimizationNode, &OptimizationEdge)> {
            let mut incoming = Vec::new();
            for owner_block in &function.blocks {
                for (node_index, owner_node) in owner_block.nodes.iter().enumerate() {
                    for edge in &owner_node.successors {
                        if edge.target == dispatch {
                            incoming.push((owner_block.id, node_index, owner_node, edge));
                        }
                    }
                }
            }
            incoming
        }

        pub(crate) fn admit_incoming_edge(
            evidence: &DispatchEvidence<'_>,
            owner_block: BlockId,
            node_index: usize,
            owner_node: &OptimizationNode,
            edge: &OptimizationEdge,
            constants: &ScalarConstantAnalysis,
        ) -> Option<SpecializedStateEdgeRow> {
            let predecessor = NodeLocation {
                machine: evidence.machine,
                block: owner_block,
                node: u32::try_from(node_index).ok()?,
            };
            match &owner_node.operation {
                O::Jump {
                    psi_edge,
                    target,
                    structural_bindings,
                    cleanup_actions,
                    ..
                } => {
                    if *psi_edge != edge.psi_edge
                        || *target != evidence.dispatch
                        || !structural_bindings.is_empty()
                        || !cleanup_actions.is_empty()
                    {
                        return None;
                    }
                }
                O::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    let arm = [when_true, when_false]
                        .into_iter()
                        .find(|arm| arm.psi_edge == edge.psi_edge)?;
                    if arm.target != evidence.dispatch
                        || !arm.structural_bindings.is_empty()
                        || !arm.cleanup_actions.is_empty()
                    {
                        return None;
                    }
                }
                _ => return None,
            }
            if !edge.structural_bindings.is_empty() || !edge.cleanup_actions.is_empty() {
                return None;
            }
            let binding = edge.binding(evidence.parameter)?;
            let fact = constants.facts.iter().find(|fact| {
                fact.valid_in.machine == evidence.machine
                    && fact.valid_in.revision == evidence.unit.identity
                    && fact.value == binding.argument
            });
            let argument_constant = match fact {
                Some(fact) => fact.constant,
                None => {
                    let delivered = delivered_argument(evidence, owner_block, binding.argument);
                    call_result_constant(evidence, delivered, constants)?
                }
            };
            let constant = evidence.condition.resolve(&argument_constant)?;
            let (resolved, rejected) = if constant {
                (evidence.when_true, evidence.when_false)
            } else {
                (evidence.when_false, evidence.when_true)
            };
            if !resolved.structural_bindings.is_empty() || !resolved.cleanup_actions.is_empty() {
                return None;
            }
            let resolved_block = evidence.function.block(resolved.target)?;
            if !resolved_block.structural_parameters.is_empty() {
                return None;
            }
            Some(SpecializedStateEdgeRow {
                incoming_edge: edge.psi_edge,
                predecessor,
                parameter: evidence.parameter,
                argument: binding.argument,
                constant,
                taken_edge: resolved.psi_edge,
                rejected_edge: rejected.psi_edge,
                resolved_target: resolved.target,
            })
        }

        fn delivered_argument(
            evidence: &DispatchEvidence<'_>,
            owner_block: BlockId,
            argument: ValueId,
        ) -> ValueId {
            let mut owner_block = owner_block;
            let mut argument = argument;
            for _ in 0..evidence.function.blocks.len() {
                let Some(block) = evidence.function.block(owner_block) else {
                    break;
                };
                if !block
                    .parameters
                    .iter()
                    .any(|parameter| parameter.value == argument)
                {
                    break;
                }
                let mut incoming = evidence.function.blocks.iter().flat_map(|candidate| {
                    candidate.nodes.iter().flat_map(move |node| {
                        node.successors
                            .iter()
                            .filter(move |successor| successor.target == owner_block)
                            .map(move |successor| (candidate.id, successor))
                    })
                });
                let (Some((predecessor_block, predecessor_edge)), None) =
                    (incoming.next(), incoming.next())
                else {
                    break;
                };
                let Some(binding) = predecessor_edge.binding(argument) else {
                    break;
                };
                argument = binding.argument;
                owner_block = predecessor_block;
            }
            argument
        }

        fn call_result_constant(
            evidence: &DispatchEvidence<'_>,
            argument: ValueId,
            constants: &ScalarConstantAnalysis,
        ) -> Option<ScalarConstantValue> {
            let callee =
                evidence
                    .function
                    .block_nodes()
                    .find_map(|node| match &node.operation {
                        O::Call { result, callee, .. } if *result == argument => Some(*callee),
                        _ => None,
                    })?;
            let callee_function = evidence.unit.function(callee)?;
            let mut returned =
                callee_function
                    .block_nodes()
                    .filter_map(|node| match &node.operation {
                        O::Return { value, .. } => Some(*value),
                        _ => None,
                    });
            let value = returned.next()?;
            if returned.next().is_some() {
                return None;
            }
            let fact = constants.facts.iter().find(|fact| {
                fact.valid_in.machine == callee
                    && fact.valid_in.revision == evidence.unit.identity
                    && fact.value == value
            })?;
            Some(fact.constant)
        }
    }
    pub(crate) mod propose {
        use super::{
            BlockId, PsiOptimizationFunction, PsiOptimizationUnit, ScalarConstantAnalysis,
            StateArgumentSpecializationRewrite, admission,
        };

        pub(crate) fn plan(
            unit: &PsiOptimizationUnit,
            function: &PsiOptimizationFunction,
            dispatch: BlockId,
            constants: &ScalarConstantAnalysis,
        ) -> Option<StateArgumentSpecializationRewrite> {
            let evidence = admission::dispatch_evidence(unit, function, dispatch, constants)?;
            let incoming = admission::incoming_edges(function, dispatch);
            if incoming.is_empty() {
                return None;
            }
            let mut edges: Vec<_> = incoming
                .iter()
                .filter_map(|(owner_block, node_index, owner_node, edge)| {
                    admission::admit_incoming_edge(
                        &evidence,
                        *owner_block,
                        *node_index,
                        owner_node,
                        edge,
                        constants,
                    )
                })
                .collect();
            edges.sort_by_key(|row| row.incoming_edge);
            if edges.len() == incoming.len() {
                edges.clear();
            }
            Some(StateArgumentSpecializationRewrite {
                machine: evidence.machine,
                dispatch,
                edges,
            })
        }
    }
}
