//! Answers prepared once from an admitted installed-interrupt cohort.
//!
//! Preparation (stage 02) checks the target program against these answers.
//! Instruction selection (stage 03) cannot depend on admission, so
//! `bind_installations` projects the same answers, with the scopes, entries,
//! installed entry and handler stacks selection reads, into the plain
//! `bound_installations::BoundInstallations` record the driver hands it.

use crate::{
    AdmittedInstalledInterruptCohort, AdmittedInstalledInterruptEntry,
    AdmittedInstalledInterruptHelper, AdmittedInstalledInterruptScope,
};
use language_core::static_interrupts::StaticInterruptOperation;
use semantic_vocabulary::OperationId;
use terminal_psi::{OperationKind, TerminalModule, TerminalPsiIdentity};

/// Rows grouped by the operation they belong to, in operation order.
pub struct Rows<T>(Vec<(OperationId, Vec<T>)>);

type HelperAnswer = (StaticInterruptOperation, Result<(), &'static str>);
impl<T> Rows<T> {
    pub fn new(rows: impl Iterator<Item = (OperationId, T)>) -> Self {
        let mut rows: Vec<_> = rows.collect();
        rows.sort_unstable_by_key(|(operation, _)| *operation);
        let mut groups: Vec<(OperationId, Vec<T>)> = Vec::new();
        for (operation, row) in rows {
            if let Some((_, values)) = groups.last_mut().filter(|(last, _)| *last == operation) {
                values.push(row);
            } else {
                groups.push((operation, vec![row]));
            }
        }
        Self(groups)
    }
    pub fn get(&self, operation: OperationId) -> &[T] {
        self.0
            .binary_search_by_key(&operation, |(id, _)| *id)
            .map_or(&[], |index| self.0[index].1.as_slice())
    }
}

struct Associations<T>(Vec<((OperationId, u32), T)>);
impl<T> Associations<T> {
    fn new(rows: impl Iterator<Item = ((OperationId, u32), T)>) -> Self {
        let mut rows: Vec<_> = rows.collect();
        rows.sort_unstable_by_key(|(key, _)| *key);
        Self(rows)
    }
    fn unique(&self, installation: OperationId, application: u32) -> Option<&T> {
        let key = (installation, application);
        let index = self.0.partition_point(|(candidate, _)| *candidate < key);
        let (actual, value) = self.0.get(index)?;
        if *actual != key || self.0.get(index + 1).is_some_and(|(next, _)| *next == key) {
            return None;
        }
        Some(value)
    }
}

pub(crate) struct PreparedHelperCohort<'a> {
    helpers: Rows<&'a AdmittedInstalledInterruptHelper>,
    scopes: Rows<&'a AdmittedInstalledInterruptScope>,
    entries: Associations<&'a AdmittedInstalledInterruptEntry>,
    requirements: Vec<(OperationId, HelperAnswer)>,
}
impl<'a> PreparedHelperCohort<'a> {
    pub(crate) fn new(cohort: &'a AdmittedInstalledInterruptCohort) -> Self {
        let mut result = Self {
            helpers: Rows::new(cohort.helpers().iter().map(|row| (row.operation(), row))),
            scopes: Rows::new(cohort.scopes().iter().map(|row| (row.operation(), row))),
            entries: Associations::new(
                cohort
                    .entries()
                    .iter()
                    .map(|row| ((row.operation, row.placement.application_index), row)),
            ),
            requirements: Vec::new(),
        };
        result.requirements = result
            .helpers
            .0
            .iter()
            .filter_map(|(operation, helpers)| {
                let first = helpers.first()?;
                Some((
                    *operation,
                    (first.kind(), result.validate(*operation, first.kind())),
                ))
            })
            .collect();
        result
    }
    fn scope(
        &self,
        helper: &AdmittedInstalledInterruptHelper,
    ) -> Result<&'a AdmittedInstalledInterruptScope, &'static str> {
        let [scope] = self.scopes.get(helper.installation()) else {
            return Err("interrupt helper does not have exactly one admitted Scope");
        };
        if scope.acknowledgement_application() != helper.callback_application() {
            return Err("interrupt helper token belongs to another acknowledgement entry");
        }
        Ok(scope)
    }
    fn entry(
        &self,
        helper: &AdmittedInstalledInterruptHelper,
    ) -> Result<&'a AdmittedInstalledInterruptEntry, &'static str> {
        self.entries
            .unique(helper.installation(), helper.callback_application())
            .copied()
            .ok_or("interrupt helper does not have exactly one admitted native entry")
    }
    pub(crate) fn require(
        &self,
        operation: OperationId,
        kind: StaticInterruptOperation,
    ) -> Result<(), &'static str> {
        require_answer(&self.requirements, operation, kind)
    }
    fn validate(
        &self,
        operation: OperationId,
        kind: StaticInterruptOperation,
    ) -> Result<(), &'static str> {
        if kind == StaticInterruptOperation::Install {
            return Err("an installation operation is not an arrival token helper");
        }
        let helpers = self.helpers.get(operation);
        if helpers.is_empty() {
            return Err("interrupt helper has no admitted installation association");
        }
        let mut roots: Vec<_> = helpers
            .iter()
            .map(|helper| (helper.installation(), helper.callback_application()))
            .collect();
        roots.sort_unstable();
        if roots.windows(2).any(|pair| pair[0] == pair[1])
            || helpers.iter().any(|helper| helper.kind() != kind)
        {
            return Err("interrupt helper association has a changed kind or duplicate root");
        }
        for helper in helpers {
            self.scope(helper)?;
            self.entry(helper)?;
        }
        Ok(())
    }
}

fn require_answer(
    answers: &[(OperationId, HelperAnswer)],
    operation: OperationId,
    kind: StaticInterruptOperation,
) -> Result<(), &'static str> {
    if kind == StaticInterruptOperation::Install {
        return Err("an installation operation is not an arrival token helper");
    }
    let index = answers
        .binary_search_by_key(&operation, |(id, _)| *id)
        .map_err(|_| "interrupt helper has no admitted installation association")?;
    let (expected, answer) = &answers[index].1;
    if kind != *expected {
        return Err("interrupt helper association has a changed kind or duplicate root");
    }
    *answer
}

pub struct PreparedInstalledInterruptHelpers {
    psi: TerminalPsiIdentity,
    cohort_psi: TerminalPsiIdentity,
    answers: Vec<(OperationId, HelperAnswer)>,
}
impl PreparedInstalledInterruptHelpers {
    pub fn new(
        psi: TerminalPsiIdentity,
        module: &TerminalModule,
        cohort: &AdmittedInstalledInterruptCohort,
    ) -> Self {
        if cohort.helpers().is_empty() {
            return Self {
                psi,
                cohort_psi: cohort.psi(),
                answers: Vec::new(),
            };
        }
        let prepared = PreparedHelperCohort::new(cohort);
        let operations = Rows::new(module.operations().map(|row| (row.id, row)));
        let registrations = Rows::new(
            module
                .installed_interrupt_registrations
                .iter()
                .map(|row| (row.operation, row)),
        );
        let callbacks = Associations::new(
            module
                .installed_interrupt_registrations
                .iter()
                .flat_map(|row| {
                    row.callbacks.iter().map(move |callback| {
                        ((row.operation, callback.application_index), callback)
                    })
                }),
        );
        let mut answers = Vec::with_capacity(prepared.helpers.0.len());
        for (operation, helpers) in &prepared.helpers.0 {
            let Some(first) = helpers.first() else {
                continue;
            };
            let kind = first.kind();
            let answer = (|| {
                prepared.require(*operation, kind)?;
                let [source] = operations.get(*operation) else {
                    return Err("interrupt helper lost its unique Terminal operation");
                };
                let OperationKind::BoundaryCall { boundary, .. } = source.kind else {
                    return Err("interrupt helper is no longer its admitted boundary call");
                };
                if module
                    .boundary_machine(boundary)
                    .and_then(|row| row.declaration.as_ref())
                    .and_then(|row| row.execution.static_interrupt_operation())
                    != Some(kind)
                {
                    return Err("interrupt helper changed its authenticated operation kind");
                }
                for helper in helpers {
                    let scope = prepared.scope(helper)?;
                    let [registration] = registrations.get(scope.operation()) else {
                        return Err(
                            "interrupt helper lost its unique retained installation request",
                        );
                    };
                    let [source] = operations.get(scope.operation()) else {
                        return Err("interrupt helper installation has no unique source operation");
                    };
                    if !matches!(&source.kind, OperationKind::BoundaryCall { boundary, arguments, structural_arguments, .. }
                        if *boundary == scope.boundary() && arguments.is_empty() && structural_arguments.as_slice() == scope.arguments())
                        || registration.installer != scope.boundary()
                    {
                        return Err(
                            "interrupt helper installation changed its boundary or retained operands",
                        );
                    }
                    let callback = callbacks
                        .unique(scope.operation(), helper.callback_application())
                        .ok_or("interrupt helper lost its designated acknowledgement entry")?;
                    let entry = prepared.entry(helper)?;
                    if !callback.entry_admitted
                        || callback.registrar != scope.operation()
                        || entry.placement.static_machine_ordinal != callback.static_machine_ordinal
                        || entry.placement.canonical_requirement_overload != callback.requirement
                        || entry.placement.contract_commitment != callback.body.contract
                        || entry.callback_function.caller() != psi
                        || entry.callback_function.callback_thunk_placement_index()
                            != Some(entry.placement_index)
                    {
                        return Err(
                            "interrupt helper changed its admitted callback identity or contract",
                        );
                    }
                }
                Ok(())
            })();
            answers.push((*operation, (kind, answer)));
        }
        Self {
            psi,
            cohort_psi: cohort.psi(),
            answers,
        }
    }
    pub fn verify(
        &self,
        operation: OperationId,
        kind: StaticInterruptOperation,
    ) -> Result<(), &'static str> {
        if self.psi != self.cohort_psi {
            return Err("interrupt helper cohort belongs to another Terminal program");
        }
        require_answer(&self.answers, operation, kind)
    }
}

/// Project admitted installation authority into the plain record instruction
/// selection reads. The driver calls this after it has resolved the installed
/// entry and measured each handler's stack; selection then checks that the
/// record names the plan's program, target and entry.
pub fn bind_installations(
    cohort: &AdmittedInstalledInterruptCohort,
    entry_execution: Option<&crate::AdmittedInstalledEntryExecution>,
    stack_requirements: &[installation_evidence::InstalledInterruptStackRequirement],
) -> bound_installations::BoundInstallations {
    use bound_installations as bound;
    let prepared = PreparedHelperCohort::new(cohort);
    let installations = cohort
        .scopes()
        .iter()
        .map(|scope| bound::BoundInterruptInstallation {
            operation: scope.operation(),
            arguments: scope.arguments().clone(),
            table_type: scope.table_type(),
            acknowledgement_application: scope.acknowledgement_application(),
            fatal_entries: scope
                .fatal_entries()
                .iter()
                .map(|fatal| bound::BoundFatalInterruptEntry {
                    application: fatal.application(),
                    gate_byte_offset: fatal.gate_byte_offset(),
                    machine: fatal.machine(),
                })
                .collect(),
            context_anchor: scope.context_anchor().map(|anchor| {
                bound::BoundInterruptContextAnchor {
                    application: anchor.application(),
                    function: anchor.function(),
                    gate_byte_offset: anchor.gate_byte_offset(),
                    table_bytes: anchor.table_bytes(),
                }
            }),
            completion: scope.completion(),
        })
        .collect();
    let entries = cohort
        .entries()
        .iter()
        .map(|entry| bound::BoundInterruptEntry {
            operation: entry.operation,
            placement_index: entry.placement_index,
            application: entry.placement.application_index,
            callback_function: entry.callback_function,
            table_placements: entry.placement.private_materialization.as_ref().and_then(
                |materialization| {
                    materialization
                        .destination_application
                        .placements()
                        .cloned()
                },
            ),
            callback: bound::BoundCallbackEntry {
                psi: entry.entry().psi(),
                target: entry.entry().target(),
                machine: entry.entry().machine(),
                inbound: entry.entry().inbound().clone(),
            },
        })
        .collect();
    let helpers = prepared
        .requirements
        .iter()
        .map(|(operation, (kind, answer))| bound::BoundInterruptHelper {
            operation: *operation,
            kind: *kind,
            refusal: answer.err(),
        })
        .collect();
    bound::BoundInstallations {
        psi: cohort.psi(),
        installations,
        entries,
        helpers,
        installed_entry: entry_execution.map(|execution| {
            let callback = execution.entry().entry();
            bound::BoundInstalledEntry {
                psi: callback.psi(),
                target: callback.target(),
                machine: callback.machine(),
            }
        }),
        stack_requirements: stack_requirements.to_vec(),
    }
}
