//! Rejoin helper accesses to the installation that admitted their token route.
//!
//! Prepare operation associations once per owning pass. A shared helper retains
//! every admitted installation; its token selects the live root at runtime.
//! These checks retain admission, not allocation or stack-lease authority.

use language_core::static_interrupts::StaticInterruptOperation;
use provider_admission::{
    AdmittedInstalledInterruptCohort, AdmittedInstalledInterruptEntry,
    AdmittedInstalledInterruptHelper, AdmittedInstalledInterruptScope,
};
use semantic_vocabulary::OperationId;
use terminal_psi::{OperationKind, TerminalModule, TerminalPsiIdentity};

/// Operation identities are opaque, not array positions. Group only the rows
/// present in this pass, preserving duplicates and using compact binary lookup.
struct Rows<T>(Vec<(OperationId, Vec<T>)>);

type HelperAnswer = (StaticInterruptOperation, Result<(), &'static str>);
impl<T> Rows<T> {
    fn new(rows: impl Iterator<Item = (OperationId, T)>) -> Self {
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
    fn get(&self, operation: OperationId) -> &[T] {
        self.0
            .binary_search_by_key(&operation, |(id, _)| *id)
            .map_or(&[], |index| self.0[index].1.as_slice())
    }
}

/// The same callback application may occur at several installations. Sort the
/// complete pair once so each query checks only its own row and duplicate.
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

/// Source and Scope lookups shared by helper and installation target validation.
pub(crate) struct PreparedInstallationSources<'a> {
    psi: TerminalPsiIdentity,
    scopes: Rows<&'a AdmittedInstalledInterruptScope>,
    operations: Rows<&'a abstract_operations::AbstractOperation>,
}
impl<'a> PreparedInstallationSources<'a> {
    fn new(
        cohort: &'a AdmittedInstalledInterruptCohort,
        source: &'a abstract_operations::AbstractOperationPlan,
    ) -> Self {
        Self {
            psi: cohort.psi(),
            scopes: Rows::new(
                cohort
                    .scopes()
                    .iter()
                    .map(|scope| (scope.operation(), scope)),
            ),
            operations: Rows::new(source.operations().filter_map(|row| {
                if let abstract_operations::AbstractOperation::BoundaryCall {
                    psi_operation, ..
                } = row
                {
                    Some((*psi_operation, row))
                } else {
                    None
                }
            })),
        }
    }
    pub(crate) fn psi(&self) -> TerminalPsiIdentity {
        self.psi
    }
    pub(crate) fn scope(
        &self,
        operation: OperationId,
    ) -> Option<&'a AdmittedInstalledInterruptScope> {
        let [scope] = self.scopes.get(operation) else {
            return None;
        };
        Some(scope)
    }
    pub(crate) fn boundary_calls(
        &self,
        operation: OperationId,
    ) -> &[&'a abstract_operations::AbstractOperation] {
        self.operations.get(operation)
    }
}

/// Cohort-only associations shared by all functions in instruction selection.
/// No source authority is created by indexing these already sealed records.
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
    pub(crate) fn installation_scope(
        &self,
        operation: OperationId,
    ) -> Result<&'a AdmittedInstalledInterruptScope, &'static str> {
        let [scope] = self.scopes.get(operation) else {
            return Err("installation does not have exactly one admitted scope");
        };
        Ok(scope)
    }
    pub(crate) fn installation_entries(
        &self,
        operation: OperationId,
    ) -> impl Iterator<Item = &'a AdmittedInstalledInterruptEntry> + '_ {
        let first = self
            .entries
            .0
            .partition_point(|((owner, _), _)| *owner < operation);
        let last = self
            .entries
            .0
            .partition_point(|((owner, _), _)| *owner <= operation);
        self.entries.0[first..last].iter().map(|(_, entry)| *entry)
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

/// Source association answers prepared once at an independent artifact boundary.
/// Each answer covers all roots of its helper and retains a precise refusal.
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

pub(crate) fn validate_target(
    target: &target_operations::TargetOperationPlan,
    module: &TerminalModule,
    cohort: &AdmittedInstalledInterruptCohort,
    abstract_plan: &abstract_operations::AbstractOperationPlan,
) -> Result<(), crate::LegalizationError> {
    let fail = |operation, reason| crate::LegalizationError::InstalledInterruptAssociation {
        operation,
        reason,
    };
    if cohort.psi() != target.psi {
        return Err(fail(
            None,
            "interrupt helper cohort belongs to another Terminal program",
        ));
    }
    let helpers = PreparedInstalledInterruptHelpers::new(target.psi, module, cohort);
    let sources = PreparedInstallationSources::new(cohort, abstract_plan);
    for row in target
        .functions
        .iter()
        .flat_map(|function| function.graph.operations())
    {
        if let target_operations::TargetUnitOperation::StaticInterrupt {
            psi_operation,
            boundary,
            operation,
            token,
            ..
        } = row
        {
            helpers
                .verify(*psi_operation, *operation)
                .map_err(|reason| fail(Some(*psi_operation), reason))?;
            if !sources.boundary_calls(*psi_operation).iter().any(|source|
                matches!(source, abstract_operations::AbstractOperation::BoundaryCall { psi_operation: actual_operation, boundary: actual, arguments, structural_arguments, .. }
                    if actual_operation == psi_operation && actual == boundary && arguments.is_empty() && structural_arguments.as_slice() == std::slice::from_ref(token))) {
                return Err(fail(Some(*psi_operation), "interrupt helper changed its exact boundary or token operand"));
            }
        }
    }
    crate::interrupt_installation::validate_target(target, module, &sources)
}
