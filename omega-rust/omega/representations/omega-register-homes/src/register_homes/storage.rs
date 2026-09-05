//! Function-local physical views assigned to virtual registers.

use omega_register_model::{RegisterClassId, RegisterViewId};
use omega_selected_instructions::VirtualRegisterId;
use psi_core::MachineId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionRegisterHomes {
    pub machine: MachineId,
    pub assignments: Vec<VirtualRegisterHome>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualRegisterHome {
    pub virtual_register: VirtualRegisterId,
    pub class: RegisterClassId,
    pub view: RegisterViewId,
}
