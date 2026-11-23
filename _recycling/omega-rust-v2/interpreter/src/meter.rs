//! Logical work and live storage for one invocation.
//!
//! Work follows schedule 1 of `wiki/spec/resources/logical_work.md`: one unit
//! for each executed Psi operation and one for each taken control-flow edge,
//! charged before the site runs. Cleanup on an edge is part of that edge.
//! Storage counts the cells of [`crate::CELL_SCHEDULE`] that are live at once.
//!
//! A ceiling is the root's choice. `None` means the root set none for that
//! account; the meter still counts, so the receipt is the same either way and
//! a ceiling can change whether a result is obtained but never which result.
use omega_psi::{EdgeId, MachineId, OperationId};

/// Schedule 1: one unit per executed Psi operation or taken control-flow edge.
pub const WORK_SCHEDULE: u32 = 1;

/// Cell schedule 1. One cell is held for each activation frame, each scalar
/// value a frame binds, and each scalar, case tag, sequence length or
/// descriptor stored in a structural value a frame or the sponsor holds. A
/// transfer moves cells; a copy takes new ones. A frame's cells are released
/// when it returns, except the cells of the result it transfers to its caller.
pub const CELL_SCHEDULE: u32 = 1;

/// Root-selected ceilings for one invocation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Ceilings {
    /// The work budget in schedule-1 units, when the root set one.
    pub work: Option<u64>,
    /// The greatest number of cells that may be live at once, when the root set one.
    pub live_cells: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChargeSite {
    Operation(OperationId),
    Edge(EdgeId),
}

impl std::fmt::Display for ChargeSite {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            Self::Operation(id) => write!(f, "operation {}", id.get()),
            Self::Edge(id) => write!(f, "edge {}", id.get()),
        }
    }
}

/// Executions of one site of one machine, in first-execution order. Under
/// schedule 1 a site's units equal its executions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SiteUsage {
    pub machine: MachineId,
    pub site: ChargeSite,
    pub executions: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Usage {
    pub work_schedule: u32,
    pub cell_schedule: u32,
    pub work_used: u64,
    pub peak_live_cells: u64,
    pub sites: Vec<SiteUsage>,
}

/// Why the meter stopped an invocation. Neither is a program result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeterStop {
    /// The budget cannot pay the next charge. `remaining` need not be zero.
    WorkExhausted { used: u64, remaining: u64, unpaid: u64 },
    /// Taking `requested` more cells would exceed the ceiling.
    LiveCells { live: u64, requested: u64, ceiling: u64 },
    /// A counter left the range of its account.
    Overflow,
}

pub(crate) struct Meter {
    ceilings: Ceilings,
    work_used: u64,
    live_cells: u64,
    peak_live_cells: u64,
    /// Executions per admitted body, indexed by its roster slot and then by
    /// site: each operation in order, followed by the body's one edge.
    executions: Vec<Vec<u64>>,
}

impl Meter {
    /// `sites` gives the number of charge sites of each admitted body.
    pub(crate) fn new(
        ceilings: Ceilings,
        sites: impl Iterator<Item = usize>,
    ) -> Self {
        Self {
            ceilings,
            work_used: 0,
            live_cells: 0,
            peak_live_cells: 0,
            executions: sites.map(|count| vec![0; count]).collect(),
        }
    }

    /// Pay for one site before it runs.
    pub(crate) fn charge(
        &mut self,
        body: usize,
        site: usize,
    ) -> Result<(), MeterStop> {
        const UNITS: u64 = 1;
        if let Some(budget) = self.ceilings.work {
            let remaining = budget.saturating_sub(self.work_used);
            if remaining < UNITS {
                return Err(MeterStop::WorkExhausted {
                    used: self.work_used,
                    remaining,
                    unpaid: UNITS,
                });
            }
        }
        let used = self.work_used.checked_add(UNITS).ok_or(MeterStop::Overflow)?;
        let count = self
            .executions
            .get_mut(body)
            .and_then(|sites| sites.get_mut(site))
            .ok_or(MeterStop::Overflow)?;
        *count = count.checked_add(1).ok_or(MeterStop::Overflow)?;
        self.work_used = used;
        Ok(())
    }

    /// Take `cells` more live cells, or refuse before any storage exists.
    pub(crate) fn reserve(
        &mut self,
        cells: u64,
    ) -> Result<(), MeterStop> {
        let live = self.live_cells.checked_add(cells).ok_or(MeterStop::Overflow)?;
        if let Some(ceiling) = self.ceilings.live_cells
            && live > ceiling
        {
            return Err(MeterStop::LiveCells {
                live: self.live_cells,
                requested: cells,
                ceiling,
            });
        }
        self.live_cells = live;
        self.peak_live_cells = self.peak_live_cells.max(live);
        Ok(())
    }

    pub(crate) fn release(
        &mut self,
        cells: u64,
    ) -> Result<(), MeterStop> {
        self.live_cells = self.live_cells.checked_sub(cells).ok_or(MeterStop::Overflow)?;
        Ok(())
    }

    /// The totals, and the executions of each body's sites in roster order.
    pub(crate) fn finish(self) -> (u64, u64, Vec<Vec<u64>>) {
        (self.work_used, self.peak_live_cells, self.executions)
    }
}
