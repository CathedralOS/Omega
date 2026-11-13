/*
Control-flow cleanup (`control_flow_cleanup`) and proof-value and ranking
coverage retention policies (`retained_identities`).
*/

pub(crate) mod control_flow_cleanup {
    pub(crate) mod cleanup {
        use semantic_vocabulary::{BlockId, OperationId, ValueId};
        use std::collections::{BTreeMap, BTreeSet};
        use terminal_psi::{Block, OperationKind, OperationResult, TerminalMachine, Terminator};

        use super::retention::BlockLocalEvidence;

        pub(crate) fn cleanup(
            machine: &mut TerminalMachine,
            evidence: &BlockLocalEvidence,
            sidecar_operations: &BTreeSet<OperationId>,
            sidecar_values: &BTreeSet<ValueId>,
        ) {
            if machine.ranked_scc.is_some() {
                return;
            }
            let mut literals = BTreeMap::new();
            for block in &machine.blocks {
                for operation in &block.operations {
                    if let OperationKind::BooleanConstant { value } = &operation.kind
                        && let Some(result) = operation.result.scalar()
                    {
                        literals.insert(result.id, *value);
                    }
                }
            }
            let mut position = 0;
            while position < machine.blocks.len() {
                let (target, folded) = {
                    let Terminator::Conditional {
                        condition,
                        when_true,
                        when_false,
                    } = &machine.blocks[position].terminator
                    else {
                        position += 1;
                        continue;
                    };
                    let Some(selected) = literals.get(condition).copied() else {
                        position += 1;
                        continue;
                    };
                    let (taken, untaken) = if selected {
                        (when_true, when_false)
                    } else {
                        (when_false, when_true)
                    };
                    if evidence.edges.contains(&untaken.edge) {
                        position += 1;
                        continue;
                    }
                    (
                        taken.target,
                        Terminator::Jump {
                            edge: taken.edge,
                            target: taken.target,
                            arguments: taken.arguments.clone(),
                            erased_arguments: taken.erased_arguments.clone(),
                            erased_proof_arguments: taken.erased_proof_arguments.clone(),
                            structural_arguments: taken.structural_arguments.clone(),
                            cleanup_actions: taken.cleanup_actions.clone(),
                        },
                    )
                };
                let stranded = stranded_blocks(machine, position, target);
                if stranded
                    .iter()
                    .all(|id| removable(machine, *id, evidence, sidecar_operations, sidecar_values))
                {
                    machine.blocks[position].terminator = folded;
                    machine.blocks.retain(|block| !stranded.contains(&block.id));
                }
                position += 1;
            }
        }

        fn stranded_blocks(
            machine: &TerminalMachine,
            position: usize,
            target: BlockId,
        ) -> BTreeSet<BlockId> {
            let positions = machine
                .blocks
                .iter()
                .enumerate()
                .map(|(index, block)| (block.id, index))
                .collect::<BTreeMap<_, _>>();
            let mut reachable = BTreeSet::from([machine.entry]);
            let mut pending = vec![machine.entry];
            while let Some(id) = pending.pop() {
                let Some(&index) = positions.get(&id) else {
                    continue;
                };
                let targets: Vec<BlockId> = if index == position {
                    vec![target]
                } else {
                    machine.blocks[index].terminator.successor_targets()
                };
                for target in targets {
                    if reachable.insert(target) {
                        pending.push(target);
                    }
                }
            }
            machine
                .blocks
                .iter()
                .filter(|block| !reachable.contains(&block.id))
                .map(|block| block.id)
                .collect()
        }

        fn removable(
            machine: &TerminalMachine,
            block_id: BlockId,
            evidence: &BlockLocalEvidence,
            sidecar_operations: &BTreeSet<OperationId>,
            sidecar_values: &BTreeSet<ValueId>,
        ) -> bool {
            let Some(block) = machine.block(&block_id) else {
                return false;
            };
            removable_block(block, evidence, sidecar_operations, sidecar_values)
        }

        fn removable_block(
            block: &Block,
            evidence: &BlockLocalEvidence,
            sidecar_operations: &BTreeSet<OperationId>,
            sidecar_values: &BTreeSet<ValueId>,
        ) -> bool {
            if evidence.blocks.contains(&block.id) || !block.structural_parameters.is_empty() {
                return false;
            }
            for operation in &block.operations {
                if operation.static_reach_binding.is_some()
                    || evidence.operations.contains(&operation.id)
                    || sidecar_operations.contains(&operation.id)
                    || matches!(operation.result, OperationResult::Structural(_))
                {
                    return false;
                }
                if let Some(result) = operation.result.scalar()
                    && (evidence.values.contains(&result.id) || sidecar_values.contains(&result.id))
                {
                    return false;
                }
            }
            if block.parameters.iter().any(|parameter| {
                evidence.values.contains(&parameter.id) || sidecar_values.contains(&parameter.id)
            }) {
                return false;
            }
            !block
                .terminator
                .edges()
                .any(|edge| evidence.edges.contains(&edge))
        }
    }
    pub(crate) mod machines {
        use semantic_vocabulary::{EdgeId, OperationId, ValueId};
        use std::collections::BTreeSet;
        use terminal_psi::{TerminalMachine, TerminalModule};

        pub(crate) fn prune(
            module: &mut TerminalModule,
            application_roots: &[semantic_vocabulary::MachineId],
            sidecar_operations: &BTreeSet<OperationId>,
            sidecar_edges: &BTreeSet<EdgeId>,
            sidecar_values: &BTreeSet<ValueId>,
        ) {
            let mut kept = super::retention::retained_machines_with_roots(
                module,
                application_roots.iter().copied(),
            );
            loop {
                if module
                    .machines
                    .iter()
                    .all(|machine| kept.contains(&machine.id))
                {
                    return;
                }
                let mut evidence = super::retention::block_local_evidence_over_machines(
                    module,
                    module
                        .machines
                        .iter()
                        .filter(|machine| kept.contains(&machine.id)),
                );
                evidence.edges.extend(sidecar_edges.iter().copied());
                let bound = module.machines.iter().filter(|machine| {
                    !kept.contains(&machine.id)
                        && machine_bound(machine, &evidence, sidecar_operations, sidecar_values)
                });
                let next = super::retention::retained_machines_with_roots(
                    module,
                    kept.iter().copied().chain(bound.map(|machine| machine.id)),
                );
                if next == kept {
                    break;
                }
                kept = next;
            }
            module.machines.retain(|machine| kept.contains(&machine.id));
            let callables = std::mem::take(&mut module.scalar_applications.proof_scope.callables);
            module
                .scalar_applications
                .proof_scope
                .callables
                .insert_many(callables.iter().map(|(_, row)| row.clone()).filter(|row| {
                    !matches!(row, terminal_psi::ProofCallableAssociation::Callable {
                callable: terminal_psi::TargetMachineCallable::Machine(body), ..
            } if !kept.contains(body))
                }));
        }

        fn machine_bound(
            machine: &TerminalMachine,
            evidence: &super::retention::BlockLocalEvidence,
            sidecar_operations: &BTreeSet<OperationId>,
            sidecar_values: &BTreeSet<ValueId>,
        ) -> bool {
            if super::retention::machine_evidence_bound(machine, evidence) {
                return true;
            }
            let sidecar_value =
                |value: &terminal_psi::ValueDeclaration| sidecar_values.contains(&value.id);
            if machine.parameters.iter().any(&sidecar_value)
                || machine.result.scalar_ref().is_some_and(&sidecar_value)
            {
                return true;
            }
            machine.blocks.iter().any(|block| {
                block.parameters.iter().any(&sidecar_value)
                    || block.operations.iter().any(|operation| {
                        sidecar_operations.contains(&operation.id)
                            || operation.result.scalar_ref().is_some_and(&sidecar_value)
                    })
            })
        }
    }
    pub(crate) mod retention {
        use crate::retained_identities::proof_values::{
            retain_contract_values, retain_proposition,
        };
        use semantic_vocabulary::{BlockId, EdgeId, MachineId, OperationId, ValueId};
        use std::collections::{BTreeMap, BTreeSet};
        use terminal_psi::OperationKind;
        use terminal_psi::TerminalModule;
        use terminal_psi::call_structural_pair_pattern;

        #[derive(Debug, Clone, Default, PartialEq, Eq)]
        pub(crate) struct BlockLocalEvidence {
            pub blocks: BTreeSet<BlockId>,
            pub edges: BTreeSet<EdgeId>,
            pub operations: BTreeSet<OperationId>,
            pub values: BTreeSet<ValueId>,
        }

        fn retain_invocation(
            site: semantic_vocabulary::InvocationSite,
            evidence: &mut BlockLocalEvidence,
        ) {
            match site {
                semantic_vocabulary::InvocationSite::Operation(operation) => {
                    evidence.operations.insert(operation);
                }
                semantic_vocabulary::InvocationSite::InvokeNoNormal(edge)
                | semantic_vocabulary::InvocationSite::ExternalCompletion(edge) => {
                    evidence.edges.insert(edge);
                }
            }
        }

        pub(crate) fn block_local_evidence(module: &TerminalModule) -> BlockLocalEvidence {
            block_local_evidence_over_machines(module, &module.machines)
        }

        pub(crate) fn block_local_evidence_over_machines<'machine>(
            module: &TerminalModule,
            machines: impl IntoIterator<Item = &'machine terminal_psi::TerminalMachine>,
        ) -> BlockLocalEvidence {
            let mut evidence = BlockLocalEvidence::default();
            let named = crate::retained_identities::record_named_rows(module);
            evidence.blocks.extend(named.blocks);
            evidence.operations.extend(named.operations);
            evidence.values.extend(named.values);
            evidence.operations.extend(
                module
                    .callback_applications
                    .iter()
                    .map(|application| application.registrar),
            );
            evidence.operations.extend(
                module
                    .installed_interrupt_registrations
                    .iter()
                    .map(|registration| registration.operation),
            );
            for coercion in &module.scalar_qualifications.coercions {
                evidence.edges.insert(coercion.edge);
                evidence.values.insert(coercion.source);
                evidence.values.insert(coercion.destination);
            }
            for returned in &module.scalar_qualifications.returns {
                evidence.edges.insert(returned.edge);
                evidence.values.extend([returned.source, returned.result]);
            }
            for range in &module.scalar_qualifications.float_entry_ranges {
                evidence.values.insert(range.parameter);
            }
            for range in &module.scalar_qualifications.integer_entry_ranges {
                evidence.values.insert(range.parameter);
            }
            for invariant in &module.scalar_block_invariants {
                evidence.blocks.insert(invariant.header);
                retain_proposition(&invariant.predicate, &mut evidence.values);
                for arrival in &invariant.arrivals {
                    evidence.edges.insert(arrival.edge);
                }
            }
            for contract in &module.operation_crash_contracts {
                evidence.operations.insert(contract.operation);
                for bucket in &contract.crash_continuations {
                    for alternative in &bucket.alternatives {
                        if let terminal_psi::CrashRouteGuard::Predicate(term) = alternative {
                            retain_proposition(term.proposition(), &mut evidence.values);
                        }
                    }
                }
            }
            for call in &module.proof_output_calls {
                if let Some(runtime) = &call.runtime_call {
                    retain_invocation(runtime.site, &mut evidence);
                }
            }
            for restoration in &module.reborrow_restored_call_uses {
                evidence.operations.insert(restoration.operation);
            }
            for machine in machines {
                for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
                    if let OperationKind::TransformContent { derivations, .. } = &operation.kind {
                        for step in derivations.iter().flat_map(|derived| &derived.ancestry) {
                            evidence.operations.insert(step.producer_operation);
                        }
                    }
                }
                if let Some(application) = &machine.closed_reach_application {
                    for call in &application.calls {
                        retain_invocation(call.site, &mut evidence);
                    }
                }
                for composition in &machine.content_partition_compositions {
                    evidence.operations.insert(composition.producer_operation);
                }
                retain_contract_values(&machine.contract, &mut evidence.values);
            }
            evidence
        }

        pub(crate) fn retained_machines_with_roots(
            module: &TerminalModule,
            additional_roots: impl IntoIterator<Item = MachineId>,
        ) -> BTreeSet<MachineId> {
            let mut retained = machine_retention_roots(module);
            retained.extend(additional_roots);
            let by_id: BTreeMap<MachineId, &terminal_psi::TerminalMachine> = module
                .machines
                .iter()
                .map(|machine| (machine.id, machine))
                .collect();
            let mut frontier: Vec<MachineId> = retained.iter().copied().collect();
            while let Some(machine) = frontier.pop() {
                let Some(body) = by_id.get(&machine) else {
                    continue;
                };
                for target in machine_transitions(body) {
                    if retained.insert(target) {
                        frontier.push(target);
                    }
                }
            }
            retained
        }

        fn machine_retention_roots(module: &TerminalModule) -> BTreeSet<MachineId> {
            let mut roots = BTreeSet::new();
            for application in module.scalar_applications.declarations() {
                if let terminal_psi::ScalarApplication::Evaluate {
                    entry: terminal_psi::TargetMachineCallable::Machine(machine),
                    ..
                } = application
                {
                    roots.insert(*machine);
                }
            }
            for observation in module.scalar_applications.conditions.observations() {
                if let terminal_psi::ScalarRangeObservation::EntryResult { machine, .. } =
                    observation
                {
                    roots.insert(*machine);
                }
            }
            for operation in &module.quotients.operations {
                roots.insert(operation.public_machine);
                roots.insert(operation.representative_machine);
            }
            roots.extend(module.quotients.adaptations.iter().map(|row| row.machine));

            if let Some(terminal_psi::TargetMachineCallable::Machine(entry)) = module.entry {
                roots.insert(entry);
            }
            roots.extend(
                module
                    .dynamic_dispatch
                    .tables
                    .iter()
                    .flat_map(|table| table.rows.iter().flatten())
                    .copied(),
            );
            roots.extend(module.target_machine_families.iter().flat_map(|family| {
                family
                    .members
                    .iter()
                    .filter_map(|member| match member.callable {
                        terminal_psi::TargetMachineCallable::Machine(machine) => Some(machine),
                        terminal_psi::TargetMachineCallable::Boundary(_) => None,
                    })
            }));
            for coercion in &module.scalar_qualifications.coercions {
                roots.insert(coercion.machine);
            }
            roots.extend(
                module
                    .scalar_qualifications
                    .returns
                    .iter()
                    .map(|row| row.machine),
            );
            for range in &module.scalar_qualifications.float_entry_ranges {
                roots.insert(range.machine);
            }
            for range in &module.scalar_qualifications.integer_entry_ranges {
                roots.insert(range.machine);
            }
            for invariant in &module.scalar_block_invariants {
                roots.insert(invariant.machine);
            }
            for contract in &module.operation_crash_contracts {
                roots.insert(contract.machine);
            }
            for site in &module.suspension_call_sites {
                if let terminal_psi::TerminalSuspensionCallTarget::Machine(machine) = site.target {
                    roots.insert(machine);
                }
            }
            for plan in &module.suspension_call_plans {
                if let terminal_psi::TerminalSuspensionCallTarget::Machine(machine) = plan.target {
                    roots.insert(machine);
                }
            }
            for call in &module.proof_output_calls {
                roots.insert(call.caller);
                if let Some(runtime) = &call.runtime_call
                    && let terminal_psi::EvidenceCallableTarget::Machine(machine) = runtime.target
                {
                    roots.insert(machine);
                }
                if let Some(dispatch) = &call.static_requirement_dispatch {
                    roots.insert(dispatch.realization);
                }
            }
            for application in &module.closed_conformance_applications {
                roots.insert(application.owner);
                for callable in &application.realization_callables {
                    if let terminal_psi::TargetMachineCallable::Machine(machine) = callable.callable
                    {
                        roots.insert(machine);
                    }
                }
            }
            for use_row in &module.reborrow_restored_call_uses {
                roots.insert(use_row.machine);
                roots.insert(use_row.call_target_machine);
            }
            for input in &module.access_view_inputs {
                roots.insert(input.machine);
            }
            for lane in &module.evidence_contract_lanes {
                if let terminal_psi::EvidenceCallableOwner::Machine(machine) = lane.owner {
                    roots.insert(machine);
                }
            }
            for candidate in &module.provider_candidates {
                roots.insert(candidate.candidate);
            }
            for projection in &module.float_meaning_projections {
                match &projection.source {
                    terminal_psi::FloatMeaningSource::DirectMachineParameter(row) => {
                        roots.insert(row.owner);
                    }
                    terminal_psi::FloatMeaningSource::DirectMachineResult(row) => {
                        roots.insert(row.owner);
                    }
                    terminal_psi::FloatMeaningSource::DirectBlockParameter(row) => {
                        roots.insert(row.owner);
                    }
                    terminal_psi::FloatMeaningSource::DirectOperationResult(row) => {
                        roots.insert(row.owner);
                    }
                    terminal_psi::FloatMeaningSource::DirectCallResult(row) => {
                        roots.insert(row.owner);
                    }
                    terminal_psi::FloatMeaningSource::DirectStructuralLeaf(row) => {
                        roots.insert(row.owner);
                    }
                    terminal_psi::FloatMeaningSource::TransitionalInput(_)
                    | terminal_psi::FloatMeaningSource::ExactBinary32Literal(_)
                    | terminal_psi::FloatMeaningSource::ExactBinary64Literal(_)
                    | terminal_psi::FloatMeaningSource::SemanticApplication(_) => {}
                }
            }
            for machine in &module.machines {
                if machine.attachment.is_some() || machine.ranked_scc.is_some() {
                    roots.insert(machine.id);
                }
            }
            roots
        }

        fn machine_transitions(machine: &terminal_psi::TerminalMachine) -> Vec<MachineId> {
            let mut targets = Vec::new();
            for block in &machine.blocks {
                targets.extend(block.module_invocations().map(|call| call.inputs.callee));
                for operation in &block.operations {
                    if let call_structural_pair_pattern!(selected_evidence, ..) = &operation.kind {
                        for evidence in selected_evidence {
                            for evidence_use in &evidence.uses {
                                targets.push(evidence_use.target);
                            }
                        }
                    }
                }
                targets.extend(
                    block
                        .terminator
                        .cleanup_rosters()
                        .flat_map(|(_, actions)| actions)
                        .filter_map(|action| match action {
                            terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                                Some(cleanup.cleanup_machine)
                            }
                            _ => None,
                        }),
                );
            }
            if let Some(application) = &machine.closed_reach_application {
                for parameter in application.parameters() {
                    if let terminal_psi::ClosedReachParameter::Machine(binding) = parameter
                        && let Some(callee) = binding.callee
                    {
                        targets.push(callee);
                    }
                }
                for call in &application.calls {
                    if let Some(application) = &call.application {
                        targets.push(application.callee);
                    }
                }
            }
            targets
        }

        pub(crate) fn machine_evidence_bound(
            machine: &terminal_psi::TerminalMachine,
            evidence: &BlockLocalEvidence,
        ) -> bool {
            if machine
                .parameters
                .iter()
                .any(|parameter| evidence.values.contains(&parameter.id))
                || machine
                    .result
                    .scalar_ref()
                    .is_some_and(|result| evidence.values.contains(&result.id))
            {
                return true;
            }
            for block in &machine.blocks {
                if evidence.blocks.contains(&block.id)
                    || block
                        .parameters
                        .iter()
                        .any(|parameter| evidence.values.contains(&parameter.id))
                    || block
                        .terminator
                        .edges()
                        .any(|edge| evidence.edges.contains(&edge))
                {
                    return true;
                }
                for operation in &block.operations {
                    if evidence.operations.contains(&operation.id)
                        || operation
                            .result
                            .scalar_ref()
                            .is_some_and(|result| evidence.values.contains(&result.id))
                    {
                        return true;
                    }
                }
            }
            false
        }
    }

    use crate::PsiOptimizationStageError;
    use lowered_psi::LoweredPsi;
    use std::collections::BTreeSet;
    use terminal_psi::{DebugSubject, OperationResult};

    pub(crate) fn cleanup(before: LoweredPsi) -> Result<LoweredPsi, PsiOptimizationStageError> {
        let mut after = before.clone();
        let mut sidecar_operations = BTreeSet::new();
        let mut sidecar_edges = BTreeSet::new();
        let mut sidecar_values = BTreeSet::new();
        for occurrence in &before.source_call_occurrences {
            match occurrence.terminal_site {
                semantic_vocabulary::InvocationSite::Operation(operation) => {
                    sidecar_operations.insert(operation);
                }
                semantic_vocabulary::InvocationSite::InvokeNoNormal(edge)
                | semantic_vocabulary::InvocationSite::ExternalCompletion(edge) => {
                    sidecar_edges.insert(edge);
                }
            }
            occurrence.frontier_before_call.visit_value_ids(|value| {
                sidecar_values.insert(value);
            });
        }
        for occurrence in &before.selected_comparison_occurrences {
            sidecar_operations.insert(occurrence.terminal_operation);
        }
        let mut evidence = retention::block_local_evidence(&before.semantic_module);
        evidence.edges.extend(sidecar_edges.iter().copied());
        for machine in &mut after.semantic_module.machines {
            cleanup::cleanup(machine, &evidence, &sidecar_operations, &sidecar_values);
        }
        machines::prune(
            &mut after.semantic_module,
            &before.retained_roots,
            &sidecar_operations,
            &sidecar_edges,
            &sidecar_values,
        );
        if let Some(debug) = after.debug_map.as_mut() {
            let surviving_machines = after
                .semantic_module
                .machines
                .iter()
                .map(|machine| machine.id)
                .collect::<BTreeSet<_>>();
            let surviving_blocks = after
                .semantic_module
                .blocks()
                .map(|block| block.id)
                .collect::<BTreeSet<_>>();
            let surviving_edges = after
                .semantic_module
                .blocks()
                .flat_map(|block| block.terminator.edges())
                .collect::<BTreeSet<_>>();
            let surviving_operations = after
                .semantic_module
                .operations()
                .map(|operation| operation.id)
                .collect::<BTreeSet<_>>();
            let surviving_values = after
                .semantic_module
                .machines
                .iter()
                .flat_map(|machine| {
                    machine
                        .parameters
                        .iter()
                        .map(|parameter| parameter.id)
                        .chain(machine.result.scalar().map(|result| result.id))
                        .chain(machine.blocks.iter().flat_map(|block| {
                            block.parameters.iter().map(|parameter| parameter.id).chain(
                                block.operations.iter().filter_map(|operation| {
                                    match operation.result {
                                        OperationResult::Scalar(result) => Some(result.id),
                                        _ => None,
                                    }
                                }),
                            )
                        }))
                })
                .collect::<BTreeSet<_>>();
            debug.sites.retain(|site| match site.subject {
                DebugSubject::Machine(machine) => surviving_machines.contains(&machine),
                DebugSubject::Block(block) => surviving_blocks.contains(&block),
                DebugSubject::Edge(edge) => surviving_edges.contains(&edge),
                DebugSubject::Operation(operation) => surviving_operations.contains(&operation),
                DebugSubject::Value(value) => surviving_values.contains(&value),
                DebugSubject::Claim { machine, .. } => surviving_machines.contains(&machine),
                _ => true,
            });
            debug.semantic = terminal_codec::terminal_psi_identity(&after.semantic_module)
                .map_err(PsiOptimizationStageError::InvalidSemantic)?;
        }
        Ok(after)
    }
}

pub(crate) mod retained_identities {
    pub(crate) mod proof_values {
        use semantic_vocabulary::ValueId;
        use std::collections::BTreeSet;
        use terminal_psi::{CrashRouteBucket, CrashRouteGuard, OperationKind as O};

        pub(crate) fn retain_proposition(
            proposition: &semantic_vocabulary::Proposition,
            retained_values: &mut BTreeSet<ValueId>,
        ) {
            proposition.visit_value_ids(|value| {
                retained_values.insert(value);
            });
        }

        pub(crate) fn retain_contract_values(
            contract: &terminal_psi::MachineContract,
            retained_values: &mut BTreeSet<ValueId>,
        ) {
            for proposition in &contract.requires {
                retain_proposition(proposition, retained_values);
            }
            for clause in &contract.ensures {
                retain_proposition(&clause.proposition, retained_values);
            }
            for guarantee in &contract.outcome_specific_ensures {
                retain_proposition(&guarantee.proposition, retained_values);
            }
            retain_crash_routes(&contract.crash_routes, retained_values);
        }

        pub(crate) fn retain_erased_arguments(
            terms: &[semantic_vocabulary::ScalarTerm],
            retained_values: &mut BTreeSet<ValueId>,
        ) {
            for term in terms {
                term.visit_value_ids(|value| {
                    retained_values.insert(value);
                    true
                });
            }
        }

        pub(crate) fn call_erased_arguments(kind: &O) -> &[semantic_vocabulary::ScalarTerm] {
            match kind {
                O::Call {
                    erased_arguments, ..
                }
                | O::CallUnit {
                    erased_arguments, ..
                }
                | O::CallStructuralScalar {
                    erased_arguments, ..
                }
                | O::CallStructuralWithScalarArguments {
                    erased_arguments, ..
                } => erased_arguments,
                _ => &[],
            }
        }

        pub(crate) fn retain_crash_routes(
            routes: &[CrashRouteBucket],
            retained_values: &mut BTreeSet<ValueId>,
        ) {
            for bucket in routes {
                for alternative in &bucket.alternatives {
                    if let CrashRouteGuard::Predicate(term) = alternative {
                        retain_proposition(term.proposition(), retained_values);
                    }
                }
            }
        }

        pub(crate) fn crash_continuations(kind: &O) -> &[CrashRouteBucket] {
            match kind {
                O::Call {
                    crash_continuations,
                    ..
                }
                | O::CallUnit {
                    crash_continuations,
                    ..
                }
                | O::CallStructuralScalar {
                    crash_continuations,
                    ..
                }
                | terminal_psi::call_structural_pair_pattern!(crash_continuations, ..) => {
                    crash_continuations
                }
                _ => &[],
            }
        }
    }
    pub(crate) mod ranked_coverage {
        use semantic_vocabulary::{BlockId, ValueId};
        use std::collections::BTreeSet;
        use terminal_psi::TerminalMachine;

        #[derive(Debug, Default)]
        pub(crate) struct RankedCoverage {
            pub blocks: BTreeSet<BlockId>,
            pub values: BTreeSet<ValueId>,
            pub member_uses: BTreeSet<ValueId>,
        }

        pub(crate) fn ranked_coverage(machine: &TerminalMachine) -> Option<RankedCoverage> {
            let mut coverage = RankedCoverage::default();
            let (blocks, values) = terminal_semantics::scalar_demand::ranked_coordinates(machine)?;
            coverage.blocks.extend(blocks);
            coverage.values.extend(values);
            for block in &machine.blocks {
                if !coverage.blocks.contains(&block.id) {
                    continue;
                }
                for operation in &block.operations {
                    let mut kind = operation.kind.clone();
                    kind.map_scalar_uses(&mut |value| {
                        coverage.member_uses.insert(value);
                        value
                    });
                }
                let mut terminator = block.terminator.clone();
                terminator.map_scalar_uses(&mut |value| {
                    coverage.member_uses.insert(value);
                    value
                });
            }
            Some(coverage)
        }
    }

    use semantic_vocabulary::{BlockId, OperationId, ValueId};
    use std::collections::BTreeSet;

    #[derive(Debug, Default)]
    pub(crate) struct RecordNamedRows {
        pub(crate) blocks: BTreeSet<BlockId>,
        pub(crate) operations: BTreeSet<OperationId>,
        pub(crate) values: BTreeSet<ValueId>,
    }

    pub(crate) fn record_named_rows(module: &terminal_psi::TerminalModule) -> RecordNamedRows {
        let rows = terminal_semantics::scalar_demand::record_named_rows(module);
        RecordNamedRows {
            blocks: rows.blocks.into_iter().collect(),
            operations: rows.operations.into_iter().collect(),
            values: rows.values.into_iter().collect(),
        }
    }

    pub(crate) fn has_boundary_application(
        demands: &[terminal_psi::TerminalBoundaryApplicationDemand],
        operation: semantic_vocabulary::OperationId,
    ) -> bool {
        demands
            .binary_search_by_key(&operation, |demand| demand.terminal_operation())
            .is_ok()
    }

    pub(crate) fn boundary_application_results(
        module: &terminal_psi::TerminalModule,
    ) -> impl Iterator<Item = semantic_vocabulary::ValueId> + '_ {
        module
            .operations()
            .filter(|operation| {
                has_boundary_application(&module.boundary_application_demands, operation.id)
            })
            .filter_map(|operation| operation.result.scalar().map(|result| result.id))
    }

    pub(crate) fn contains_source_call(
        machine: &terminal_psi::TerminalMachine,
        occurrence: &lowered_psi::LoweredSourceCallOccurrence,
    ) -> bool {
        match occurrence.terminal_site {
            semantic_vocabulary::InvocationSite::Operation(operation) => machine
                .blocks
                .iter()
                .any(|block| block.operations.iter().any(|row| row.id == operation)),
            semantic_vocabulary::InvocationSite::InvokeNoNormal(edge) => {
                machine.blocks.iter().any(|block| {
                    matches!(block.terminator,
                    terminal_psi::Terminator::InvokeNoNormal { edge: actual, .. } if actual == edge)
                })
            }
            semantic_vocabulary::InvocationSite::ExternalCompletion(edge) => {
                machine.blocks.iter().any(|block| {
                    matches!(block.terminator,
                terminal_psi::Terminator::ExternalCompletion { edge: actual, .. } if actual == edge)
                })
            }
        }
    }

    pub(crate) fn prune_debug_map(
        before: &lowered_psi::LoweredPsi,
        after: &mut lowered_psi::LoweredPsi,
    ) -> Result<(), crate::PsiOptimizationStageError> {
        use std::collections::BTreeSet;
        let Some(debug) = after.debug_map.as_mut() else {
            return Ok(());
        };
        let operations = after
            .semantic_module
            .operations()
            .map(|operation| operation.id)
            .collect::<BTreeSet<_>>();
        let removed_values = before
            .semantic_module
            .machines
            .iter()
            .zip(&after.semantic_module.machines)
            .flat_map(|(old_machine, new_machine)| {
                let removed_results = old_machine
                    .operations()
                    .filter(|operation| !operations.contains(&operation.id))
                    .filter_map(|operation| operation.result.scalar().map(|value| value.id));
                let removed_parameters =
                    old_machine.blocks.iter().zip(&new_machine.blocks).flat_map(
                        |(old_block, new_block)| {
                            let retained = new_block
                                .parameters
                                .iter()
                                .map(|parameter| parameter.id)
                                .collect::<BTreeSet<_>>();
                            old_block
                                .parameters
                                .iter()
                                .filter(move |parameter| !retained.contains(&parameter.id))
                                .map(|parameter| parameter.id)
                        },
                    );
                removed_results.chain(removed_parameters)
            })
            .collect::<BTreeSet<_>>();
        debug.sites.retain(|site| match site.subject {
            terminal_psi::DebugSubject::Operation(operation) => operations.contains(&operation),
            terminal_psi::DebugSubject::Value(value) => !removed_values.contains(&value),
            _ => true,
        });
        debug.semantic = terminal_codec::terminal_psi_identity(&after.semantic_module)
            .map_err(crate::PsiOptimizationStageError::InvalidSemantic)?;
        Ok(())
    }
}
