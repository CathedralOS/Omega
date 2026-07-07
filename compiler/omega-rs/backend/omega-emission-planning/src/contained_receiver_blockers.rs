use crate::EmissionPlanningInput;
use crate::blocker;
use omega_backend_report_types::EmissionBlocker;
use omega_core::arena::Arena;
use omega_core::symbols::SymbolHandle;
use omega_layout::{FieldLayout, LayoutPlan, MachineLayout};

/// Contained-machine method dispatch resolves the receiver's storage region by
/// TYPE: `nested_machine_storage_offset` (instruction selection, machine_owned.rs)
/// walks the caller's fields depth-first and takes the FIRST field whose type
/// matches the callee's attached data. With two same-type contained machines
/// (`a: Counter; b: Counter`), a call through `b` silently mutates `a` -- the
/// receiver FIELD never reaches storage resolution (the deep fix is threading
/// it through the dispatch; memory: contained-machine-same-type-aliasing).
///
/// Until that lands, reject exactly the calls the walk would misresolve: the
/// state-call plan carries `receiver_symbol`, so compare the receiver field's
/// actual offset against the offset the by-type walk picks. Equal -> the call
/// is sound (single instance, or the receiver IS the first match) and stays
/// accepted; different -> the call would run on another instance's storage,
/// which is never intended. Receivers that are not a direct field of the
/// caller's data (nested `self.x.y.method()`) are skipped -- conservative,
/// same as before this fence.
pub(crate) fn collect_contained_receiver_blockers(
    input: &EmissionPlanningInput<'_>,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, state_call) in input.state_calls.calls.iter() {
        if !state_call.reachable || state_call.receiver_name.as_str().is_empty() {
            continue;
        }
        if state_call.source_key.machine == state_call.target_key.machine {
            continue;
        }
        let Some(source_layout) =
            machine_layout_by_symbol(input.layouts, state_call.source_key.machine)
        else {
            continue;
        };
        let Some(target_layout) =
            machine_layout_by_symbol(input.layouts, state_call.target_key.machine)
        else {
            continue;
        };
        let Some(target_attached_data) = target_layout.attached_data.as_deref() else {
            continue;
        };
        // The receiver must be a DIRECT field of the caller's data for the
        // offset comparison to be meaningful here. Matched by NAME: the
        // receiver symbol and the layout field symbol live in different
        // arenas, so handle equality can never hold.
        let Some(receiver_field) = input
            .layouts
            .fields
            .span(source_layout.fields)
            .and_then(|fields| {
                fields
                    .iter()
                    .find(|field| field.name == state_call.receiver_name)
            })
        else {
            continue;
        };
        let walk_offset = first_type_match_offset(
            input.layouts,
            source_layout,
            state_call.target_key.machine,
            target_attached_data,
            0,
            &mut Vec::new(),
        );
        let Some(walk_offset) = walk_offset else {
            continue;
        };
        if walk_offset == receiver_field.offset {
            continue;
        }
        blockers.insert(blocker(
            "state calls",
            &format!(
                "contained-machine call receiver `{}` (a `{}` at offset {}) would run on \
                 ANOTHER instance's storage: dispatch currently resolves the receiver \
                 region by TYPE and picks the first `{}` (offset {}). Until per-instance \
                 dispatch lands, give each instance a distinct data type, or mutate \
                 `{}`'s fields directly instead of through a method call.",
                receiver_field.name,
                receiver_field.type_name,
                receiver_field.offset,
                target_attached_data,
                walk_offset,
                receiver_field.name,
            ),
        ));
    }
}

fn machine_layout_by_symbol(
    layouts: &LayoutPlan,
    machine: SymbolHandle,
) -> Option<&MachineLayout> {
    layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| machine_layout.symbol == machine)
        .map(|(_, machine_layout)| machine_layout)
}

/// Mirror of instruction selection's `nested_machine_storage_offset` FIELD WALK
/// (machine_owned.rs): depth-first over the caller's fields, first match by the
/// callee's attached-data type name (or by nested machine symbol), earlier
/// fields' nested matches win over later direct matches. Keep the two in
/// lockstep -- this predicts which offset the backend will resolve.
fn first_type_match_offset(
    layouts: &LayoutPlan,
    machine_layout: &MachineLayout,
    target_machine: SymbolHandle,
    target_attached_data: &str,
    base_offset: usize,
    visited: &mut Vec<SymbolHandle>,
) -> Option<usize> {
    if visited.contains(&machine_layout.symbol) {
        return None;
    }
    visited.push(machine_layout.symbol);

    let Some(fields) = layouts.fields.span(machine_layout.fields) else {
        visited.pop();
        return None;
    };

    for field in fields {
        let field_offset = base_offset + field.offset;

        if field.type_name.as_ref() == target_attached_data {
            visited.pop();
            return Some(field_offset);
        }

        let nested = field_machine_layout(layouts, field);

        if nested.is_some_and(|nested| nested.symbol == target_machine) {
            visited.pop();
            return Some(field_offset);
        }

        let Some(nested) = nested else {
            continue;
        };

        if let Some(offset) = first_type_match_offset(
            layouts,
            nested,
            target_machine,
            target_attached_data,
            field_offset,
            visited,
        ) {
            visited.pop();
            return Some(offset);
        }
    }

    visited.pop();
    None
}

fn field_machine_layout<'plan>(
    layouts: &'plan LayoutPlan,
    field: &FieldLayout,
) -> Option<&'plan MachineLayout> {
    layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| {
            machine_layout.symbol == field.type_symbol
                || machine_layout.name.as_str() == field.type_name.as_ref()
                || machine_layout
                    .attached_data
                    .as_deref()
                    .is_some_and(|attached_data| attached_data == field.type_name.as_ref())
        })
        .map(|(_, machine_layout)| machine_layout)
}
