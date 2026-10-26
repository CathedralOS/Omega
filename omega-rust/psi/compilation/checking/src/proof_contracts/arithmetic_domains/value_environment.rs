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

/// Translate a storage label only through explicitly bound source roots.
pub(super) fn rebound_paths(path: &str, bindings: &[(String, String)]) -> Vec<String> {
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
}

#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct ValueEnvironment {
    pub(super) numeric_observations: StatementNumericObservations,
    pub(super) integer_ranges: Vec<StoredIntegerRange>,
    pub(super) boolean_values: Vec<super::guard_narrowing::BooleanValue>,
    pub(super) ordered_values: Vec<ordered_values::Relation>,
    pub(super) intervals: BTreeMap<String, Interval>,
    pub(super) known_u64_values: BTreeMap<String, u64>,
    pub(super) float_intervals: BTreeMap<String, FloatInterval>,
    pub(super) non_nan: BTreeSet<String>,
    pub(super) float_aliases: BTreeSet<(String, String)>,
    pub(super) joint_subtract_bounds: BTreeSet<(String, String)>,
    pub(super) signed_joint_subtract_lower_bounds: BTreeSet<(String, String)>,
    pub(super) signed_joint_subtract_upper_bounds: BTreeSet<(String, String)>,
    pub(super) joint_multiply_bounds: BTreeSet<(String, String)>,
    pub(super) signed_joint_multiply_lower_bounds: BTreeSet<(String, String)>,
    pub(super) signed_joint_multiply_upper_bounds: BTreeSet<(String, String)>,
    pub(super) signed_joint_multiply_negation_bounds: BTreeSet<String>,
}

/// Proof scope, not a runtime value identity. The normal-value owner has
/// already intersected these observations over every normal evaluation path.
/// Expression joins may retain one shared statement batch; state rebinding
/// starts a new environment and never transports this scope.
#[derive(Debug, Default, Clone, PartialEq)]
pub(super) struct StatementNumericObservations {
    /// The invalid state handle denotes absent observations.
    state: symbols::SymbolHandle,
    statement_index: usize,
    pub(super) integer_operations: Vec<crate::CapturedIntegerOperation>,
    pub(super) scalar_associations:
        Vec<crate::proof_contracts::contract_entailment::CapturedScalarAssociation>,
}

/// The IEEE portion of one Boolean polarity: the ordered float enclosure,
/// non-NaN identities and alias groups the evaluated condition established.
/// A Boolean snapshot keeps these beside its integer relations, so narrowing a
/// guard through a Boolean subject retains exactly the evidence the direct
/// comparison would have supplied. Without them a checked IEEE guard reached
/// through an evaluated Boolean subject lost its interval and non-NaN proof.
#[derive(Debug, Default, Clone, PartialEq)]
pub(in crate::proof_contracts::arithmetic_domains) struct IeeeConditionalFacts {
    pub(in crate::proof_contracts::arithmetic_domains) intervals: BTreeMap<String, FloatInterval>,
    pub(in crate::proof_contracts::arithmetic_domains) non_nan: BTreeSet<String>,
    pub(in crate::proof_contracts::arithmetic_domains) aliases: BTreeSet<(String, String)>,
}

impl IeeeConditionalFacts {
    pub(in crate::proof_contracts::arithmetic_domains) fn is_empty(&self) -> bool {
        self.intervals.is_empty() && self.non_nan.is_empty() && self.aliases.is_empty()
    }

    /// A completed scalar copy records the same current value on both sides.
    /// Preserve each conditional observation on the copied value before a later
    /// write withdraws the source path. This never re-evaluates the initializer.
    fn connect_float_aliases(&mut self, left: &[String], right: &[String]) {
        let paths = || left.iter().chain(right);
        let interval = paths()
            .filter_map(|path| self.intervals.get(path).copied())
            .reduce(FloatInterval::intersect);
        let is_non_nan = paths().any(|path| self.non_nan.contains(path));
        if let Some(interval) = interval {
            for path in paths() {
                self.intervals.insert(path.clone(), interval);
            }
        }
        if is_non_nan {
            self.non_nan.extend(paths().cloned());
        }
        for left in left {
            for right in right {
                self.aliases
                    .insert(canonical_path_pair(left.clone(), right.clone()));
            }
        }
    }

    /// Re-establish these facts on a consumer environment. Alias groups land
    /// first so the interval narrowing below fans out through them, and the
    /// non-NaN identities last because they carry no interval of their own.
    pub(in crate::proof_contracts::arithmetic_domains) fn apply_to(
        &self,
        environment: &mut ValueEnvironment,
    ) {
        for (left, right) in &self.aliases {
            environment.mark_float_alias(left.clone(), right.clone());
        }
        for (path, interval) in &self.intervals {
            environment.narrow_float(path.clone(), *interval);
        }
        for path in &self.non_nan {
            environment.mark_non_nan(path.clone());
        }
    }

    /// Withdraw every fact about a path a later write may change: its
    /// enclosure, its non-NaN identity and every alias pair through it.
    pub(in crate::proof_contracts::arithmetic_domains) fn retain(
        &mut self,
        keep_path: impl Fn(&str) -> bool,
    ) {
        self.intervals.retain(|path, _| keep_path(path));
        self.non_nan.retain(|path| keep_path(path));
        self.aliases
            .retain(|(left, right)| keep_path(left) && keep_path(right));
    }

    /// The facts two polarities share at a join: an enclosure survives only
    /// where both sides hold one, at their union, and identities and alias
    /// pairs survive only when both sides kept them.
    fn intersect(&self, other: &Self) -> Self {
        let mut joined = Self::default();
        for (path, interval) in &self.intervals {
            if let Some(other_interval) = other.intervals.get(path) {
                joined
                    .intervals
                    .insert(path.clone(), interval.union(*other_interval));
            }
        }
        joined.non_nan = self.non_nan.intersection(&other.non_nan).cloned().collect();
        joined.aliases = self.aliases.intersection(&other.aliases).cloned().collect();
        joined
    }

    /// Translate only explicitly bound paths, by the same rule the enclosing
    /// environment's own IEEE facts use.
    pub(in crate::proof_contracts::arithmetic_domains) fn rebound(
        &self,
        paths: &impl Fn(&str) -> Vec<String>,
    ) -> Self {
        let mut rebound = Self::default();
        for (path, interval) in &self.intervals {
            for path in paths(path) {
                rebound.intervals.insert(path, *interval);
            }
        }
        for path in &self.non_nan {
            rebound.non_nan.extend(paths(path));
        }
        for (left, right) in &self.aliases {
            for left in paths(left) {
                for right in paths(right) {
                    rebound
                        .aliases
                        .insert(canonical_path_pair(left.clone(), right));
                }
            }
        }
        rebound
    }
}

impl ValueEnvironment {
    pub(crate) fn clear_numeric_observations(&mut self) {
        self.numeric_observations = StatementNumericObservations::default();
    }

    /// Both statement checking and collection-bound arrivals consume the same
    /// prepared normal-value batch. Copies borrow these final-prefix rows;
    /// they never re-enter the collector for each expression or destination.
    pub(crate) fn prepare_numeric_observations(
        &mut self,
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        statement_index: usize,
        statement: &typed_trees::statement::StatementNode,
    ) {
        self.clear_numeric_observations();
        let binding = matches!(
            statement,
            typed_trees::statement::StatementNode::LocalData(_)
                | typed_trees::statement::StatementNode::Assignment(_)
        );
        let needs_observations = binding
            || program
                .tables
                .expression_roots(statement)
                .into_iter()
                .any(|(_, root)| {
                    let mut expressions = Vec::new();
                    crate::expression_traversal::collect_expression_tree(
                        program,
                        root,
                        &mut expressions,
                    );
                    expressions.iter().any(|expression| {
                        program
                            .expression_table
                            .binary_node(*expression)
                            .is_some_and(|binary| {
                                binary.operator == typed_trees::expression::BinaryOperator::Subtract
                            })
                    })
                });
        if needs_observations {
            let observations = crate::captured_numeric_observations(
                program,
                machine,
                state,
                statement_index,
                false,
                crate::NumericObservationDemand {
                    scalar_values: false,
                    integer_operations: true,
                    scalar_associations: true,
                },
            );
            self.numeric_observations = StatementNumericObservations {
                state: state.symbol,
                statement_index,
                integer_operations: observations.integer_operations,
                scalar_associations: observations.scalar_associations,
            };
        }
    }

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
        let paths = |path: &str| rebound_paths(path, bindings);
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
            float_aliases, joint_multiply_bounds, signed_joint_multiply_lower_bounds,
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
        self.clear_numeric_observations();
        self.integer_ranges.clear();
        self.boolean_values.clear();
        self.intervals.clear();
        self.known_u64_values.clear();
        self.float_intervals.clear();
        self.non_nan.clear();
        self.float_aliases.clear();
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
    /// and aggregate content paths are excluded. Ordered facts may separately
    /// retain a private descriptor's metadata, never its referent contents.
    /// Control-flow joins must still use their ordinary merge or
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
        self.float_aliases
            .retain(|(left, right)| retained(left) && retained(right));
        self.ordered_values.retain(|relation| {
            relation.left.is_immutable_snapshot(program, state)
                && relation.right.is_immutable_snapshot(program, state)
        });
        self.boolean_values.retain_mut(|value| {
            if !value.binding.is_immutable_snapshot(program, state) {
                return false;
            }
            for alternative in &mut value.alternatives {
                alternative.relations.retain(|relation| {
                    relation.left.is_immutable_snapshot(program, state)
                        && relation.right.is_immutable_snapshot(program, state)
                });
                alternative.ieee.retain(|path| retained(path));
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

    /// Call frames describe referent writes coarsely. A separately proven
    /// private descriptor home can keep metadata, never its backing contents.
    pub(crate) fn invalidate_call_paths(
        &mut self,
        program: &dyn crate::CheckingInput,
        state: Option<&State>,
        written: &[String],
    ) {
        self.invalidate_storage(written, |operand| {
            operand.survives_storage_writes(program, state, written)
        });
    }

    fn invalidate_storage(
        &mut self,
        written: &[String],
        survives: impl Fn(&ordered_values::Operand) -> bool,
    ) {
        let overlaps = |path: &str| {
            written
                .iter()
                .any(|written| place_paths_overlap(path, written))
        };
        self.boolean_values.retain_mut(|value| {
            if !survives(&value.binding) {
                return false;
            }
            for alternative in &mut value.alternatives {
                alternative
                    .relations
                    .retain(|relation| survives(&relation.left) && survives(&relation.right));
                alternative.ieee.retain(|path| !overlaps(path));
            }
            true
        });
        self.ordered_values
            .retain(|relation| survives(&relation.left) && survives(&relation.right));
        let keep_pair = |(left, right): &(String, String)| !overlaps(left) && !overlaps(right);
        self.integer_ranges.retain(|value| !overlaps(&value.path));
        self.intervals.retain(|path, _| !overlaps(path));
        self.known_u64_values.retain(|path, _| !overlaps(path));
        self.float_intervals.retain(|path, _| !overlaps(path));
        self.non_nan.retain(|path| !overlaps(path));
        self.float_aliases.retain(keep_pair);
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

    fn narrow_float_single(&mut self, path: String, interval: FloatInterval) {
        let merged = match self.float_intervals.get(&path) {
            Some(existing) => existing.intersect(interval),
            None => interval,
        };
        self.float_intervals.insert(path, merged);
    }

    pub(super) fn float_alias_partners<'a>(
        &'a self,
        target: &'a str,
    ) -> impl Iterator<Item = String> + 'a {
        self.float_aliases.iter().filter_map(move |(left, right)| {
            if left == target {
                Some(right.clone())
            } else if right == target {
                Some(left.clone())
            } else {
                None
            }
        })
    }

    pub(super) fn narrow_float(&mut self, path: String, interval: FloatInterval) {
        let aliases: Vec<String> = self.float_alias_partners(&path).collect();
        self.narrow_float_single(path, interval);
        for alias in aliases {
            self.narrow_float_single(alias, interval);
        }
    }

    pub(super) fn mark_non_nan(&mut self, path: String) {
        let aliases: Vec<String> = self.float_alias_partners(&path).collect();
        self.non_nan.insert(path);
        self.non_nan.extend(aliases);
    }

    pub(super) fn mark_float_alias(&mut self, left: String, right: String) {
        if left == right || place_paths_overlap(&left, &right) {
            return;
        }
        let mut left_group = vec![left.clone()];
        left_group.extend(self.float_alias_partners(&left));
        let mut right_group = vec![right.clone()];
        right_group.extend(self.float_alias_partners(&right));
        for left_member in &left_group {
            for right_member in &right_group {
                if left_member != right_member && !place_paths_overlap(left_member, right_member) {
                    self.float_aliases.insert(canonical_path_pair(
                        left_member.clone(),
                        right_member.clone(),
                    ));
                }
            }
        }
        // The copy establishes equality now, including conditional facts
        // saved by earlier Boolean evaluations. Later writes invalidate each
        // receiving path independently through the ordinary storage fence.
        for value in &mut self.boolean_values {
            for facts in &mut value.alternatives {
                facts.ieee.connect_float_aliases(&left_group, &right_group);
            }
        }
        let combined_interval = match (
            self.float_intervals.get(&left).copied(),
            self.float_intervals.get(&right).copied(),
        ) {
            (Some(left_interval), Some(right_interval)) => {
                Some(left_interval.intersect(right_interval))
            }
            (left_interval, right_interval) => left_interval.or(right_interval),
        };
        let is_non_nan = self.non_nan.contains(&left) || self.non_nan.contains(&right);
        if let Some(interval) = combined_interval {
            self.narrow_float(left.clone(), interval);
        }
        if is_non_nan {
            self.mark_non_nan(left);
        }
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

    /// Snapshot the IEEE facts this environment currently holds, for a Boolean
    /// snapshot that must re-establish them under one polarity later.
    pub(super) fn ieee_conditional_facts(&self) -> IeeeConditionalFacts {
        IeeeConditionalFacts {
            intervals: self.float_intervals.clone(),
            non_nan: self.non_nan.clone(),
            aliases: self.float_aliases.clone(),
        }
    }

    /// The JOIN of two envs at a control-flow merge: only places tracked in
    /// BOTH survive, each at the UNION of its intervals (the fact that holds
    /// regardless of which path was taken). Used to seed a multi-predecessor
    /// state from its incoming edge guards.
    pub(crate) fn join(&self, other: &ValueEnvironment) -> ValueEnvironment {
        let mut joined = ValueEnvironment::new();
        // A branch merge inside one initializer must not erase the completed
        // result equations its copy will consume. This is the same complete
        // normal-path batch on BOTH predecessors, not a union of arm facts.
        // Different statements, absent evidence, or differing batches grant
        // nothing. rebind() deliberately starts without this occurrence scope.
        if self.numeric_observations.state.is_valid()
            && self.numeric_observations == other.numeric_observations
        {
            joined.numeric_observations = self.numeric_observations.clone();
        }
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
                for (alternative, other) in common.alternatives.iter_mut().zip(&other.alternatives)
                {
                    alternative
                        .relations
                        .retain(|relation| other.relations.contains(relation));
                    alternative.ieee = alternative.ieee.intersect(&other.ieee);
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
        joined.float_aliases.extend(
            self.float_aliases
                .intersection(&other.float_aliases)
                .cloned(),
        );
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
