//! The read view of the tables every pipeline stage derefs to.
//!
//! `TypedTrees` derefs to `TypedTreeTables`, so a table read has two spellings:
//! through the column (`program.type_reference_table.type_reference_handles`)
//! or through the owner (`program.type_reference_handles`). The second is one
//! call on a field root rather than a two-step chain, so a call site that names
//! an occurrence stays on one line where the column-qualified chain wraps.
//!
//! Each accessor is a one-expression delegation to the column that owns the
//! rows; no lookup, projection or fallback moves here. Only read chains that
//! repeat across the crate get one, and only where the name is unambiguous
//! crate-wide: a `_mut` twin stays on its column because a mutable borrow of
//! one table is not a view of the tables struct.

use crate::expression::StaticMachineArgument;
use crate::static_applications::{
    StaticMachineApplication, StaticMachineApplicationHandle, StaticMachineBinding,
    StaticMachineLifetimeBinding,
};
use crate::types::TypeReferenceHandle;
use arena::HandleSpan;

impl crate::typed_trees::TypedTreeTables {
    /// The type references a declaration span owns, in authored order.
    pub(crate) fn type_reference_handles(
        &self,
        span: HandleSpan<TypeReferenceHandle>,
    ) -> &[TypeReferenceHandle] {
        self.type_reference_table.type_reference_handles(span)
    }

    /// Whether `handle` still names a live type reference row.
    pub(crate) fn contains_type_reference(&self, handle: TypeReferenceHandle) -> bool {
        self.type_reference_table.contains_type_reference(handle)
    }

    /// One static machine application row.
    pub(crate) fn static_machine_application(
        &self,
        application: StaticMachineApplicationHandle,
    ) -> &StaticMachineApplication {
        self.expression_table
            .static_machine_application(application)
    }

    /// The argument rows a static machine application supplies.
    pub(crate) fn static_machine_application_arguments(
        &self,
        application: StaticMachineApplicationHandle,
    ) -> &[StaticMachineArgument] {
        self.expression_table
            .static_machine_application_arguments(application)
    }

    /// The bindings a static machine application row captures.
    pub(crate) fn static_machine_application_bindings(
        &self,
        application: StaticMachineApplicationHandle,
    ) -> &[StaticMachineBinding] {
        self.expression_table
            .static_machine_application_bindings(application)
    }

    /// The lifetime bindings a static machine application row captures.
    pub(crate) fn static_machine_application_lifetimes(
        &self,
        application: StaticMachineApplicationHandle,
    ) -> &[StaticMachineLifetimeBinding] {
        self.expression_table
            .static_machine_application_lifetimes(application)
    }
}
