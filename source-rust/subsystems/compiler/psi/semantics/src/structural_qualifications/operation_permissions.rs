use super::*;
use crate::borrowed_storage::Projection;
use crate::descriptor_lifetimes::{
    DescriptorContentsQuery, DescriptorContentsStep, DescriptorFlowSource, DescriptorPosition,
};
use crate::descriptor_results::{
    DescriptorAnalysis, DescriptorCallOrigin, DescriptorDefinition, DescriptorMachine,
    DescriptorPayloadTarget, DescriptorSlotKind, DescriptorSourceOrigin, contents_membership_path,
    require_unchanged_produced_payload_in, terminal_payload_contract,
};
use crate::module_analyses::DescriptorRowView;
use access_vocabulary::AccessOperation;

// Reuse a judgment only for this function, instruction and value. The
// same helper can receive different storage on its separate invocations.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Subject {
    machine: MachineId,
    position: DescriptorPosition,
    source: DescriptorSourceOrigin,
}
type Caller = (
    schema_reflection::CallableDescription,
    StructuralEstablishmentRoute,
);

struct RuleJudgment<'a> {
    target: &'a Subject,
    allowed: &'a [Caller],
    original: &'a Subject,
    denial: &'static str,
    answers: std::collections::BTreeMap<(Subject, bool, bool), Option<Judged<bool>>>,
}

struct Permissions<'a> {
    module: &'a TerminalModule,
    graph: DescriptorRowView<'a>,
    /// The same rows sorted by machine, for lookup and for checking callers
    /// in a fixed order.
    by_id: Vec<&'a DescriptorMachine>,
    analysis: DescriptorAnalysis<'a>,
    // One borrowed lookup serves this pass. Source declarations and
    // blocks keep their original order in the module.
    declarations: Vec<(&'a TerminalMachine, Vec<&'a terminal_psi::Block>)>,
    // These rows borrow original calls. No operand vectors, descriptor
    // queries or transitive call paths are copied into this index.
    incoming: Vec<(
        MachineId,
        MachineId,
        DescriptorPosition,
        terminal_psi::InvocationOwner,
        Judged<terminal_psi::ModuleInvocationInputs<'a>>,
    )>,
}
fn failure(machine: MachineId, detail: &'static str) -> StructuralQualificationError {
    StructuralQualificationError {
        violation: None,
        machine: Some(machine),
        detail,
    }
}
impl<'a> Permissions<'a> {
    fn declaration(&self, identity: MachineId) -> Judged<usize> {
        self.declarations
            .binary_search_by_key(&identity, |(body, _)| body.id)
            .map_err(|_| {
                failure(
                    identity,
                    "the compiler cannot find the function performing this access",
                )
            })
    }
    fn machine(&self, identity: MachineId) -> Judged<&'a TerminalMachine> {
        Ok(self.declarations[self.declaration(identity)?].0)
    }
    fn block(&self, identity: MachineId, block: BlockId) -> Judged<&'a terminal_psi::Block> {
        let blocks = &self.declarations[self.declaration(identity)?].1;
        blocks
            .binary_search_by_key(&block, |row| row.id)
            .map(|index| blocks[index])
            .map_err(|_| failure(identity, "memory access has no original block"))
    }
    fn original_call(
        &self,
        caller: MachineId,
        position: DescriptorPosition,
        owner: terminal_psi::InvocationOwner,
    ) -> Judged<terminal_psi::ModuleInvocationInputs<'a>> {
        let block = self.block(caller, position.0)?;
        match owner {
            terminal_psi::InvocationOwner::Operation(id) => block.operations
                .get(position.1 as usize)
                .filter(|operation| operation.id == id)
                .and_then(|operation| operation.module_invocation()),
            terminal_psi::InvocationOwner::InvokeNoNormal(edge)
                if position.1 as usize == block.operations.len()
                    && matches!(block.terminator, terminal_psi::Terminator::InvokeNoNormal { edge: actual, .. } if actual == edge) =>
            {
                block.terminator.module_invocation()
            }
            _ => None,
        }.ok_or_else(|| failure(caller,
            "memory access call has no supported original invocation"))
    }
    fn descriptor(&self, identity: MachineId) -> Judged<&DescriptorMachine> {
        self.by_id
            .binary_search_by_key(&identity, |row| row.id)
            .map(|index| self.by_id[index])
            .map_err(|_| {
                failure(
                    identity,
                    "the compiler cannot trace this value through its storage history",
                )
            })
    }
    fn requirements(
        &mut self,
        machine: MachineId,
        position: DescriptorPosition,
        query: &DescriptorContentsQuery,
        requirements: &mut Vec<(StructuralDomainId, Vec<StructuralPathSegment>)>,
    ) -> Judged<()> {
        if query
            .steps
            .iter()
            .any(|step| matches!(step, DescriptorContentsStep::Element { selector: None, .. }))
        {
            return Err(failure(
                machine,
                "the compiler cannot identify which element this operation uses",
            ));
        }
        let body = self.machine(machine)?;
        let contract = terminal_payload_contract(body, query.root).ok_or_else(|| {
            failure(
                machine,
                "the compiler cannot identify the access rules on this value",
            )
        })?;
        query
            .validate(&self.module.types.structural_types, contract.carrier)
            .ok_or_else(|| {
                failure(
                    machine,
                    "this operation does not select a part of its declared value",
                )
            })?;
        // Reserving a home declares what a later store must deliver; it
        // does not install that value's rules on empty storage. Exempt
        // only a positively proved, untouched empty producer. A moved-out
        // hole or an unknown write cannot recreate that initial state.
        let descriptor = self.descriptor(machine)?;
        let producer = descriptor
            .places
            .binary_search_by_key(&query.root, |place| place.place)
            .ok()
            .and_then(|index| descriptor.places[index].producer);
        if let Some(producer) = producer {
            let empty = self
                .block(machine, producer.0)?
                .operations
                .get(producer.1 as usize)
                .is_some_and(|operation| {
                    matches!(operation.kind, OperationKind::EstablishStructuralLocal {})
                });
            if empty
                && require_unchanged_produced_payload_in(
                    &self.module.types.structural_types,
                    self.graph,
                    &mut self.analysis,
                    machine,
                    producer,
                    position,
                    &crate::descriptor_lifetimes::DescriptorLocation::whole(query.root),
                )
                .is_ok()
            {
                return Ok(());
            }
        }
        let selected = contents_membership_path(
            &self.module.types.structural_types,
            contract.carrier,
            &query.steps,
        )
        .ok_or_else(|| {
            failure(
                machine,
                "the compiler cannot match this access to its declared field",
            )
        })?;
        let mut add = |id, prefix: &[StructuralPathSegment]| -> Judged<()> {
            let row = domain(self.module, id)
                .ok_or_else(|| failure(machine, "memory access names an absent domain"))?;
            if row.access_policy.is_none() {
                return Ok(());
            }
            let relative = if prefix.len() <= selected.len()
                && StructuralPathQualification::selected_suffix(
                    prefix,
                    &selected[..prefix.len()],
                )
                .is_some()
            {
                selected[prefix.len()..].to_vec()
            } else if StructuralPathQualification::selected_suffix(prefix, &selected).is_some()
            {
                // Observing a whole value also observes its governed parts.
                Vec::new()
            } else {
                return Ok(());
            };
            if relative
                .iter()
                .any(|segment| matches!(segment, StructuralPathSegment::Referent))
            {
                return Err(failure(
                    machine,
                    "access through a reference stored inside this value has not been checked yet",
                ));
            }
            requirements.push((id, relative));
            Ok(())
        };
        for id in &contract.qualifications {
            add(*id, &[])?;
        }
        for row in &contract.projected_qualifications {
            add(row.domain, &row.path)?;
        }
        requirements.sort();
        requirements.dedup();
        Ok(())
    }
    fn query(
        &self,
        machine: MachineId,
        origin: &DescriptorSourceOrigin,
    ) -> Judged<DescriptorContentsQuery> {
        match origin {
            DescriptorSourceOrigin::IncomingSlot(location) => {
                Ok(DescriptorContentsQuery::from_location(location))
            }
            DescriptorSourceOrigin::IncomingContents { query, projection } => {
                let root = *self
                    .descriptor(machine)?
                    .place_types
                    .get(&query.root)
                    .ok_or_else(|| {
                        failure(
                            machine,
                            "the compiler cannot identify this argument's value type",
                        )
                    })?;
                query
                    .projected(&self.module.types.structural_types, root, projection)
                    .ok_or_else(|| {
                        failure(
                            machine,
                            "the compiler cannot identify which part of this argument is used",
                        )
                    })
            }
            DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument {
                source,
                projection,
            }) if projection.exact
                && !projection.indirect
                && projection.window.is_none()
                && projection.captured_at.is_none() =>
            {
                let root = *self
                    .descriptor(machine)?
                    .place_types
                    .get(source)
                    .ok_or_else(|| {
                        failure(machine, "memory access backing has no declared carrier")
                    })?;
                crate::descriptor_results::descriptor_contents_query(
                    &self.module.types.structural_types,
                    *source,
                    root,
                    &projection.path,
                    None,
                )
                .ok_or_else(|| failure(machine, "memory access backing path is unresolved"))
            }
            _ => Err(failure(
                machine,
                "memory access backing or invocation scope is unresolved",
            )),
        }
    }

    fn callers(&mut self, subject: &Subject) -> Judged<Vec<Subject>> {
        let current = self.descriptor(subject.machine)?;
        let query = self.query(subject.machine, &subject.source)?;
        if !matches!(
            current.place_definition(query.root),
            Some(DescriptorDefinition::Parameter { .. })
        ) {
            return Ok(Vec::new());
        }
        let range = self.incoming.partition_point(|row| row.0 < subject.machine)
            ..self
                .incoming
                .partition_point(|row| row.0 <= subject.machine);
        let mut result = Vec::new();
        for index in range {
            let (_, caller, position, owner, ref invocation) = self.incoming[index];
            let invocation = invocation.as_ref().map_err(Clone::clone)?;
            if invocation.required_contract.is_some() {
                return Err(failure(
                    caller,
                    "the compiler cannot check this helper's public call contract yet",
                ));
            }
            let actuals = self
                .analysis
                .substitute_call_source_at(caller, position, owner, &subject.source)
                .filter(|actuals| !actuals.is_empty())
                .ok_or_else(|| {
                    failure(
                        caller,
                        "the compiler cannot trace every argument to the storage supplied to this helper",
                    )
                })?;
            result.extend(actuals.into_iter().map(|source| Subject {
                machine: caller,
                position,
                source,
            }));
        }
        Ok(result)
    }
    fn is_entry(&self, subject: &Subject) -> Judged<bool> {
        if self.module.entry.is_none()
            || self.module.entry == Some(TargetMachineCallable::Machine(subject.machine))
        {
            return Ok(true);
        }
        let current = self.machine(subject.machine)?;
        Ok(current.declaration.as_ref().is_some_and(|declaration| {
            self.module.interfaces.callback_applications.iter().any(|application| {
                declaration.same_identity_as(
                    &self.module.identities,
                    &application.body,
                    &self.module.identities,
                )
            })
        }))
    }
    // Check one rule independently. Only two facts travel through each
    // actual subject context: this occurrence was reached, and a live
    // function on that lineage is explicitly permitted by this rule.
    // Merging principal lists would enumerate exponentially many call
    // strings; checking each conjunct this way does not retain them.
    fn route(
        &mut self,
        subject: Subject,
        mut granted: bool,
        mut encountered: bool,
        rule: &mut RuleJudgment<'_>,
    ) -> Judged<bool> {
        encountered |= subject == *rule.target;
        let current = self.machine(subject.machine)?;
        if current.selected_application.is_some() {
            return Err(failure(
                subject.machine,
                "the compiler cannot check the selected public caller's access yet",
            ));
        }
        if !granted {
            let mut unavailable = None;
            for caller in rule.allowed {
                match caller_matches_machine(self.module, subject.machine, caller) {
                    Ok(true) => {
                        granted = true;
                        break;
                    }
                    Ok(false) => {}
                    Err(error) => unavailable = Some(error),
                }
            }
            if !granted && let Some(error) = unavailable {
                return Err(error);
            }
        }
        let key = (subject.clone(), granted, encountered);
        if let Some(result) = rule.answers.get(&key) {
            return result.clone().unwrap_or_else(|| {
                Err(failure(
                    subject.machine,
                    "recursive memory access has no completed permission judgment",
                ))
            });
        }
        rule.answers.insert(key.clone(), None);
        let result = (|| {
            let callers = self.callers(&subject)?;
            let entry = self.is_entry(&subject)? || callers.is_empty();
            if entry && encountered && !granted {
                // The unauthorized instruction owns this refusal. An
                // outer caller ending the search is not that instruction.
                let at = rule.original;
                let operation = self
                    .block(at.machine, at.position.0)?
                    .operations
                    .get(at.position.1 as usize)
                    .ok_or_else(|| failure(at.machine, "memory access has no original operation"))?;
                return Err(StructuralQualificationError {
                    machine: Some(at.machine),
                    detail: rule.denial,
                    violation: Some(QualificationViolation::MemoryAccess {
                        machine: at.machine,
                        operation: operation.id,
                    }),
                });
            }
            let mut found = entry && encountered;
            for caller in callers {
                found |= self.route(caller, granted, encountered, rule)?;
            }
            Ok(found)
        })();
        rule.answers.insert(key, Some(result.clone()));
        result
    }
    fn rules(
        &mut self,
        original: &Subject,
        subject: &Subject,
        operation: AccessOperation,
        requirements: Vec<(StructuralDomainId, Vec<StructuralPathSegment>)>,
    ) -> Judged<()> {
        let module = self.module;
        let (read, write) = operation.reads_and_writes();
        for (id, selected) in requirements {
            let declaration = domain(module, id)
                .ok_or_else(|| failure(subject.machine, "memory access domain disappeared"))?;
            let Some(plan) = &declaration.access_policy else {
                continue;
            };
            let mut enforce = |path: &[StructuralPathSegment], allowed: &[Caller], denied| {
                if StructuralPathQualification::selected_suffix(path, &selected).is_none()
                    && StructuralPathQualification::selected_suffix(&selected, path).is_none()
                {
                    return Ok(());
                }
                let mut judgment = RuleJudgment {
                    target: subject,
                    allowed,
                    original,
                    denial: denied,
                    answers: std::collections::BTreeMap::new(),
                };
                if self.route(original.clone(), false, false, &mut judgment)? {
                    Ok(())
                } else {
                    Err(failure(
                        subject.machine,
                        "the compiler cannot match this access to the value supplied by its caller",
                    ))
                }
            };
            // An inaccessible field permits no observation by any caller.
            // An erased field has no storage path that an operation can touch.
            for ((_, path), demand) in &plan.observations {
                if demand.is_none() && let Some(path) = path {
                    enforce(path, &[], "this field is inaccessible")?;
                }
            }
            for (needed, rules, denied) in [
                (
                    read,
                    &plan.readers,
                    "this function is not allowed to read this value",
                ),
                (
                    write,
                    &plan.writers,
                    "this function is not allowed to write this value",
                ),
            ] {
                if needed {
                    for ((_, path), allowed) in rules {
                        let path = path.as_deref().ok_or_else(|| {
                            failure(
                                subject.machine,
                                "these access rules name a field without supported runtime storage",
                            )
                        })?;
                        enforce(path, allowed, denied)?;
                    }
                }
            }
        }
        Ok(())
    }
    fn subject(
        &mut self,
        original: Subject,
        operation: AccessOperation,
        requirements: Vec<(StructuralDomainId, Vec<StructuralPathSegment>)>,
    ) -> Judged<()> {
        // Original displayed qualifications apply to every possible
        // backing. Further owners add independent restrictions; neither
        // alternative intersection nor a bare formal can erase them.
        self.rules(&original, &original, operation, requirements)?;
        let mut pending = vec![original.clone()];
        let mut visited = std::collections::BTreeSet::new();
        while let Some(subject) = pending.pop() {
            if !visited.insert(subject.clone()) {
                continue;
            }
            let query = self.query(subject.machine, &subject.source)?;
            let mut requirements = Vec::new();
            self.requirements(subject.machine, subject.position, &query, &mut requirements)?;
            self.rules(&original, &subject, operation, requirements)?;
            pending.extend(self.callers(&subject)?);
        }
        Ok(())
    }
    fn operation(
        &mut self,
        machine: MachineId,
        position: DescriptorPosition,
        input: usize,
    ) -> Judged<()> {
        let (operation, source) = self
            .analysis
            .storage_access(machine, (position, input))
            .ok_or_else(|| failure(machine, "memory access lost its original operand"))?;
        let query = match source {
            DescriptorFlowSource::Location(location) => {
                DescriptorContentsQuery::from_location(location)
            }
            DescriptorFlowSource::Contents(query) => query.clone(),
            DescriptorFlowSource::Unavailable(_) => {
                return Err(failure(
                    machine,
                    "the compiler cannot identify the memory this operation uses",
                ));
            }
        };
        let mut requirements = Vec::new();
        self.requirements(machine, position, &query, &mut requirements)?;
        let targets = self
            .analysis
            .payload_targets(machine, position, &query)
            .filter(|targets| !targets.is_empty())
            .ok_or_else(|| failure(machine, "memory access has no complete live backing"))?;
        for target in targets {
            let source = match target {
                DescriptorPayloadTarget::Incoming(query) => {
                    DescriptorSourceOrigin::IncomingContents {
                        query,
                        projection: Projection::new(&[]),
                    }
                }
                DescriptorPayloadTarget::Storage {
                    location,
                    completed: None,
                } => {
                    let path = self
                        .descriptor(machine)?
                        .location_path(&self.module.types.structural_types, &location, None)
                        .ok_or_else(|| {
                            failure(machine, "memory access storage has no typed path")
                        })?;
                    DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument {
                        source: location.root,
                        projection: Projection::new(&path),
                    })
                }
                DescriptorPayloadTarget::Storage {
                    completed: Some(_), ..
                } => {
                    return Err(failure(
                        machine,
                        "the compiler cannot check access to this earlier stored value yet",
                    ));
                }
            };
            self.subject(
                Subject {
                    machine,
                    position,
                    source,
                },
                operation,
                requirements.clone(),
            )?;
        }
        Ok(())
    }
}
pub(super) fn validate(rows: &DescriptorRowSource<'_, '_>) -> Judged<()> {
    let module = rows.module();
    let has_rules = |id| {
        domain(module, id)
            .and_then(|row| row.access_policy.as_ref())
            .is_some_and(|plan| plan.has_operation_rules())
    };
    let mut roots = Vec::new();
    for machine in &module.machines {
        let declarations = Declared::new(machine)?;
        let governed = declarations.declared.iter().any(|(_, _, row)| {
            row.whole.iter().any(|id| has_rules(*id))
                || row.projected.iter().any(|row| has_rules(row.domain))
        }) || machine.operations().any(|operation| {
            matches!(&operation.kind, OperationKind::EstablishByteSequenceLiteral { qualifications, .. }
                if qualifications.iter().any(|id| has_rules(*id)))
        });
        if governed {
            roots.push(machine.id);
        }
    }
    if roots.is_empty() {
        return Ok(());
    }
    if matches!(module.entry, Some(TargetMachineCallable::Boundary(_))) {
        return malformed(
            None,
            "the compiler cannot check access through this public entry yet",
        );
    }
    let graph = rows
        .closure(DescriptorSlotKind::BorrowedStorage, roots.iter().copied())
        .map_err(|error| StructuralQualificationError {
            violation: None,
            machine: Some(error.machine()),
            detail: error
                .detail()
                .unwrap_or("memory access storage history cannot be reconstructed"),
        })?;
    let mut by_id = graph.iter().collect::<Vec<_>>();
    by_id.sort_by_key(|row| row.id);
    let mut declarations = module
        .machines
        .iter()
        .filter(|body| by_id.binary_search_by_key(&body.id, |row| row.id).is_ok())
        .map(|body| {
            let mut blocks = body.blocks.iter().collect::<Vec<_>>();
            blocks.sort_unstable_by_key(|block| block.id);
            if blocks.windows(2).any(|rows| rows[0].id == rows[1].id) {
                return Err(failure(
                    body.id,
                    "memory access function repeats a block identity",
                ));
            }
            Ok((body, blocks))
        })
        .collect::<Judged<Vec<_>>>()?;
    declarations.sort_unstable_by_key(|(body, _)| body.id);
    if let Some(rows) = declarations
        .windows(2)
        .find(|rows| rows[0].0.id == rows[1].0.id)
    {
        return Err(failure(
            rows[0].0.id,
            "memory access repeats a function identity",
        ));
    }
    let mut permissions = Permissions {
        module,
        graph: graph.view(),
        by_id: by_id.clone(),
        analysis: DescriptorAnalysis::with_borrowed_storage_slots(
            &module.types.structural_types,
            &graph,
        ),
        declarations,
        incoming: Vec::new(),
    };
    for caller in &by_id {
        for (position, call) in caller.known_invocations() {
            if let Some(callee) = call.callee() {
                let invocation = permissions
                    .original_call(caller.id, *position, call.owner)
                    .and_then(|invocation| {
                        if invocation.callee == callee {
                            Ok(invocation)
                        } else {
                            Err(failure(
                                caller.id,
                                "memory access call has a different source target",
                            ))
                        }
                    });
                permissions
                    .incoming
                    .push((callee, caller.id, *position, call.owner, invocation));
            }
        }
    }
    permissions
        .incoming
        .sort_by_key(|row| (row.0, row.1, row.2, row.3));
    for identity in roots {
        permissions
            .analysis
            .storage_access_frame(identity)
            .map_err(|detail| failure(identity, detail))?
            .ok_or_else(|| failure(identity, "a helper's memory accesses are unresolved"))?;
    }
    for machine in &by_id {
        let accesses = permissions
            .analysis
            .storage_access_frame(machine.id)
            .map_err(|detail| failure(machine.id, detail))?
            .ok_or_else(|| failure(machine.id, "a helper's memory accesses are unresolved"))?
            .to_vec();
        for (position, input) in accesses {
            permissions.operation(machine.id, position, input)?;
        }
    }
    Ok(())
}
