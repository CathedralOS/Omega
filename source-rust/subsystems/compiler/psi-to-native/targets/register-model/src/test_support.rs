/*
We provide shared architecture-neutral test assertions for ISA register-model
test suites, verifying that normalized foreign-call templates,
aggregate-result call rows, Linux system-call rows and scalar-float call rows
keep their exact operands, implicit effects and clobbers.
*/

use crate::{
    FloatScalarCallRoster, RegisterConstraintCatalog, RegisterConstraintKey,
    RegisterInstructionConstraint, RegisterOperandAccess, RegisterOperandConstraint,
    RegisterUnitId, SystemCallRoster, ValidatedPhysicalRegisterModel, find_register_constraint_mut,
};

pub fn row(
    catalog: &RegisterConstraintCatalog,
    key: RegisterConstraintKey,
) -> &RegisterInstructionConstraint {
    catalog.constraint(key).unwrap()
}

pub fn row_mut(
    catalog: &mut RegisterConstraintCatalog,
    key: RegisterConstraintKey,
) -> &mut RegisterInstructionConstraint {
    find_register_constraint_mut(&mut catalog.constraints, key).unwrap()
}

pub struct ForeignTemplateRoster {
    pub templates: Vec<(RegisterConstraintKey, Vec<RegisterConstraintKey>)>, // Base register-unit call row and its imported ABI templates.
    pub signature_views: [&'static str; 2], // Signature registers a template must never pin (integer result, vector result).
    pub effect_view: &'static str, // Register whose write units the template must keep clobbered.
}

fn imported_template(
    template: &(RegisterConstraintKey, Vec<RegisterConstraintKey>),
) -> RegisterConstraintKey {
    let [key] = template.1.as_slice() else {
        panic!("one import template per ABI");
    };
    *key
}

pub fn assert_templates_retain_call_effects(
    catalog: &RegisterConstraintCatalog,
    roster: &ForeignTemplateRoster,
) {
    for template in &roster.templates {
        let imported = row(catalog, imported_template(template));
        let base = row(catalog, template.0);
        assert!(imported.operands.is_empty());
        assert_eq!(imported.implicit_uses, base.implicit_uses);
        assert_eq!(imported.implicit_defs, base.implicit_defs);
        assert_eq!(imported.clobbers, base.clobbers);
    }
}

pub fn assert_templates_reject_changed_effects(
    model: &ValidatedPhysicalRegisterModel,
    catalog: &RegisterConstraintCatalog,
    roster: &ForeignTemplateRoster,
    validates: impl Fn(RegisterConstraintCatalog, &ValidatedPhysicalRegisterModel) -> bool,
) {
    let signature = roster
        .signature_views
        .map(|name| model.model().view_named(name).unwrap());
    let effect = model.model().view_named(roster.effect_view).unwrap();
    for template in &roster.templates {
        let key = imported_template(template);
        for mutation in 0..5 {
            let mut changed = catalog.clone();
            let imported = row_mut(&mut changed, key);
            match mutation {
                0..=2 => {
                    let view = signature[usize::from(mutation == 2)];
                    imported.operands.push(RegisterOperandConstraint {
                        operand: 0,
                        access: if mutation == 0 {
                            RegisterOperandAccess::Use
                        } else {
                            RegisterOperandAccess::Def
                        },
                        class: view.class,
                        fixed_view: Some(view.id),
                        tied_to: None,
                        early_clobber: false,
                    });
                }
                3 => imported.implicit_defs.clear(),
                4 => imported
                    .clobbers
                    .retain(|unit| !effect.write_units.contains(unit)),
                _ => unreachable!(),
            }
            assert!(!validates(changed, model), "{key:?}: mutation {mutation}");
        }
    }
}

pub struct AggregateFragmentRoster {
    pub keys: Vec<RegisterConstraintKey>, // Aggregate-result call rows in declaration order.
    pub keys_per_group: usize,            // Consecutive rows stating the same fragment count.
    pub second_view: &'static str,        // Register a one-fragment call leaves unclobbered.
    pub scratch_view: &'static str,       // Register every aggregate-result call must clobber.
}

pub fn assert_aggregate_fragments_are_definitions(
    model: &ValidatedPhysicalRegisterModel,
    catalog: &RegisterConstraintCatalog,
    roster: &AggregateFragmentRoster,
    validates: impl Fn(RegisterConstraintCatalog, &ValidatedPhysicalRegisterModel) -> bool,
) {
    let second = model.model().view_named(roster.second_view).unwrap();
    let scratch = model.model().view_named(roster.scratch_view).unwrap();
    for (ordinal, key) in roster.keys.iter().copied().enumerate() {
        let fragments = ordinal % (roster.keys_per_group * 2) / roster.keys_per_group + 1;
        let call = row(catalog, key);
        assert_eq!(
            call.operands
                .iter()
                .filter(|operand| operand.access == RegisterOperandAccess::Def)
                .count(),
            fragments
        );
        for unit in &second.write_units {
            assert_eq!(call.clobbers.contains(unit), fragments == 1, "{key:?}");
        }
        assert!(
            scratch
                .write_units
                .iter()
                .all(|unit| call.clobbers.contains(unit))
        );
        let mut altered = catalog.clone();
        row_mut(&mut altered, key)
            .clobbers
            .retain(|unit| !scratch.write_units.contains(unit));
        assert!(!validates(altered, model), "{key:?}");
    }
}

fn view_units(model: &ValidatedPhysicalRegisterModel, names: &[&str]) -> Vec<RegisterUnitId> {
    let mut units = names
        .iter()
        .flat_map(|name| {
            model
                .model()
                .view_named(name)
                .unwrap()
                .units
                .iter()
                .copied()
        })
        .collect::<Vec<_>>();
    units.sort_unstable();
    units.dedup();
    units
}

/// `forged_argument` is a real register that is not the fourth kernel argument.
pub fn assert_system_call_rows(
    model: &ValidatedPhysicalRegisterModel,
    catalog: &RegisterConstraintCatalog,
    roster: &SystemCallRoster,
    forged_argument: &str,
    validates: impl Fn(RegisterConstraintCatalog, &ValidatedPhysicalRegisterModel) -> bool,
) {
    let view = |name: &str| model.model().view_named(name).unwrap();
    assert!(validates(catalog.clone(), model), "canonical syscall rows");
    for (position, key) in roster.keys.iter().copied().enumerate() {
        let argument_count = position / 2;
        let returns_value = position % 2 != 0;
        let row = row(catalog, key);
        assert_eq!(
            row.operands.len(),
            argument_count + usize::from(returns_value)
        );
        for (operand, name) in row.operands.iter().zip(&roster.arguments[..argument_count]) {
            assert_eq!(operand.access, RegisterOperandAccess::Use);
            assert_eq!(operand.fixed_view, Some(view(name).id));
        }
        if returns_value {
            let result = row.operands.last().unwrap();
            assert_eq!(result.access, RegisterOperandAccess::Def);
            assert_eq!(result.fixed_view, Some(view(roster.result).id));
        }
        let mut written = row.clobbers.clone();
        if returns_value {
            written.extend_from_slice(&view(roster.result).units);
        }
        written.sort_unstable();
        written.dedup();
        assert_eq!(written, view_units(model, roster.clobbers));
        assert_eq!(row.implicit_uses, view(roster.instruction_pointer).units);
        assert_eq!(row.implicit_defs, row.implicit_uses);
    }

    let key = roster.keys[8];
    let mut wrong_argument = catalog.clone();
    row_mut(&mut wrong_argument, key).operands[3].fixed_view = Some(view(forged_argument).id);
    assert!(!validates(wrong_argument, model));
    let mut missing_clobbers = catalog.clone();
    row_mut(&mut missing_clobbers, key).clobbers.clear();
    assert!(!validates(missing_clobbers, model));
}

/// `home` is the scalar-float result register and `other` a different vector register.
pub fn assert_float_scalar_call_rows(
    model: &ValidatedPhysicalRegisterModel,
    catalog: &RegisterConstraintCatalog,
    rosters: &[FloatScalarCallRoster],
    [home, other]: [&str; 2],
    validates: impl Fn(RegisterConstraintCatalog, &ValidatedPhysicalRegisterModel) -> bool,
) {
    assert!(validates(catalog.clone(), model), "complete catalog");
    let mut keys = rosters
        .iter()
        .flat_map(|roster| roster.keys.iter().copied().chain([roster.return_key]))
        .collect::<Vec<_>>();
    let count = keys.len();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), count, "ABI key ranges do not overlap");
    for key in keys {
        assert_eq!(
            catalog
                .constraints
                .iter()
                .filter(|row| row.key == key)
                .count(),
            1
        );
    }
    let result = model.model().view_named(home).unwrap();
    let call_key = rosters[0].keys[0];
    let call = row(catalog, call_key);
    assert_eq!(call.operands.last().unwrap().fixed_view, Some(result.id));
    assert!(
        result
            .write_units
            .iter()
            .all(|unit| !call.clobbers.contains(unit))
    );
    let input = row(catalog, rosters[0].inputs[0]);
    assert!(
        input
            .clobbers
            .iter()
            .all(|unit| call.clobbers.contains(unit) || result.write_units.contains(unit)),
        "every caller-save effect remains either a Def write or a clobber"
    );
    let mut wrong_home = catalog.clone();
    row_mut(&mut wrong_home, call_key)
        .operands
        .last_mut()
        .unwrap()
        .fixed_view = Some(model.model().view_named(other).unwrap().id);
    assert!(!validates(wrong_home, model));
    let mut wrong_clobbers = catalog.clone();
    row_mut(&mut wrong_clobbers, call_key).clobbers.clear();
    assert!(!validates(wrong_clobbers, model));
}
