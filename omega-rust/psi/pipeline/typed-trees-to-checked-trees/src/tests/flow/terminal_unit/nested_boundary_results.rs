//! Ordinary affine temporaries retain their source ownership at boundary calls.

use super::*;

#[test]
fn nested_ordinary_results_keep_postorder_and_exact_boundary_operand_roles() {
    for (declaration, invocation, nominal) in [
        (
            "pub data Sink {} boundary machine Sink::take(first: u16, token: Token, last: u16) ensures true;",
            "Sink::take",
            false,
        ),
        (
            "boundary trait Sink { machine take(first: u16, token: Token, last: u16); }",
            "Sink::take",
            false,
        ),
        (
            "boundary trait Sink { machine take(first: u16, token: Token, last: u16); }",
            "Take",
            true,
        ),
    ] {
        let signature = if nominal {
            "machine Root::enter<machine Take>(token: Token, input: u16) where machine Take satisfies Sink::take;"
        } else {
            "machine Root::enter(token: Token, input: u16)"
        };
        let source = format!(
            r#"
            pub data Token {{ flag: bool; }}
            {declaration}
            machine identity(input: u16) -> u16 {{ input }}
            machine forward(first: u16, token: Token, last: u16) -> Token {{ token }}
            data Root {{}}
            {signature} {{
                let prior: u16 = input;
                {invocation}(identity(prior),
                    forward(identity(11u16), forward(identity(22u16), token, identity(33u16)), identity(44u16)),
                    identity(55u16));
            }}
        "#
        );
        let checked = checked(&source);
        let machine = machine_named(&checked, "enter");
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine)
            .expect("nested ordinary result boundary call");
        let [
            CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. },
            CheckedUnitEffectOperationPlan::StructuralCall {
                coordinate: inner,
                result: inner_result,
                discard_result_on_return: false,
                ..
            },
            CheckedUnitEffectOperationPlan::StructuralCall {
                coordinate: outer,
                result: outer_result,
                discard_result_on_return: false,
                structural_arguments: outer_arguments,
                ..
            },
            CheckedUnitEffectOperationPlan::BoundaryCall {
                coordinate: boundary,
                structural_arguments,
                completion_receipts,
                ..
            },
            CheckedUnitEffectOperationPlan::ReturnUnit { .. },
        ] = plan.operations.as_slice()
        else {
            panic!("two producers execute before their boundary consumer")
        };
        assert_eq!(
            (
                inner.statement_index,
                outer.statement_index,
                boundary.statement_index
            ),
            (1, 1, 1)
        );
        assert!(
            inner.call_ordinal > outer.call_ordinal && outer.call_ordinal > boundary.call_ordinal
        );
        assert_eq!(boundary.call_ordinal, 0);
        assert_eq!(
            (inner_result.binding_ordinal, outer_result.binding_ordinal),
            (0, 1)
        );
        assert_eq!(
            outer_arguments[0].source_structural_result_binding_ordinal(),
            Some(0)
        );
        assert_eq!(
            structural_arguments[0].source_structural_result_binding_ordinal(),
            Some(1)
        );
        assert!(completion_receipts.is_empty());
        let computations = &checked.facts.values.scalar_computations;
        for coordinate in [inner, outer, boundary] {
            for argument_ordinal in 0..2 {
                let role = if coordinate.call_ordinal == 0 {
                    CheckedScalarExpressionRole::BoundaryCallArgument {
                        call_ordinal: 0,
                        argument_ordinal,
                    }
                } else {
                    CheckedScalarExpressionRole::UnitCallArgument {
                        call_ordinal: coordinate.call_ordinal,
                        argument_ordinal,
                    }
                };
                let root = computations
                    .root_at(plan.state, 1, role)
                    .expect("exact operand role");
                assert_eq!(root.machine, machine);
            }
        }
        let temporary_moves = checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .map(|(_, event)| event)
            .filter(|event| {
                event.machine_symbol == machine
                    && event.state_symbol == plan.state
                    && matches!(event.root, ::facts::PlaceRoot::Expression(_))
            })
            .collect::<Vec<_>>();
        assert_eq!(temporary_moves.len(), 2);
        assert!(temporary_moves.iter().all(|event| event.kind
            == language_semantics::PermissionEventKind::Transfer
            && event.multiplicity == Multiplicity::Affine
            && event.claim_identity == language_semantics::PermissionClaimIdentity::Unknown
            && !event.obligation_live));
    }
}
