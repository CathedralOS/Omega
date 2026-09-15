//! Interrupt table descriptor operands and descriptor table state.

use crate::InterruptTablePublicationId;
use extents::Extent;

/// Byte width of the pseudo-descriptor a checked x86-64 descriptor-table
/// load reads: a `u16` limit followed by a `u64` base.
pub const INTERRUPT_TABLE_DESCRIPTOR_OPERAND_BYTES: u64 = 10;

/// The pseudo-descriptor operand a checked table-load instruction reads:
/// the provider-declared `{limit, base}` staged at one readable operand
/// extent.
///
/// The provider stages the operand bytes itself; this carrier is the
/// declared decode plus the accounted read site. The edge replays that the
/// declared value names exactly the carrier's established destination and
/// that the read site is a distinct 10-byte extent in the table's own
/// address space — a load answered from any other geometry installs a
/// different table.
#[derive(Debug, PartialEq, Eq)]
pub struct InterruptTableDescriptorOperand {
    pub(crate) site: Extent,
    pub(crate) limit: u16,
    pub(crate) base: u64,
}

impl InterruptTableDescriptorOperand {
    pub fn from_provider(site: Extent, limit: u16, base: u64) -> Self {
        Self { site, limit, base }
    }

    /// The accounted read site the instruction consumes.
    pub const fn site(&self) -> &Extent {
        &self.site
    }

    /// Declared pseudo-descriptor limit field.
    pub const fn limit(&self) -> u16 {
        self.limit
    }

    /// Declared pseudo-descriptor base field.
    pub const fn base(&self) -> u64 {
        self.base
    }
}

/// The descriptor-table register state one published carrier installed: the
/// executing processor's table register now names exactly this
/// `{base, limit}`. Minted only by the checked provider edge on a published
/// answer; a declined attempt installs nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterruptDescriptorTableState {
    pub(crate) publication: InterruptTablePublicationId,
    pub(crate) base: u64,
    pub(crate) limit: u16,
}

impl InterruptDescriptorTableState {
    pub const fn publication(&self) -> InterruptTablePublicationId {
        self.publication
    }

    pub const fn base(&self) -> u64 {
        self.base
    }

    pub const fn limit(&self) -> u16 {
        self.limit
    }
}
