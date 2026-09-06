//! Flow-sensitive value facts used by arithmetic validation.
//!
//! This module owns the mutable abstract environment and its merge/invalidation
//! rules. Expression analysis and guard recognition remain in the parent.

use std::collections::{BTreeMap, BTreeSet};

use super::*;

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
    const UNBOUNDED: FloatInterval = FloatInterval {
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
pub(crate) struct ValueEnv {
    pub(super) ordered_values: Vec<ordered_values::Relation>,
    pub(super) intervals: BTreeMap<String, Interval>,
    pub(super) known_u64_values: BTreeMap<String, u64>,
    pub(super) float_intervals: BTreeMap<String, FloatInterval>,
    pub(super) non_nan: BTreeSet<String>,
    pub(super) joint_add_upper_bounds: BTreeSet<(String, String)>,
    pub(super) joint_add_lower_bounds: BTreeSet<(String, String)>,
    pub(super) joint_subtract_bounds: BTreeSet<(String, String)>,
    pub(super) signed_joint_subtract_lower_bounds: BTreeSet<(String, String)>,
    pub(super) signed_joint_subtract_upper_bounds: BTreeSet<(String, String)>,
    pub(super) joint_multiply_bounds: BTreeSet<(String, String)>,
    pub(super) signed_joint_multiply_lower_bounds: BTreeSet<(String, String)>,
    pub(super) signed_joint_multiply_upper_bounds: BTreeSet<(String, String)>,
    pub(super) signed_joint_multiply_negation_bounds: BTreeSet<String>,
}

impl ValueEnv {
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
                            .filter(|suffix| suffix.starts_with('.'))
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
        pairs!(true; joint_add_upper_bounds, joint_add_lower_bounds,
            joint_multiply_bounds, signed_joint_multiply_lower_bounds,
            signed_joint_multiply_upper_bounds);
        pairs!(false; joint_subtract_bounds, signed_joint_subtract_lower_bounds,
            signed_joint_subtract_upper_bounds);
        rebound
    }

    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Drop all tracked values (after an opaque effect like a call that may
    /// mutate fields through `&mut`, or when leaving the linear prefix).
    pub(crate) fn clear(&mut self) {
        self.ordered_values.clear();
        self.intervals.clear();
        self.known_u64_values.clear();
        self.float_intervals.clear();
        self.non_nan.clear();
        self.joint_add_upper_bounds.clear();
        self.joint_add_lower_bounds.clear();
        self.joint_subtract_bounds.clear();
        self.signed_joint_subtract_lower_bounds.clear();
        self.signed_joint_subtract_upper_bounds.clear();
        self.joint_multiply_bounds.clear();
        self.signed_joint_multiply_lower_bounds.clear();
        self.signed_joint_multiply_upper_bounds.clear();
        self.signed_joint_multiply_negation_bounds.clear();
    }

    /// Invalidate only facts overlapping a callee's known may-write paths.
    /// A write to a parent invalidates descendants; a write to a descendant
    /// also invalidates any fact recorded for the parent value itself.
    pub(crate) fn invalidate_written_paths(&mut self, written: &[String]) {
        self.ordered_values
            .retain(|relation| relation.survives(written));
        let overlaps = |path: &str| {
            written
                .iter()
                .any(|written| place_paths_overlap(path, written))
        };
        self.intervals.retain(|path, _| !overlaps(path));
        self.known_u64_values.retain(|path, _| !overlaps(path));
        self.float_intervals.retain(|path, _| !overlaps(path));
        self.non_nan.retain(|path| !overlaps(path));
        self.joint_add_upper_bounds
            .retain(|(left, right)| !overlaps(left) && !overlaps(right));
        self.joint_add_lower_bounds
            .retain(|(left, right)| !overlaps(left) && !overlaps(right));
        self.joint_subtract_bounds
            .retain(|(left, right)| !overlaps(left) && !overlaps(right));
        self.signed_joint_subtract_lower_bounds
            .retain(|(left, right)| !overlaps(left) && !overlaps(right));
        self.signed_joint_subtract_upper_bounds
            .retain(|(left, right)| !overlaps(left) && !overlaps(right));
        self.joint_multiply_bounds
            .retain(|(left, right)| !overlaps(left) && !overlaps(right));
        self.signed_joint_multiply_lower_bounds
            .retain(|(left, right)| !overlaps(left) && !overlaps(right));
        self.signed_joint_multiply_upper_bounds
            .retain(|(left, right)| !overlaps(left) && !overlaps(right));
        self.signed_joint_multiply_negation_bounds
            .retain(|value| !overlaps(value));
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
    /// Used by guard narrowing so an arm's guard refines the env without
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

    pub(super) fn mark_joint_add_upper_bound(&mut self, left: String, right: String) {
        self.joint_add_upper_bounds
            .insert(canonical_path_pair(left, right));
    }

    pub(super) fn mark_joint_add_lower_bound(&mut self, left: String, right: String) {
        self.joint_add_lower_bounds
            .insert(canonical_path_pair(left, right));
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

    pub(super) fn proves_joint_add_upper_bound(
        &self,
        program: &TypedTrees,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> bool {
        let Some(left) = place_path(program, left) else {
            return false;
        };
        let Some(right) = place_path(program, right) else {
            return false;
        };
        self.joint_add_upper_bounds
            .contains(&canonical_path_pair(left, right))
    }

    pub(super) fn proves_joint_add_lower_bound(
        &self,
        program: &TypedTrees,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> bool {
        let Some(left) = place_path(program, left) else {
            return false;
        };
        let Some(right) = place_path(program, right) else {
            return false;
        };
        self.joint_add_lower_bounds
            .contains(&canonical_path_pair(left, right))
    }

    pub(super) fn proves_joint_subtract_bound(
        &self,
        program: &TypedTrees,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> bool {
        let Some(left) = place_path(program, left) else {
            return false;
        };
        let Some(right) = place_path(program, right) else {
            return false;
        };
        self.joint_subtract_bounds.contains(&(left, right))
    }

    pub(super) fn proves_signed_joint_subtract_lower_bound(
        &self,
        program: &TypedTrees,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> bool {
        let Some(left) = place_path(program, left) else {
            return false;
        };
        let Some(right) = place_path(program, right) else {
            return false;
        };
        self.signed_joint_subtract_lower_bounds
            .contains(&(left, right))
    }

    pub(super) fn proves_signed_joint_subtract_upper_bound(
        &self,
        program: &TypedTrees,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> bool {
        let Some(left) = place_path(program, left) else {
            return false;
        };
        let Some(right) = place_path(program, right) else {
            return false;
        };
        self.signed_joint_subtract_upper_bounds
            .contains(&(left, right))
    }

    pub(super) fn proves_joint_multiply_bound(
        &self,
        program: &TypedTrees,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> bool {
        let Some(left) = place_path(program, left) else {
            return false;
        };
        let Some(right) = place_path(program, right) else {
            return false;
        };
        self.joint_multiply_bounds
            .contains(&canonical_path_pair(left, right))
    }

    pub(super) fn proves_signed_joint_multiply_bounds(
        &self,
        program: &TypedTrees,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> bool {
        let Some(left) = place_path(program, left) else {
            return false;
        };
        let Some(right) = place_path(program, right) else {
            return false;
        };
        let pair = canonical_path_pair(left, right);
        self.signed_joint_multiply_lower_bounds.contains(&pair)
            && self.signed_joint_multiply_upper_bounds.contains(&pair)
    }

    pub(super) fn proves_signed_joint_multiply_negation_bound(
        &self,
        program: &TypedTrees,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> bool {
        for (value, factor) in [(left, right), (right, left)] {
            let (Some(value), Some(factor)) =
                (place_path(program, value), place_path(program, factor))
            else {
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
    pub(crate) fn join(&self, other: &ValueEnv) -> ValueEnv {
        let mut joined = ValueEnv::new();
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
        joined.joint_add_upper_bounds.extend(
            self.joint_add_upper_bounds
                .intersection(&other.joint_add_upper_bounds)
                .cloned(),
        );
        joined.joint_add_lower_bounds.extend(
            self.joint_add_lower_bounds
                .intersection(&other.joint_add_lower_bounds)
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
