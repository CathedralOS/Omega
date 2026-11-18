/*
We provide shared architecture-neutral test assertions for ISA register-model
test suites, verifying that normalized foreign-call templates and
aggregate-result call rows preserve their base implicit effects and clobbers.
*/

use crate::{
    RegisterConstraintCatalog, RegisterConstraintKey, RegisterInstructionConstraint,
    RegisterOperandAccess, RegisterOperandConstraint, ValidatedPhysicalRegisterModel,
    find_register_constraint_mut,
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
