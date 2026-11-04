//! Inspect an explicitly supplied completed native image. These addresses belong
//! to that image's linked coordinate space, not the interpreter's entry IDs or
//! a claim about a separately launched process's load slide. The receiving host
//! authenticates the completed image; admission here keeps its exact program,
//! target and selected-machine identities attached to every immutable row.

use crate::boundary::errors::TerminalInterpretError;
use semantic_vocabulary::MachineId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalNativeCodeRegion {
    pub base: u64,
    pub length: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalNativeCodeLayout {
    pub machine: MachineId,
    pub entry: u64,
    pub regions: Vec<TerminalNativeCodeRegion>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalNativeCodeLayoutEnvironment {
    terminal_psi: terminal_psi::TerminalPsiIdentity,
    target: String,
    native_artifact: [u8; 32],
    layouts: Vec<TerminalNativeCodeLayout>,
}

impl TerminalNativeCodeLayoutEnvironment {
    /// Supply already finalized image observations. This trusted host input
    /// grants no memory access, code installation or semantic evaluation.
    /// The Omega runtime adapter authenticates the actual completed artifact;
    /// a bare Terminal program cannot manufacture the missing image.
    pub fn linked_image(
        terminal_psi: terminal_psi::TerminalPsiIdentity,
        target: &str,
        native_artifact: [u8; 32],
        mut layouts: Vec<TerminalNativeCodeLayout>,
    ) -> Result<Self, TerminalInterpretError> {
        let invalid = || {
            TerminalInterpretError::UnsupportedSemanticVariant(
                "native layout supply has invalid completed-image metadata",
            )
        };
        if target.is_empty() || native_artifact == [0; 32] {
            return Err(invalid());
        }
        layouts.sort_by_key(|layout| layout.machine);
        if layouts
            .windows(2)
            .any(|pair| pair[0].machine == pair[1].machine)
        {
            return Err(invalid());
        }
        for layout in &layouts {
            let mut previous_end = None;
            let mut contains_entry = false;
            for region in &layout.regions {
                let end = region.base.checked_add(region.length).ok_or_else(invalid)?;
                if region.base == 0
                    || region.length == 0
                    || previous_end.is_some_and(|previous| region.base <= previous)
                {
                    return Err(invalid());
                }
                contains_entry |= region.base <= layout.entry && layout.entry < end;
                previous_end = Some(end);
            }
            if layout.entry == 0 || !contains_entry {
                return Err(invalid());
            }
        }
        Ok(Self {
            terminal_psi,
            target: target.to_owned(),
            native_artifact,
            layouts,
        })
    }

    pub(crate) fn accepts(
        &self,
        code: terminal_codec::TerminalExecutionIdentity,
        target: &str,
    ) -> bool {
        code == terminal_codec::TerminalExecutionIdentity::portable(self.terminal_psi)
            && self.target == target
    }

    pub(crate) fn layouts(&self) -> &[TerminalNativeCodeLayout] {
        &self.layouts
    }

    pub(crate) fn layout(&self, machine: MachineId) -> Option<&TerminalNativeCodeLayout> {
        self.layouts
            .binary_search_by_key(&machine, |layout| layout.machine)
            .ok()
            .map(|position| &self.layouts[position])
    }
}
