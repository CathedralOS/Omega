//! The register-model test bodies, stated once for every architecture. Only
//! the rows and the registers that prove them differ, so each ISA's test
//! module hands its own roster to these.

use crate::{
    RegisterConstraintCatalog, RegisterConstraintKey, RegisterInstructionConstraint,
    RegisterOperandAccess, RegisterOperandConstraint, ValidatedPhysicalRegisterModel,
    find_register_constraint_mut,
};

/// The catalog row keyed `key`. Every roster a test module states is total, so
/// a miss means the roster and the catalog disagree rather than that the row is
/// optional.
pub fn row(
    catalog: &RegisterConstraintCatalog,
    key: RegisterConstraintKey,
) -> &RegisterInstructionConstraint {
    catalog.constraint(key).unwrap()
}

/// The catalog row keyed `key`, mutable for the mutation the test proves.
pub fn row_mut(
    catalog: &mut RegisterConstraintCatalog,
    key: RegisterConstraintKey,
) -> &mut RegisterInstructionConstraint {
    find_register_constraint_mut(&mut catalog.constraints, key).unwrap()
}

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

/// The one template key a roster entry imports from its base row. A roster that
/// named two would leave the ABI of one of them unstated.
fn imported_template(
    template: &(RegisterConstraintKey, Vec<RegisterConstraintKey>),
) -> RegisterConstraintKey {
    let [key] = template.1.as_slice() else {
        panic!("one import template per ABI");
    };
    *key
}

/// Every template repeats its base row's effects and carries no signature
/// operands of its own.
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

/// One architecture's aggregate-result call rows and the registers that prove
/// their fragment accounting.
pub struct AggregateFragmentRoster {
    /// Every aggregate-result call row of one architecture, one group per
    /// fragment count, in declaration order.
    pub keys: Vec<RegisterConstraintKey>,
    /// How many consecutive rows state the same fragment count.
    pub keys_per_group: usize,
    /// A register a one-fragment call leaves unclobbered.
    pub second_view: &'static str,
    /// A register every aggregate-result call must clobber.
    pub scratch_view: &'static str,
}

/// Each aggregate-result row defines exactly its fragment count, leaves the
/// second fragment's register alone unless it writes two fragments, keeps the
/// scratch register clobbered, and is rejected once that clobber is dropped.
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
