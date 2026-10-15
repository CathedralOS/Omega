//! Flow-sensitive value facts used by arithmetic validation.
//!
//! This module owns the mutable abstract environment and its merge/invalidation
//! rules. Expression analysis and guard recognition remain in the parent.

use super::{ExpressionHandle, Interval, Machine, State, ordered_values};
use crate::proof_contracts::arithmetic_domains::place_paths::{
    canonical_path_pair, place_path_pair, place_paths_overlap,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq)]
pub(super) struct StoredIntegerRange {
    pub place: crate::value_custody::places::CanonicalPlace,
    pub path: String,
    pub recipe: checked_trees::IntegerRangeRecipe,
}

/// S4 flow-sensitive value environment: the proven interval of each place
/// (`self.field`, local) along the straight-line prefix of a state body. Lets the
/// overflow proof discharge `self.v = 10; self.v += 5` (v is known to be 10, so
/// 15 fits) instead of falling back to the full type range. Conservative: an
/// entry is only present when its value is definitely established on the linear
/// path; on anything we cannot model (a call that may mutate, a branch) the
/// relevant entries are dropped and the place falls back to its type bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct FloatInterval {
    pub(super) low: Option<f64>,
    pub(super) high: Option<f64>,
}

impl FloatInterval {
    pub(super) const UNBOUNDED: FloatInterval = FloatInterval {
        low: None,
        high: None,
    };

    pub(super) fn intersect(self, other: FloatInterval) -> FloatInterval {
        FloatInterval {
            low: match (self.low, other.low) {
                (Some(left), Some(right)) => Some(left.max(right)),
                (left, right) => left.or(right),
            },
            high: match (self.high, other.high) {
                (Some(left), Some(right)) => Some(left.min(right)),
                (left, right) => left.or(right),
            },
        }
    }

    pub(super) fn union(self, other: FloatInterval) -> FloatInterval {
        FloatInterval {
            low: self.low.zip(other.low).map(|(left, right)| left.min(right)),
            high: self
                .high
                .zip(other.high)
                .map(|(left, right)| left.max(right)),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct ValueEnvironment {
    pub(super) integer_ranges: Vec<StoredIntegerRange>,
    pub(super) boolean_values: Vec<super::guard_narrowing::BooleanValue>,
    pub(super) ordered_values: Vec<ordered_values::Relation>,
    pub(super) intervals: BTreeMap<String, Interval>,
    pub(super) known_u64_values: BTreeMap<String, u64>,
    pub(super) float_intervals: BTreeMap<String, FloatInterval>,
    pub(super) non_nan: BTreeSet<String>,
    pub(super) joint_subtract_bounds: BTreeSet<(String, String)>,
    pub(super) signed_joint_subtract_lower_bounds: BTreeSet<(String, String)>,
    pub(super) signed_joint_subtract_upper_bounds: BTreeSet<(String, String)>,
    pub(super) joint_multiply_bounds: BTreeSet<(String, String)>,
    pub(super) signed_joint_multiply_lower_bounds: BTreeSet<(String, String)>,
    pub(super) signed_joint_multiply_upper_bounds: BTreeSet<(String, String)>,
    pub(super) signed_joint_multiply_negation_bounds: BTreeSet<String>,
}

impl ValueEnvironment {
    pub(super) fn range_recipe(
        &self,
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<&checked_trees::IntegerRangeRecipe> {
        let place =
            crate::value_custody::places::canonical_place_from_expression(program, expression)?;
        self.integer_ranges
            .iter()
            .find(|value| value.place == place)
            .map(|value| &value.recipe)
    }
    /// Current scalar-place relations, identified by resolved symbols. Fields
    /// and computed operands cannot become a scalar atom merely by sharing a
    /// display path. Existing joins, edge rebinding and writes own fact lifetime.
    pub(crate) fn ordered_scalar_bounds(
        &self,
    ) -> impl Iterator<Item = (symbols::SymbolHandle, symbols::SymbolHandle, i64)> + '_ {
        self.ordered_values.iter().filter_map(|relation| {
            let (
                ordered_values::Operand::Place { root: left, .. },
                ordered_values::Operand::Place { root: right, .. },
            ) = (&relation.left, &relation.right)
            else {
                return None;
            };
            Some((*left, *right, relation.minimum_difference()?))
        })
    }

    /// Translate only explicitly bound roots. A source root may feed several
    /// target parameters; unrelated same-spelled roots never survive the edge.
    pub(super) fn rebind(&self, bindings: &[(String, String)]) -> Self {
        let paths = |path: &str| -> Vec<String> {
            bindings
                .iter()
                .filter_map(|(source, target)| {
                    if path == source {
                        Some(target.clone())
                    } else {
                        path.strip_prefix(source)
                            .filter(|suffix| suffix.starts_with('.') || suffix.starts_with('['))
                            .map(|suffix| format!("{target}{suffix}"))
                    }
                })
                .collect()
        };
        let mut rebound = Self::new();
        macro_rules! maps {
            ($($field:ident),* $(,)?) => {$(
                for (path, value) in &self.$field {
                    for path in paths(path) { rebound.$field.insert(path, *value); }
                }
            )*};
        }
        macro_rules! sets {
            ($($field:ident),* $(,)?) => {$(
                for path in &self.$field { rebound.$field.extend(paths(path)); }
            )*};
        }
        macro_rules! pairs {
            ($canonical:literal; $($field:ident),* $(,)?) => {$(
                for (left, right) in &self.$field {
                    for left in paths(left) {
                        for right in paths(right) {
                            let pair = if $canonical { canonical_path_pair(left.clone(), right) }
                                else { (left.clone(), right) };
                            rebound.$field.insert(pair);
                        }
                    }
                }
            )*};
        }
        maps!(intervals, known_u64_values, float_intervals);
        sets!(non_nan, signed_joint_multiply_negation_bounds);
        pairs!(true;
            joint_multiply_bounds, signed_joint_multiply_lower_bounds,
            signed_joint_multiply_upper_bounds);
        pairs!(false; joint_subtract_bounds, signed_joint_subtract_lower_bounds,
            signed_joint_subtract_upper_bounds);
        rebound
    }

    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Drop all tracked values when no incoming environment can be retained.
    /// Unknown mutation instead preserves existing immutable scalar snapshots.
    pub(crate) fn clear(&mut self) {
        self.integer_ranges.clear();
        self.boolean_values.clear();
        self.intervals.clear();
        self.known_u64_values.clear();
        self.float_intervals.clear();
        self.non_nan.clear();
        self.ordered_values.clear();
        self.clear_joint_relations();
    }

    fn clear_joint_relations(&mut self) {
        self.joint_subtract_bounds.clear();
        self.signed_joint_subtract_lower_bounds.clear();
        self.signed_joint_subtract_upper_bounds.clear();
        self.joint_multiply_bounds.clear();
        self.signed_joint_multiply_lower_bounds.clear();
        self.signed_joint_multiply_upper_bounds.clear();
        self.signed_joint_multiply_negation_bounds.clear();
    }

    /// An unknown write can change aliased storage, but cannot change a copied
    /// immutable scalar binding. Keep only its already established scalar facts;
    /// never reread an initializer whose source may since have changed. References
    /// (including immutable bindings of mutable references) and aggregate paths
    /// are excluded. Control-flow joins must still use their ordinary merge or
    /// `clear`, since immutability alone does not establish an incoming fact.
    pub(crate) fn invalidate_unknown_writes(
        &mut self,
        program: &dyn crate::CheckingInput,
        state: Option<&State>,
    ) {
        let Some(state) = state else {
            self.clear();
            return;
        };
        // This environment uses display paths. A bare name must identify one
        // declaration in this exact state; ambiguous names retain no evidence.
        let mut frozen = BTreeMap::new();
        let mut declaration = |name, immutable, reference| {
            let copied_scalar = immutable && program.primitive_type_reference(reference).is_some();
            frozen
                .entry(name)
                .and_modify(|unique| *unique = false)
                .or_insert(copied_scalar);
        };
        for parameter in program.state_parameters(state) {
            declaration(
                parameter.name.as_str(),
                !parameter.is_mutable,
                parameter.type_reference,
            );
        }
        for statement in program.statement_table.statements(state.statement_nodes) {
            if let Some(local) = statement.as_local_data() {
                declaration(local.name.as_str(), !local.is_mutable, local.type_reference);
            }
        }
        let retained = |path: &str| frozen.get(path) == Some(&true);
        self.integer_ranges.retain(|value| {
            if !value.place.segments.is_empty() {
                return false;
            }
            let facts::PlaceRoot::Symbol(symbol) = value.place.root else {
                return false;
            };
            program
                .state_parameter_by_symbol(state, symbol)
                .is_some_and(|parameter| {
                    !parameter.is_mutable
                        && program
                            .primitive_type_reference(parameter.type_reference)
                            .is_some()
                })
                || program
                    .state_local_declaration(state, symbol)
                    .is_some_and(|(_, local)| {
                        !local.is_mutable
                            && program
                                .primitive_type_reference(local.type_reference)
                                .is_some()
                    })
        });
        self.intervals.retain(|path, _| retained(path));
        self.known_u64_values.retain(|path, _| retained(path));
        self.float_intervals.retain(|path, _| retained(path));
        self.non_nan.retain(|path| retained(path));
        self.ordered_values.retain(|relation| {
            relation.left.is_immutable_snapshot(program, state)
                && relation.right.is_immutable_snapshot(program, state)
        });
        self.boolean_values.retain_mut(|value| {
            if !value.binding.is_immutable_snapshot(program, state) {
                return false;
            }
            for relations in &mut value.alternatives {
                relations.retain(|relation| {
                    relation.left.is_immutable_snapshot(program, state)
                        && relation.right.is_immutable_snapshot(program, state)
                });
            }
            true
        });
        self.clear_joint_relations();
    }

    /// Invalidate only facts overlapping a callee's known may-write paths.
    /// A write to a parent invalidates descendants; a write to a descendant
    /// also invalidates any fact recorded for the parent value itself.
    pub(crate) fn invalidate_written_paths(&mut self, written: &[String]) {
        self.invalidate_storage(written, |operand| operand.survives(written));
    }

    fn invalidate_storage(
        &mut self,
        written: &[String],
        survives: impl Fn(&ordered_values::Operand) -> bool,
    ) {
        self.boolean_values.retain_mut(|value| {
            if !survives(&value.binding) {
                return false;
            }
            for relations in &mut value.alternatives {
                relations.retain(|relation| survives(&relation.left) && survives(&relation.right));
            }
            true
        });
        self.ordered_values
            .retain(|relation| survives(&relation.left) && survives(&relation.right));
        let overlaps = |path: &str| {
            written
                .iter()
                .any(|written| place_paths_overlap(path, written))
        };
        let keep_pair = |(left, right): &(String, String)| !overlaps(left) && !overlaps(right);
        self.integer_ranges.retain(|value| !overlaps(&value.path));
        self.intervals.retain(|path, _| !overlaps(path));
        self.known_u64_values.retain(|path, _| !overlaps(path));
        self.float_intervals.retain(|path, _| !overlaps(path));
        self.non_nan.retain(|path| !overlaps(path));
        self.joint_subtract_bounds.retain(keep_pair);
        self.signed_joint_subtract_lower_bounds.retain(keep_pair);
        self.signed_joint_subtract_upper_bounds.retain(keep_pair);
        self.joint_multiply_bounds.retain(keep_pair);
        self.signed_joint_multiply_lower_bounds.retain(keep_pair);
        self.signed_joint_multiply_upper_bounds.retain(keep_pair);
        self.signed_joint_multiply_negation_bounds
            .retain(|value| !overlaps(value));
    }

    /// Apply the store frame after its target and RHS effects have completed.
    pub(crate) fn invalidate_assignment_paths(
        &mut self,
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        statement: &typed_trees::statement::StatementNode,
        frames: Option<&crate::CallFrameResolver<'_>>,
        written: &[String],
    ) {
        let storage = (!self.ordered_values.is_empty() || !self.boolean_values.is_empty())
            .then(|| {
                let target = statement.as_assignment()?.target;
                crate::place_has_builtin_coordinates(program, machine, state, target)
                    .then_some(())?;
                frames?.assignment_storage_origins(machine, statement)
            })
            .flatten();
        self.invalidate_storage(written, |operand| {
            operand.survives_assignment(written, storage.as_ref())
        });
    }

    pub(super) fn get(&self, path: &str) -> Option<Interval> {
        self.intervals.get(path).copied()
    }

    pub(super) fn set(&mut self, path: String, interval: Interval) {
        self.intervals.insert(path, interval);
    }

    pub(super) fn mark_known_u64(&mut self, path: String, value: u64) {
        self.known_u64_values.insert(path, value);
    }

    /// Intersect a place's tracked interval with `interval` (tightening it).
    /// Used by guard narrowing so an arm's guard refines the environment without
    /// discarding a value already proven on the linear path.
    pub(super) fn narrow(&mut self, path: String, interval: Interval) {
        let merged = match self.intervals.get(&path) {
            Some(existing) => existing.intersect(interval),
            None => interval,
        };
        self.intervals.insert(path, merged);
    }

    pub(super) fn narrow_float(&mut self, path: String, interval: FloatInterval) {
        let merged = match self.float_intervals.get(&path) {
            Some(existing) => existing.intersect(interval),
            None => interval,
        };
        self.float_intervals.insert(path, merged);
    }

    pub(super) fn mark_non_nan(&mut self, path: String) {
        self.non_nan.insert(path);
    }

    pub(super) fn mark_joint_subtract_bound(&mut self, left: String, right: String) {
        self.joint_subtract_bounds.insert((left, right));
    }

    pub(super) fn mark_signed_joint_subtract_lower_bound(&mut self, left: String, right: String) {
        self.signed_joint_subtract_lower_bounds
            .insert((left, right));
    }

    pub(super) fn mark_signed_joint_subtract_upper_bound(&mut self, left: String, right: String) {
        self.signed_joint_subtract_upper_bounds
            .insert((left, right));
    }

    pub(super) fn mark_joint_multiply_bound(&mut self, left: String, right: String) {
        self.joint_multiply_bounds
            .insert(canonical_path_pair(left, right));
    }

    pub(super) fn mark_signed_joint_multiply_lower_bound(&mut self, left: String, right: String) {
        self.signed_joint_multiply_lower_bounds
            .insert(canonical_path_pair(left, right));
    }

    pub(super) fn mark_signed_joint_multiply_upper_bound(&mut self, left: String, right: String) {
        self.signed_joint_multiply_upper_bounds
            .insert(canonical_path_pair(left, right));
    }

    pub(super) fn mark_signed_joint_multiply_negation_bound(&mut self, value: String) {
        self.signed_joint_multiply_negation_bounds.insert(value);
    }

    pub(super) fn proves_joint_subtract_bound(
        &self,
        program: &dyn crate::CheckingInput,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> bool {
        let Some((left, right)) = place_path_pair(program, left, right) else {
            return false;
        };
        self.joint_subtract_bounds.contains(&(left, right))
    }

    pub(super) fn proves_signed_joint_subtract_lower_bound(
        &self,
        program: &dyn crate::CheckingInput,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> bool {
        let Some((left, right)) = place_path_pair(program, left, right) else {
            return false;
        };
        self.signed_joint_subtract_lower_bounds
            .contains(&(left, right))
    }

    pub(super) fn proves_signed_joint_subtract_upper_bound(
        &self,
        program: &dyn crate::CheckingInput,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> bool {
        let Some((left, right)) = place_path_pair(program, left, right) else {
            return false;
        };
        self.signed_joint_subtract_upper_bounds
            .contains(&(left, right))
    }

    pub(super) fn proves_joint_multiply_bound(
        &self,
        program: &dyn crate::CheckingInput,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> bool {
        let Some((left, right)) = place_path_pair(program, left, right) else {
            return false;
        };
        self.joint_multiply_bounds
            .contains(&canonical_path_pair(left, right))
    }

    pub(super) fn proves_signed_joint_multiply_bounds(
        &self,
        program: &dyn crate::CheckingInput,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> bool {
        let Some((left, right)) = place_path_pair(program, left, right) else {
            return false;
        };
        let pair = canonical_path_pair(left, right);
        self.signed_joint_multiply_lower_bounds.contains(&pair)
            && self.signed_joint_multiply_upper_bounds.contains(&pair)
    }

    pub(super) fn proves_signed_joint_multiply_negation_bound(
        &self,
        program: &dyn crate::CheckingInput,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> bool {
        for (value, factor) in [(left, right), (right, left)] {
            let Some((value, factor)) = place_path_pair(program, value, factor) else {
                continue;
            };
            let factor_is_negative_one = self
                .get(&factor)
                .is_some_and(|interval| interval.low == Some(-1) && interval.high == Some(-1));
            if self.signed_joint_multiply_negation_bounds.contains(&value) && factor_is_negative_one
            {
                return true;
            }
        }
        false
    }

    pub(super) fn float_fact(&self, path: &str) -> (FloatInterval, bool) {
        (
            self.float_intervals
                .get(path)
                .copied()
                .unwrap_or(FloatInterval::UNBOUNDED),
            self.non_nan.contains(path),
        )
    }

    /// The JOIN of two envs at a control-flow merge: only places tracked in
    /// BOTH survive, each at the UNION of its intervals (the fact that holds
    /// regardless of which path was taken). Used to seed a multi-predecessor
    /// state from its incoming edge guards.
    pub(crate) fn join(&self, other: &ValueEnvironment) -> ValueEnvironment {
        let mut joined = ValueEnvironment::new();
        // Unequal incoming snapshots are forgotten. In particular cyclic
        // arrivals must not grow a fresh symbolic expression every round.
        joined.integer_ranges.extend(
            self.integer_ranges
                .iter()
                .filter(|value| {
                    other
                        .integer_ranges
                        .iter()
                        .any(|other| other.place == value.place && other.recipe == value.recipe)
                })
                .cloned(),
        );
        for value in &self.boolean_values {
            if let Some(other) = other
                .boolean_values
                .iter()
                .find(|other| other.binding == value.binding)
            {
                let mut common = value.clone();
                for (relations, other) in common.alternatives.iter_mut().zip(&other.alternatives) {
                    relations.retain(|relation| other.contains(relation));
                }
                joined.boolean_values.push(common);
            }
        }
        joined.ordered_values.extend(
            self.ordered_values
                .iter()
                .filter(|relation| other.ordered_values.contains(relation))
                .cloned(),
        );
        for (path, interval) in &self.intervals {
            if let Some(other_interval) = other.intervals.get(path) {
                joined
                    .intervals
                    .insert(path.clone(), interval.union(*other_interval));
            }
        }
        for (path, value) in &self.known_u64_values {
            if other.known_u64_values.get(path) == Some(value) {
                joined.known_u64_values.insert(path.clone(), *value);
            }
        }
        for (path, interval) in &self.float_intervals {
            if let Some(other_interval) = other.float_intervals.get(path) {
                joined
                    .float_intervals
                    .insert(path.clone(), interval.union(*other_interval));
            }
        }
        joined
            .non_nan
            .extend(self.non_nan.intersection(&other.non_nan).cloned());
        joined.joint_subtract_bounds.extend(
            self.joint_subtract_bounds
                .intersection(&other.joint_subtract_bounds)
                .cloned(),
        );
        joined.signed_joint_subtract_lower_bounds.extend(
            self.signed_joint_subtract_lower_bounds
                .intersection(&other.signed_joint_subtract_lower_bounds)
                .cloned(),
        );
        joined.signed_joint_subtract_upper_bounds.extend(
            self.signed_joint_subtract_upper_bounds
                .intersection(&other.signed_joint_subtract_upper_bounds)
                .cloned(),
        );
        joined.joint_multiply_bounds.extend(
            self.joint_multiply_bounds
                .intersection(&other.joint_multiply_bounds)
                .cloned(),
        );
        joined.signed_joint_multiply_lower_bounds.extend(
            self.signed_joint_multiply_lower_bounds
                .intersection(&other.signed_joint_multiply_lower_bounds)
                .cloned(),
        );
        joined.signed_joint_multiply_upper_bounds.extend(
            self.signed_joint_multiply_upper_bounds
                .intersection(&other.signed_joint_multiply_upper_bounds)
                .cloned(),
        );
        joined.signed_joint_multiply_negation_bounds.extend(
            self.signed_joint_multiply_negation_bounds
                .intersection(&other.signed_joint_multiply_negation_bounds)
                .cloned(),
        );
        joined
    }
}
