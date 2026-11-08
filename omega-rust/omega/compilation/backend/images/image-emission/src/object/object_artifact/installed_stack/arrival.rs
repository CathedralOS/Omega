//! Compose the finite arrival histories admitted for one installation.
//!
//! Work may first enter any configured handler. Once its exact body and entry
//! transition are fault-free, only NMI and machine check can interrupt the
//! masked handler. The explicit delivery condition and configuration lease
//! preserve NMI blocking and MCIP, so neither can recur on that fatal lineage.
//! This computes conditional demand; it does not establish those premises.

use crate::FunctionFragmentObjectArtifactError as Error;
use calling_conventions::EntryStack;
use installation_evidence::InstalledPhysicalStackRoot;
use installation_evidence::{InstalledArrivalStackDemand, InstalledInterruptStackRequirement};
use provider_admission::{AdmittedInstalledInterruptEntry, AdmittedInstalledInterruptScope};

struct Entry {
    placement: usize,
    vector: u16,
    domain: Option<u16>,
    bytes: u64,
    alignment: u32,
}

/// Each requirement belongs to the exact complete Scope roster. A physical
/// entry is only a proposed destination for the root reserve here; final entry
/// publication must bind and provision it before the object can execute.
pub fn compose(
    scope: &AdmittedInstalledInterruptScope,
    entries: &[&AdmittedInstalledInterruptEntry],
    requirements: &mut [InstalledInterruptStackRequirement],
    physical_entry: Option<InstalledPhysicalStackRoot>,
) -> Result<(), Error> {
    if scope.fatal_entries().is_empty() {
        if requirements.iter().any(|row| row.arrival.is_some()) {
            return Err(Error::Mismatch(
                "single timer demand has an unrelated arrival composition",
            ));
        }
        return Ok(());
    }
    let delivery = scope
        .interrupt_delivery_assumption()
        .ok_or(Error::Mismatch(
            "fatal arrival composition lacks its explicit delivery condition",
        ))?;
    if entries.len() != scope.fatal_entries().len() + 1 || requirements.len() != entries.len() {
        return Err(Error::Mismatch(
            "fatal arrival composition lost part of its entry roster",
        ));
    }
    let anchor = scope.context_anchor().ok_or(Error::Mismatch(
        "arrival composition has no exact timer anchor",
    ))?;
    let mut inputs = Vec::with_capacity(entries.len());
    for entry in entries {
        let mut matching = requirements
            .iter()
            .filter(|row| row.placement_index == entry.placement_index);
        let Some(row) = matching.next() else {
            return Err(Error::Mismatch("arrival entry has no measured body"));
        };
        if matching.next().is_some()
            || row.operation != scope.operation()
            || entry.operation != scope.operation()
            || row.psi != entry.entry().psi()
            || row.target != entry.entry().target()
            || row.entry != entry.entry().machine()
            || row.fault_exclusion.is_none()
            || delivery.psi != row.psi
            || delivery.installation != row.operation
        {
            return Err(Error::Mismatch(
                "arrival entry changed its body, installation or fault premises",
            ));
        }
        let gate = if entry.placement.application_index == scope.acknowledgement_application() {
            anchor.gate_byte_offset()
        } else {
            let mut matching = scope.fatal_entries().iter().filter(|fatal| {
                fatal.application() == entry.placement.application_index
                    && fatal.machine() == row.entry
            });
            let Some(fatal) = matching.next() else {
                return Err(Error::Mismatch(
                    "arrival entry has no admitted fatal identity",
                ));
            };
            if matching.next().is_some() {
                return Err(Error::Mismatch("fatal arrival identity is duplicated"));
            }
            fatal.gate_byte_offset()
        };
        if gate % 16 != 0 || gate / 16 > 255 {
            return Err(Error::Mismatch(
                "arrival vector is outside its exact x86 gate",
            ));
        }
        let domain = match entry.entry().inbound().state.stack {
            EntryStack::Interrupted => None,
            EntryStack::Dedicated { class } => Some(class),
            EntryStack::ProviderSelected => {
                return Err(Error::Mismatch("arrival stack domain is unresolved"));
            }
        };
        inputs.push(Entry {
            placement: row.placement_index,
            vector: (gate / 16) as u16,
            domain,
            bytes: row.own_bytes().ok_or(Error::Overflow)?,
            alignment: row.alignment,
        });
    }
    inputs.sort_unstable_by_key(|entry| entry.placement);
    if inputs
        .windows(2)
        .any(|pair| pair[0].placement == pair[1].placement)
    {
        return Err(Error::Mismatch("arrival placement is duplicated"));
    }
    let mut vectors: Vec<_> = inputs.iter().map(|entry| entry.vector).collect();
    vectors.sort_unstable();
    if vectors.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(Error::Mismatch("arrival vector is duplicated"));
    }
    // Work may raise any architectural exception. Until a narrower exact
    // fault-set proof exists, every exception destination must be installed.
    // OpaqueFatal does not assert whether that vector pushes an error word.
    if !(0..32).all(|vector| vectors.binary_search(&vector).is_ok()) {
        return Err(Error::Mismatch(
            "fatal installation does not cover every architectural exception destination",
        ));
    }
    // Dedicated arrivals also coexist with Work's ordinary live frames.
    // Retain their exact physical root even when no arrival adds interrupted
    // demand, so runtime separation cannot silently omit that active stack.
    let physical_entry = physical_entry
        .filter(|root| root.semantic_entry == delivery.entry)
        .ok_or(Error::Mismatch(
            "fatal arrival has no matching physical entry stack provision",
        ))?;
    let mut peaks = vec![0; inputs.len()];
    let mut simultaneous = vec![Vec::new(); inputs.len()];
    let mut root_peak = 0;
    for first in 0..inputs.len() {
        let mut history = vec![first];
        histories(
            &inputs,
            &mut history,
            &mut peaks,
            &mut simultaneous,
            &mut root_peak,
        )?;
    }
    for row in requirements {
        let index = inputs
            .binary_search_by_key(&row.placement_index, |entry| entry.placement)
            .map_err(|_| Error::Mismatch("arrival requirement is outside its complete roster"))?;
        simultaneous[index].sort_unstable();
        simultaneous[index].dedup();
        row.arrival = Some(InstalledArrivalStackDemand {
            physical_entry,
            root_reserve_bytes: root_peak,
            domain_bytes: peaks[index],
            simultaneous_placements: std::mem::take(&mut simultaneous[index]),
        });
    }
    Ok(())
}

fn histories(
    entries: &[Entry],
    history: &mut Vec<usize>,
    peaks: &mut [u64],
    simultaneous: &mut [Vec<usize>],
    root_peak: &mut u64,
) -> Result<(), Error> {
    // None is the physical Work stack. A dedicated switch leaves earlier
    // storage live; an Interrupted arrival adds to the current active domain.
    let mut active = None;
    let mut occupancy: Vec<(Option<u16>, u64)> = vec![(None, 0)];
    for &index in history.iter() {
        let entry = &entries[index];
        if let Some(domain) = entry.domain {
            active = Some(domain);
        }
        let position = occupancy
            .iter()
            .position(|(domain, _)| *domain == active)
            .unwrap_or_else(|| {
                occupancy.push((active, 0));
                occupancy.len() - 1
            });
        let alignment = u64::from(entry.alignment);
        if alignment == 0 || !alignment.is_power_of_two() {
            return Err(Error::Mismatch("arrival alignment is invalid"));
        }
        let bytes = occupancy[position]
            .1
            .checked_add(alignment - 1)
            .map(|bytes| bytes & !(alignment - 1))
            .and_then(|bytes| bytes.checked_add(entry.bytes))
            .ok_or(Error::Overflow)?;
        occupancy[position].1 = bytes;
    }
    for (domain, bytes) in occupancy {
        if domain.is_none() {
            *root_peak = (*root_peak).max(bytes);
        }
        for (index, entry) in entries.iter().enumerate() {
            if entry.domain == domain {
                peaks[index] = peaks[index].max(bytes);
            }
        }
    }
    for (position, &left) in history.iter().enumerate() {
        if entries[left].domain.is_none() {
            continue;
        }
        for &right in &history[position + 1..] {
            if entries[right].domain.is_none() {
                continue;
            }
            if entries[left].domain == entries[right].domain {
                return Err(Error::Mismatch(
                    "simultaneous arrivals reload one dedicated stack top",
                ));
            }
            simultaneous[left].push(entries[right].placement);
            simultaneous[right].push(entries[left].placement);
        }
    }
    for vector in [2, 18] {
        if history.iter().any(|&index| entries[index].vector == vector) {
            continue;
        }
        if let Some(next) = entries.iter().position(|entry| entry.vector == vector) {
            history.push(next);
            histories(entries, history, peaks, simultaneous, root_peak)?;
            history.pop();
        }
    }
    Ok(())
}
