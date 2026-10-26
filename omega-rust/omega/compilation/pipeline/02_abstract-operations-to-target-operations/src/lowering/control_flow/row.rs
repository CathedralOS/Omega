//! One operation row's lowering site.
//!
//! A row reads the live environment — the function, its structural catalog and
//! the definitions live at this point — and publishes into two sinks: the
//! target rows and the Psi provenance that names them. Every row entry point
//! takes this site instead of restating that environment as its own parameter
//! list, so a new row cannot get the environment subtly wrong and a caller
//! cannot hand one row a different definition set than its siblings.

use super::LiveDefinitions;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::AbstractFunction;
use target_operations::{TargetUnitOperation, TerminalPsiProvenance};

/// A row's environment and sinks, in the order every row spells them:
/// `(function, types, live, operations, provenance)`.
pub(super) type RowParts<'s, 'a> = (
    &'a AbstractFunction,
    &'a StructuralTypeLookup<'a>,
    &'s mut LiveDefinitions,
    &'s mut Vec<TargetUnitOperation>,
    &'s mut TerminalPsiProvenance,
);

/// The same site for a store or descriptor read, which only reads the live
/// definitions: a row that never writes them does not demand write access.
pub(super) type RowView<'s, 'a> = (
    &'a AbstractFunction,
    &'a StructuralTypeLookup<'a>,
    &'s LiveDefinitions,
    &'s mut Vec<TargetUnitOperation>,
    &'s mut TerminalPsiProvenance,
);

/// The live environment a row reads and the sinks it publishes into.
pub(super) struct RowSite<'a> {
    pub(super) function: &'a AbstractFunction,
    pub(super) types: &'a StructuralTypeLookup<'a>,
    pub(super) live: &'a mut LiveDefinitions,
    pub(super) operations: &'a mut Vec<TargetUnitOperation>,
    pub(super) provenance: &'a mut TerminalPsiProvenance,
}

impl<'a> RowSite<'a> {
    pub(super) fn new(
        function: &'a AbstractFunction,
        types: &'a StructuralTypeLookup<'a>,
        live: &'a mut LiveDefinitions,
        operations: &'a mut Vec<TargetUnitOperation>,
        provenance: &'a mut TerminalPsiProvenance,
    ) -> Self {
        Self {
            function,
            types,
            live,
            operations,
            provenance,
        }
    }

    /// The environment a row writes through, plus its two sinks.
    pub(super) fn parts(&mut self) -> RowParts<'_, 'a> {
        (
            self.function,
            self.types,
            self.live,
            self.operations,
            self.provenance,
        )
    }

    /// The same site for a row that only reads the live definitions.
    pub(super) fn view(&mut self) -> RowView<'_, 'a> {
        (
            self.function,
            self.types,
            self.live,
            self.operations,
            self.provenance,
        )
    }
}
