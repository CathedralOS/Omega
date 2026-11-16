//! The installation authority instruction selection reads, as plain data.
//!
//! Provider admission checks every installed interrupt, its handler entries
//! and its arrival-token helpers, and only admission can construct the sealed
//! records that say so. Instruction selection needs a handful of answers from
//! those records, but depending on admission would let it reach every earlier
//! program form admission itself reads. The native driver therefore projects
//! the admitted authority into this record once, after it has resolved the
//! handler stacks and the installed entry, and hands it to selection beside
//! the prepared execution plan.
//!
//! The record holds only what selection reads. Selection refuses a record
//! that names another program, target or entry than the plan it is bound to.

use calling_conventions::{BoundaryEntryPlan, PrivateCallbackPlacements};
use function_identity::MachineFunctionIdentity;
use installation_evidence::InstalledInterruptStackRequirement;
use language_core::static_interrupts::StaticInterruptOperation;
use semantic_vocabulary::{MachineId, OperationId, StructuralTypeId};
use target::NativeTarget;
use terminal_psi::{StructuralArgument, TerminalPsiIdentity};

/// Installation authority bound to one program, projected from admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundInstallations {
    /// The program the installations were admitted for.
    pub psi: TerminalPsiIdentity,
    /// One row per admitted installation request.
    pub installations: Vec<BoundInterruptInstallation>,
    /// The handler entries every installation writes into its table.
    pub entries: Vec<BoundInterruptEntry>,
    /// What admission answered for each arrival-token helper operation.
    pub helpers: Vec<BoundInterruptHelper>,
    /// The installed entry this program runs as, when it is a handler body.
    pub installed_entry: Option<BoundInstalledEntry>,
    /// The measured stack each installed handler needs, one row per
    /// `(operation, placement_index)`.
    pub stack_requirements: Vec<InstalledInterruptStackRequirement>,
}

/// One installation request and the table it fills.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundInterruptInstallation {
    pub operation: OperationId,
    pub arguments: [StructuralArgument; 6],
    pub table_type: StructuralTypeId,
    pub acknowledgement_application: u32,
    pub fatal_entries: Vec<BoundFatalInterruptEntry>,
    pub context_anchor: Option<BoundInterruptContextAnchor>,
    pub completion: MachineId,
}

/// A fatal handler entry an installation admits, and the gate it occupies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundFatalInterruptEntry {
    pub application: u32,
    pub gate_byte_offset: u32,
    pub machine: MachineId,
}

/// Where the acknowledgement entry finds its installation context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundInterruptContextAnchor {
    pub application: u32,
    pub function: MachineFunctionIdentity,
    pub gate_byte_offset: u32,
    pub table_bytes: u32,
}

/// One handler entry an installation writes into its table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundInterruptEntry {
    pub operation: OperationId,
    pub placement_index: usize,
    pub application: u32,
    pub callback_function: MachineFunctionIdentity,
    /// The table field the entry address is written to; `None` when the
    /// entry is not materialized into a private table field.
    pub table_placements: Option<PrivateCallbackPlacements>,
    pub callback: BoundCallbackEntry,
}

/// The admitted callback body behind a handler entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundCallbackEntry {
    pub psi: TerminalPsiIdentity,
    pub target: NativeTarget,
    pub machine: MachineId,
    pub inbound: BoundaryEntryPlan,
}

/// Admission's answer for one arrival-token helper operation: the kind it
/// was admitted as, and why it is refused when it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundInterruptHelper {
    pub operation: OperationId,
    pub kind: StaticInterruptOperation,
    pub refusal: Option<&'static str>,
}

/// The installed entry a handler body is compiled as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundInstalledEntry {
    pub psi: TerminalPsiIdentity,
    pub target: NativeTarget,
    pub machine: MachineId,
}

impl BoundInstallations {
    /// No installations, for a program that installs no interrupt.
    pub const fn empty(psi: TerminalPsiIdentity) -> Self {
        Self {
            psi,
            installations: Vec::new(),
            entries: Vec::new(),
            helpers: Vec::new(),
            installed_entry: None,
            stack_requirements: Vec::new(),
        }
    }

    /// The single installation admitted for `operation`.
    pub fn installation(
        &self,
        operation: OperationId,
    ) -> Result<&BoundInterruptInstallation, &'static str> {
        let mut rows = self
            .installations
            .iter()
            .filter(|row| row.operation == operation);
        match (rows.next(), rows.next()) {
            (Some(row), None) => Ok(row),
            _ => Err("installation does not have exactly one admitted scope"),
        }
    }

    /// The handler entries admitted for `operation`, in application order.
    pub fn installation_entries(
        &self,
        operation: OperationId,
    ) -> impl Iterator<Item = &BoundInterruptEntry> + '_ {
        let mut entries: Vec<_> = self
            .entries
            .iter()
            .filter(|entry| entry.operation == operation)
            .collect();
        entries.sort_by_key(|entry| entry.application);
        entries.into_iter()
    }

    /// Accept `operation` as an arrival-token helper of `kind` only when
    /// admission admitted it as exactly that.
    pub fn require(
        &self,
        operation: OperationId,
        kind: StaticInterruptOperation,
    ) -> Result<(), &'static str> {
        if kind == StaticInterruptOperation::Install {
            return Err("an installation operation is not an arrival token helper");
        }
        let mut rows = self.helpers.iter().filter(|row| row.operation == operation);
        let (Some(helper), None) = (rows.next(), rows.next()) else {
            return Err("interrupt helper has no admitted installation association");
        };
        if helper.kind != kind {
            return Err("interrupt helper association has a changed kind or duplicate root");
        }
        helper.refusal.map_or(Ok(()), Err)
    }

    /// The measured stack requirement of one handler entry; `None` when it
    /// has none or more than one.
    pub fn stack_requirement(
        &self,
        operation: OperationId,
        placement_index: usize,
    ) -> Option<&InstalledInterruptStackRequirement> {
        let mut rows = self
            .stack_requirements
            .iter()
            .filter(|row| (row.operation, row.placement_index) == (operation, placement_index));
        match (rows.next(), rows.next()) {
            (Some(row), None) => Some(row),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn psi() -> TerminalPsiIdentity {
        TerminalPsiIdentity {
            vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
            program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([1; 32]),
        }
    }

    #[test]
    fn a_helper_is_accepted_only_as_the_kind_admission_answered() {
        let operation = OperationId::new(3).unwrap();
        let mut bound = BoundInstallations::empty(psi());
        assert!(
            bound
                .require(operation, StaticInterruptOperation::Context)
                .is_err(),
            "an operation admission never answered is refused"
        );
        bound.helpers.push(BoundInterruptHelper {
            operation,
            kind: StaticInterruptOperation::Context,
            refusal: None,
        });
        assert_eq!(
            bound.require(operation, StaticInterruptOperation::Context),
            Ok(())
        );
        assert!(
            bound
                .require(operation, StaticInterruptOperation::Install)
                .is_err()
        );
        bound.helpers[0].refusal = Some("refused by admission");
        assert_eq!(
            bound.require(operation, StaticInterruptOperation::Context),
            Err("refused by admission")
        );
    }
}
