//! Writes retire live storage observations, not completed value identities.
//!
//! The normal-return path owns effect order. Its complete caller-relative frame
//! may replace a binding's current value while earlier copies and equations
//! still describe their captured values. A later read of retired storage gets
//! a new unknown observation; unrelated owned bindings retain their facts.
//! Whole aggregate roots are retired on an overlapping field write. Reference-
//! bearing or opaque roots remain conservative until their exact preservation
//! is available: a disjoint spelling alone cannot preserve aliased contents.

use super::StructuralJudge;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;

type StorageScope<'scope> = Option<(&'scope Machine, &'scope State)>;

/// A canonical place with live selectors replaced by completed value identities.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct CapturedStorageCoordinate {
    root: facts::PlaceRoot,
    segments: Vec<CapturedStorageSegment>,
}

#[derive(Clone, PartialEq, Eq)]
enum CapturedStorageSegment {
    Place(facts::PlaceSegment),
    Index(super::StructuralTerm),
}

/// One evaluated place keeps its detached entry snapshot beside its live value.
/// Writes retire only the latter. An entry value is captured by an actual read
/// before that parameter is invalidated, never reconstructed from an initializer.
#[derive(Clone)]
pub(super) struct CapturedStorageValue {
    coordinate: CapturedStorageCoordinate,
    entry: Option<super::StructuralTerm>,
    current: Option<super::StructuralTerm>,
}

impl StructuralJudge<'_> {
    pub(in crate::proof_contracts::contract_entailment) fn observe_storage_writes(
        &mut self,
        state: &State,
        written: &[String],
    ) -> Option<()> {
        if written.is_empty() || !self.requires_value_preservation() {
            return Some(());
        }
        self.storage_write_sequence += 1;
        // Exact stored observations are live contents. Conservatively retire
        // all of them on any subsequent write; detached copies keep their own
        // value identities in the normal-return environment.
        std::sync::Arc::make_mut(&mut self.stored_values).retain_mut(|stored| {
            stored.current = None;
            stored.entry.is_some()
        });
        let program = self.program;
        let frames = self.frame_resolver()?;
        let mut invalidated = Vec::new();
        let mut consider = |symbol: SymbolHandle, reference| {
            let overlaps = crate::frame_coordinate_path(program, symbol, &[]).is_none_or(|path| {
                written
                    .iter()
                    .any(|written| crate::frame_paths_overlap(written, &path))
            });
            if overlaps || !frames.value_is_reference_free(reference) {
                invalidated.push(symbol);
            }
        };
        for parameter in program.state_parameters(state) {
            consider(parameter.symbol, parameter.type_reference);
        }
        // A named state may receive an earlier state's reference under a new
        // binder. The exit contract still names the original binder; retiring
        // only this state's spelling would revive its entry observation. As
        // with current reference-bearing roots, a write conservatively retires
        // reference-bearing parameters in the enclosing invocation as well.
        if let Some(machine) = program.machine(self.machine_symbol) {
            for earlier in program.machine_states(machine) {
                if earlier.symbol == state.symbol {
                    continue;
                }
                for parameter in program.state_parameters(earlier) {
                    if !frames.value_is_reference_free(parameter.type_reference) {
                        consider(parameter.symbol, parameter.type_reference);
                    }
                }
            }
        }
        for statement in program.statement_table.statements(state.statement_nodes) {
            if let Some(local) = statement.as_local_data() {
                consider(local.symbol, local.type_reference);
            }
        }
        let storage = std::sync::Arc::make_mut(&mut self.invalidated_storage);
        for symbol in invalidated {
            if !storage.contains(&symbol) {
                storage.push(symbol);
            }
        }
        // These rows are source predicates, unlike the captured term equations.
        // A later branch must not reinterpret an old entry fact over new storage.
        self.premise_facts.retain(|fact| {
            let mut expressions = Vec::new();
            crate::expression_traversal::collect_expression_tree(program, *fact, &mut expressions);
            !expressions.iter().any(|expression| {
                crate::value_custody::places::expression_root_symbol(
                    *expression,
                    &program.expression_table,
                    self.machine_symbol,
                )
                .is_some_and(|symbol| storage.contains(&symbol))
            })
        });
        Some(())
    }

    fn ordinary_storage_path(&self, scope: StorageScope<'_>, expression: ExpressionHandle) -> bool {
        match self.program.expression_table.expression(expression) {
            typed_trees::expression::ExpressionNode::Name(_) => true,
            typed_trees::expression::ExpressionNode::Member(member) => {
                self.ordinary_storage_path(scope, member.receiver)
            }
            typed_trees::expression::ExpressionNode::Indexed(indexed) => {
                let builtin = scope.is_some_and(|(machine, state)| {
                    crate::value_custody::places::place_has_builtin_coordinates(
                        self.program,
                        machine,
                        Some(state),
                        expression,
                    )
                });
                let collection = self.ordinary_storage_path(scope, indexed.collection);
                builtin && collection
            }
            // Canonical places deliberately erase borrow recasts to their
            // backing coordinate. That coordinate does not authorize reusing
            // the captured value under a different representation.
            _ => false,
        }
    }

    fn captured_storage_coordinate(
        &self,
        scope: StorageScope<'_>,
        expression: ExpressionHandle,
    ) -> Option<CapturedStorageCoordinate> {
        if !self.ordinary_storage_path(scope, expression) {
            return None;
        }
        let place = crate::value_custody::places::canonical_place_from_expression(
            self.program,
            expression,
        )?;
        self.capture_storage_place(place)
    }

    pub(super) fn captured_reference_subject(
        &self,
        subject: &crate::machine_calls::calls::InvocationReferenceSubject,
    ) -> Option<CapturedStorageCoordinate> {
        self.capture_storage_place(crate::CanonicalPlace {
            root: facts::PlaceRoot::Symbol(subject.source.root),
            segments: subject.source.segments.clone(),
        })
    }

    fn capture_storage_place(
        &self,
        place: crate::CanonicalPlace,
    ) -> Option<CapturedStorageCoordinate> {
        let segments = place
            .segments
            .into_iter()
            .map(|segment| match segment {
                facts::PlaceSegment::FixedIndex { index } => Some(CapturedStorageSegment::Index(
                    super::StructuralTerm::Integer(numerics::bignum::BigInt::from_u64(
                        u64::try_from(index).ok()?,
                    )),
                )),
                facts::PlaceSegment::Index { expression } => {
                    let completed = self
                        .normal_results
                        .iter()
                        .find(|(source, _)| *source == expression);
                    let (_, value) = completed?;
                    let value = self.completed_value_identity(value.clone());
                    value.observation_identity_key()?;
                    Some(CapturedStorageSegment::Index(value))
                }
                facts::PlaceSegment::Field { symbol }
                    if symbol.is_valid()
                        && self.program.symbols.get(symbol).kind == symbols::SymbolKind::Field =>
                {
                    Some(CapturedStorageSegment::Place(segment))
                }
                facts::PlaceSegment::Case { variant } if variant.is_valid() => {
                    Some(CapturedStorageSegment::Place(segment))
                }
                facts::PlaceSegment::Field { .. } | facts::PlaceSegment::Case { .. } => None,
                // A window is not one storage coordinate. Unknown selection cannot
                // borrow the identity of its source expression or an arbitrary slot.
                facts::PlaceSegment::FixedRange { .. } | facts::PlaceSegment::OpaqueRange => None,
            })
            .collect::<Option<Vec<_>>>()?;
        Some(CapturedStorageCoordinate {
            root: place.root,
            segments,
        })
    }

    pub(super) fn stored_value_observation(
        &self,
        scope: StorageScope<'_>,
        expression: ExpressionHandle,
    ) -> Option<super::StructuralTerm> {
        // Case-only judgments intentionally do not track storage writes.
        self.requires_value_preservation().then_some(())?;
        let place = self.captured_storage_coordinate(scope, expression)?;
        self.current_storage_value(&place)
    }

    pub(super) fn current_storage_value(
        &self,
        place: &CapturedStorageCoordinate,
    ) -> Option<super::StructuralTerm> {
        if let Some(value) = self
            .stored_values
            .iter()
            .find(|stored| &stored.coordinate == place)
            .and_then(|stored| stored.current.clone())
        {
            return Some(value);
        }
        // A fresh aggregate observation owns its fields too. Projection uses
        // that exact current value, never the retired source initializer.
        self.stored_values.iter().find_map(|stored| {
            if stored.coordinate.root != place.root
                || !place.segments.starts_with(&stored.coordinate.segments)
            {
                return None;
            }
            let mut value = stored.current.clone()?;
            for segment in &place.segments[stored.coordinate.segments.len()..] {
                let CapturedStorageSegment::Place(facts::PlaceSegment::Field { symbol }) = segment
                else {
                    return None;
                };
                value = self.observed_field_term(value, self.program.symbols.name(*symbol), 0)?;
            }
            Some(value)
        })
    }

    /// A whole-value read after a field replacement captures the current field
    /// snapshots alongside its otherwise unknown value. It cannot reconstruct
    /// unobserved siblings from the old aggregate or grant reference contents.
    pub(super) fn intake_current_storage_fields(
        &mut self,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        value: &super::StructuralTerm,
    ) {
        let Some(coordinate) = self.captured_storage_coordinate(Some((machine, state)), expression)
        else {
            return;
        };
        let Some(reference) =
            crate::expression_result_type_reference(self.program, machine, state, expression)
        else {
            return;
        };
        let equations = self
            .stored_values
            .iter()
            .filter_map(|stored| {
                if stored.coordinate.root != coordinate.root
                    || stored.coordinate.segments.len() <= coordinate.segments.len()
                    || !stored.coordinate.segments.starts_with(&coordinate.segments)
                {
                    return None;
                }
                let current = stored.current.clone()?;
                let suffix = stored.coordinate.segments[coordinate.segments.len()..]
                    .iter()
                    .map(|segment| match segment {
                        CapturedStorageSegment::Place(segment) => Some(*segment),
                        CapturedStorageSegment::Index(_) => None,
                    })
                    .collect::<Option<Vec<_>>>()?;
                let projection = super::super::observed_projections::project_observed_value(
                    self.program,
                    reference,
                    value.clone(),
                    &suffix,
                )?;
                Some((projection, current))
            })
            .collect::<Vec<_>>();
        for (projection, current) in equations {
            self.intake_equation(projection, current, 0);
        }
    }

    /// Capture a completed scalar read using the same storage roster as exact
    /// stores. Unrepresentable coordinates still have a detached event value,
    /// but cannot authorize a later read of another coordinate.
    pub(super) fn observe_completed_storage_read(
        &mut self,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        value: super::StructuralTerm,
    ) {
        let Some(place) = self.captured_storage_coordinate(Some((machine, state)), expression)
        else {
            return;
        };
        let entry = self
            .program
            .machine_states(machine)
            .first()
            .is_some_and(|entry| entry.symbol == state.symbol)
            && place.root.as_symbol().is_some_and(|root| {
                self.program
                    .state_parameter_by_symbol(state, root)
                    .is_some()
                    && !self.storage_binding_invalidated(root)
            });
        self.retain_storage_value(place, value, entry);
    }

    pub(super) fn retain_storage_value(
        &mut self,
        coordinate: CapturedStorageCoordinate,
        value: super::StructuralTerm,
        entry: bool,
    ) {
        let values = std::sync::Arc::make_mut(&mut self.stored_values);
        if let Some(stored) = values
            .iter_mut()
            .find(|stored| stored.coordinate == coordinate)
        {
            if entry && stored.entry.is_none() {
                stored.entry = Some(value.clone());
            }
            stored.current = Some(value);
        } else {
            values.push(CapturedStorageValue {
                coordinate,
                entry: entry.then(|| value.clone()),
                current: Some(value),
            });
        }
    }

    /// Rejoin a normalized content projection to the completed read of that
    /// exact parameter place and revision. This supplies no arithmetic or loan
    /// authority; absent reads and retired current values remain unavailable.
    pub(in crate::proof_contracts::contract_entailment) fn captured_parameter_projection(
        &self,
        root: SymbolHandle,
        path: &[language_semantics::content::ContentPlaceSegment],
        version: language_semantics::content::ContentPlaceVersion,
    ) -> Option<super::StructuralTerm> {
        use language_semantics::content::{ContentPlaceSegment, ContentPlaceVersion};
        self.requires_value_preservation().then_some(())?;
        let segments = path
            .iter()
            .map(|segment| match segment {
                ContentPlaceSegment::Field(field) => {
                    CapturedStorageSegment::Place(facts::PlaceSegment::Field {
                        symbol: field.symbol,
                    })
                }
                ContentPlaceSegment::Case(case) => {
                    CapturedStorageSegment::Place(facts::PlaceSegment::Case {
                        variant: case.symbol,
                    })
                }
                ContentPlaceSegment::FixedIndex(index) => CapturedStorageSegment::Index(
                    super::StructuralTerm::Integer(numerics::bignum::BigInt::from_u64(*index)),
                ),
            })
            .collect::<Vec<_>>();
        let stored = self.stored_values.iter().find(|stored| {
            stored.coordinate.root.as_symbol() == Some(root)
                && stored.coordinate.segments == segments
        })?;
        match version {
            ContentPlaceVersion::Entry => stored.entry.clone(),
            ContentPlaceVersion::Current => stored.current.clone(),
        }
    }

    pub(in crate::proof_contracts::contract_entailment) fn observe_exact_store(
        &mut self,
        machine: &typed_trees::machine::Machine,
        state: &State,
        statement: &typed_trees::statement::StatementNode,
        value: super::StructuralTerm,
    ) -> Option<()> {
        let assignment = statement.as_assignment()?;
        self.ordinary_storage_path(Some((machine, state)), assignment.target)
            .then_some(())?;
        let program = self.program;
        let place = crate::value_custody::places::canonical_place_from_expression(
            program,
            assignment.target,
        )?;
        let root = place.root.as_symbol()?;
        if !(program.state_parameter_by_symbol(state, root).is_some()
            || program.state_local_declaration(state, root).is_some())
            || !place.segments.iter().all(|segment| {
                matches!(segment,
                facts::PlaceSegment::Field { symbol } if symbol.is_valid())
            })
        {
            return None;
        }
        let reference = crate::value_custody::places::declared_place_type_raw(
            program,
            machine,
            Some(state),
            assignment.target,
        )?;
        let frames = self.frame_resolver()?;
        if !frames.value_is_reference_free(reference)
            || !frames
                .statement_value_write_frame(machine, statement)
                .into_complete_paths()?
                .is_empty()
        {
            return None;
        }
        let written = frames
            .assignment_write_frame(machine, statement)
            .into_complete_paths()?;
        let coordinate = crate::frame_coordinate_path(program, root, &place.segments)?;
        // No coarse region or rebased alias can stand in for this destination.
        if written.as_slice() != [coordinate] {
            return None;
        }
        self.observe_storage_writes(state, &written)?;
        let coordinate =
            self.captured_storage_coordinate(Some((machine, state)), assignment.target)?;
        self.retain_storage_value(coordinate, value, false);
        Some(())
    }

    pub(super) fn storage_observation_invalidated(&self, expression: ExpressionHandle) -> bool {
        crate::value_custody::places::expression_root_symbol(
            expression,
            &self.program.expression_table,
            self.machine_symbol,
        )
        .is_some_and(|symbol| self.invalidated_storage.contains(&symbol))
    }

    pub(super) fn storage_binding_invalidated(&self, symbol: SymbolHandle) -> bool {
        self.invalidated_storage.contains(&symbol)
    }
}
