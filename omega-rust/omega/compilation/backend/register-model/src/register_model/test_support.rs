//! The normalized foreign-call template conformance bodies, stated once for
//! every architecture. Only the template keys and the registers that prove
//! them differ, so each ISA's test module hands its own roster to these.

use crate::{
    RegisterConstraintCatalog, RegisterConstraintKey, RegisterOperandAccess,
    RegisterOperandConstraint, ValidatedPhysicalRegisterModel, find_register_constraint_mut,
};

/// One architecture's normalized foreign-call templates and the registers
/// that prove their effects stay complete.
pub struct ForeignTemplateRoster {
    /// Each base register-unit call row with the normalized templates
    /// imported from it, one template per ABI.
    pub templates: Vec<(RegisterConstraintKey, Vec<RegisterConstraintKey>)>,
    /// The signature registers a template must never pin: the integer result
    /// and the vector result.
    pub signature_views: [&'static str; 2],
    /// A register whose write units the template must keep clobbered.
    pub effect_view: &'static str,
}

/// Every template repeats its base row's effects and carries no signature
/// operands of its own.
pub fn assert_templates_retain_call_effects(
    catalog: &RegisterConstraintCatalog,
    roster: &ForeignTemplateRoster,
) {
    for template in &roster.templates {
        let [key] = template.1.as_slice() else {
            panic!("one import template per ABI");
        };
        let row = catalog.constraint(*key).unwrap();
        let base = catalog.constraint(template.0).unwrap();
        assert!(row.operands.is_empty());
        assert_eq!(row.implicit_uses, base.implicit_uses);
        assert_eq!(row.implicit_defs, base.implicit_defs);
        assert_eq!(row.clobbers, base.clobbers);
    }
}

/// A template that gains a signature operand, drops an implicit definition or
/// loses a clobbered unit is rejected.
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
        for key in &template.1 {
            for mutation in 0..5 {
                let mut changed = catalog.clone();
                let row = find_register_constraint_mut(&mut changed.constraints, *key).unwrap();
                match mutation {
                    0..=2 => {
                        let view = signature[usize::from(mutation == 2)];
                        row.operands.push(RegisterOperandConstraint {
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
                    3 => row.implicit_defs.clear(),
                    4 => row
                        .clobbers
                        .retain(|unit| !effect.write_units.contains(unit)),
                    _ => unreachable!(),
                }
                assert!(!validates(changed, model), "{key:?}: mutation {mutation}");
            }
        }
    }
}
