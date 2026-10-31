use super::{
    assert_rejected, catalog_rejection, instruction_key, miniature_catalog, miniature_model,
    validated_miniature_model,
};
use crate::{
    RegisterClassId, RegisterConstraintCatalogValidationError as Rejected, RegisterConstraintId,
    RegisterUnitId, RegisterViewId, validate_physical_register_model,
    validate_register_constraint_catalog,
};
use target::Architecture;

#[test]
fn constraint_catalog_accepts_a_closed_required_inventory() {
    let validated =
        validate_register_constraint_catalog(miniature_catalog(), &validated_miniature_model())
            .expect("closed catalog must validate");

    assert_eq!(validated.architecture(), Architecture::X86_64);
    assert_eq!(validated.catalog().required, vec![instruction_key(7)]);
}

#[test]
fn constraint_catalog_rejects_missing_and_unexpected_inventory_rows() {
    catalog_rejection(
        |catalog| catalog.required.push(instruction_key(8)),
        Rejected::MissingRequiredConstraint(instruction_key(8)),
    );

    catalog_rejection(
        |catalog| {
            let mut row = catalog.constraints[0].clone();
            row.id = RegisterConstraintId(1);
            row.key = instruction_key(8);
            catalog.constraints.push(row);
        },
        Rejected::UnexpectedConstraint(instruction_key(8)),
    );
}

#[test]
fn constraint_catalog_rejects_noncanonical_ids_keys_and_operands() {
    catalog_rejection(
        |catalog| catalog.constraints[0].id = RegisterConstraintId(1),
        Rejected::NonCanonicalConstraintIds,
    );
    catalog_rejection(
        |catalog| catalog.required.push(instruction_key(7)),
        Rejected::NonCanonicalRequiredKeys,
    );
    catalog_rejection(
        |catalog| {
            let mut second_row = catalog.constraints[0].clone();
            second_row.id = RegisterConstraintId(1);
            catalog.constraints.push(second_row);
        },
        Rejected::NonCanonicalConstraintKeys,
    );
    catalog_rejection(
        |catalog| catalog.constraints[0].operands[1].operand = 0,
        Rejected::NonCanonicalOperands(RegisterConstraintId(0)),
    );
}

#[test]
fn constraint_catalog_rejects_fixed_view_class_corruption() {
    catalog_rejection(
        |catalog| catalog.constraints[0].operands[1].class = RegisterClassId(1),
        Rejected::FixedViewClassMismatch {
            constraint: RegisterConstraintId(0),
            operand: 1,
        },
    );
}

#[test]
fn constraint_catalog_rejects_unknown_or_unallocatable_operand_domains() {
    catalog_rejection(
        |catalog| catalog.constraints[0].operands[0].class = RegisterClassId(u16::MAX),
        Rejected::UnknownClass {
            constraint: RegisterConstraintId(0),
            class: RegisterClassId(u16::MAX),
        },
    );
    catalog_rejection(
        |catalog| catalog.constraints[0].operands[1].fixed_view = Some(RegisterViewId(u16::MAX)),
        Rejected::UnknownFixedView {
            constraint: RegisterConstraintId(0),
            view: RegisterViewId(u16::MAX),
        },
    );

    let mut physical = miniature_model();
    physical.views[1].allocatable = false;
    let physical = validate_physical_register_model(physical).unwrap();
    assert_rejected(
        miniature_catalog(),
        |catalog| catalog.constraints[0].operands[0].class = RegisterClassId(1),
        |catalog| validate_register_constraint_catalog(catalog, &physical),
        Rejected::UnallocatableOperandClass {
            constraint: RegisterConstraintId(0),
            operand: 0,
        },
    );
}

#[test]
fn constraint_catalog_rejects_malformed_ties_and_early_clobbers() {
    catalog_rejection(
        |catalog| catalog.constraints[0].operands[1].tied_to = Some(1),
        Rejected::InvalidOperandTie {
            constraint: RegisterConstraintId(0),
            operand: 1,
        },
    );
    catalog_rejection(
        |catalog| {
            catalog.constraints[0].operands[1].operand = 2;
            catalog.constraints[0].operands[1].tied_to = Some(1);
        },
        Rejected::InvalidOperandTie {
            constraint: RegisterConstraintId(0),
            operand: 2,
        },
    );
    catalog_rejection(
        |catalog| {
            catalog.constraints[0].operands[1].class = RegisterClassId(1);
            catalog.constraints[0].operands[1].fixed_view = None;
        },
        Rejected::IncompatibleOperandTie {
            constraint: RegisterConstraintId(0),
            operand: 1,
            tied_to: 0,
        },
    );
    catalog_rejection(
        |catalog| catalog.constraints[0].operands[0].early_clobber = true,
        Rejected::InvalidEarlyClobber {
            constraint: RegisterConstraintId(0),
            operand: 0,
        },
    );
}

#[test]
fn constraint_catalog_rejects_implicit_effect_corruption() {
    catalog_rejection(
        |catalog| catalog.constraints[0].implicit_uses.push(RegisterUnitId(0)),
        Rejected::NonCanonicalImplicitUses(RegisterConstraintId(0)),
    );
    catalog_rejection(
        |catalog| catalog.constraints[0].clobbers = vec![RegisterUnitId(2)],
        Rejected::UnknownUnit {
            constraint: RegisterConstraintId(0),
            unit: RegisterUnitId(2),
        },
    );
    catalog_rejection(
        |catalog| catalog.constraints[0].implicit_defs = vec![RegisterUnitId(1)],
        Rejected::DefClobberOverlap {
            constraint: RegisterConstraintId(0),
            unit: RegisterUnitId(1),
        },
    );
}

#[test]
fn constraint_catalog_is_bound_to_the_validated_model_architecture() {
    catalog_rejection(
        |catalog| catalog.architecture = Architecture::Aarch64,
        Rejected::ArchitectureMismatch,
    );
}

#[test]
fn constraint_catalog_accepts_an_explicit_register_free_row() {
    let model = validated_miniature_model();
    let mut catalog = miniature_catalog();
    let row = &mut catalog.constraints[0];
    row.operands.clear();
    row.implicit_uses.clear();
    row.implicit_defs.clear();
    row.clobbers.clear();
    assert!(validate_register_constraint_catalog(catalog, &model).is_ok());
}

#[test]
fn validated_lookup_preserves_full_keys_missing_rows_and_revalidated_snapshots() {
    use crate::{RegisterConstraintFamily, RegisterConstraintKey, find_register_constraint};

    let families = [
        RegisterConstraintFamily::Call,
        RegisterConstraintFamily::Return,
        RegisterConstraintFamily::SystemCall,
        RegisterConstraintFamily::InlineAssembly,
        RegisterConstraintFamily::Instruction,
    ];
    let model = validated_miniature_model();
    let mut raw = miniature_catalog();
    let template = raw.constraints[0].clone();
    let keys: Vec<_> = families
        .iter()
        .flat_map(|&family| {
            [0, 1, u32::MAX].map(|variant| RegisterConstraintKey { family, variant })
        })
        .collect();
    raw.required = keys.clone();
    raw.constraints = keys
        .iter()
        .enumerate()
        .map(|(position, &key)| crate::RegisterInstructionConstraint {
            id: RegisterConstraintId(u16::try_from(position).unwrap()),
            key,
            ..template.clone()
        })
        .collect();
    let validated = validate_register_constraint_catalog(raw.clone(), &model).unwrap();
    for family in families {
        for variant in [0, 1, 2, u32::MAX - 1, u32::MAX] {
            let key = RegisterConstraintKey { family, variant };
            let original = find_register_constraint(&validated.catalog().constraints, key);
            let found = validated.constraint(key);
            assert_eq!(found, original);
            if let Some(found) = found {
                assert!(std::ptr::eq(found, original.unwrap()));
                assert!(std::ptr::eq(
                    found,
                    &validated.catalog().constraints[usize::from(found.id.0)]
                ));
            }
        }
    }
    // Raw catalogs retain arbitrary order and exact first-match behavior.
    let mut duplicate = vec![template.clone(), template.clone()];
    duplicate[0].id = RegisterConstraintId(24);
    duplicate[1].id = RegisterConstraintId(25);
    assert_eq!(
        find_register_constraint(&duplicate, template.key),
        Some(&duplicate[0])
    );
    duplicate.reverse();
    assert_eq!(
        find_register_constraint(&duplicate, template.key),
        Some(&duplicate[0])
    );
    for duplicate in [true, false] {
        let mut invalid = raw.clone();
        if duplicate {
            invalid.constraints[1].key = invalid.constraints[0].key;
        } else {
            invalid.constraints.swap(0, 1);
            for (position, row) in invalid.constraints.iter_mut().enumerate() {
                row.id = RegisterConstraintId(u16::try_from(position).unwrap());
            }
        }
        assert_eq!(
            validate_register_constraint_catalog(invalid, &model),
            Err(Rejected::NonCanonicalConstraintKeys)
        );
    }
    // Appending a raw row requires a new validation; it cannot stale a borrow.
    let appended_key = RegisterConstraintKey::call(2);
    let mut appended = template;
    appended.id = RegisterConstraintId(u16::try_from(raw.constraints.len()).unwrap());
    appended.key = appended_key;
    raw.constraints.push(appended);
    raw.required.push(appended_key);
    raw.required.sort();
    assert_eq!(
        validate_register_constraint_catalog(raw.clone(), &model),
        Err(Rejected::NonCanonicalConstraintKeys)
    );
    raw.constraints.sort_by_key(|row| row.key);
    for (position, row) in raw.constraints.iter_mut().enumerate() {
        row.id = RegisterConstraintId(u16::try_from(position).unwrap());
    }
    let replacement = validate_register_constraint_catalog(raw.clone(), &model).unwrap();
    assert!(validated.constraint(appended_key).is_none());
    assert_eq!(
        replacement.constraint(appended_key),
        find_register_constraint(&raw.constraints, appended_key)
    );
    raw.required.clear();
    raw.constraints.clear();
    let empty = validate_register_constraint_catalog(raw, &model).unwrap();
    assert!(empty.constraint(appended_key).is_none());
    assert!(replacement.constraint(appended_key).is_some());
}
