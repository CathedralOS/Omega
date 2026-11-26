/*
Both ISA constraint catalogs derive some rows the same way and differ only in
register names and key ranges: Linux system-call rows come from the kernel's
argument, result and clobber registers, scalar-float call rows come from the
integer call rows by pinning a result home, and each catalog validator compares
a submitted catalog with its canonical rows. We keep each mechanism here once so
an ISA supplies only its data and neither ISA can drift from the other.
*/

use crate::{
    RegisterClassId, RegisterConstraintCatalog, RegisterConstraintId, RegisterConstraintKey,
    RegisterInstructionConstraint, RegisterOperandAccess, RegisterOperandConstraint, RegisterView,
    ValidatedPhysicalRegisterModel, find_register_constraint,
};

impl RegisterOperandConstraint {
    pub fn fixed(operand: u16, access: RegisterOperandAccess, view: &RegisterView) -> Self {
        Self {
            operand,
            access,
            class: view.class,
            fixed_view: Some(view.id),
            tied_to: None,
            early_clobber: false,
        }
    }

    pub fn allocatable(
        operand: u16,
        access: RegisterOperandAccess,
        class: RegisterClassId,
    ) -> Self {
        Self {
            operand,
            access,
            class,
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        }
    }
}

/// One ISA's Linux supervisor-call registers. Key `position` carries
/// `position / 2` arguments and returns a value when `position` is odd.
pub struct SystemCallRoster {
    pub keys: Vec<RegisterConstraintKey>,
    pub arguments: [&'static str; 6],
    pub result: &'static str,
    pub clobbers: &'static [&'static str], // Every register the kernel entry may write.
    pub instruction_pointer: &'static str,
}

pub fn append_system_call_constraints(
    constraints: &mut Vec<RegisterInstructionConstraint>,
    model: &ValidatedPhysicalRegisterModel,
    roster: &SystemCallRoster,
) {
    let physical = model.model();
    let view = |name| {
        physical
            .view_named(name)
            .expect("canonical kernel register")
    };
    let result = view(roster.result);
    let clobbers = roster
        .clobbers
        .iter()
        .flat_map(|name| view(name).units.iter().copied())
        .collect::<Vec<_>>();
    let instruction_pointer = &view(roster.instruction_pointer).units;
    for (position, key) in roster.keys.iter().copied().enumerate() {
        let argument_count = position / 2;
        let returns_value = position % 2 != 0;
        let operands = roster.arguments[..argument_count]
            .iter()
            .map(|name| (view(name), RegisterOperandAccess::Use))
            .chain(returns_value.then_some((result, RegisterOperandAccess::Def)))
            .enumerate()
            .map(|(operand, (register, access))| {
                RegisterOperandConstraint::fixed(operand as u16, access, register)
            })
            .collect();
        let mut clobbers = clobbers
            .iter()
            .copied()
            .filter(|unit| !returns_value || !result.units.contains(unit))
            .collect::<Vec<_>>();
        clobbers.sort_unstable();
        clobbers.dedup();
        constraints.push(RegisterInstructionConstraint {
            id: RegisterConstraintId(0),
            key,
            operands,
            implicit_uses: instruction_pointer.clone(),
            implicit_defs: instruction_pointer.clone(),
            clobbers,
        });
    }
}

/// One calling convention's scalar-float call rows. Each input call row,
/// followed again by each mixed input row, becomes the next key in `keys` with
/// one more `Def` operand: `float_result` for the inputs and `integer_result`
/// for the repeated mixed rows.
pub struct FloatScalarCallRoster {
    pub inputs: Vec<RegisterConstraintKey>, // Register-unit call rows, then mixed-unit call rows.
    pub mixed: Vec<RegisterConstraintKey>,
    pub keys: Vec<RegisterConstraintKey>,
    pub return_base: RegisterConstraintKey,
    pub return_key: RegisterConstraintKey,
}

/// Appends every convention's rows, or stops at the first missing register or
/// base row so the catalog validator refuses the incomplete roster.
pub fn append_float_scalar_call_constraints(
    constraints: &mut Vec<RegisterInstructionConstraint>,
    model: &ValidatedPhysicalRegisterModel,
    [float_result, integer_result]: [&str; 2],
    rosters: &[FloatScalarCallRoster],
) {
    let Some(float_home) = model.model().view_named(float_result) else {
        return;
    };
    for roster in rosters {
        let inputs = roster
            .inputs
            .iter()
            .map(|key| (*key, float_result))
            .chain(roster.mixed.iter().map(|key| (*key, integer_result)));
        for ((input, result_name), key) in inputs.zip(roster.keys.iter().copied()) {
            let Some(result) = model.model().view_named(result_name) else {
                return;
            };
            let Some(mut row) = find_register_constraint(constraints, input).cloned() else {
                return;
            };
            row.key = key;
            row.operands.push(RegisterOperandConstraint::fixed(
                row.operands.len() as u16,
                RegisterOperandAccess::Def,
                result,
            ));
            row.clobbers
                .retain(|unit| !result.write_units.contains(unit));
            constraints.push(row);
        }
        let Some(mut row) = find_register_constraint(constraints, roster.return_base).cloned()
        else {
            return;
        };
        row.key = roster.return_key;
        row.operands = vec![RegisterOperandConstraint::fixed(
            0,
            RegisterOperandAccess::Use,
            float_home,
        )];
        constraints.push(row);
    }
}

/// Returns the first required key whose row is missing from `actual` or differs
/// from `canonical`, or else the first row `canonical` does not require.
pub fn canonical_row_mismatch(
    actual: &RegisterConstraintCatalog,
    canonical: &RegisterConstraintCatalog,
) -> Option<RegisterConstraintKey> {
    let actual_rows = &actual.constraints;
    for key in canonical.required.iter().copied() {
        let Ok(actual_position) = actual_rows.binary_search_by_key(&key, |row| row.key) else {
            return Some(key);
        };
        let expected_position = canonical
            .constraints
            .binary_search_by_key(&key, |row| row.key)
            .expect("target-owned inventory and rows are closed together");
        if actual_rows[actual_position] != canonical.constraints[expected_position] {
            return Some(key);
        }
    }
    actual_rows
        .iter()
        .find(|constraint| canonical.required.binary_search(&constraint.key).is_err())
        .map(|constraint| constraint.key)
}
