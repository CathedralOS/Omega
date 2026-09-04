//! Fixed-use cut and legality-transition authentication indexes.

use std::collections::{BTreeMap, BTreeSet};

use omega_register_model::{RegisterOperandAccess, RegisterViewId};

use crate::{
    EntryFixedViewTransition, FixedPrecoloredInterval, FixedPrecoloredSplitRequirementError,
    VirtualFixedConstraintSite, VirtualPointLegality,
};

type SiteKey = (u8, u32, u32, u32, u16, u8);

pub(super) struct CutIndex<'a> {
    fixed: BTreeMap<(u32, u32), Vec<&'a FixedPrecoloredInterval>>,
    transitions: BTreeMap<(SiteKey, u16), BTreeSet<u16>>,
    used: BTreeSet<(SiteKey, u16, u16)>,
    transition_count: usize,
}

impl<'a> CutIndex<'a> {
    pub(super) fn new(
        fixed: &'a [FixedPrecoloredInterval],
        transitions: &[EntryFixedViewTransition],
    ) -> Self {
        let mut fixed_by_point = BTreeMap::<_, Vec<_>>::new();
        for row in fixed {
            fixed_by_point
                .entry((row.block.0, row.start.0))
                .or_default()
                .push(row);
        }
        let mut transition_index = BTreeMap::<_, BTreeSet<_>>::new();
        for row in transitions {
            transition_index
                .entry((site_key(row.to_site), row.to_view.0))
                .or_default()
                .insert(row.from_view.0);
        }
        Self {
            fixed: fixed_by_point,
            transitions: transition_index,
            used: BTreeSet::new(),
            transition_count: transitions.len(),
        }
    }

    pub(super) fn boundary(
        &self,
        function: usize,
        register: u32,
        point: &VirtualPointLegality,
    ) -> Result<(VirtualFixedConstraintSite, RegisterViewId), FixedPrecoloredSplitRequirementError>
    {
        let rows = self.fixed.get(&(point.block.0, point.point.0));
        let Some(rows) = rows else {
            return Err(
                FixedPrecoloredSplitRequirementError::UnauthenticatedDomainBreak {
                    function,
                    register,
                    point: point.point.0,
                },
            );
        };
        if rows.len() != 1 {
            return Err(
                FixedPrecoloredSplitRequirementError::AmbiguousFixedCutSite {
                    function,
                    register,
                    point: point.point.0,
                },
            );
        }
        let row = rows[0];
        let VirtualFixedConstraintSite::Operand {
            instruction,
            operand,
            access,
            ..
        } = row.site
        else {
            return Err(
                FixedPrecoloredSplitRequirementError::UnauthenticatedDomainBreak {
                    function,
                    register,
                    point: point.point.0,
                },
            );
        };
        if access != RegisterOperandAccess::Use {
            return Err(
                FixedPrecoloredSplitRequirementError::UnsupportedFixedTransitionAccess {
                    function,
                    register,
                    instruction: instruction.0,
                    operand,
                },
            );
        }
        if point.candidates.as_slice() != [row.view] {
            return Err(
                FixedPrecoloredSplitRequirementError::UnauthenticatedDomainBreak {
                    function,
                    register,
                    point: point.point.0,
                },
            );
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
        if self.use_transition(source, site, destination) {
            Ok(())
        } else {
            Err(
                FixedPrecoloredSplitRequirementError::UnauthenticatedDomainBreak {
                    function,
                    register,
                    point: site_point(site),
                },
            )
        }
    }

    pub(super) fn use_transition(
        &mut self,
        source: &BTreeSet<RegisterViewId>,
        site: VirtualFixedConstraintSite,
        destination: RegisterViewId,
    ) -> bool {
        let key = (site_key(site), destination.0);
        let Some(from) = self.transitions.get(&key) else {
            return false;
        };
        let mut matching = from
            .iter()
            .copied()
            .filter(|view| source.contains(&RegisterViewId(*view)));
        let Some(chosen) = matching.next() else {
            return false;
        };
        if matching.next().is_some() {
            return false;
        }
        self.used.insert((key.0, key.1, chosen));
        true
    }

    pub(super) fn finish(
        self,
        function: usize,
        register: u32,
    ) -> Result<(), FixedPrecoloredSplitRequirementError> {
        if self.used.len() == self.transition_count {
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
