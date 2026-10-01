//! Returning Linux kernel entries use the ordinary foreign argument/result
//! transport, but not the user-space call register contract. Only ABI arity
//! and presence of a result vary here; widths, signedness, source expressions,
//! and syscall numbers do not create more constraint families.
//!
//! The encoding materializes the selected number internally. It therefore
//! clobbers the number register rather than reading a fabricated source operand.
//! A discarded result still clobbers its register, and supervisor entry neither
//! pushes a user-space return address nor writes a link register.

use register_model::{
    RegisterConstraintFamily, RegisterConstraintId, RegisterConstraintKey,
    RegisterInstructionConstraint, RegisterOperandAccess, RegisterOperandConstraint,
    ValidatedPhysicalRegisterModel,
};

/// Argument-count-major kernel entry rows, Unit then integer result.
pub fn aarch64_linux_system_call_keys() -> Vec<RegisterConstraintKey> {
    (100..114)
        .map(|variant| RegisterConstraintKey {
            family: RegisterConstraintFamily::SystemCall,
            variant,
        })
        .collect()
}

pub(super) fn append_constraints(
    constraints: &mut Vec<RegisterInstructionConstraint>,
    model: &ValidatedPhysicalRegisterModel,
) {
    let physical = model.model();
    let view = |name| {
        physical
            .view_named(name)
            .expect("canonical kernel register")
    };
    let arguments = ["x0", "x1", "x2", "x3", "x4", "x5"];
    let result = view("x0");
    let clobbers = ["x0", "x1", "x2", "x3", "x4", "x5", "x8", "nzcv"]
        .into_iter()
        .flat_map(|name| view(name).units.iter().copied())
        .collect::<Vec<_>>();
    for (position, key) in aarch64_linux_system_call_keys().into_iter().enumerate() {
        let argument_count = position / 2;
        let returns_value = position % 2 != 0;
        let operands = arguments[..argument_count]
            .iter()
            .map(|name| (view(name), RegisterOperandAccess::Use))
            .chain(returns_value.then_some((result, RegisterOperandAccess::Def)))
            .enumerate()
            .map(|(operand, (register, access))| RegisterOperandConstraint {
                operand: operand as u16,
                access,
                class: register.class,
                fixed_view: Some(register.id),
                tied_to: None,
                early_clobber: false,
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
            implicit_uses: view("pc").units.clone(),
            implicit_defs: view("pc").units.clone(),
            clobbers,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_call_rows_retain_exact_arguments_results_and_clobbers() {
        let physical = crate::validated_aarch64_physical_register_model();
        let catalog = crate::aarch64_register_constraint_catalog(physical);
        let validated =
            crate::validate_aarch64_register_constraint_catalog(catalog.clone(), physical)
                .expect("canonical syscall constraints");
        for (position, key) in aarch64_linux_system_call_keys().into_iter().enumerate() {
            let argument_count = position / 2;
            let returns_value = position % 2 != 0;
            let row = validated
                .catalog()
                .constraints
                .iter()
                .find(|row| row.key == key)
                .unwrap();
            assert_eq!(
                row.operands.len(),
                argument_count + usize::from(returns_value)
            );
            let expected = ["x0", "x1", "x2", "x3", "x4", "x5"];
            for (operand, name) in row.operands.iter().zip(&expected[..argument_count]) {
                assert_eq!(operand.access, RegisterOperandAccess::Use);
                assert_eq!(
                    operand.fixed_view,
                    Some(physical.model().view_named(name).unwrap().id)
                );
            }
            if returns_value {
                let result = row.operands.last().unwrap();
                assert_eq!(result.access, RegisterOperandAccess::Def);
                assert_eq!(
                    result.fixed_view,
                    Some(physical.model().view_named("x0").unwrap().id)
                );
            }
            let mut written = row.clobbers.clone();
            if returns_value {
                written.extend_from_slice(&physical.model().view_named("x0").unwrap().units);
            }
            written.sort_unstable();
            written.dedup();
            let mut expected_written = ["x0", "x1", "x2", "x3", "x4", "x5", "x8", "nzcv"]
                .into_iter()
                .flat_map(|name| {
                    physical
                        .model()
                        .view_named(name)
                        .unwrap()
                        .units
                        .iter()
                        .copied()
                })
                .collect::<Vec<_>>();
            expected_written.sort_unstable();
            expected_written.dedup();
            assert_eq!(written, expected_written);
            assert_eq!(
                row.implicit_uses,
                physical.model().view_named("pc").unwrap().units
            );
            assert_eq!(row.implicit_defs, row.implicit_uses);
        }

        let key = aarch64_linux_system_call_keys()[8];
        let mut wrong_argument = catalog.clone();
        let row = wrong_argument
            .constraints
            .iter_mut()
            .find(|row| row.key == key)
            .unwrap();
        row.operands[3].fixed_view = Some(physical.model().view_named("x4").unwrap().id);
        assert!(
            crate::validate_aarch64_register_constraint_catalog(wrong_argument, physical).is_err()
        );

        let mut missing_clobbers = catalog;
        let row = missing_clobbers
            .constraints
            .iter_mut()
            .find(|row| row.key == key)
            .unwrap();
        row.clobbers.clear();
        assert!(
            crate::validate_aarch64_register_constraint_catalog(missing_clobbers, physical)
                .is_err()
        );
    }
}
