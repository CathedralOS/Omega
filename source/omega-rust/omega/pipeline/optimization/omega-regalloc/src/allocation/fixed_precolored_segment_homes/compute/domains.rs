use std::collections::BTreeMap;

use omega_register_model::{RegisterClassId, RegisterViewId};
use omega_selected_instructions::{SelectedBlockId, VirtualRegisterId};

use crate::{
    FixedPrecoloredHomeDomainId, FixedPrecoloredSegmentHomeError, FixedPrecoloredSourceSegmentId,
    FixedPrecoloredSourceSegmentOpening, FunctionFixedPrecoloredSplitRequirements, LiveRangePoint,
};

use super::work::Work;

#[derive(Debug, Clone)]
pub(super) struct Segment {
    pub(super) id: FixedPrecoloredSourceSegmentId,
    pub(super) block: SelectedBlockId,
    pub(super) start: LiveRangePoint,
    pub(super) end: LiveRangePoint,
}

#[derive(Debug, Clone)]
pub(super) struct Domain {
    pub(super) id: FixedPrecoloredHomeDomainId,
    pub(super) virtual_register: VirtualRegisterId,
    pub(super) class: RegisterClassId,
    pub(super) segments: Vec<Segment>,
    pub(super) candidates: Vec<RegisterViewId>,
}

impl Domain {
    pub(super) fn first_point(&self) -> LiveRangePoint {
        self.segments
            .iter()
            .map(|segment| segment.start)
            .min()
            .expect("domain owns a source segment")
    }

    pub(super) fn first_segment(&self) -> FixedPrecoloredSourceSegmentId {
        self.segments
            .iter()
            .map(|segment| segment.id)
            .min()
            .expect("domain owns a source segment")
    }
}

pub(super) fn build(
    function: usize,
    requirements: &FunctionFixedPrecoloredSplitRequirements,
    work: &mut Work,
) -> Result<Vec<Domain>, FixedPrecoloredSegmentHomeError> {
    work.function()?;
    let mut domains = Vec::<Domain>::new();
    for register in &requirements.registers {
        work.register()?;
        let mut closing_by_block = BTreeMap::<SelectedBlockId, usize>::new();
        for fragment in &register.fragments {
            let mut current = None;
            for segment in &fragment.segments {
                work.segment()?;
                let domain = match segment.opening {
                    FixedPrecoloredSourceSegmentOpening::SourceRangeStartV1 => new_domain(
                        function,
                        register.virtual_register,
                        register.class,
                        segment.candidates.clone(),
                        &mut domains,
                        work,
                    )?,
                    FixedPrecoloredSourceSegmentOpening::IncomingSourceEdgeV1 { connector } => {
                        let domain = closing_by_block.get(&connector.source).copied().ok_or(
                            FixedPrecoloredSegmentHomeError::MissingIncomingDomain {
                                function,
                                register: register.virtual_register.0,
                                block: connector.source.0,
                            },
                        )?;
                        intersect(
                            function,
                            register.virtual_register,
                            segment.id,
                            &mut domains[domain].candidates,
                            &segment.candidates,
                        )?;
                        domain
                    }
                    FixedPrecoloredSourceSegmentOpening::IncompatibleFixedUseDomainBoundaryV1 {
                        ..
                    } => new_domain(
                        function,
                        register.virtual_register,
                        register.class,
                        segment.candidates.clone(),
                        &mut domains,
                        work,
                    )?,
                };
                domains[domain].segments.push(Segment {
                    id: segment.id,
                    block: fragment.block,
                    start: segment.start,
                    end: segment.end,
                });
                current = Some(domain);
            }
            let current = current.ok_or(FixedPrecoloredSegmentHomeError::SegmentMismatch {
                function,
                register: register.virtual_register.0,
                segment: 0,
            })?;
            closing_by_block.insert(fragment.block, current);
        }
    }
    Ok(domains)
}

fn new_domain(
    function: usize,
    virtual_register: VirtualRegisterId,
    class: RegisterClassId,
    candidates: Vec<RegisterViewId>,
    domains: &mut Vec<Domain>,
    work: &mut Work,
) -> Result<usize, FixedPrecoloredSegmentHomeError> {
    let raw = u32::try_from(domains.len())
        .map_err(|_| FixedPrecoloredSegmentHomeError::DomainIdentityOverflow { function })?;
    if candidates.is_empty() {
        return Err(FixedPrecoloredSegmentHomeError::EmptyDomain {
            function,
            register: virtual_register.0,
            segment: 0,
        });
    }
    work.domain()?;
    domains.push(Domain {
        id: FixedPrecoloredHomeDomainId(raw),
        virtual_register,
        class,
        segments: Vec::new(),
        candidates,
    });
    Ok(domains.len() - 1)
}

fn intersect(
    function: usize,
    register: VirtualRegisterId,
    segment: FixedPrecoloredSourceSegmentId,
    current: &mut Vec<RegisterViewId>,
    incoming: &[RegisterViewId],
) -> Result<(), FixedPrecoloredSegmentHomeError> {
    current.retain(|candidate| incoming.binary_search(candidate).is_ok());
    if current.is_empty() {
        return Err(FixedPrecoloredSegmentHomeError::EmptyDomain {
            function,
            register: register.0,
            segment: segment.0,
        });
    }
    Ok(())
}
