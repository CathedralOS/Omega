//! Substitute returned reference leaves through their completed input values.
//! The chronological evaluator owns these observations. Queries may inspect
//! their typed structure and checked return routes, never evaluate an argument
//! again against the enclosing call's later descriptor inventory.

use super::super::path_instantiation::aggregate_arguments::{
    AggregateOrigins, ArgumentOrigins, CapturedReferenceSource, ReferenceSource, aggregate_value,
    partition_sources, reference_leaves,
};
use super::super::{caller_aliases, result_origins, stored_origins};
use super::{FrameInference, Machine, SlotArgumentCapture, SlotSource, TopLevelSymbols};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;
use typed_trees::types::TypeReferenceHandle;

/// Instantiate backing and descriptor transport from the same checked callee
/// source pair. Distinct formal sources require one exact matching observation;
/// independent possible-alias sets cannot establish that association.
pub(super) struct InstantiatedSource {
    pub source: SlotSource,
    pub substitutions: Vec<(symbols::SymbolHandle, super::super::FrameSourcePlace)>,
}

pub(super) fn instantiate_sources(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    parameters: &[StateParameter],
    arguments: &[SlotArgumentCapture],
    supplied: &SlotSource,
    entry: &super::CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
) -> Option<Vec<InstantiatedSource>> {
    let mut returned = if supplied.origin.source.is_static()
        || supplied.origin.source.output_identity().is_some()
    {
        vec![InstantiatedSource {
            source: supplied.clone(),
            substitutions: Vec::new(),
        }]
    } else {
        let index = typed_trees::signature::state_parameter_position(
            parameters,
            supplied.origin.source.root,
        )?;
        let spatial = SlotSource {
            carrier: supplied.origin.source.clone(),
            ..supplied.clone()
        };
        observations(
            program,
            machine,
            &parameters[index],
            arguments.get(index)?,
            &spatial,
            entry,
            symbols,
        )?
    };
    if supplied.carrier == supplied.origin.source {
        return Some(returned);
    }
    if supplied.carrier.is_static() {
        for source in &mut returned {
            source.source.carrier = supplied.carrier.clone();
        }
        return Some(returned);
    }
    let index =
        typed_trees::signature::state_parameter_position(parameters, supplied.carrier.root)?;
    let carrier = SlotSource {
        origin: super::super::FramePlaceOrigin {
            path: super::path(program, &supplied.carrier),
            precision: supplied.origin.precision,
            source: supplied.carrier.clone(),
        },
        carrier: supplied.carrier.clone(),
        lifetime_access: supplied.lifetime_access,
    };
    let mut observations = observations(
        program,
        machine,
        &parameters[index],
        arguments.get(index)?,
        &carrier,
        entry,
        symbols,
    )?
    .into_iter();
    let observed = observations.next()?;
    if returned.len() != 1 || observations.next().is_some() {
        return None;
    }
    let source = &mut returned[0];
    if source.source.origin.source != observed.source.origin.source
        || source.source.origin.precision != observed.source.origin.precision
    {
        return None;
    }
    source.source.carrier = observed.source.carrier;
    source.source.lifetime_access = source
        .source
        .lifetime_access
        .or(observed.source.lifetime_access);
    for substitution in observed.substitutions {
        if !source.substitutions.contains(&substitution) {
            source.substitutions.push(substitution);
        }
    }
    Some(returned)
}

pub(super) fn sources(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    parameter: &StateParameter,
    captured: &SlotArgumentCapture,
    supplied: &SlotSource,
    entry: &super::CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
) -> Option<Vec<SlotSource>> {
    Some(
        observations(
            program, machine, parameter, captured, supplied, entry, symbols,
        )?
        .into_iter()
        .map(|observed| observed.source)
        .collect(),
    )
}

fn observations(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    parameter: &StateParameter,
    captured: &SlotArgumentCapture,
    supplied: &SlotSource,
    entry: &super::CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
) -> Option<Vec<InstantiatedSource>> {
    // Carrier transport and spatial substitution must select the same relative
    // input leaf. Equal backing alone cannot pair independent alternatives.
    if supplied.carrier != supplied.origin.source || !captured.context.divergent.is_empty() {
        return None;
    }
    let query = Inputs {
        program,
        machine,
        captured,
        symbols,
        entry,
    };
    let returned = result_origins::instantiate_source(
        program,
        machine,
        parameter,
        captured.expression,
        &supplied.origin,
        symbols,
        &mut captured.inference.clone(),
        true,
        &query,
    )?;
    let mut sources = Vec::new();
    for source in returned {
        let (candidate, substitution) = match source {
            ReferenceSource::Captured(observation) => {
                if observation.base != observation.source.origin {
                    return None;
                }
                (
                    observation.source,
                    Some((parameter.symbol, observation.substitution)),
                )
            }
            ReferenceSource::Symbolic(origin) if origin.source.is_static() => (
                SlotSource {
                    carrier: origin.source.clone(),
                    origin,
                    lifetime_access: None,
                },
                None,
            ),
            ReferenceSource::Symbolic(_) => return None,
        };
        for mut source in [candidate] {
            if (!source.origin.source.root.is_valid() && !source.origin.source.is_static())
                || !source.origin.source.builtin_coordinates
                || (!source.carrier.root.is_valid() && !source.carrier.is_static())
                || !source.carrier.builtin_coordinates
            {
                return None;
            }
            source.lifetime_access = source.lifetime_access.or(supplied.lifetime_access);
            let substitutions = substitution.clone().into_iter().collect();
            if !sources.iter().any(|prior: &InstantiatedSource| {
                prior.source == source && prior.substitutions == substitutions
            }) {
                sources.push(InstantiatedSource {
                    source,
                    substitutions,
                });
            }
        }
    }
    (!sources.is_empty()).then_some(sources)
}

struct Inputs<'request, 'symbols> {
    program: &'request dyn crate::CheckingInput,
    machine: &'request Machine,
    captured: &'request SlotArgumentCapture,
    symbols: &'request TopLevelSymbols<'symbols>,
    entry: &'request super::CollectedStatementPrefix,
}

impl ArgumentOrigins for Inputs<'_, '_> {
    fn reference(
        &self,
        expression: ExpressionHandle,
        reference: TypeReferenceHandle,
        _implicit_borrow: bool,
        inference: &mut FrameInference,
    ) -> Option<Vec<ReferenceSource>> {
        let captured = self.captured.observation(expression)?;
        if !captured.context.divergent.is_empty() {
            return None;
        }
        if matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Call(_) | ExpressionNode::Match(_)
        ) || self.computed_projection(expression)
        {
            let returned = if matches!(
                self.program.expression_table.expression(expression),
                ExpressionNode::Call(_)
            ) {
                aggregate_value(
                    self.program,
                    self.machine,
                    expression,
                    reference,
                    self.symbols,
                    inference,
                    true,
                    self,
                )?
            } else if self.computed_projection(expression) {
                self.project_value(expression, reference, inference)?
            } else {
                reference_leaves(
                    self.program,
                    self.machine,
                    expression,
                    reference,
                    "",
                    self.symbols,
                    inference,
                    true,
                    self,
                )?
            };
            if !returned.cases.is_empty()
                || !returned.moves.is_empty()
                || returned.references.is_empty()
            {
                return None;
            }
            let mut sources = Vec::new();
            for leaf in returned.references {
                if !matches!(leaf.kind, stored_origins::StoredOriginKind::Reference(_))
                    || !leaf.local_segments.is_empty()
                    || !leaf.local_suffix.is_empty()
                {
                    return None;
                }
                sources.push(leaf.origin?);
                sources.extend(
                    leaf.lifetime_dependencies
                        .into_iter()
                        .map(completed_dependency),
                );
            }
            return Some(sources);
        }
        let (state, _, index) = caller_aliases::caller_statement_owner(
            self.program,
            self.machine,
            caller_aliases::CallerWriteSite::Expression(expression),
        )?;
        // Frozen local evidence and active body guards have different
        // lifetimes. Restore the observation's locals without removing any
        // recursion guard required by the enclosing result query.
        let mut observed_inference = inference_at(captured, inference);
        let returned = result_origins::reference_result_relation(
            self.program,
            self.machine,
            state,
            index,
            expression,
            reference,
            self.symbols,
            &mut observed_inference,
        )?;
        let mut sources = Vec::new();
        for leaf in returned.references {
            for dependency in leaf.lifetime_dependencies {
                if !sources.contains(&completed_dependency(dependency.clone())) {
                    sources.push(completed_dependency(dependency));
                }
            }
            for source in self.capture_source(captured, leaf.origin?)? {
                if !sources.contains(&source) {
                    sources.push(source);
                }
            }
        }
        (!sources.is_empty()).then_some(sources)
    }

    fn stored(
        &self,
        expression: ExpressionHandle,
        reference: TypeReferenceHandle,
        inference: &FrameInference,
    ) -> Option<AggregateOrigins> {
        if self.computed_projection(expression) {
            return self.project_value(expression, reference, &mut inference.clone());
        }
        let captured = self.captured.observation(expression)?;
        if !captured.context.divergent.is_empty() {
            return None;
        }
        let (state, before, _) = caller_aliases::caller_statement_owner(
            self.program,
            self.machine,
            caller_aliases::CallerWriteSite::Expression(expression),
        )?;
        // The ordinary projection query owns exact type, case and selector
        // validation. Its symbolic descriptor homes are loaded only in the
        // already completed operand inventory below; no initializer is replayed.
        let mut returned = stored_origins::reference_leaves_before_statement_for_query(
            self.program,
            state,
            before,
            expression,
            reference,
            None,
            None,
            true,
        )?;
        let mut references = Vec::new();
        for mut leaf in returned.references {
            let Some(origin) = leaf.origin.clone() else {
                references.push(leaf);
                continue;
            };
            let (sources, dependencies) = partition_sources(self.capture_source(captured, origin)?);
            for dependency in dependencies {
                if !leaf.lifetime_dependencies.contains(&dependency) {
                    leaf.lifetime_dependencies.push(dependency);
                }
            }
            if sources.is_empty() {
                leaf.origin = None;
                references.push(leaf);
                continue;
            }
            for source in sources {
                let mut leaf = leaf.clone();
                leaf.origin = Some(source);
                references.push(leaf);
            }
        }
        returned.references = references;
        Some(returned)
    }

    fn call_result(
        &self,
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
        symbols: &TopLevelSymbols<'_>,
        inference: &mut FrameInference,
        include_shared: bool,
    ) -> Option<AggregateOrigins> {
        let captured = self.captured.observation(expression)?;
        if !captured.context.divergent.is_empty() {
            return None;
        }
        // The evaluator retained this invocation's entry before its own normal
        // effects. Its operands keep their distinct earlier descriptor values.
        let query = Inputs {
            captured,
            entry: &captured.context,
            ..*self
        };
        let mut observed_inference = inference_at(captured, inference);
        result_origins::call_result_origins(
            program,
            machine,
            program.expression_table.call_node(expression)?,
            expected,
            symbols,
            &mut observed_inference,
            include_shared,
            &query,
        )
    }

    fn project(
        &self,
        program: &dyn crate::CheckingInput,
        source: ReferenceSource,
        suffix: &str,
        precision: super::super::FramePathPrecision,
        relative: &super::super::FrameSourcePlace,
    ) -> Option<Vec<ReferenceSource>> {
        if matches!(&source, ReferenceSource::Captured(observed) if observed.source.lifetime_access.is_some())
        {
            // This row is an existing lifetime obligation, not another pointer
            // through which the callee may append its output geometry.
            return Some(vec![source]);
        }
        let source = result_origins::compose_source(program, source, suffix, precision, relative);
        let ReferenceSource::Captured(observed) = source else {
            return source.place().source.is_static().then_some(vec![source]);
        };
        let captured = SlotSource {
            origin: observed.base,
            ..observed.source.clone()
        };
        let completed = stored_origins::completed_reference_projection_sources(
            program,
            &captured,
            &observed.source.origin,
            &self.entry.stored,
        )?;
        Some(
            completed
                .into_iter()
                .map(|source| {
                    ReferenceSource::Captured(CapturedReferenceSource {
                        base: source.origin.clone(),
                        source,
                        substitution: captured.origin.source.clone(),
                    })
                })
                .collect(),
        )
    }
}

impl Inputs<'_, '_> {
    /// A Symbol-root projection reads its selected storage at completion. An
    /// Expression-root projection instead selects a completed receiver value;
    /// the latter must carry the receiver's produced association, not recover
    /// an initializer or reinterpret the receiver at a later caller prefix.
    fn computed_projection(&self, expression: ExpressionHandle) -> bool {
        if !matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
        ) {
            return false;
        }
        let mut facts = facts::FactPlan::default();
        let place = facts.append_place_from_expression(self.program, expression);
        facts.places.get(place).root.as_expression().is_some()
    }

    fn project_value(
        &self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
        inference: &mut FrameInference,
    ) -> Option<AggregateOrigins> {
        let captured = self.captured.observation(expression)?;
        let receiver = match self.program.expression_table.expression(expression) {
            ExpressionNode::Member(member) => member.receiver,
            ExpressionNode::Indexed(indexed) => indexed.collection,
            _ => return None,
        };
        let (state, _, _) = caller_aliases::caller_statement_owner(
            self.program,
            self.machine,
            caller_aliases::CallerWriteSite::Expression(expression),
        )?;
        let receiver_type =
            crate::expression_result_type_reference(self.program, self.machine, state, receiver)?;
        let storage =
            super::super::reference_subjects::carrier_storage_type(self.program, receiver_type)?;
        let mut facts = facts::FactPlan::default();
        let receiver_place = facts.append_place_from_expression(self.program, receiver);
        let selected_place = facts.append_place_from_expression(self.program, expression);
        let receiver_place = facts.places.get(receiver_place);
        let selected_place = facts.places.get(selected_place);
        let prefix = facts.place_segments.span(receiver_place.segments)?;
        let selected = facts.place_segments.span(selected_place.segments)?;
        if receiver_place.root != selected_place.root || !selected.starts_with(prefix) {
            return None;
        }
        let relative = &selected[prefix.len()..];
        let selected_type =
            stored_origins::projected_storage_type(self.program, storage, relative)?;
        if !super::super::isolation::aggregate_storage_types_match(
            self.program,
            selected_type,
            expected,
        ) {
            return None;
        }
        let origins = if super::super::type_reference_is_reference(self.program, receiver_type) {
            // Reading through an already produced descriptor observes memory
            // at this projection's completion, after any selector effects.
            // Its descriptor value still comes from the receiver observation.
            let (sources, dependencies) =
                partition_sources(self.reference(receiver, receiver_type, false, inference)?);
            let mut origins = AggregateOrigins::default();
            for source in sources {
                let origin = source.place();
                if !origin.source.root.is_valid() || !origin.source.builtin_coordinates {
                    return None;
                }
                let observed = if let Some(record) = captured
                    .context
                    .stored
                    .iter()
                    .find(|record| record.local_symbol == origin.source.root)
                {
                    stored_origins::project_stored_origins(
                        self.program,
                        record,
                        &origin.source.segments,
                        record.symbolic,
                    )?
                } else {
                    // No local current slot was installed. The ordinary type
                    // frontier is a conservative possible roster, never proof
                    // of a selected case or an absent reference boundary.
                    let declared = stored_origins::declared_origins_for_query(
                        self.program,
                        origin.source.root,
                        self.program.symbols.name(origin.source.root),
                        storage,
                        true,
                    )?;
                    let mut declared =
                        stored_origins::project_stored_origins(self.program, &declared, &[], true)?;
                    for leaf in &mut declared.references {
                        if let Some(source) = &mut leaf.origin {
                            let relative = source.place().source.clone();
                            source.place_mut().source =
                                origin.source.append_relative(self.program, &relative);
                            source.place_mut().path =
                                super::path(self.program, &source.place().source);
                        }
                    }
                    declared
                };
                for mut leaf in observed.references {
                    leaf.lifetime_dependencies
                        .extend(dependencies.iter().cloned());
                    if let Some(source) = leaf.origin.clone() {
                        let (sources, debts) =
                            partition_sources(self.capture_source(captured, source)?);
                        for debt in debts {
                            if !leaf.lifetime_dependencies.contains(&debt) {
                                leaf.lifetime_dependencies.push(debt);
                            }
                        }
                        if sources.is_empty() {
                            leaf.origin = None;
                            origins.references.push(leaf);
                        } else {
                            for source in sources {
                                let mut leaf = leaf.clone();
                                leaf.origin = Some(source);
                                origins.references.push(leaf);
                            }
                        }
                    } else {
                        origins.references.push(leaf);
                    }
                }
                origins.cases.extend(observed.cases);
                origins.moves.extend(observed.moves);
            }
            origins
        } else {
            reference_leaves(
                self.program,
                self.machine,
                receiver,
                receiver_type,
                "",
                self.symbols,
                inference,
                true,
                self,
            )?
        };
        stored_origins::project_aggregate_origins(
            self.program,
            origins,
            symbols::SymbolHandle::invalid(),
            relative,
        )
    }

    fn capture_source(
        &self,
        captured: &SlotArgumentCapture,
        origin: ReferenceSource,
    ) -> Option<Vec<ReferenceSource>> {
        let ReferenceSource::Symbolic(carrier) = origin else {
            return Some(vec![origin]);
        };
        let mut sources = Vec::new();
        for source in stored_origins::canonical_reference_sources(
            self.program,
            &Some(carrier.clone()),
            &captured.context.aliases,
            &captured.context.stored,
        ) {
            let mut source = source?;
            if source.lifetime_access.is_none() {
                source.carrier = carrier.source.clone();
            }
            let source = ReferenceSource::Captured(CapturedReferenceSource {
                base: source.origin.clone(),
                substitution: source.origin.source.clone(),
                source,
            });
            if !sources.contains(&source) {
                sources.push(source);
            }
        }
        (!sources.is_empty()).then_some(sources)
    }
}

/// Preserve completed local evidence without losing any enclosing active-body
/// guard. A later caller prefix cannot substitute its locals into this clock.
fn inference_at(captured: &SlotArgumentCapture, enclosing: &FrameInference) -> FrameInference {
    let mut observed = captured.inference.clone();
    for state in &enclosing.active_states {
        if !observed.active_states.contains(state) {
            observed.active_states.push(*state);
        }
    }
    observed
}

/// Reintroduce an existing leaf debt at a query boundary without projecting it.
fn completed_dependency(source: SlotSource) -> ReferenceSource {
    ReferenceSource::Captured(CapturedReferenceSource {
        base: source.origin.clone(),
        substitution: source.origin.source.clone(),
        source,
    })
}
