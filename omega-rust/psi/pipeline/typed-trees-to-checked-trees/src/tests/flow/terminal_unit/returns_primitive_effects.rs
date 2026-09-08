use super::*;

#[test]
fn primitive_store_scalar_return_retains_source_effect_and_borrow() {
    for (access, expected_access) in [
        ("mut", checked_trees::CheckedStructuralAccess::MutableBorrow),
        (
            "write",
            checked_trees::CheckedStructuralAccess::WriteOnlyBorrow,
        ),
    ] {
        let checked = checked(&format!(
            "machine reset(value: &{access} u64) -> u64 {{ value = 0; 0 }}"
        ));
        let machine = machine_named(&checked, "reset");
        let plan = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine)
            .expect("exclusive primitive store followed by scalar return");
        assert_eq!(plan.attachment_type_identity, None);
        assert!(plan.bindings.is_empty());
        assert!(plan.cleanup_actions.is_empty());
        assert!(plan.scalar_parameters.is_empty());
        assert_eq!(plan.result_type, PrimitiveType::U64);
        assert_eq!(plan.return_statement_ordinal, 1);
        assert!(matches!(
            plan.structural_parameters.as_slice(),
            [parameter]
                if parameter.position == 0
                    && parameter.access == expected_access
                    && parameter.multiplicity == Multiplicity::Unrestricted
                    && parameter.qualifications.is_empty()
        ));
        assert!(matches!(
            plan.effects.as_slice(),
            [CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                statement_index: 0,
                destination: checked_trees::CheckedPrimitiveStoreDestination::Parameter { parameter_index: 0 },
                value: CheckedScalarExpression::IntegerLiteral { literal },
            }] if literal.value_i64() == Some(0)
        ));
        assert!(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(machine)
                .is_none()
        );
        assert!(matches!(
            checked.facts.values.scalar_expressions.expression_at(
                plan.state,
                1,
                CheckedScalarExpressionRole::Return,
            ),
            Some(CheckedScalarExpression::IntegerLiteral { literal })
                if literal.value_i64() == Some(0)
        ));
    }
}

#[test]
fn primitive_store_scalar_return_rejects_unrepresented_prefix_statements() {
    for body in [
        "value = 0; value = 1; 0",
        "let saved: u64 = 0; value = saved; 0",
    ] {
        let checked = checked(&format!(
            "machine reset(value: &mut u64) -> u64 {{ {body} }}"
        ));
        assert!(
            checked
                .facts
                .flow
                .terminal_structural_scalar_returns
                .for_machine(machine_named(&checked, "reset"))
                .is_none(),
            "unsupported effect prefix must not disappear: {body}",
        );
    }
}
