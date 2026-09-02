//! Sorted fixed-use and transition rows used only by canonical replay.

use std::collections::BTreeSet;

use omega_register_model::{RegisterOperandAccess, RegisterViewId};

use crate::{
    EntryFixedViewTransition, FixedPrecoloredInterval, FixedPrecoloredSplitRequirementError,
    VirtualFixedConstraintSite, VirtualPointLegality,
};

type SiteKey = (u8, u32, u32, u32, u16, u8);
type TransitionKey = (SiteKey, u16, u16);

pub(super) struct CutRows<'a> {
    fixed: Vec<((u32, u32), &'a FixedPrecoloredInterval)>,
    transitions: Vec<TransitionKey>,
    used: BTreeSet<TransitionKey>,
}

impl<'a> CutRows<'a> {
    pub(super) fn collect(
        fixed: &[&'a FixedPrecoloredInterval],
        transitions: &[EntryFixedViewTransition],
    ) -> Self {
        let mut fixed = fixed
            .iter()
            .map(|row| ((row.block.0, row.start.0), *row))
            .collect::<Vec<_>>();
        fixed.sort_by_key(|row| row.0);
        let mut transitions = transitions
            .iter()
            .map(|row| (site_key(row.to_site), row.to_view.0, row.from_view.0))
            .collect::<Vec<_>>();
        transitions.sort_unstable();
        Self {
            fixed,
            transitions,
            used: BTreeSet::new(),
        }
    }

    pub(super) fn boundary(
        &self,
        function: usize,
        register: u32,
        point: &VirtualPointLegality,
    ) -> Result<(VirtualFixedConstraintSite, RegisterViewId), FixedPrecoloredSplitRequirementError>
    {
        let key = (point.block.0, point.point.0);
        let start = self.fixed.partition_point(|row| row.0 < key);
        let end = self.fixed.partition_point(|row| row.0 <= key);
        if start == end {
            return unauthenticated(function, register, point.point.0);
        }
        if end - start != 1 {
            return Err(
                FixedPrecoloredSplitRequirementError::AmbiguousFixedCutSite {
                    function,
                    register,
                    point: point.point.0,
                },
            );
        }
        let row = self.fixed[start].1;
        let VirtualFixedConstraintSite::Operand {
            instruction,
            operand,
            access,
            ..
        } = row.site
        else {
            return unauthenticated(function, register, point.point.0);
        };
        if !matches!(access, RegisterOperandAccess::Use) {
            return Err(
                FixedPrecoloredSplitRequirementError::UnsupportedFixedTransitionAccess {
                    function,
                    register,
                    instruction: instruction.0,
                    operand,
                },
            );
        }
        if point.candidates.len() != 1 || point.candidates[0] != row.view {
            return unauthenticated(function, register, point.point.0);
        }
        Ok((row.site, row.view))
    }

    pub(super) fn require_transition(
        &mut self,
        function: usize,
        register: u32,
        source: &BTreeSet<RegisterViewId>,
        site: VirtualFixedConstraintSite,
        destination: RegisterViewId,
    ) -> Result<(), FixedPrecoloredSplitRequirementError> {
        if self.mark_transition(source, site, destination) {
            Ok(())
        } else {
            unauthenticated(function, register, site_point(site))
        }
    }

    pub(super) fn mark_transition(
        &mut self,
        source: &BTreeSet<RegisterViewId>,
        site: VirtualFixedConstraintSite,
        destination: RegisterViewId,
    ) -> bool {
        let prefix = (site_key(site), destination.0);
        let start = self
            .transitions
            .partition_point(|row| (row.0, row.1) < prefix);
        let end = self
            .transitions
            .partition_point(|row| (row.0, row.1) <= prefix);
        let matches = self.transitions[start..end]
            .iter()
            .copied()
            .filter(|row| source.contains(&RegisterViewId(row.2)))
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return false;
        }
        self.used.insert(matches[0]);
        true
    }

    pub(super) fn finish(
        self,
        function: usize,
        register: u32,
    ) -> Result<(), FixedPrecoloredSplitRequirementError> {
        if self.used.len() == self.transitions.len() {
            Ok(())
        } else {
            Err(
                FixedPrecoloredSplitRequirementError::UnsupportedCrossBlockRange {
                    function,
                    register,
                },
            )
        }
    }
}

fn unauthenticated<T>(
    function: usize,
    register: u32,
    point: u32,
) -> Result<T, FixedPrecoloredSplitRequirementError> {
    Err(
        FixedPrecoloredSplitRequirementError::UnauthenticatedDomainBreak {
            function,
            register,
            point,
        },
    )
}

fn site_key(site: VirtualFixedConstraintSite) -> SiteKey {
    match site {
        VirtualFixedConstraintSite::Entry => (0, 0, 0, 0, 0, 0),
        VirtualFixedConstraintSite::Operand {
            position,
            point,
            instruction,
            operand,
            access,
        } => (
            1,
            position.0,
            point.0,
            instruction.0,
            operand,
            match access {
                RegisterOperandAccess::Use => 0,
                RegisterOperandAccess::Def => 1,
                RegisterOperandAccess::UseDef => 2,
            },
        ),
    }
}

fn site_point(site: VirtualFixedConstraintSite) -> u32 {
    match site {
        VirtualFixedConstraintSite::Entry => 0,
        VirtualFixedConstraintSite::Operand { point, .. } => point.0,
    }
}
