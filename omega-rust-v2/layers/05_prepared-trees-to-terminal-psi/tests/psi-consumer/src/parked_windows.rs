//! Rebuild, from published Psi only, which restoration windows are open when a
//! call parks, and compare that with the windows each suspension site names.
//!
//! A `BorrowedMove` opens a window on its captured place. A `StructuralStore`
//! to the identical captured place closes it
//! (`wiki/spec/terminal-psi/ownership.md#borrowed-storage-restoration`). This
//! reader compares places exactly and refuses any runtime element selector,
//! disagreeing arrival or partly overlapping store instead of guessing.
use omega_psi::{
    BlockTerminator, CallSite, LoweredPackage, MachineBody, OperationId, OperationKind, OwnedCustody, PlaceId, PlaceOrigin, PlaceProjection, ProjectedPlace,
    ReferenceAccess, StructuralStoreDisposition,
};

use super::machine;

/// One window that is open at a parked call, with the custody that surrounds it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParkedWindow {
    pub opened: OperationId,
    /// The vacated place, with named reborrows expanded to their lender.
    pub place: ProjectedPlace,
    /// The value the move displaced.
    pub displaced: PlaceId,
    /// Custody of the displaced value at the crossing, when it is still live there.
    pub displaced_custody: Option<OwnedCustody>,
    /// Access of the retained loan on the lender, when the crossing retains it.
    pub lender: Option<ReferenceAccess>,
}

/// How a store re-established a place: by filling a vacancy or by displacing
/// a value that was there.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreKind {
    Fill,
    Displace,
}

/// Every parked call of `caller` with the windows open there, in site order.
pub fn parked_windows(
    package: &LoweredPackage,
    caller: &str,
) -> Result<Vec<(CallSite, Vec<ParkedWindow>)>, &'static str> {
    let body = &machine(package, caller).ok_or("missing caller")?.body;
    let entries = entry_states(body)?;
    let mut result = Vec::new();
    for site in &body.suspension_sites {
        let (block, end) = match site.call {
            CallSite::Operation(id) => {
                let index = id.index();
                let block = body
                    .blocks
                    .iter()
                    .find(|b| b.operations.contains(&index))
                    .ok_or("a parked operation outside every block")?;
                (block, index)
            }
            CallSite::Edge(id) => {
                let block = body
                    .blocks
                    .iter()
                    .find(|b| matches!(b.terminator, BlockTerminator::TerminalCall { id: e, .. } if e == id))
                    .ok_or("a parked edge without its terminal call")?;
                (block, block.operations.end)
            }
            CallSite::Cleanup(_) => return Err("a cleanup call does not park"),
        };
        let entry = entries[block.id.get() - 1].clone().ok_or("a parked call in an unreached block")?;
        let open = advance(body, entry, block.operations.start..end)?;
        if site.open_windows.len() != open.len()
            || site
                .open_windows
                .iter()
                .zip(&open)
                .any(|(row, &moved)| row.opened != body.operations[moved].id || row.place != captured(body, moved_place(body, moved)))
        {
            return Err("the published windows disagree with the operations before the call");
        }
        let windows = open
            .into_iter()
            .map(|moved| {
                let place = captured(body, moved_place(body, moved));
                let omega_psi::OperationResult::Structural(displaced) = body.operations[moved].result else {
                    return Err("a borrowed move must produce a structural value");
                };
                Ok(ParkedWindow {
                    opened: body.operations[moved].id,
                    displaced,
                    displaced_custody: site
                        .retained_owners
                        .iter()
                        .find(|owner| owner.place == displaced)
                        .map(|owner| owner.custody),
                    lender: site
                        .retained_loans
                        .iter()
                        .find(|loan| loan.descriptor == place.root)
                        .map(|loan| loan.access),
                    place,
                })
            })
            .collect::<Result<_, _>>()?;
        result.push((site.call, windows));
    }
    Ok(result)
}

/// Each store to a place that some `BorrowedMove` in the body vacated, in
/// operation order, with its published disposition.
pub fn window_stores(
    package: &LoweredPackage,
    caller: &str,
) -> Result<Vec<(OperationId, ProjectedPlace, StoreKind)>, &'static str> {
    let body = &machine(package, caller).ok_or("missing caller")?.body;
    let mut stores = Vec::new();
    for op in &body.operations {
        let OperationKind::StructuralStore { destination, disposition, .. } = &op.kind else { continue };
        let place = captured(body, destination);
        let kind = match disposition {
            StructuralStoreDisposition::FillVacancy => StoreKind::Fill,
            StructuralStoreDisposition::ReplaceUnrestricted | StructuralStoreDisposition::ReplaceWriteOnly(_) | StructuralStoreDisposition::Dispose(_) => StoreKind::Displace,
        };
        stores.push((op.id, place, kind));
    }
    Ok(stores)
}

fn moved_place(
    body: &MachineBody,
    index: usize,
) -> &ProjectedPlace {
    match &body.operations[index].kind {
        OperationKind::BorrowedMove { source } => source,
        _ => unreachable!("only moves open windows"),
    }
}

/// Expand a leading `Referent` through the `BorrowPlace` that formed a named reborrow.
fn captured(
    body: &MachineBody,
    place: &ProjectedPlace,
) -> ProjectedPlace {
    let mut current = place.clone();
    while current.path.first() == Some(&PlaceProjection::Referent)
        && let PlaceOrigin::Operation(id) = body.places[current.root.index()].origin
        && let OperationKind::BorrowPlace { source, .. } = &body.operations[id.index()].kind
    {
        let mut path = source.path.clone();
        path.extend(current.path.drain(1..));
        current = ProjectedPlace { root: source.root, path };
    }
    current
}

/// Moves still open after `range`, starting from `open`.
fn advance(
    body: &MachineBody,
    mut open: Vec<usize>,
    range: std::ops::Range<usize>,
) -> Result<Vec<usize>, &'static str> {
    for index in range {
        match &body.operations[index].kind {
            OperationKind::BorrowedMove { source } => {
                if captured(body, source).path.iter().any(|step| matches!(step, PlaceProjection::ViewElement { .. })) {
                    return Err("this reader does not compare runtime element selectors");
                }
                open.push(index);
            }
            OperationKind::StructuralStore { destination, .. } => {
                let target = captured(body, destination);
                let mut kept = Vec::new();
                for moved in open {
                    let place = captured(body, moved_place(body, moved));
                    if place == target {
                        continue;
                    }
                    let shared = place.path.len().min(target.path.len());
                    if place.root == target.root && place.path[..shared] == target.path[..shared] {
                        return Err("a store overlaps an open window without matching it");
                    }
                    kept.push(moved);
                }
                open = kept;
            }
            _ => {}
        }
    }
    Ok(open)
}

/// Open moves at each block's entry. Every arrival must agree.
fn entry_states(body: &MachineBody) -> Result<Vec<Option<Vec<usize>>>, &'static str> {
    let mut entries: Vec<Option<Vec<usize>>> = vec![None; body.blocks.len()];
    let Some(first) = entries.first_mut() else { return Ok(entries) };
    *first = Some(Vec::new());
    let mut pending = vec![0];
    while let Some(index) = pending.pop() {
        let block = &body.blocks[index];
        let exit = advance(body, entries[index].clone().unwrap_or_default(), block.operations.clone())?;
        let targets = match &block.terminator {
            BlockTerminator::Jump(edge) => vec![edge.target],
            BlockTerminator::Conditional { when_true, when_false, .. } => vec![when_true.target, when_false.target],
            _ => Vec::new(),
        };
        for target in targets {
            match &entries[target.get() - 1] {
                None => {
                    entries[target.get() - 1] = Some(exit.clone());
                    pending.push(target.get() - 1);
                }
                Some(previous) if *previous == exit => {}
                Some(_) => return Err("arrivals disagree on which windows are open"),
            }
        }
    }
    Ok(entries)
}
