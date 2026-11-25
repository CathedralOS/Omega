//! The private resident initialization and extraction machines operate on an
//! independent layout selection for the exact semantic value being stored.

use super::{
    Checked, Construct, MachineId, Program, SourceOrigin, StaticInvocation, Unserved, refuse,
};
use crate::source_state::{
    AccessContentShape, AccessExecutionDeclaration, AccessLifecycleOperation,
};

impl Program<'_> {
    pub(super) fn check_resident_storage(
        &self,
        id: MachineId,
        invocation: &StaticInvocation<'_>,
        source: SourceOrigin,
    ) -> Checked<()> {
        if invocation.authored_arguments.is_empty() {
            return Ok(());
        }
        let typed = self.typed;
        let unavailable = |reason| refuse(id, Construct::Machine, Unserved::Call(reason));
        let Some(operation) = AccessExecutionDeclaration::authenticate(typed, invocation.target)
            .map_err(|error| crate::reject(id, source, error.message))?
        else {
            return Ok(());
        };
        if operation.content() != AccessContentShape::Resident
            || !matches!(
                operation.operation(),
                AccessLifecycleOperation::Initialize | AccessLifecycleOperation::MoveOut
            )
        {
            return Ok(());
        }
        self.visible_to(id, operation.template())?;
        self.direct_selection(
            id,
            invocation.target,
            invocation.selection,
            invocation.binding,
        )?;
        if invocation.unsupported || invocation.runtime_receiver {
            return Err(unavailable(
                "resident placement requires a direct source call to its sealed declaration",
            ));
        }
        typed.bodyless_callable_signature(invocation.application, invocation.target)
            .map_err(unavailable)?
            .ok_or_else(|| unavailable("resident placement needs the prepared source call and its closed type arguments"))?;
        if !self.owns_call_arguments(
            id,
            invocation.application,
            operation.template(),
            invocation.authored_arguments,
        ) {
            return Err(unavailable(
                "resident placement lost the operands of its source call",
            ));
        }
        let template = typed
            .machine(operation.template())
            .ok_or_else(|| unavailable("resident placement lost its sealed declaration"))?;
        let Some([_, schema, storage, _]) = typed.machine_type_parameters(template).as_array()
        else {
            return Err(unavailable(
                "resident placement lost its source and storage binders",
            ));
        };
        let selected = |parameter| self.bound_type_argument(invocation.application, parameter);
        let (Some(schema), Some(storage)) = (selected(schema.symbol), selected(storage.symbol))
        else {
            return Err(unavailable(
                "resident placement needs the selected source and storage type arguments",
            ));
        };
        let storage = self
            .source_scope
            .supplied_type_reference(typed, storage)
            .map_err(unavailable)?;
        let Some(layout) = typed.layout_storage_arguments(storage.reference) else {
            // A closed checked data shape or primitive cannot be the opaque layout
            // constructor. An unresolved argument still needs preparation.
            if typed
                .closed_data_definition(storage.reference)
                .is_some_and(|data| data.supply_mode == language_core::DataSupplyMode::CheckedShape)
                || typed.primitive_type_reference(storage.reference).is_some()
            {
                return Err(crate::reject(
                    id,
                    source,
                    format!(
                        "resident placement requires LayoutStorage<T, Layout> for its storage argument, but `{}` is an ordinary value type",
                        typed.display_type_reference(storage.reference)
                    ),
                ));
            }
            return Err(unavailable(
                "resident placement needs its closed LayoutStorage<T, Layout> selection",
            ));
        };
        let stored = storage
            .scope
            .supplied_type_reference(typed, layout.semantic_type)
            .map_err(unavailable)?;
        let supplied = self
            .source_scope
            .supplied_type_reference(typed, schema)
            .map_err(unavailable)?;
        let stored_id = stored.scope.index_identity(typed, stored.reference);
        let supplied_id = supplied.scope.index_identity(typed, supplied.reference);
        let (Some(stored_id), Some(supplied_id)) = (stored_id, supplied_id) else {
            return Err(unavailable(
                "resident placement needs closed semantic types for the storage and value",
            ));
        };
        if stored_id != supplied_id {
            return Err(crate::reject(
                id,
                source,
                format!(
                    "resident placement storage represents `{}`, but the operation requires `{}`",
                    typed.display_type_reference(stored.reference),
                    typed.display_type_reference(supplied.reference)
                ),
            ));
        }
        Err(unavailable(
            "resident placement needs evaluated layout, access policy, live content and the selected resource profile for this grant",
        ))
    }
}
