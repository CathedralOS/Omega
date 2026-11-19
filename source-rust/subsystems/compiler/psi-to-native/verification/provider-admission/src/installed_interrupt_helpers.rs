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
use semantic_vocabulary::{MachineId, OperationId};

type OperationSite = (MachineId, OperationId);
use terminal_psi::{OperationKind, TerminalModule, TerminalPsiIdentity};

/// Rows grouped by the operation they belong to, in operation order.
pub struct Rows<T>(Vec<(OperationSite, Vec<T>)>);

type HelperAnswer = (StaticInterruptOperation, Result<(), &'static str>);
impl<T> Rows<T> {
    pub fn new(rows: impl Iterator<Item = (OperationSite, T)>) -> Self {
        let mut rows: Vec<_> = rows.collect();
        rows.sort_unstable_by_key(|(operation, _)| *operation);
        let mut groups: Vec<(OperationSite, Vec<T>)> = Vec::new();
        for (operation, row) in rows {
            if let Some((_, values)) = groups.last_mut().filter(|(last, _)| *last == operation) {
                values.push(row);
            } else {
                groups.push((operation, vec![row]));
            }
        }
        Self(groups)
    }
    pub fn get(&self, operation: OperationSite) -> &[T] {
        self.0
            .binary_search_by_key(&operation, |(id, _)| *id)
            .map_or(&[], |index| self.0[index].1.as_slice())
    }
}

struct Associations<T>(Vec<((OperationSite, u32), T)>);
impl<T> Associations<T> {
    fn new(rows: impl Iterator<Item = ((OperationSite, u32), T)>) -> Self {
        let mut rows: Vec<_> = rows.collect();
        rows.sort_unstable_by_key(|(key, _)| *key);
        Self(rows)
    }
    fn unique(&self, installation: OperationSite, application: u32) -> Option<&T> {
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
    requirements: Vec<(OperationSite, HelperAnswer)>,
}
impl<'a> PreparedHelperCohort<'a> {
    pub(crate) fn new(cohort: &'a AdmittedInstalledInterruptCohort) -> Self {
        let mut result = Self {
            helpers: Rows::new(
                cohort
                    .helpers()
                    .iter()
                    .map(|row| ((row.machine(), row.operation()), row)),
            ),
            scopes: Rows::new(
                cohort
                    .scopes()
                    .iter()
                    .map(|row| ((row.machine(), row.operation()), row)),
            ),
            entries: Associations::new(cohort.entries().iter().map(|row| {
                (
                    (
                        (row.machine, row.operation),
                        row.placement.application_index,
                    ),
                    row,
                )
            })),
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
        let [scope] = self
            .scopes
            .get((helper.installation_machine(), helper.installation()))
        else {
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
            .unique(
                (helper.installation_machine(), helper.installation()),
                helper.callback_application(),
            )
            .copied()
            .ok_or("interrupt helper does not have exactly one admitted native entry")
    }
    pub(crate) fn require(
        &self,
        operation: OperationSite,
        kind: StaticInterruptOperation,
    ) -> Result<(), &'static str> {
        require_answer(&self.requirements, operation, kind)
    }
    fn validate(
        &self,
        operation: OperationSite,
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
            .map(|helper| {
                (
                    (helper.installation_machine(), helper.installation()),
                    helper.callback_application(),
                )
            })
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
    answers: &[(OperationSite, HelperAnswer)],
    operation: OperationSite,
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
    answers: Vec<(OperationSite, HelperAnswer)>,
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
        let operations = Rows::new(module.machines.iter().flat_map(|machine| {
            machine
                .operations()
                .map(move |row| ((machine.id, row.id), row))
        }));
        let registrations = Rows::new(
            module
                .interfaces.installed_interrupt_registrations
                .iter()
                .map(|row| ((row.machine, row.operation), row)),
        );
        let callbacks = Associations::new(
            module
                .interfaces.installed_interrupt_registrations
                .iter()
                .flat_map(|row| {
                    row.callbacks.iter().map(move |callback| {
                        (
                            ((row.machine, row.operation), callback.application_index),
                            callback,
                        )
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
                    let [registration] = registrations.get((scope.machine(), scope.operation()))
                    else {
                        return Err(
                            "interrupt helper lost its unique retained installation request",
                        );
                    };
                    let [source] = operations.get((scope.machine(), scope.operation())) else {
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
                        .unique(
                            (scope.machine(), scope.operation()),
                            helper.callback_application(),
                        )
                        .ok_or("interrupt helper lost its designated acknowledgement entry")?;
                    let entry = prepared.entry(helper)?;
                    if !callback.entry_admitted
                        || callback.registrar_machine != scope.machine()
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
        operation: OperationSite,
        kind: StaticInterruptOperation,
    ) -> Result<(), &'static str> {
        if self.psi != self.cohort_psi {
            return Err("interrupt helper cohort belongs to another Terminal program");
        }
        require_answer(&self.answers, operation, kind)
    }
}

/// The entry and scope checks for each admitted helper, in operation order.
pub fn checked_helpers(
    cohort: &AdmittedInstalledInterruptCohort,
) -> Vec<bound_installations::BoundInterruptHelper> {
    PreparedHelperCohort::new(cohort)
        .requirements
        .into_iter()
        .map(
            |(operation, (kind, answer))| bound_installations::BoundInterruptHelper {
                machine: operation.0,
                operation: operation.1,
                kind,
                refusal: answer.err(),
            },
        )
        .collect()
}
