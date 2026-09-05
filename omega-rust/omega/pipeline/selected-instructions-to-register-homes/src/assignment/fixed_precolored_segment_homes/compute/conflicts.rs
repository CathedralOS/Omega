use std::collections::BTreeSet;

use register_model::{RegisterView, RegisterViewId, ValidatedPhysicalRegisterModel};

use crate::{
    FixedPrecoloredHomeDomainId, FixedPrecoloredSegmentHomeError, FunctionLiveRanges,
    VirtualInterference,
};

use super::{domains::Domain, work::Work};

type DomainPair = (FixedPrecoloredHomeDomainId, FixedPrecoloredHomeDomainId);
type ViewConflict = (
    FixedPrecoloredHomeDomainId,
    RegisterViewId,
    FixedPrecoloredHomeDomainId,
    RegisterViewId,
);

pub(super) struct Conflicts {
    constrained: BTreeSet<DomainPair>,
    views: BTreeSet<ViewConflict>,
}

impl Conflicts {
    pub(super) fn domains(
        &self,
        left: FixedPrecoloredHomeDomainId,
        right: FixedPrecoloredHomeDomainId,
    ) -> bool {
        self.constrained.contains(&ordered_domains(left, right))
    }

    pub(super) fn views(
        &self,
        left_domain: FixedPrecoloredHomeDomainId,
        left_view: RegisterViewId,
        right_domain: FixedPrecoloredHomeDomainId,
        right_view: RegisterViewId,
    ) -> bool {
        self.views.contains(&ordered_views(
            left_domain,
            left_view,
            right_domain,
            right_view,
        ))
    }

    #[cfg(test)]
    pub(super) fn from_rows(constrained: &[DomainPair], views: &[ViewConflict]) -> Self {
        Self {
            constrained: constrained.iter().copied().collect(),
            views: views.iter().copied().collect(),
        }
    }
}

pub(super) fn build(
    function: usize,
    domains: &[Domain],
    ranges: &FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
    work: &mut Work,
) -> Result<Conflicts, FixedPrecoloredSegmentHomeError> {
    let mut constrained = BTreeSet::new();
    let mut views = BTreeSet::new();
    for left_index in 0..domains.len() {
        for right_index in left_index + 1..domains.len() {
            work.pair()?;
            let left = &domains[left_index];
            let right = &domains[right_index];
            if !live_overlap(left, right) || !registers_interfere(left, right, &ranges.interference)
            {
                continue;
            }
            constrained.insert((left.id, right.id));
            for &left_candidate in &left.candidates {
                let left_view = checked_view(function, left, left_candidate, physical)?;
                for &right_candidate in &right.candidates {
                    work.candidate_pair()?;
                    let right_view = checked_view(function, right, right_candidate, physical)?;
                    if footprints_overlap(left_view, right_view) {
                        views.insert((left.id, left_candidate, right.id, right_candidate));
                    }
                }
            }
        }
    }
    Ok(Conflicts { constrained, views })
}

fn live_overlap(left: &Domain, right: &Domain) -> bool {
    left.segments.iter().any(|left| {
        right.segments.iter().any(|right| {
            left.block == right.block && left.start < right.end && right.start < left.end
        })
    })
}

fn registers_interfere(
    left: &Domain,
    right: &Domain,
    interference: &[VirtualInterference],
) -> bool {
    if left.virtual_register == right.virtual_register {
        return false;
    }
    let (lower, higher) = if left.virtual_register < right.virtual_register {
        (left.virtual_register, right.virtual_register)
    } else {
        (right.virtual_register, left.virtual_register)
    };
    interference
        .binary_search(&VirtualInterference { lower, higher })
        .is_ok()
}

fn checked_view<'a>(
    function: usize,
    domain: &Domain,
    candidate: RegisterViewId,
    physical: &'a ValidatedPhysicalRegisterModel,
) -> Result<&'a RegisterView, FixedPrecoloredSegmentHomeError> {
    physical
        .model()
        .views
        .iter()
        .find(|view| view.id == candidate && view.class == domain.class)
        .ok_or(FixedPrecoloredSegmentHomeError::UnknownOrIncompatibleView {
            function,
            register: domain.virtual_register.0,
            view: candidate.0,
        })
}

fn footprints_overlap(left: &RegisterView, right: &RegisterView) -> bool {
    left.units
        .iter()
        .chain(&left.write_units)
        .any(|unit| right.units.contains(unit) || right.write_units.contains(unit))
}

fn ordered_domains(
    left: FixedPrecoloredHomeDomainId,
    right: FixedPrecoloredHomeDomainId,
) -> DomainPair {
    if left < right {
        (left, right)
    } else {
        (right, left)
    }
}

fn ordered_views(
    left_domain: FixedPrecoloredHomeDomainId,
    left_view: RegisterViewId,
    right_domain: FixedPrecoloredHomeDomainId,
    right_view: RegisterViewId,
) -> ViewConflict {
    if left_domain < right_domain {
        (left_domain, left_view, right_domain, right_view)
    } else {
        (right_domain, right_view, left_domain, left_view)
    }
}
