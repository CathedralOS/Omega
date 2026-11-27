//! Check that every access reference native program planning established lands on
//! backing the selected program actually allocates. The check joins stage 02's
//! abstract program and stage 04's target operations, so it runs only when an
//! explicit target-operation audit retained them.

mod incoming_calls {
    mod source_sites {
        use abstract_operations::{AbstractOperation, AbstractOperationPlan};
        use semantic_vocabulary::{
            BoundaryMachineId, ExternalCompletionKind, InvocationSite, MachineId,
        };
        use terminal_psi::StructuralArgument;
        use terminal_semantics::descriptor_lifetimes::DescriptorPosition;

        #[derive(Clone, Copy)]
        pub(super) enum SourceCallTarget {
            Machine(MachineId),
            Boundary {
                boundary: BoundaryMachineId,
                kind: ExternalCompletionKind,
            },
        }
        pub(super) struct SourceSite<'a> {
            pub position: Result<DescriptorPosition, &'static str>,
            pub target: SourceCallTarget,
            pub arguments: &'a [StructuralArgument],
            pub cleanup: Option<&'a terminal_psi::OwnedCleanupInvocation>,
        }
        struct Row<'a> {
            caller: MachineId,
            site: InvocationSite,
            source: SourceSite<'a>,
        }
        pub(super) struct SourceSites<'a> {
            rows: Vec<Row<'a>>,
        }
        impl<'a> SourceSites<'a> {
            pub(super) fn new(source: &'a AbstractOperationPlan) -> Self {
                let mut rows = Vec::new();
                for body in &source.functions {
                    for block in &body.blocks {
                        for (offset, operation) in block.operations.iter().enumerate() {
                            for (edge, actions) in operation.cleanup_rosters() {
                                for (ordinal, action) in actions.iter().enumerate() {
                                    let terminal_psi::TerminalAffineCleanupAction::InvokeOwned(
                                        cleanup,
                                    ) = action
                                    else {
                                        continue;
                                    };
                                    let Ok(action_ordinal) = u32::try_from(ordinal) else {
                                        continue;
                                    };
                                    rows.push(Row {
                                        caller: body.machine,
                                        site: InvocationSite::Cleanup { edge, action_ordinal },
                                        source: SourceSite {
                                            position: u32::try_from(offset).map(|offset| (block.block, offset)).map_err(|_| "cleanup caller position exceeds descriptor range"),
                                            target: SourceCallTarget::Machine(cleanup.cleanup_machine),
                                            arguments: std::slice::from_ref(&cleanup.subject),
                                            cleanup: Some(cleanup),
                                        },
                                    });
                                }
                            }
                            let (site, target, arguments) = match operation {
                                AbstractOperation::InvokeNoNormal {
                                    psi_edge,
                                    callee,
                                    structural_arguments,
                                    ..
                                } => (
                                    InvocationSite::InvokeNoNormal(*psi_edge),
                                    SourceCallTarget::Machine(*callee),
                                    structural_arguments.as_slice(),
                                ),
                                AbstractOperation::ExternalCompletion {
                                    psi_edge,
                                    boundary,
                                    kind,
                                    structural_arguments,
                                    ..
                                } => (
                                    InvocationSite::ExternalCompletion(*psi_edge),
                                    SourceCallTarget::Boundary {
                                        boundary: *boundary,
                                        kind: *kind,
                                    },
                                    structural_arguments.as_slice(),
                                ),
                                _ => continue,
                            };
                            let position = u32::try_from(offset)
                                .map(|offset| (block.block, offset))
                                .map_err(|_| "terminal caller position exceeds descriptor range");
                            rows.push(Row {
                                caller: body.machine,
                                site,
                                source: SourceSite {
                                    position,
                                    target,
                                    arguments,
                                    cleanup: None,
                                },
                            });
                        }
                    }
                }
                rows.sort_by_key(|row| (row.caller, row.site));
                Self { rows }
            }
            pub(super) fn get(
                &self,
                caller: MachineId,
                site: InvocationSite,
            ) -> Result<&SourceSite<'a>, &'static str> {
                let key = (caller, site);
                let start = self
                    .rows
                    .partition_point(|row| (row.caller, row.site) < key);
                let end = self
                    .rows
                    .partition_point(|row| (row.caller, row.site) <= key);
                if end != start + 1 {
                    return Err("terminal call has no unique abstract snapshot");
                }
                Ok(&self.rows[start].source)
            }
        }
        #[cfg(test)]
        mod tests {
            use super::super::super::source_operation_position;
            use super::{SourceCallTarget, SourceSites};
            use abstract_operations::{
                AbstractBlock, AbstractFunction, AbstractFunctionResult, AbstractOperation,
                AbstractOperationPlan,
            };
            use semantic_vocabulary::{
                BlockId, BoundaryMachineId, EdgeId, ExternalCompletionKind, InvocationSite,
                MachineId, OperationId, PlaceId, ValueId,
            };
            use terminal_psi::{
                SemanticFingerprint, StructuralAccess, StructuralArgument, TerminalPsiIdentity,
                VocabularyMarker,
            };

            fn constant(id: u64) -> AbstractOperation {
                AbstractOperation::BooleanConstant {
                    psi_operation: OperationId::new(id).unwrap(),
                    result: ValueId::new(id).unwrap(),
                    value: true,
                }
            }

            fn source() -> AbstractOperationPlan {
                let machine = MachineId::new(1).unwrap();
                let argument = StructuralArgument {
                    place: PlaceId::new(5).unwrap(),
                    access: StructuralAccess::SharedBorrow,
                    path: Vec::new(),
                };
                let block = |id, operations| AbstractBlock {
                    block: BlockId::new(id).unwrap(),
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    operations,
                };
                AbstractOperationPlan {
                    psi: TerminalPsiIdentity {
                        vocabulary_marker: VocabularyMarker::CURRENT,
                        program_fingerprint: SemanticFingerprint::from_bytes([1; 32]),
                    },
                    entry: machine,
                    identities: Default::default(),
                    structural_types: Vec::new().into(),
                    boundary_machines: Vec::new(),
                    provider_candidates: Vec::new(),
                    callables: Default::default(),
                    dynamic_dispatch: Default::default(),
                    functions: vec![AbstractFunction {
                        machine,
                        attachment: None,
                        entry: BlockId::new(10).unwrap(),
                        parameters: Vec::new(),
                        structural_parameters: Vec::new(),
                        result: AbstractFunctionResult::Unit,
                        entry_claims: Vec::new(),
                        block_entry_claims: Vec::new(),
                        published_service_ceiling: Vec::new(),
                        blocks: vec![
                            block(
                                10,
                                vec![
                                    constant(1),
                                    AbstractOperation::InvokeNoNormal {
                                        psi_edge: EdgeId::new(100).unwrap(),
                                        callee: MachineId::new(2).unwrap(),
                                        arguments: Vec::new(),
                                        structural_arguments: vec![argument.clone()],
                                        claim_transfers: Vec::new(),
                                        requirement_obligations: Vec::new(),
                                        crash_continuations: Vec::new(),
                                    },
                                ],
                            ),
                            block(
                                20,
                                vec![
                                    constant(2),
                                    constant(3),
                                    AbstractOperation::ExternalCompletion {
                                        psi_edge: EdgeId::new(101).unwrap(),
                                        boundary: BoundaryMachineId::new(3).unwrap(),
                                        kind: ExternalCompletionKind::ProcessExit,
                                        arguments: Vec::new(),
                                        structural_arguments: vec![argument],
                                        requirement_obligations: Vec::new(),
                                        frontier_lower_bound: Vec::new(),
                                    },
                                ],
                            ),
                        ],
                    }],
                }
            }

            #[test]
            fn source_sites_keep_block_local_positions_and_unique_invocations() {
                let mut source = source();
                let caller = source.entry;
                let invoke = InvocationSite::InvokeNoNormal(EdgeId::new(100).unwrap());
                let completion = InvocationSite::ExternalCompletion(EdgeId::new(101).unwrap());
                let sites = SourceSites::new(&source);
                let call = sites.get(caller, invoke).unwrap();
                assert_eq!(call.position, Ok((BlockId::new(10).unwrap(), 1)));
                assert!(
                    matches!(call.target, SourceCallTarget::Machine(machine) if machine == MachineId::new(2).unwrap())
                );
                let exit = sites.get(caller, completion).unwrap();
                assert_eq!(exit.position, Ok((BlockId::new(20).unwrap(), 2)));
                assert!(
                    matches!(exit.target, SourceCallTarget::Boundary { boundary, kind: ExternalCompletionKind::ProcessExit } if boundary == BoundaryMachineId::new(3).unwrap())
                );
                assert_eq!(call.arguments, exit.arguments);
                assert_eq!(call.arguments.len(), 1);
                assert!(sites.get(MachineId::new(9).unwrap(), invoke).is_err());
                assert!(
                    sites
                        .get(
                            caller,
                            InvocationSite::ExternalCompletion(EdgeId::new(100).unwrap())
                        )
                        .is_err()
                );

                let repeated = source.functions[0].blocks[0].operations[1].clone();
                source.functions[0].blocks[1].operations.push(repeated);
                let sites = SourceSites::new(&source);
                assert!(sites.get(caller, invoke).is_err());
                assert_eq!(
                    sites.get(caller, completion).unwrap().position,
                    Ok((BlockId::new(20).unwrap(), 2))
                );
                source.functions[0].blocks[1].operations.pop();
                source.functions[0].blocks[0].operations.pop();
                assert!(SourceSites::new(&source).get(caller, invoke).is_err());
            }

            #[test]
            fn cleanup_sites_keep_action_subjects_and_requirements() {
                use calling_conventions::{ValuePlacement, ValueShape};
                use semantic_vocabulary::{ObligationId, StructuralTypeId};
                use target_operations::{TargetStructuralArgument, TargetStructuralArgumentSource};
                use terminal_psi::{OwnedCleanupInvocation, TerminalAffineCleanupAction};
                let mut source = source();
                let caller = source.entry;
                let cleanup = |place, requirement| OwnedCleanupInvocation {
                    subject: StructuralArgument {
                        place: PlaceId::new(place).unwrap(),
                        access: StructuralAccess::Owned,
                        path: Vec::new(),
                    },
                    structural_type: StructuralTypeId::new(1).unwrap(),
                    cleanup_machine: MachineId::new(2).unwrap(),
                    claim_transfers: Vec::new(),
                    requirement_obligations: vec![ObligationId::new(requirement).unwrap()],
                };
                let first = cleanup(5, 7);
                let second = cleanup(6, 8);
                for (block, edge, actions) in [
                    (
                        30,
                        102,
                        vec![
                            TerminalAffineCleanupAction::RetireHome(first.subject.place),
                            TerminalAffineCleanupAction::InvokeOwned(first.clone()),
                            TerminalAffineCleanupAction::InvokeOwned(second.clone()),
                        ],
                    ),
                    (
                        40,
                        103,
                        vec![TerminalAffineCleanupAction::InvokeOwned(first.clone())],
                    ),
                ] {
                    source.functions[0].blocks.push(AbstractBlock {
                        block: BlockId::new(block).unwrap(),
                        parameters: Vec::new(),
                        structural_parameters: Vec::new(),
                        operations: vec![AbstractOperation::ReturnUnit {
                            psi_edge: EdgeId::new(edge).unwrap(),
                            cleanup_actions: actions,
                        }],
                    });
                }
                {
                    let sites = SourceSites::new(&source);
                    for (edge, action_ordinal, expected) in
                        [(102, 1, &first), (102, 2, &second), (103, 0, &first)]
                    {
                        let site = sites
                            .get(
                                caller,
                                InvocationSite::Cleanup {
                                    edge: EdgeId::new(edge).unwrap(),
                                    action_ordinal,
                                },
                            )
                            .unwrap();
                        assert_eq!(site.cleanup, Some(expected));
                        assert_eq!(site.arguments, std::slice::from_ref(&expected.subject));
                        let placement = ValuePlacement {
                            shape: ValueShape::integer(8, 8),
                            locations: Vec::new(),
                        };
                        let target = TargetStructuralArgument {
                            place: expected.subject.place,
                            access: StructuralAccess::Owned,
                            path: Vec::new(),
                            root_structural_type: expected.structural_type,
                            structural_type: expected.structural_type,
                            shape: placement.shape,
                            source_byte_offset: 0,
                            fixed_array_length: None,
                            element_stride: None,
                            indices: Vec::new(),
                            source: TargetStructuralArgumentSource::Placement(placement.clone()),
                            destination: placement,
                        };
                        assert!(super::super::cleanup_source_matches(
                            site,
                            expected.cleanup_machine,
                            std::slice::from_ref(&target),
                            &expected.requirement_obligations
                        ));
                        assert!(!super::super::cleanup_source_matches(
                            site,
                            expected.cleanup_machine,
                            std::slice::from_ref(&target),
                            &[ObligationId::new(99).unwrap()]
                        ));
                        let mut changed = target.clone();
                        changed.place = PlaceId::new(99).unwrap();
                        assert!(!super::super::cleanup_source_matches(
                            site,
                            expected.cleanup_machine,
                            &[changed],
                            &expected.requirement_obligations
                        ));
                        assert!(!super::super::cleanup_source_matches(
                            site,
                            MachineId::new(9).unwrap(),
                            &[target],
                            &expected.requirement_obligations
                        ));
                    }
                    assert!(
                        sites
                            .get(
                                caller,
                                InvocationSite::Cleanup {
                                    edge: EdgeId::new(102).unwrap(),
                                    action_ordinal: 0
                                }
                            )
                            .is_err()
                    );
                    assert!(
                        sites
                            .get(
                                caller,
                                InvocationSite::InvokeNoNormal(EdgeId::new(102).unwrap())
                            )
                            .is_err()
                    );
                }
                let repeated = source.functions[0].blocks[2].operations[0].clone();
                source.functions[0].blocks[3].operations.push(repeated);
                assert!(
                    SourceSites::new(&source)
                        .get(
                            caller,
                            InvocationSite::Cleanup {
                                edge: EdgeId::new(102).unwrap(),
                                action_ordinal: 1
                            }
                        )
                        .is_err()
                );
            }

            #[test]
            fn source_operation_positions_reject_missing_and_duplicate_identities() {
                let mut source = source();
                let function = &mut source.functions[0];
                let id = OperationId::new(3).unwrap();
                assert_eq!(
                    source_operation_position(function, id),
                    Ok((BlockId::new(20).unwrap(), 1))
                );
                assert_eq!(
                    source_operation_position(function, OperationId::new(1).unwrap()),
                    Ok((BlockId::new(10).unwrap(), 0))
                );
                assert!(source_operation_position(function, OperationId::new(9).unwrap()).is_err());
                function.blocks[0].operations.push(constant(3));
                assert!(source_operation_position(function, id).is_err());
                function.blocks[0].operations.pop();
                function.blocks[1].operations[1] = constant(4);
                assert!(source_operation_position(function, id).is_err());
                assert_eq!(
                    source_operation_position(function, OperationId::new(4).unwrap()),
                    Ok((BlockId::new(20).unwrap(), 1))
                );
            }
        }
    }

    use abstract_operations::AbstractOperationPlan;
    use semantic_vocabulary::{InvocationSite, MachineId};
    use source_sites::{SourceCallTarget, SourceSite, SourceSites};
    use target_operations::{
        CallSiteOwner, TargetControlTerminator, TargetOperationPlan, TargetUnitOperation,
    };
    use terminal_semantics::descriptor_lifetimes::{
        DescriptorContentsQuery, DescriptorLocation, DescriptorPosition, descriptor_location,
    };
    use terminal_semantics::descriptor_results::{
        DescriptorActualInputs, DescriptorAnalysis, DescriptorInvocationOwner, DescriptorMachine,
        DescriptorSourceOrigin, descriptor_contents_query, descriptor_machine,
    };

    pub(super) struct IncomingCalls {
        rows: Vec<IncomingCall>,
    }
    struct IncomingCall {
        caller: MachineId,
        callee: MachineId,
        site: CallSiteOwner,
        inputs: Result<Inputs, &'static str>,
    }
    enum Inputs {
        Invocation {
            position: DescriptorPosition,
            owner: DescriptorInvocationOwner,
        },
        Entry {
            position: DescriptorPosition,
            arguments: Vec<Option<DescriptorLocation>>,
            contents: Vec<Option<DescriptorContentsQuery>>,
        },
    }
    impl IncomingCalls {
        pub(super) fn new(
            source: &AbstractOperationPlan,
            targeted: &TargetOperationPlan,
            graph: &[DescriptorMachine],
        ) -> Self {
            let mut rows = Vec::new();
            let source_sites = SourceSites::new(source);
            for caller in &targeted.functions {
                for operation in caller.graph.operations() {
                    match operation {
                        TargetUnitOperation::Call {
                            target,
                            psi_operation,
                            ..
                        } => {
                            let Some(callee) = target.machine() else {
                                continue;
                            };
                            let inputs = invocation(
                                graph,
                                caller.machine,
                                DescriptorInvocationOwner::Operation(*psi_operation),
                                callee,
                            );
                            rows.push(IncomingCall {
                                caller: caller.machine,
                                callee,
                                site: CallSiteOwner::Operation(*psi_operation),
                                inputs,
                            });
                        }
                        TargetUnitOperation::CleanupCall {
                            owner,
                            callee,
                            arguments,
                            requirement_obligations,
                            ..
                        } => {
                            let inputs = (|| {
                                let CallSiteOwner::CleanupAction {
                                    edge,
                                    action_ordinal,
                                } = *owner
                                else {
                                    return Err("cleanup target has no edge/action owner");
                                };
                                let site = source_sites.get(
                                    caller.machine,
                                    InvocationSite::Cleanup {
                                        edge,
                                        action_ordinal,
                                    },
                                )?;
                                if !cleanup_source_matches(
                                    site,
                                    *callee,
                                    arguments,
                                    requirement_obligations,
                                ) {
                                    return Err(
                                        "cleanup target changes its original subject, callee, type or requirement evidence",
                                    );
                                }
                                let inputs = invocation(
                                    graph,
                                    caller.machine,
                                    DescriptorInvocationOwner::Cleanup {
                                        edge,
                                        action_ordinal,
                                    },
                                    *callee,
                                )?;
                                let Inputs::Invocation { position, .. } = &inputs else {
                                    return Err("cleanup lost its exact invocation observation");
                                };
                                if *position != site.position? {
                                    return Err(
                                        "cleanup source and descriptor owner positions disagree",
                                    );
                                }
                                Ok(inputs)
                            })();
                            rows.push(IncomingCall {
                                caller: caller.machine,
                                callee: *callee,
                                site: *owner,
                                inputs,
                            });
                        }
                        TargetUnitOperation::InstallStaticInterrupt {
                            psi_operation,
                            calls,
                            ..
                        } => {
                            for call in calls {
                                rows.push(IncomingCall {
                                    caller: caller.machine, callee: call.callee,
                                    site: CallSiteOwner::Installation { operation: *psi_operation, role: call.role },
                                    inputs: Err("installation helper arrival has no reconstructed backing observation"),
                                });
                            }
                        }
                        _ => {}
                    }
                }
                for block in &caller.graph.blocks {
                    match &block.terminator {
                        TargetControlTerminator::InvokeNoNormal {
                            psi_edge,
                            callee,
                            arguments,
                            ..
                        } => {
                            let inputs = source_sites.get(caller.machine, InvocationSite::InvokeNoNormal(*psi_edge)).and_then(|site| {
                                if !matches!(site.target, SourceCallTarget::Machine(source_callee) if source_callee == *callee) {
                                    return Err("no-normal source and target callee disagree");
                                }
                                if site.arguments.len() != arguments.len() || site.arguments.iter().zip(arguments).any(|(source, target)| source.place != target.place || source.path != target.path || source.access != target.access) {
                                    return Err("no-normal source and target argument transport disagree");
                                }
                                entry(source, graph, caller.machine, *callee, site)
                            });
                            rows.push(IncomingCall {
                                caller: caller.machine,
                                callee: *callee,
                                site: CallSiteOwner::InvokeNoNormal(*psi_edge),
                                inputs,
                            });
                        }
                        TargetControlTerminator::ExternalCompletion {
                            psi_edge,
                            boundary,
                            kind,
                            binding:
                                target_operations::ExternalCompletionBinding::Checked {
                                    callee,
                                    origin,
                                    ..
                                },
                            structural_arguments,
                            ..
                        } => {
                            let inputs = source_sites.get(caller.machine, InvocationSite::ExternalCompletion(*psi_edge)).and_then(|site| {
                                if !matches!(site.target, SourceCallTarget::Boundary { boundary: source_boundary, kind: source_kind } if source_boundary == *boundary && source_kind == *kind)
                                    || site.arguments != structural_arguments
                                    || !selected_origin(origin, *boundary, *callee)
                                {
                                    return Err("external completion differs from its selected source binding");
                                }
                                entry(source, graph, caller.machine, *callee, site)
                            });
                            rows.push(IncomingCall {
                                caller: caller.machine,
                                callee: *callee,
                                site: CallSiteOwner::ExternalCompletion(*psi_edge),
                                inputs,
                            });
                        }
                        _ => {}
                    }
                }
            }
            rows.sort_by_key(|row| (row.callee, row.caller, row.site));
            Self { rows }
        }
        pub(super) fn range(&self, callee: MachineId) -> std::ops::Range<usize> {
            self.rows.partition_point(|row| row.callee < callee)
                ..self.rows.partition_point(|row| row.callee <= callee)
        }
        pub(super) fn substitute(
            &self,
            index: usize,
            analysis: &mut DescriptorAnalysis<'_>,
            origin: &DescriptorSourceOrigin,
        ) -> Result<(MachineId, Vec<DescriptorSourceOrigin>), &'static str> {
            let row = self
                .rows
                .get(index)
                .ok_or("incoming backing site is absent")?;
            let result = match row.inputs.as_ref().map_err(|reason| *reason)? {
                Inputs::Invocation { position, owner } => {
                    analysis.substitute_call_source_at(row.caller, *position, *owner, origin)
                }
                Inputs::Entry {
                    position,
                    arguments,
                    contents,
                } => analysis.substitute_input_source(
                    row.caller,
                    *position,
                    row.callee,
                    DescriptorActualInputs {
                        scalar_arguments: &[],
                        arguments,
                        argument_contents: contents,
                    },
                    origin,
                ),
            }
            .ok_or("call argument has no complete backing substitution")?;
            if result.is_empty() {
                return Err("call argument has no backing alternatives");
            }
            Ok((row.caller, result))
        }
    }
    fn cleanup_source_matches(
        site: &SourceSite<'_>,
        callee: MachineId,
        arguments: &[target_operations::TargetStructuralArgument],
        requirements: &[semantic_vocabulary::ObligationId],
    ) -> bool {
        let Some(cleanup) = site.cleanup else {
            return false;
        };
        matches!(site.target, SourceCallTarget::Machine(machine) if machine == callee)
            && cleanup.cleanup_machine == callee
            && cleanup.subject.access == terminal_psi::StructuralAccess::Owned
            && cleanup.claim_transfers.is_empty()
            && cleanup.requirement_obligations == requirements
            && arguments.len() == 1
            && arguments.iter().all(|actual| {
                actual.place == cleanup.subject.place
                    && actual.path == cleanup.subject.path
                    && actual.access == cleanup.subject.access
                    && actual.structural_type == cleanup.structural_type
            })
    }

    fn invocation(
        graph: &[DescriptorMachine],
        caller: MachineId,
        owner: DescriptorInvocationOwner,
        callee: MachineId,
    ) -> Result<Inputs, &'static str> {
        let caller = descriptor_machine(graph, caller)
            .ok_or("selected caller has no reference-origin graph")?;
        let mut calls = caller
            .known_invocations()
            .filter(|(_, call)| call.owner == owner);
        let (Some((position, call)), None) = (calls.next(), calls.next()) else {
            return Err("selected call has no unique reference-origin owner observation");
        };
        if call.callee() != Some(callee) {
            return Err("selected target differs from the reference-origin call");
        }
        if matches!(owner, DescriptorInvocationOwner::Cleanup { .. })
            && (call.arguments.iter().any(Option::is_none)
                || call.argument_contents.iter().any(Option::is_none))
        {
            return Err(
                "selected cleanup has incomplete owned argument or contents correspondence",
            );
        }
        Ok(Inputs::Invocation {
            position: *position,
            owner,
        })
    }
    fn selected_origin(
        origin: &target_operations::NativeCallOrigin,
        boundary: semantic_vocabulary::BoundaryMachineId,
        callee: MachineId,
    ) -> bool {
        match origin {
            target_operations::NativeCallOrigin::Authored => false,
            target_operations::NativeCallOrigin::ProgramEntry {
                boundary: actual,
                entry,
                ..
            } => *actual == boundary && *entry == callee,
            target_operations::NativeCallOrigin::InstalledProvider {
                boundary: actual,
                provider,
                ..
            } => {
                *actual == boundary && provider.boundary == boundary && provider.candidate == callee
            }
        }
    }
    fn entry(
        source: &AbstractOperationPlan,
        graph: &[DescriptorMachine],
        caller: MachineId,
        callee: MachineId,
        site: &SourceSite<'_>,
    ) -> Result<Inputs, &'static str> {
        let declaration = source
            .functions
            .iter()
            .find(|body| body.machine == callee)
            .ok_or("terminal callee has no abstract declaration")?;
        if site.arguments.len() != declaration.structural_parameters.len() {
            return Err("terminal source and callee argument arity disagree");
        }
        let position = site.position?;
        let caller =
            descriptor_machine(graph, caller).ok_or("terminal caller has no descriptor graph")?;
        if !caller.flow.contains_key(&position) {
            return Err("terminal call lost its current descriptor snapshot");
        }
        let mut locations = Vec::with_capacity(site.arguments.len());
        let mut contents = Vec::with_capacity(site.arguments.len());
        for actual in site.arguments {
            let root = caller
                .place_types
                .get(&actual.place)
                .copied()
                .ok_or("terminal actual has no declared structural type")?;
            locations.push(descriptor_location(
                source.structural_types.as_slice(),
                actual.place,
                root,
                &actual.path,
                None,
            ));
            contents.push(descriptor_contents_query(
                source.structural_types.as_slice(),
                actual.place,
                root,
                &actual.path,
                None,
            ));
        }
        Ok(Inputs::Entry {
            position,
            arguments: locations,
            contents,
        })
    }
}

use abstract_operations::AbstractOperationPlan;
use semantic_vocabulary::{MachineId, PlaceId};
use terminal_psi::{AccessRamCorrespondence, StructuralAccess, StructuralArgument};
use terminal_semantics::descriptor_lifetimes::{
    descriptor_location, fixed_runtime_index_alternatives,
};
use terminal_semantics::descriptor_results::{
    DescriptorAnalysis, DescriptorCallOrigin, DescriptorDefinition, DescriptorMachine,
    DescriptorSourceOrigin, descriptor_machine,
};

pub(crate) struct BackingGeometry<'a> {
    abstracted: &'a AbstractOperationPlan,
    selected: &'a post_allocation_machine::PostAllocationMachineProgram,
    graph: &'a [DescriptorMachine],
    analysis: DescriptorAnalysis<'a>,
    incoming: incoming_calls::IncomingCalls,
}

// Source operations keep their block-local identity through native realization.
// Duplicate identities cannot supply an arbitrary activation or formation position.
fn source_operation_position(
    source: &abstract_operations::AbstractFunction,
    operation: semantic_vocabulary::OperationId,
) -> Result<terminal_semantics::descriptor_lifetimes::DescriptorPosition, &'static str> {
    let mut position = None;
    for block in &source.blocks {
        for (ordinal, row) in block.operations.iter().enumerate() {
            if row.psi_operation() != Some(operation) {
                continue;
            }
            let ordinal = u32::try_from(ordinal)
                .map_err(|_| "source operation position exceeds descriptor range")?;
            if position.replace((block.block, ordinal)).is_some() {
                return Err("source operation has no unique pre-operation position");
            }
        }
    }
    position.ok_or("source operation has no pre-operation position")
}

pub(crate) fn record_selected_formation_calls(
    abstracted: &AbstractOperationPlan,
    targeted: &target_operations::TargetOperationPlan,
    graph: &mut [DescriptorMachine],
) -> Option<()> {
    use terminal_semantics::descriptor_results::{
        DescriptorInvocationOwner, DescriptorMachineSignature, DescriptorSlotKind,
        machine_invocation_for_signature,
    };
    for function in &targeted.functions {
        for operation in function.graph.operations() {
            let target_operations::TargetUnitOperation::Call {
                psi_operation,
                target: target_operations::NativeCallTarget::Machine(callee),
                scalar_arguments: targeted_scalars,
                ..
            } = operation
            else {
                continue;
            };
            let caller = graph.iter_mut().find(|row| row.id == function.machine)?;
            let owner = DescriptorInvocationOwner::Operation(*psi_operation);
            if caller
                .known_invocations()
                .any(|(_, call)| call.owner == owner)
            {
                continue;
            }
            let source = abstracted
                .functions
                .iter()
                .find(|row| row.machine == function.machine)?;
            let position = source_operation_position(source, *psi_operation).ok()?;
            let mut originals = source.operations().filter_map(|operation| match operation {
                abstract_operations::AbstractOperation::BoundaryCall {
                    psi_operation: identity,
                    arguments,
                    structural_arguments,
                    ..
                } if identity == psi_operation => Some((arguments, structural_arguments)),
                _ => None,
            });
            let Some((scalars, actuals)) = originals.next() else {
                continue;
            };
            if originals.next().is_some() {
                return None;
            }
            let mut candidates = abstracted
                .functions
                .iter()
                .filter(|row| row.machine == *callee);
            let declaration = candidates.next()?;
            if candidates.next().is_some()
                || scalars.len() != declaration.parameters.len()
                || scalars.len() != targeted_scalars.len()
            {
                return None;
            }
            let mut call = machine_invocation_for_signature(
                owner,
                abstracted.structural_types.as_slice(),
                DescriptorSlotKind::References,
                DescriptorMachineSignature {
                    machine: declaration.machine,
                    structural_parameters: &declaration.structural_parameters,
                    result: declaration.result.structural(),
                },
                actuals,
                None,
                actuals
                    .iter()
                    .map(|actual| caller.place_types.get(&actual.place).copied()),
            )?;
            for ((formal, actual), targeted) in declaration
                .parameters
                .iter()
                .zip(scalars)
                .zip(targeted_scalars)
            {
                if targeted.source_value() != *actual
                    || targeted.scalar_type() != formal.scalar_type
                {
                    return None;
                }
                call.scalar_arguments.push((
                    formal.value,
                    semantic_vocabulary::ScalarTerm::value(*actual, formal.scalar_type),
                ));
            }
            call.scalar_arguments
                .sort_unstable_by_key(|(formal, _)| *formal);
            if call
                .scalar_arguments
                .windows(2)
                .any(|rows| rows[0].0 == rows[1].0)
                || call.arguments.iter().any(Option::is_none)
                || call.argument_contents.iter().any(Option::is_none)
            {
                return None;
            }
            if let Some(row) = caller.calls.get_mut(&position) {
                if row.is_some() {
                    return None;
                }
                // Native selection has resolved this exact boundary invocation
                // to the checked machine above; source-only graphs retain None.
                *row = Some(vec![call]);
            } else {
                caller.insert_call(position, call)?;
            }
        }
    }
    Some(())
}

pub(crate) fn graph(source: &AbstractOperationPlan) -> Option<Vec<DescriptorMachine>> {
    optimization_unit_semantics::descriptor_results::abstract_reference_catalog_graph(
        &source.functions,
        &source.boundary_machines,
        source.structural_types.as_slice(),
        &source.dynamic_dispatch,
        &source.callables,
    )
}

pub(crate) fn completed_graph(
    source: &AbstractOperationPlan,
    targeted: &target_operations::TargetOperationPlan,
) -> Option<Vec<DescriptorMachine>> {
    let mut graph = graph(source)?;
    record_selected_formation_calls(source, targeted, &mut graph)?;
    Some(graph)
}

impl<'a> BackingGeometry<'a> {
    pub(crate) fn new(
        abstracted: &'a AbstractOperationPlan,
        selected: &'a post_allocation_machine::PostAllocationMachineProgram,
        targeted: &'a target_operations::TargetOperationPlan,
        graph: &'a [DescriptorMachine],
    ) -> Self {
        Self {
            abstracted,
            selected,
            graph,
            incoming: incoming_calls::IncomingCalls::new(abstracted, targeted, graph),
            analysis: DescriptorAnalysis::with_reference_slots(
                abstracted.structural_types.as_slice(),
                graph,
            ),
        }
    }

    pub(crate) fn proves(
        &mut self,
        machine: MachineId,
        result: PlaceId,
        source: &StructuralArgument,
        required: &abstract_operations::AccessReferenceBacking,
    ) -> Result<(), &'static str> {
        let required = match required {
            abstract_operations::AccessReferenceBacking::OrdinaryRam(required) => required,
            abstract_operations::AccessReferenceBacking::BorrowedExtent(_) => {
                return Err("borrowed extent requires a successful View lifecycle");
            }
            abstract_operations::AccessReferenceBacking::Resident(resident) => {
                let mut plans = self
                    .abstracted
                    .structural_types
                    .access_views()
                    .iter()
                    .filter(|plan| plan.carrier == resident.carrier);
                let (Some(plan), None) = (plans.next(), plans.next()) else {
                    return Err("resident backing has no unique retained placement");
                };
                if plan.schema != resident.schema
                    || plan.placement.content_interpretation().commitment()
                        != resident.placement_commitment
                {
                    return Err("resident backing differs from its checked content interpretation");
                }
                return Ok(());
            }
        };
        let current = descriptor_machine(self.graph, machine)
            .ok_or("formation machine has no reference-origin graph")?;
        let mut positions = current
            .payload_producers
            .iter()
            .filter(|(_, place)| **place == result);
        let (Some((position, _)), None) = (positions.next(), positions.next()) else {
            return Err("formation has no unique reference-origin position");
        };
        let root = *current
            .place_types
            .get(&source.place)
            .ok_or("formation source has no retained declaration")?;
        let types = self.abstracted.structural_types.as_slice();
        let locations = fixed_runtime_index_alternatives(types, source.place, root, &source.path)
            .or_else(|| {
                descriptor_location(types, source.place, root, &source.path, None)
                    .map(|location| vec![location])
            })
            .ok_or("formation source has no exact storage locations")?;
        for location in locations {
            let alternatives = self
                .analysis
                .symbolic_source_origins(machine, *position, &location)
                .ok_or("formation source has no complete current backing origins")?;
            if alternatives.is_empty() {
                return Err("formation source has no backing alternatives");
            }
            for origin in alternatives {
                self.check_origin(machine, &origin, required, &mut Vec::new())?;
            }
        }
        Ok(())
    }

    fn check_origin(
        &mut self,
        machine: MachineId,
        origin: &DescriptorSourceOrigin,
        required: &AccessRamCorrespondence,
        pending: &mut Vec<(MachineId, DescriptorSourceOrigin)>,
    ) -> Result<(), &'static str> {
        if pending
            .iter()
            .any(|entry| entry == &(machine, origin.clone()))
        {
            return Err("backing caller equation is cyclic");
        }
        let (source, incoming_contents) = match origin {
            DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument { source, .. }) => {
                (*source, false)
            }
            DescriptorSourceOrigin::IncomingSlot(location) => (location.root, true),
            DescriptorSourceOrigin::IncomingContents { query, .. } => (query.root, true),
            _ => return Err("backing provenance has no allocation geometry guarantee"),
        };
        let current = descriptor_machine(self.graph, machine)
            .ok_or("backing machine is absent from the reference-origin graph")?;
        let definition = &current
            .places
            .iter()
            .find(|row| row.place == source)
            .ok_or("backing place is absent from the reference-origin graph")?
            .definition;
        if matches!(definition, DescriptorDefinition::Parameter { access, .. }
            if incoming_contents || *access != StructuralAccess::Owned)
        {
            if machine == self.abstracted.entry
                || self
                    .abstracted
                    .dynamic_dispatch
                    .tables
                    .iter()
                    .any(|table| table.rows.contains(&Some(machine)))
            {
                return Err("entry or dynamic input has no closed physical backing supply");
            }
            pending.push((machine, origin.clone()));
            let mut incoming = false;
            for index in self.incoming.range(machine) {
                let (caller, alternatives) =
                    self.incoming
                        .substitute(index, &mut self.analysis, origin)?;
                incoming = true;
                for alternative in alternatives {
                    self.check_origin(caller, &alternative, required, pending)?;
                }
            }
            pending.pop();
            return if incoming {
                Ok(())
            } else {
                Err("borrowed formal has no closed incoming callers")
            };
        }
        let DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument { projection, .. }) =
            origin
        else {
            return Err("incoming contents did not resolve to an actual backing");
        };
        if !projection.exact
            || projection.indirect
            || projection.window.is_some()
            || projection.captured_at.is_some()
        {
            return Err("backing projection has no exact physical address");
        }
        let root = *current
            .place_types
            .get(&source)
            .ok_or("allocation origin has no retained root declaration")?;
        let types = self.abstracted.structural_types.as_slice();
        let (_, tip) =
            terminal_semantics::canonical_static_projection(types.iter(), root, &projection.path)
                .ok_or("allocation projection has no exact schema endpoint")?;
        let region =
            terminal_semantics::reference_storage_region(types.iter(), root, &projection.path)
                .ok_or("allocation projection has no complete byte region")?;
        let mut allocations = self
            .selected
            .function(machine)
            .ok_or("allocation origin has no selected function")?
            .local_storage_slots
            .iter()
            .filter(|slot| slot.id.structural_place() == Some(source));
        let (Some(allocation), None) = (allocations.next(), allocations.next()) else {
            return Err("backing origin has no unique actual allocation");
        };
        let modulus = required.required_base.modulus();
        if tip.id == required.schema
            && region.byte_length == required.source_byte_length
            && u64::from(allocation.alignment) % modulus == 0
            && region.byte_offset % modulus == required.required_base.residue()
            && region
                .byte_offset
                .checked_add(region.byte_length)
                .is_some_and(|end| end <= u64::from(allocation.byte_size))
        {
            Ok(())
        } else {
            Err("actual backing does not meet the placed schema, extent or base congruence")
        }
    }
}
