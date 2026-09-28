//! Interrupt table gate descriptors, member plans and profiles.

use crate::{ExternalRootDiagnostic, InterruptTableProfileId};
use calling_conventions::X86_64GateKind;
use std::collections::{BTreeMap, BTreeSet};

/// The settlement contract one declared table member owes per arrival.
///
/// The compiler does not choose which vectors are fatal or which controller
/// protocol an interrupt settles; the consumer declares the obligation per
/// member and admission checks that the installed root's boundary can honor
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptTableObligation {
    /// A fatal exception entry records the fault and never resumes ordinary
    /// work. No controller acknowledgement is minted for it: the installed
    /// root must not carry an acknowledgement policy or parameter.
    FatalException,
    /// An external interrupt entry — the timer root's shape — whose handler
    /// acknowledges, records, and wakes ordinary work. The installed root
    /// must mint a `Pending` acknowledgement that its exit settles.
    AcknowledgedInterrupt,
}

/// Byte width of one x86-64 gate descriptor — the long-mode IDT entry.
pub const X86_64_GATE_DESCRIPTOR_BYTES: u64 = 16;

/// Highest encodable x86-64 interrupt-stack-table slot (IST is a 3-bit
/// descriptor field naming IST1 through IST7; zero means no IST switch).
pub const X86_64_IST_SLOT_LIMIT: u8 = 7;

/// One declared member's descriptor constants — the table's
/// consumer-authored content for that vector's x86-64 gate.
///
/// The gate offset is deliberately absent: it is the sealed entry target the
/// checked writer resolves, not declarable content. `selector` is the
/// consumer's code-segment selector, `entry_privilege` the descriptor DPL,
/// and `interrupt_stack_table_slot` the declared IST field — `None` encodes
/// the architectural zero (no IST switch), which cannot select a dedicated
/// critical stack. How these fields pack into the produced slot is the
/// consumer's authored gate layout, not a compiler-held encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterruptTableGateDescriptor {
    pub gate: X86_64GateKind,
    pub selector: u16,
    pub entry_privilege: u8,
    pub interrupt_stack_table_slot: Option<u8>,
}

/// One declared member of the consumer's table plan: the vector the table
/// owner will route, the dedicated critical stack class its entries must
/// arrive on, and the descriptor constants its produced gate bytes must
/// carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterruptTableMemberPlan {
    pub vector: u8,
    pub dedicated_stack_class: u16,
    pub obligation: InterruptTableObligation,
    pub descriptor: InterruptTableGateDescriptor,
}

/// Normalized consumer-authored plan for one interrupt table.
///
/// The member set is the complete declared coverage: publication cannot be
/// issued until every declared vector is admitted, and admitted members'
/// stack classes must be distinct so two fatal entries can never be
/// accounted onto one critical stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterruptTableProfile {
    pub(crate) identity: InterruptTableProfileId,
    pub(crate) members: BTreeMap<u8, InterruptTableMemberPlan>,
}

impl InterruptTableProfile {
    pub fn new(
        identity: InterruptTableProfileId,
        members: impl IntoIterator<Item = InterruptTableMemberPlan>,
    ) -> Result<Self, ExternalRootDiagnostic> {
        let mut declared = BTreeMap::new();
        let mut classes = BTreeSet::new();
        for member in members {
            if declared.insert(member.vector, member).is_some() {
                return Err(ExternalRootDiagnostic(format!(
                    "interrupt-table profile declares vector {} more than once",
                    member.vector
                )));
            }
            if !classes.insert(member.dedicated_stack_class) {
                return Err(ExternalRootDiagnostic(format!(
                    "interrupt-table profile accounts dedicated stack class {} to more than one vector",
                    member.dedicated_stack_class
                )));
            }
            validate_declared_gate_descriptor(&member)?;
        }
        if declared.is_empty() {
            return Err(ExternalRootDiagnostic(
                "interrupt-table profile declares no member vectors".into(),
            ));
        }
        Ok(Self {
            identity,
            members: declared,
        })
    }

    pub const fn identity(&self) -> InterruptTableProfileId {
        self.identity
    }

    pub fn member(&self, vector: u8) -> Option<&InterruptTableMemberPlan> {
        self.members.get(&vector)
    }

    pub fn members(&self) -> impl ExactSizeIterator<Item = &InterruptTableMemberPlan> {
        self.members.values()
    }
}

/// The structural checks a declared descriptor must pass before the profile
/// retains it: the selector must not be null, the DPL must encode, and an
/// IST slot must name a real IST1..=IST7 slot — `Some(0)` is the non-canonical
/// spelling of the architectural no-switch field.
fn validate_declared_gate_descriptor(
    member: &InterruptTableMemberPlan,
) -> Result<(), ExternalRootDiagnostic> {
    if member.descriptor.selector == 0 {
        return Err(ExternalRootDiagnostic(format!(
            "interrupt-table member at vector {} declares a null gate selector",
            member.vector
        )));
    }
    if member.descriptor.entry_privilege > 3 {
        return Err(ExternalRootDiagnostic(format!(
            "interrupt-table member at vector {} declares a gate privilege outside 0..=3",
            member.vector
        )));
    }
    if let Some(slot) = member.descriptor.interrupt_stack_table_slot
        && (slot == 0 || slot > X86_64_IST_SLOT_LIMIT)
    {
        return Err(ExternalRootDiagnostic(format!(
            "interrupt-table member at vector {} declares an interrupt-stack-table slot outside 1..=7",
            member.vector
        )));
    }
    Ok(())
}
