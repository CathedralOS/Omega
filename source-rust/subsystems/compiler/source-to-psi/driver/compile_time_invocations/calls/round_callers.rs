//! Freeze the callers of one evaluation round together.
//!
//! Every call a compile-time value makes is judged in a copy of the round's
//! program with one generated caller added. Copying the program and
//! preparing it for lowering (stage 04) cost the same for every call, and the
//! values of one round do not read each other's results. So the round adds
//! every caller it can name in advance to one copy and freezes that copy
//! once. Each call is still lowered, checked, published, executed and
//! charged alone, with only its own caller as the entry
//! (`PreparedBuildMachineProgram::judge_shared`).
//!
//! Sharing is only a saving, never a different answer. A caller that cannot
//! be added, a copy that does not freeze, or a call whose shared judgment
//! fails is judged again alone, exactly as it would have been without the
//! round, so every refusal keeps its own message and its place in the order.

use std::sync::atomic::{AtomicBool, Ordering};

use diagnostics::Diagnostic;
use symbols::SymbolHandle;

use super::typed_call::TypedCall;
use crate::compile_time_invocations::BuildTimeValue;
use crate::driver::judge_lowered_source::SettledSource;
use crate::source_state::TypedTrees;
use crate::source_state::callable_signature_shapes::CallableDeclaration;
use crate::source_state::expression::{StaticMachineArgument, TableCallExpression};
use prepared_typed_trees_to_lowered_program::PreparedTypedTrees;

/// The declaration a planned call reaches.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum PlannedCallee {
    /// A machine called directly.
    Machine(SymbolHandle),
    /// The public requirement of a trait, called through a selected
    /// implementation.
    Requirement(SymbolHandle),
}

/// Everything the generated caller of one call is built from. Two calls with
/// equal plans get equal callers, so a plan made before the round finds the
/// caller frozen for it when the value is evaluated.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PlannedCall {
    pub(super) requester: SymbolHandle,
    pub(super) callee: PlannedCallee,
    pub(super) call: TableCallExpression,
    pub(super) arguments: Vec<BuildTimeValue>,
    pub(super) selected: Option<(StaticMachineArgument, SymbolHandle)>,
    /// The deployment whose layout facts the call carries. Calls for
    /// different deployments are never frozen together.
    pub(super) target: Option<layout_plans::LayoutTargetFacts>,
}

impl PlannedCall {
    /// This call made for `target`'s deployment.
    pub(crate) fn for_target(mut self, target: &layout_plans::LayoutTargetFacts) -> Self {
        self.target = Some(target.clone());
        self
    }

    pub(super) fn declaration<'source>(
        &self,
        source: &'source TypedTrees,
    ) -> Result<CallableDeclaration<'source>, Vec<Diagnostic>> {
        match self.callee {
            PlannedCallee::Machine(machine) => Ok(CallableDeclaration::Machine(machine)),
            PlannedCallee::Requirement(requirement) => source
                .trait_machine_signature(requirement)
                .map(|(_, signature)| CallableDeclaration::Signature(signature))
                .ok_or_else(|| {
                    super::invocation_error(
                        "selected source invocation needs a public requirement declaration",
                    )
                }),
        }
    }
}

/// The callers of one round frozen together, one copy per deployment.
pub(super) struct RoundCallers {
    groups: Vec<CallerGroup>,
}

struct CallerGroup {
    target: Option<layout_plans::LayoutTargetFacts>,
    frozen: PreparedTypedTrees,
    callers: Vec<SharedCaller>,
}

struct SharedCaller {
    plan: PlannedCall,
    typed: TypedCall,
    /// Set when a value takes this caller. A caller is judged for at most
    /// one value, so two values with equal plans still get a caller each.
    taken: AtomicBool,
}

impl RoundCallers {
    /// Adds each plan's caller to one copy of `source` per deployment and
    /// freezes each copy. `add` builds one caller into a copy.
    ///
    /// A copy holding fewer than two callers saves nothing, so its calls
    /// stay alone. When a caller cannot be added, its partial caller would
    /// stay in the copy, so the copy is rebuilt without that plan.
    pub(super) fn freeze(
        source: &TypedTrees,
        plans: Vec<PlannedCall>,
        mut add: impl FnMut(&mut TypedTrees, &PlannedCall) -> Result<TypedCall, Vec<Diagnostic>>,
    ) -> Self {
        let mut partitions: Vec<(Option<layout_plans::LayoutTargetFacts>, Vec<PlannedCall>)> =
            Vec::new();
        for plan in plans {
            match partitions
                .iter_mut()
                .find(|(target, _)| *target == plan.target)
            {
                Some((_, members)) => members.push(plan),
                None => partitions.push((plan.target.clone(), vec![plan])),
            }
        }
        let mut groups = Vec::with_capacity(partitions.len());
        for (target, mut members) in partitions {
            while members.len() > 1 {
                let mut probe = source.clone();
                let mut typed = Vec::with_capacity(members.len());
                let refused = members.iter().position(|plan| match add(&mut probe, plan) {
                    Ok(caller) => {
                        typed.push(caller);
                        false
                    }
                    Err(_) => true,
                });
                if let Some(position) = refused {
                    members.remove(position);
                    continue;
                }
                // A copy that does not freeze leaves every call to be judged
                // alone, where each one reports its own preparation refusal.
                if let Ok(frozen) = SettledSource::materialized_call(probe).freeze() {
                    groups.push(CallerGroup {
                        target,
                        frozen: frozen.program,
                        callers: members
                            .into_iter()
                            .zip(typed)
                            .map(|(plan, typed)| SharedCaller {
                                plan,
                                typed,
                                taken: AtomicBool::new(false),
                            })
                            .collect(),
                    });
                }
                break;
            }
        }
        Self { groups }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    /// Takes the frozen caller planned for `plan`, if the round froze one
    /// that no other value has taken.
    pub(super) fn take(&self, plan: &PlannedCall) -> Option<(PreparedTypedTrees, TypedCall)> {
        let group = self
            .groups
            .iter()
            .find(|group| group.target == plan.target)?;
        let caller = group.callers.iter().find(|caller| {
            caller.plan == *plan
                && caller
                    .taken
                    .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
                    .is_ok()
        })?;
        Some((group.frozen.clone(), caller.typed.clone()))
    }

    /// How many callers each frozen copy holds, in the order the copies were
    /// made. Tests read this to see what the round shared.
    #[cfg(test)]
    pub(super) fn group_sizes(&self) -> Vec<(Option<String>, usize)> {
        self.groups
            .iter()
            .map(|group| {
                (
                    group.target.as_ref().map(|facts| facts.profile.clone()),
                    group.callers.len(),
                )
            })
            .collect()
    }
}
