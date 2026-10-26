//! Attempt-local copy-affinity incidence prepared from canonical live-range rows.

use selected_instructions::{CopyAffinity, VirtualRegisterId};

#[derive(Clone, Copy)]
pub(super) struct DomainAffinity {
    pub(super) partner: VirtualRegisterId,
    pub(super) partner_domain: Option<usize>,
}

pub(super) struct PreparedAffinities {
    offsets: Vec<u32>,
    entries: Vec<DomainAffinity>,
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
        let mut counts = vec![0u32; domain_count];
        for affinity in affinities {
            let source_domain = lookup(affinity.source);
            let destination_domain = lookup(affinity.destination);
            if let Some(source_domain) = source_domain {
                counts[source_domain] += 1;
            }
            if let Some(destination_domain) = destination_domain
                && Some(destination_domain) != source_domain
            {
                counts[destination_domain] += 1;
            }
        }
        let mut offsets = vec![0u32; domain_count + 1];
        for i in 0..domain_count {
            offsets[i + 1] = offsets[i] + counts[i];
        }
        let mut cursors = offsets.clone();
        let mut entries = vec![
            DomainAffinity {
                partner: VirtualRegisterId(0),
                partner_domain: None,
            };
            offsets[domain_count] as usize
        ];
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
                let sc = &mut cursors[source_domain];
                entries[*sc as usize] = DomainAffinity {
                    partner: affinity.destination,
                    partner_domain: destination_domain,
                };
                *sc += 1;
            }
            if let Some(destination_domain) = destination_domain
                && Some(destination_domain) != source_domain
            {
                let dc = &mut cursors[destination_domain];
                entries[*dc as usize] = DomainAffinity {
                    partner: affinity.source,
                    partner_domain: source_domain,
                };
                *dc += 1;
            }
        }
        Self { offsets, entries }
    }

    #[inline]
    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(super) fn get_domain_affinities(&self, domain: usize) -> &[DomainAffinity] {
        let start = self.offsets[domain] as usize;
        let end = self.offsets[domain + 1] as usize;
        &self.entries[start..end]
    }
}
