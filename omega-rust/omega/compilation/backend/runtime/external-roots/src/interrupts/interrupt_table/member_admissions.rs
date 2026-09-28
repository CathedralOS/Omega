//! Member-admission verdicts the consumer's authored declaration mints.

use crate::InstalledRootRecord;
use crate::interrupts::interrupt_table::InterruptTableMemberPlan;
use calling_conventions::{EntryControl, EntryStack};

/// The member-arrival facts an installed root's retained record carries,
/// decoded verbatim for the consumer's authored member-admission machine.
/// `stack_dedicated_class` reads `0` when the member's arrival context
/// selects no dedicated critical stack — a value no declared row names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterruptTableMemberFacts {
    /// The member's exit path realizes interrupt return.
    pub entry_interrupt_return: bool,
    /// The dedicated critical stack class arrivals land on, or `0` when the
    /// member does not arrive on a dedicated class.
    pub stack_dedicated_class: u16,
    /// The record carries an acknowledgement policy column.
    pub acknowledgement_policy: bool,
    /// The record's boundary names a `Pending` acknowledgement parameter.
    pub acknowledgement_parameter: bool,
}

impl InterruptTableMemberFacts {
    /// Decode one installed root record's arrival facts exactly as the
    /// consumer's authored admission verdict binds them.
    pub fn from_record(record: &InstalledRootRecord) -> Self {
        Self {
            entry_interrupt_return: record.boundary.call.entry_control
                == EntryControl::InterruptReturn,
            stack_dedicated_class: match record.boundary.state.stack {
                EntryStack::Dedicated { class } => class,
                EntryStack::Interrupted | EntryStack::ProviderSelected => 0,
            },
            acknowledgement_policy: record.acknowledgement_policy.is_some(),
            acknowledgement_parameter: record.acknowledgement_parameter_index.is_some(),
        }
    }
}

/// One member's authored admission verdict bound to the exact declared row
/// and arrival facts it accepted.
///
/// The consumer's admission machine — the interrupt-table package's
/// `TableMemberAdmission::admit` — owns the policy a member's installed
/// root must satisfy for its vector: interrupt-return exit, arrival on the
/// declared dedicated critical stack class, and the declared obligation's
/// acknowledgement shape. This record is minted only for an `Admitted`
/// verdict and carries the exact row and facts the verdict bound, so the
/// table ledger's admission replays that binding instead of re-deciding
/// the declaration's semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterruptTableMemberAdmission {
    declaration: InterruptTableMemberPlan,
    facts: InterruptTableMemberFacts,
}

impl InterruptTableMemberAdmission {
    /// Mint the admission record an authored `Admitted` verdict warrants:
    /// the declared member row beside the member facts it accepted.
    pub const fn from_consumer(
        declaration: InterruptTableMemberPlan,
        facts: InterruptTableMemberFacts,
    ) -> Self {
        Self { declaration, facts }
    }

    /// The declared member row the verdict admitted.
    pub const fn declaration(&self) -> &InterruptTableMemberPlan {
        &self.declaration
    }

    /// The member-arrival facts the verdict bound.
    pub const fn facts(&self) -> InterruptTableMemberFacts {
        self.facts
    }
}
