//! Shared construction rows for object-artifact custody fixtures.

use machine_code::MachineCodeFunction;
use semantic_vocabulary::MachineId;
use target_operations::TerminalPsiProvenance;

/// The empty machine-code function row every custody fixture starts from: no
/// ABI, no stack facts, no retained records and no bytes. Each fixture states
/// only the fields its rejection pin substitutes, so a new empty row on the
/// carrier cannot be forgotten by a fixture.
pub(crate) fn empty_machine_code_function(machine: MachineId) -> MachineCodeFunction {
    MachineCodeFunction {
        machine,
        attachment: None,
        nominal_cleanup_subject: None,
        scalar_abi: None,
        mixed_structural_scalar_abi: None,
        structural_call_scalar_return: None,
        parameter_abi: None,
        provenance: TerminalPsiProvenance {
            operations: Vec::new(),
            edges: Vec::new(),
        },
        bytes: Vec::new(),
        x86_floating_control: None,
        unit_stack: None,
        unit_parameter_homes: Vec::new(),
        unit_parameters: Vec::new(),
        scalar_stack: None,
        internal_calls: Vec::new(),
        foreign_calls: Vec::new(),
        internal_unit_calls: Vec::new(),
        internal_unit_scalar_calls: Vec::new(),
        installed_provider_unit_scalar_calls: Vec::new(),
        unit_scalar_homes: Vec::new(),
        unit_integer_constants: Vec::new(),
        unit_affine_scalar_records: Vec::new(),
        unit_structural_scalar_field_stores: Vec::new(),
        unit_write_only_primitive_stores: Vec::new(),
        scalar_structural_scalar_field_stores: Vec::new(),
        unit_affine_cleanup: None,
        unit_continuations: Vec::new(),
        scalar_affine_cleanup: None,
        scalar_control_affine_cleanups: Vec::new(),
        scalar_structural_parameters: Vec::new(),
        scalar_structural_parameter_homes: Vec::new(),
        semantic_code_attribution: Vec::new(),
        port_effects: Vec::new(),
        boundary_settlements: Vec::new(),
        structural_return: None,
    }
}
