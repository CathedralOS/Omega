//! The receiving host's limits on loading external Psi.
//!
//! A host may cap the encoded input size, and one work account covers every
//! phase of loading a received artifact: parsing its bytes, checking its
//! canonical sections, validating its structure and, when the host
//! authorizes it for execution, the checker judgments execution relies on.
//! Each phase charges its cost before it runs, so no phase starts work the
//! account cannot pay for. Exceeding a limit is a resource refusal
//! ([`CodecError::is_resource_refusal`]), distinct from malformed input and
//! from a failed semantic check. Loading never charges for, or repeats, the
//! producer's own proof search.

use crate::CodecError;
use terminal_psi::TerminalModule;

/// Work a host admits for one load when it sets no other limit. It admits
/// any artifact a 64 MiB input could describe under the charges below.
pub const DEFAULT_LOAD_WORK_UNITS: u64 = 1 << 44;

/// Each replayed judgment walks a machine's flow at most this many times per
/// position (the descriptor replay's own visit limit), so preparation charges
/// it as an upper bound before it starts.
const PREPARATION_VISITS_PER_POSITION: u64 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PsiLoadLimits {
    /// The largest encoded input the host admits, if it sets one.
    pub max_input_bytes: Option<u64>,
    /// The overall work budget across decoding, canonical re-encoding,
    /// validation and execution preparation.
    pub work_units: u64,
}

impl Default for PsiLoadLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: None,
            work_units: DEFAULT_LOAD_WORK_UNITS,
        }
    }
}

/// The remaining work of one load. A received module keeps its account, so
/// execution preparation draws on the same budget as decoding did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadAccount {
    limit: u64,
    used: u64,
}

impl LoadAccount {
    /// Opens the account for an input of `input_bytes`, refusing it first if
    /// it exceeds the host's size limit.
    pub fn open(limits: PsiLoadLimits, input_bytes: usize) -> Result<Self, CodecError> {
        let actual = u64::try_from(input_bytes).unwrap_or(u64::MAX);
        if let Some(limit) = limits.max_input_bytes
            && actual > limit
        {
            return Err(CodecError::InputTooLarge { limit, actual });
        }
        Ok(Self {
            limit: limits.work_units,
            used: 0,
        })
    }

    pub const fn limit(&self) -> u64 {
        self.limit
    }

    pub const fn used(&self) -> u64 {
        self.used
    }

    /// Pays for `units` of work in `phase` before the caller performs it.
    pub fn charge(&mut self, phase: &'static str, units: u64) -> Result<(), CodecError> {
        match self.used.checked_add(units) {
            Some(used) if used <= self.limit => {
                self.used = used;
                Ok(())
            }
            _ => Err(CodecError::LoadWorkExhausted {
                phase,
                limit: self.limit,
            }),
        }
    }

    pub(crate) fn charge_bytes(
        &mut self,
        phase: &'static str,
        bytes: usize,
    ) -> Result<(), CodecError> {
        self.charge(phase, u64::try_from(bytes).unwrap_or(u64::MAX))
    }
}

/// Positions in a module's bodies and declarations: one per operation,
/// terminator and declared type. Structural validation visits each a bounded
/// number of times.
pub(crate) fn module_positions(module: &TerminalModule) -> u64 {
    let bodies = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .map(|block| block.operations.len() as u64 + 1)
        .fold(0_u64, u64::saturating_add);
    bodies
        .saturating_add(module.machines.len() as u64)
        .saturating_add(module.types.structural_types.len() as u64)
}

/// The upper bound [`crate::prepare_received_execution`] charges: every
/// position replayed up to the descriptor replay's visit limit.
pub(crate) fn preparation_units(module: &TerminalModule) -> u64 {
    module_positions(module).saturating_mul(PREPARATION_VISITS_PER_POSITION)
}
