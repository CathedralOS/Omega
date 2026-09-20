//! The copy-placement partition the shared-exit legs make, shared by
//! production, independent replay, and work accounting so all three commit
//! the same copy count. Authenticated boundaries that leave one source
//! segment end — same register, source segment and domain, and the same
//! crossed views — through connector evidence agreeing on the exit block
//! and its terminator instruction form one emission whose copy dominates
//! every member site. Every other boundary is its own emission and copies
//! at its fixed-use site.

use selected_instructions::LiveRangeEdgeConnector;

use crate::VirtualFixedConstraintSite;

use super::evidence::AuthenticatedFixedViewBoundary;

/// One emitted copy's boundary membership: `members` in destination order —
/// site instruction, then operand index — and `exit`, the dominating
/// insertion point every member's connector evidence agrees on. `None`
/// marks a lone boundary that copies in its site's own block.
pub(crate) struct CopyEmission<'a> {
    pub(crate) members: Vec<&'a AuthenticatedFixedViewBoundary>,
    pub(crate) exit: Option<LiveRangeEdgeConnector>,
}

/// Partition the boundaries into emitted copies in first-appearance order.
/// Two members share an exit only when both depart through connectors out
/// of the same block's terminator — the end of their common source segment,
/// which dominates every member destination under the fragment tree.
pub(crate) fn copy_emissions<'a>(
    boundaries: &[&'a AuthenticatedFixedViewBoundary],
) -> Vec<CopyEmission<'a>> {
    let mut groups: Vec<Vec<&'a AuthenticatedFixedViewBoundary>> = Vec::new();
    for boundary in boundaries.iter().copied() {
        if let Some(group) = groups.iter_mut().find(|group| {
            let first = group[0];
            first.function == boundary.function
                && first.virtual_register == boundary.virtual_register
                && first.source_segment == boundary.source_segment
                && first.source_domain == boundary.source_domain
                && first.from_view == boundary.from_view
                && first.to_view == boundary.to_view
        }) {
            group.push(boundary);
        } else {
            groups.push(vec![boundary]);
        }
    }
    let mut emissions = Vec::with_capacity(groups.len());
    for mut members in groups {
        let exit = if members.len() > 1 {
            members
                .iter()
                .map(|member| member.incoming)
                .collect::<Option<Vec<_>>>()
                .and_then(|connectors| {
                    let first = connectors[0];
                    connectors
                        .iter()
                        .all(|connector| {
                            connector.source == first.source
                                && connector.terminator == first.terminator
                        })
                        .then_some(first)
                })
        } else {
            None
        };
        if let Some(exit) = exit {
            members.sort_by_key(|member| site_key(member));
            emissions.push(CopyEmission {
                members,
                exit: Some(exit),
            });
        } else {
            emissions.extend(members.into_iter().map(|member| CopyEmission {
                members: vec![member],
                exit: None,
            }));
        }
    }
    emissions
}

fn site_key(boundary: &AuthenticatedFixedViewBoundary) -> (u32, u16) {
    match boundary.site {
        VirtualFixedConstraintSite::Operand {
            instruction,
            operand,
            ..
        } => (instruction.0, operand),
        VirtualFixedConstraintSite::Entry => (u32::MAX, u16::MAX),
    }
}
