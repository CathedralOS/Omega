//! Borrowed first-match home lookup, prepared once for a function's operands.
//!
//! Home rows need not be densely numbered or already sorted. Stable sorting
//! preserves the original first row for duplicate register IDs, so preparation
//! changes neither missing-home behavior nor the operand where an error occurs.
//! The index borrows assignments; it never rewrites their order or identity.

use selected_instructions::VirtualRegisterId;
use selected_instructions_to_register_homes::{FunctionRegisterHomes, VirtualRegisterHome};

pub(super) struct PreparedHomes<'homes> {
    assignments: Vec<&'homes VirtualRegisterHome>,
}

impl<'homes> PreparedHomes<'homes> {
    pub(super) fn new(homes: &'homes FunctionRegisterHomes) -> Self {
        let mut assignments = homes.assignments.iter().collect::<Vec<_>>();
        assignments.sort_by_key(|home| home.virtual_register);
        Self { assignments }
    }

    pub(super) fn get(&self, register: VirtualRegisterId) -> Option<&'homes VirtualRegisterHome> {
        let index = self
            .assignments
            .partition_point(|home| home.virtual_register < register);
        self.assignments
            .get(index)
            .copied()
            .filter(|home| home.virtual_register == register)
    }
}
