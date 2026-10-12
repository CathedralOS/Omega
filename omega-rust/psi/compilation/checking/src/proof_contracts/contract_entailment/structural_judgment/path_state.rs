//! Produced structural contributions and their live observation dependencies.
//!
//! Retirement rebuilds judgment indexes from surviving produced terms. It never
//! rereads a source expression or executes a call to recover a convenient fact.
//! Static operands and detached values survive opaque effects; a storage fact
//! survives only a complete disjoint frame in its original state namespace.

use super::{CaseGuarantee, StructuralJudge, StructuralTerm};
use facts::NormalizedWriteFrame;
use symbols::SymbolHandle;
use typed_trees::types::TypeReferenceHandle;

#[derive(Clone)]
pub(super) enum Contribution {
    Equation(StructuralTerm, StructuralTerm),
    CaseEquation(StructuralTerm, StructuralTerm),
    Case(CaseGuarantee),
    CaseBinding(String, StructuralTerm),
}

#[derive(Clone)]
enum SubjectDependency {
    Live {
        state: SymbolHandle,
        root: SymbolHandle,
        path: String,
    },
    Unavailable,
}

#[derive(Clone)]
pub(super) struct StructuralContribution {
    payload: Contribution,
    dependencies: Vec<SubjectDependency>,
}

#[derive(Clone, Default)]
pub(super) struct StructuralPathState {
    contributions: Vec<StructuralContribution>,
    effects: Vec<(SymbolHandle, NormalizedWriteFrame)>,
    payload_dependencies: Vec<(String, Vec<SubjectDependency>)>,
    rebuilding: bool,
}

impl StructuralJudge<'_> {
    pub(in crate::proof_contracts::contract_entailment) fn record_contribution(
        &mut self,
        payload: Contribution,
    ) {
        if self.path_state.rebuilding {
            return;
        }
        let dependencies = match &payload {
            Contribution::Equation(left, right) | Contribution::CaseEquation(left, right) => {
                self.term_dependencies([left, right])
            }
            Contribution::Case(guarantee) => self.term_dependencies([&guarantee.subject]),
            Contribution::CaseBinding(name, value) => {
                self.term_dependencies([&StructuralTerm::Variable(name.clone()), value])
            }
        };
        self.path_state.contributions.push(StructuralContribution {
            payload,
            dependencies,
        });
    }

    pub(in crate::proof_contracts::contract_entailment) fn enter_observation_frame(
        &mut self,
        state: SymbolHandle,
    ) {
        if self.declaration_state != state {
            self.declaration_state = state;
        }
    }

    pub(in crate::proof_contracts::contract_entailment) fn intake_case_binding(
        &mut self,
        name: String,
        value: StructuralTerm,
    ) {
        if self.case_facts_only {
            return;
        }
        self.record_contribution(Contribution::CaseBinding(name.clone(), value.clone()));
        std::sync::Arc::make_mut(&mut self.substitutions).insert(0, (name, value));
    }

    pub(in crate::proof_contracts::contract_entailment) fn effect_epoch(&self) -> usize {
        self.path_state.effects.len()
    }

    pub(in crate::proof_contracts::contract_entailment) fn case_guarantee_survives_effects(
        &self,
        guarantee: &CaseGuarantee,
        epoch: usize,
    ) -> bool {
        self.terms_survive_effects([&guarantee.subject], epoch)
    }

    /// The case walker creates these fresh payload binders from this exact
    /// observed subject. They inherit its dependencies, never authority from
    /// their diagnostic spelling or a later case of the same storage place.
    pub(in crate::proof_contracts::contract_entailment) fn retain_case_payload_observations(
        &mut self,
        subject: &StructuralTerm,
        binders: impl IntoIterator<Item = String>,
    ) {
        let dependencies = self.term_dependencies([subject]);
        self.path_state.payload_dependencies.extend(
            binders
                .into_iter()
                .map(|binder| (binder, dependencies.clone())),
        );
    }

    fn observation_frame(&self) -> Option<&typed_trees::state::State> {
        let machine = self.program.machine(self.machine_symbol)?;
        if self.declaration_state.is_valid() {
            self.program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == self.declaration_state)
        } else {
            self.program.machine_states(machine).first()
        }
    }

    fn binding_dependency(&self, name: &str) -> Option<SubjectDependency> {
        self.declaration_dependency(name, SymbolHandle::invalid())
    }

    fn declaration_dependency(
        &self,
        name: &str,
        requested: SymbolHandle,
    ) -> Option<SubjectDependency> {
        let Some(state) = self.observation_frame() else {
            return Some(SubjectDependency::Unavailable);
        };
        let mut declarations = Vec::new();
        for parameter in self.program.state_parameters(state) {
            if parameter.name.as_str() == name {
                declarations.push((
                    parameter.symbol,
                    parameter.type_reference,
                    parameter.is_mutable,
                ));
            }
        }
        for statement in self
            .program
            .statement_table
            .statements(state.statement_nodes)
        {
            if let Some(local) = statement.as_local_data()
                && local.name.as_str() == name
            {
                declarations.push((local.symbol, local.type_reference, local.is_mutable));
            }
        }
        let [(symbol, reference, mutable)] = declarations.as_slice() else {
            return Some(SubjectDependency::Unavailable);
        };
        // The exact state roster authenticates the runtime declaration. Entry
        // parameters can retain their machine declaration parent; that parent
        // is distinct from the arithmetic/observation state role.
        if !symbol.is_valid()
            || (requested.is_valid() && *symbol != requested)
            || self.program.symbols.get(*symbol).kind == symbols::SymbolKind::Unknown
        {
            return Some(SubjectDependency::Unavailable);
        }
        if !mutable && self.value_is_detached(*reference) {
            return None;
        }
        // Reference-bearing owned aggregates do not identify their referents
        // merely by the container root. Missing backing custody is opaque.
        if crate::reference_shell(self.program, *reference).is_none()
            && !self.value_is_detached(*reference)
        {
            return Some(SubjectDependency::Unavailable);
        }
        let Some(path) =
            crate::machine_calls::calls::frame_coordinate_path(self.program, *symbol, &[])
        else {
            return Some(SubjectDependency::Unavailable);
        };
        Some(SubjectDependency::Live {
            state: state.symbol,
            root: *symbol,
            path,
        })
    }

    fn value_is_detached(&self, reference: TypeReferenceHandle) -> bool {
        self.program.primitive_type_reference(reference).is_some()
            || self
                .call_frames()
                .is_some_and(|frames| frames.proof_value_is_caller_isolated(reference))
    }

    fn term_dependencies<'term>(
        &self,
        terms: impl IntoIterator<Item = &'term StructuralTerm>,
    ) -> Vec<SubjectDependency> {
        let mut pending = terms.into_iter().collect::<Vec<_>>();
        let mut dependencies = Vec::new();
        while let Some(term) = pending.pop() {
            match term {
                StructuralTerm::Variable(name) => {
                    if let Some((_, payload)) = self
                        .path_state
                        .payload_dependencies
                        .iter()
                        .find(|(binder, _)| binder == name)
                    {
                        dependencies.extend(payload.iter().cloned());
                    } else if let Some(dependency) = self.binding_dependency(name) {
                        dependencies.push(dependency);
                    }
                }
                StructuralTerm::BoundValue(symbol) => {
                    let static_value = symbol.is_valid() && (
                        self.program.symbols.get(*symbol).kind == symbols::SymbolKind::Const
                        || self.program.machine(self.machine_symbol).is_some_and(|machine| {
                            let (parameters, _) = self.program.machine_type_reference_scope(machine, |left, right| left.symbol == right.symbol);
                            parameters.iter().any(|parameter| parameter.symbol == *symbol && matches!(parameter.kind,
                                typed_trees::data::TypeParameterKind::Const { .. } | typed_trees::data::TypeParameterKind::Value { .. }))
                        })
                    );
                    if !static_value {
                        dependencies.push(SubjectDependency::Unavailable);
                    }
                }
                StructuralTerm::BoundProjection { subject, .. } => {
                    if subject.is_valid() {
                        if let Some(dependency) = self
                            .declaration_dependency(self.program.symbols.name(*subject), *subject)
                        {
                            dependencies.push(dependency);
                        }
                    } else {
                        dependencies.push(SubjectDependency::Unavailable);
                    }
                }
                StructuralTerm::CapturedValue { result_type, .. } => {
                    if !self.value_is_detached(*result_type) {
                        dependencies.push(SubjectDependency::Unavailable);
                    }
                }
                StructuralTerm::ObservedValue { value, .. }
                | StructuralTerm::Projection { subject: value, .. } => pending.push(value),
                StructuralTerm::ScalarBinary { left, right, .. }
                | StructuralTerm::IndexedValue {
                    collection: left,
                    index: right,
                    ..
                } => {
                    pending.push(left);
                    pending.push(right);
                }
                StructuralTerm::Constructor { fields, .. } => {
                    pending.extend(fields.iter().map(|(_, value)| value))
                }
                StructuralTerm::Application { arguments, .. }
                | StructuralTerm::CallProjection { arguments, .. } => pending.extend(arguments),
                StructuralTerm::Integer(_) | StructuralTerm::Float(_) => {}
                StructuralTerm::Opaque(_) => dependencies.push(SubjectDependency::Unavailable),
            }
        }
        dependencies
    }

    pub(in crate::proof_contracts::contract_entailment) fn terms_survive_effects<'term>(
        &self,
        terms: impl IntoIterator<Item = &'term StructuralTerm>,
        epoch: usize,
    ) -> bool {
        if epoch > self.effect_epoch() {
            return false;
        }
        let dependencies = self.term_dependencies(terms);
        self.path_state
            .effects
            .iter()
            .skip(epoch)
            .all(|(state, frame)| dependencies_survive(&dependencies, *state, frame))
    }

    pub(in crate::proof_contracts::contract_entailment) fn binding_survives_effects(
        &self,
        name: &str,
        value: &StructuralTerm,
        epoch: usize,
    ) -> bool {
        if epoch > self.effect_epoch() {
            return false;
        }
        let mut dependencies = self.term_dependencies([value]);
        if let Some(dependency) = self.binding_dependency(name) {
            dependencies.push(dependency);
        }
        self.path_state
            .effects
            .iter()
            .skip(epoch)
            .all(|(state, frame)| dependencies_survive(&dependencies, *state, frame))
    }

    /// A scalar read is a detached observation, not a promise that its source
    /// storage remains unchanged. Resolve its current established value before
    /// retirement; a residual storage equation keeps its live dependencies.
    pub(in crate::proof_contracts::contract_entailment) fn capture_scalar_observation(
        &mut self,
        expression: typed_trees::expression::ExpressionHandle,
        reference: TypeReferenceHandle,
        value: StructuralTerm,
    ) -> StructuralTerm {
        if self.program.primitive_type_reference(reference).is_none()
            || self.term_dependencies([&value]).is_empty()
        {
            return value;
        }
        self.capture_sequence += 1;
        let captured = StructuralTerm::CapturedValue {
            expression,
            callable: SymbolHandle::invalid(),
            occurrence: self.capture_sequence,
            result_type: reference,
        };
        let observed = self.resolve(value);
        std::sync::Arc::make_mut(&mut self.captured_result_equations)
            .push((captured.clone(), observed.clone()));
        self.intake_equation(captured.clone(), observed, 0);
        captured
    }

    pub(in crate::proof_contracts::contract_entailment) fn retire_call_effects(
        &mut self,
        state: &typed_trees::state::State,
        frame: NormalizedWriteFrame,
    ) {
        if frame.complete_paths().is_some_and(<[String]>::is_empty) {
            return;
        }
        self.observe_storage_writes(state, &frame);
        let state = state.symbol;
        self.path_state
            .payload_dependencies
            .retain(|(_, dependencies)| dependencies_survive(dependencies, state, &frame));
        let mut records = std::mem::take(&mut self.path_state.contributions);
        records.retain(|record| dependencies_survive(&record.dependencies, state, &frame));
        self.normal_results = std::sync::Arc::new(
            self.normal_results
                .iter()
                .filter(|(_, term)| {
                    dependencies_survive(&self.term_dependencies([term]), state, &frame)
                })
                .cloned()
                .collect(),
        );
        self.normal_guarantees = std::sync::Arc::new(
            self.normal_guarantees
                .iter()
                .filter(|(left, right)| {
                    dependencies_survive(&self.term_dependencies([left, right]), state, &frame)
                })
                .cloned()
                .collect(),
        );
        self.captured_result_equations = std::sync::Arc::new(
            self.captured_result_equations
                .iter()
                .filter(|(left, right)| {
                    dependencies_survive(&self.term_dependencies([left, right]), state, &frame)
                })
                .cloned()
                .collect(),
        );
        self.path_state.contributions = records;
        self.reindex_surviving_contributions();
        self.path_state.effects.push((state, frame));
    }

    /// Case refinement changes the canonical subjects of already-produced
    /// equations. Rebuild their indexes from retained terms, under the current
    /// observed case graph; never reread a source premise or reissue dependency
    /// custody. Constructor clashes derive only from these surviving terms.
    pub(in crate::proof_contracts::contract_entailment) fn reindex_surviving_contributions(
        &mut self,
    ) {
        let records = std::mem::take(&mut self.path_state.contributions);
        self.substitutions = std::sync::Arc::new(Vec::new());
        self.rewrites = std::sync::Arc::new(super::rewrite_table::RewriteTable::default());
        self.case_facts = std::sync::Arc::new(Vec::new());
        self.case_substitutions = std::sync::Arc::new(Vec::new());
        self.hypotheses_contradictory = false;
        self.path_state.rebuilding = true;
        // Later retained case bindings retain their existing precedence. Seed
        // the observed graph before normalizing value equations: resolving a
        // call's arguments before rewrite lookup otherwise misses a premise
        // whose earlier key still contains the unrefined subject.
        for record in &records {
            if let Contribution::CaseBinding(name, value) = &record.payload {
                self.intake_case_binding(name.clone(), value.clone());
            }
        }
        for record in &records {
            match &record.payload {
                Contribution::CaseEquation(left, right) => {
                    self.intake_case_equation(left.clone(), right.clone(), 0)
                }
                Contribution::Case(guarantee) => self.intake_case_guarantee(guarantee),
                _ => {}
            }
        }
        for record in &records {
            if let Contribution::Equation(left, right) = &record.payload {
                self.intake_equation(left.clone(), right.clone(), 0);
            }
        }
        self.path_state.rebuilding = false;
        self.path_state.contributions = records;
    }
}

fn dependencies_survive(
    dependencies: &[SubjectDependency],
    state: SymbolHandle,
    frame: &NormalizedWriteFrame,
) -> bool {
    let Some(written) = frame.complete_paths() else {
        return dependencies.is_empty();
    };
    dependencies.iter().all(|dependency| match dependency {
        SubjectDependency::Live {
            state: source,
            root,
            path,
        } => {
            root.is_valid()
                && *source == state
                && !written
                    .iter()
                    .any(|write| crate::frame_paths_overlap(path, write))
        }
        SubjectDependency::Unavailable => written.is_empty(),
    })
}
