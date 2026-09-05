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

pub(super) struct ConflictIndex {
    domain_pairs: BTreeSet<DomainPair>,
    view_pairs: BTreeSet<ViewConflict>,
}

impl ConflictIndex {
    pub(super) fn domains(
        &self,
        left: FixedPrecoloredHomeDomainId,
        right: FixedPrecoloredHomeDomainId,
    ) -> bool {
        self.domain_pairs.contains(&order_domains(left, right))
    }
    pub(super) fn views(
        &self,
        left_domain: FixedPrecoloredHomeDomainId,
        left_view: RegisterViewId,
        right_domain: FixedPrecoloredHomeDomainId,
        right_view: RegisterViewId,
    ) -> bool {
        self.view_pairs.contains(&order_views(
            left_domain,
            left_view,
            right_domain,
            right_view,
        ))
    }
}

pub(super) fn reconstruct(
    function: usize,
    domains: &[Domain],
    ranges: &FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
    work: &mut Work,
) -> Result<ConflictIndex, FixedPrecoloredSegmentHomeError> {
    let mut domain_pairs = BTreeSet::new();
    let mut view_pairs = BTreeSet::new();
    for (left_position, left) in domains.iter().enumerate() {
        for right in domains.iter().skip(left_position + 1) {
            work.pair()?;
            if !overlaps(left, right) || !interferes(left, right, &ranges.interference) {
                continue;
            }
            domain_pairs.insert((left.id, right.id));
            for &left_candidate in &left.candidates {
                let left_view = view(function, left, left_candidate, physical)?;
                for &right_candidate in &right.candidates {
                    work.candidate_pair()?;
                    let right_view = view(function, right, right_candidate, physical)?;
                    if aliases(left_view, right_view) {
                        view_pairs.insert((left.id, left_candidate, right.id, right_candidate));
                    }
                }
            }
        }
    }
    Ok(ConflictIndex {
        domain_pairs,
        view_pairs,
    })
}

fn overlaps(left: &Domain, right: &Domain) -> bool {
    left.segments.iter().any(|left| {
        right.segments.iter().any(|right| {
            left.block == right.block && left.start < right.end && right.start < left.end
        })
    })
}

fn interferes(left: &Domain, right: &Domain, rows: &[VirtualInterference]) -> bool {
    if left.virtual_register == right.virtual_register {
        return false;
    }
    let (lower, higher) = if left.virtual_register < right.virtual_register {
        (left.virtual_register, right.virtual_register)
    } else {
        (right.virtual_register, left.virtual_register)
    };
    rows.binary_search(&VirtualInterference { lower, higher })
        .is_ok()
}

fn view<'a>(
    function: usize,
    domain: &Domain,
    id: RegisterViewId,
    physical: &'a ValidatedPhysicalRegisterModel,
) -> Result<&'a RegisterView, FixedPrecoloredSegmentHomeError> {
    physical
        .model()
        .views
        .iter()
        .find(|view| view.id == id && view.class == domain.class)
        .ok_or(FixedPrecoloredSegmentHomeError::UnknownOrIncompatibleView {
            function,
            register: domain.virtual_register.0,
            view: id.0,
        })
}

fn aliases(left: &RegisterView, right: &RegisterView) -> bool {
    left.units
        .iter()
        .chain(&left.write_units)
        .any(|unit| right.units.contains(unit) || right.write_units.contains(unit))
}

fn order_domains(
    left: FixedPrecoloredHomeDomainId,
    right: FixedPrecoloredHomeDomainId,
) -> DomainPair {
    if left < right {
        (left, right)
    } else {
        (right, left)
    }
}

fn order_views(
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
