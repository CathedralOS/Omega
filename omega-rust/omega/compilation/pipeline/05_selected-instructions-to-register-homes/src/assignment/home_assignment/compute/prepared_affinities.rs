//! Attempt-local copy-affinity incidence prepared from canonical live-range rows.

use selected_instructions::{CopyAffinity, VirtualRegisterId};

#[derive(Clone, Copy)]
pub(super) struct DomainAffinity {
    pub(super) partner: VirtualRegisterId,
    pub(super) partner_domain: Option<usize>,
}

pub(super) struct PreparedAffinities {
    by_domain: Vec<Vec<DomainAffinity>>,
}

impl PreparedAffinities {
    pub(super) fn new(
        domain_count: usize,
        domain_of: &[u32],
        affinities: &[CopyAffinity],
    ) -> Self {
        let lookup = |id: VirtualRegisterId| -> Option<usize> {
            domain_of
                .get(id.0 as usize)
                .copied()
                .filter(|&d| d != u32::MAX)
                .map(|d| d as usize)
        };
        let mut by_domain = vec![Vec::new(); domain_count];
        // Placement repeatedly asks only about edges incident to one domain.
        // Retain the canonical input order and multiplicity once per distinct
        // endpoint domain instead of searching every unrelated affinity for
        // every candidate view. A same-domain edge keeps the producer's
        // source-first interpretation, while an absent endpoint stays an
        // ordinary unreachable partner rather than becoming a new error.
        for affinity in affinities {
            let source_domain = lookup(affinity.source);
            let destination_domain = lookup(affinity.destination);
            if let Some(source_domain) = source_domain {
                by_domain[source_domain].push(DomainAffinity {
                    partner: affinity.destination,
                    partner_domain: destination_domain,
                });
            }
            if let Some(destination_domain) = destination_domain
                && Some(destination_domain) != source_domain
            {
                by_domain[destination_domain].push(DomainAffinity {
                    partner: affinity.source,
                    partner_domain: source_domain,
                });
            }
        }
        Self { by_domain }
    }

    pub(super) fn get_domain_affinities(&self, domain: usize) -> &[DomainAffinity] {
        &self.by_domain[domain]
    }
}
