//! Named call results retain their root while projected disposers move fields.

use super::*;

mod anonymous;
mod anonymous_shared;

#[test]
fn partial_result_cleanup_retains_exact_producer_root_and_residual() {
    for (producer, parameters, initializer, reaches) in [
        (
            "machine forward(pair: Pair) -> Pair { pair }",
            "pair: Pair",
            "forward(pair)",
            "",
        ),
        (
            "boundary trait Factory { machine create() -> Pair reaches Factory; }",
            "",
            "Factory::create()",
            "reaches Factory",
        ),
    ] {
        let checked = checked(&format!(
            r#"
            pub data Token {{ value: u64; }}
            pub data Pair {{ left: Token; right: Token; }}
            data Sink {{}}
            machine Sink::take(token: Token) {{}}
            {producer}
            data Main {{}}
            machine Main::enter({parameters}) {reaches} {{
                let result: Pair = {initializer};
                Sink::take(result.right);
            }}
        "#
        ));
        let machine = machine_named(&checked, "enter");
        assert!(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(machine)
                .is_none()
        );
        let plan = checked
            .facts
            .flow
            .terminal_partial_affine_unit_cleanups
            .for_machine(machine)
            .expect("partial cleanup owns the named result");
        assert_eq!(plan.machine.operations.len(), 3);
        let result = match &plan.machine.operations[0] {
            CheckedUnitEffectOperationPlan::StructuralCall {
                result,
                discard_result_on_return,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                result,
                discard_result_on_return,
                ..
            } => {
                assert!(
                    !discard_result_on_return,
                    "partial cleanup never discards the whole result"
                );
                result
            }
            _ => panic!("a real producer precedes the move"),
        };
        assert_eq!(result.binding_ordinal, 0);
        assert_eq!(result.statement_index, 0);
        let CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            structural_arguments,
            claim_transfers,
            ..
        } = &plan.machine.operations[1]
        else {
            panic!("projected Unit disposer")
        };
        assert_eq!(coordinate.statement_index, 1);
        assert_eq!(coordinate.call_ordinal, 0);
        assert!(claim_transfers.is_empty());
        let [argument] = structural_arguments.as_slice() else {
            panic!("one projected argument")
        };
        let source = checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: 0,
        };
        assert_eq!(argument.source, source);
        assert!(matches!(argument.path.as_slice(),
            [CheckedUnitStructuralPathSegment::Field(field)] if field.ends_with("right")));
        let [residual] = plan.residual_affine_discards.as_slice() else {
            panic!("one sibling")
        };
        assert_eq!(residual.source, source);
        assert!(matches!(residual.path.as_slice(),
            [CheckedUnitStructuralPathSegment::Field(field)] if field.ends_with("left")));
        assert_ne!(result.type_identity, argument.type_identity);
        assert_eq!(residual.type_identity, argument.type_identity);
        let transfers = checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .map(|(_, event)| event)
            .filter(|event| {
                event.machine_symbol == machine
                    && event.source
                        == language_semantics::PermissionEventSource::Call {
                            statement_index: 1,
                            call_ordinal: 0,
                            target_symbol: match &plan.machine.operations[1] {
                                CheckedUnitEffectOperationPlan::CallUnit {
                                    target_state, ..
                                } => *target_state,
                                _ => unreachable!(),
                            },
                        }
                    && event.kind == language_semantics::PermissionEventKind::Transfer
            })
            .collect::<Vec<_>>();
        let [transfer] = transfers.as_slice() else {
            panic!("one exact projected transfer")
        };
        assert_eq!(
            transfer.multiplicity,
            language_semantics::Multiplicity::Affine
        );
        assert_eq!(
            transfer.claim_identity,
            language_semantics::PermissionClaimIdentity::Unknown
        );
        assert!(!transfer.obligation_live);
        assert_eq!(
            transfer.provenance,
            language_semantics::PermissionProvenance::Established {
                machine_symbol: machine,
                state_symbol: plan.machine.state,
                source: language_semantics::PermissionEventSource::Statement { statement_index: 0 },
            }
        );
        let projected = crate::flow::CanonicalPlace {
            root: transfer.root,
            segments: checked
                .facts
                .flow
                .ownership
                .segments
                .span_or_empty(transfer.segments)
                .to_vec(),
        };
        let leaf_type = crate::flow::canonical_place_type_reference(
            &checked.typed,
            plan.machine.state,
            1,
            &projected,
        )
        .expect("projected permission names exact existing storage");
        assert!(matches!(
            projected.segments.as_slice(),
            [facts::PlaceSegment::Field { .. }]
        ));
        assert_eq!(
            checked.typed.type_multiplicity(leaf_type),
            language_semantics::Multiplicity::Affine
        );
        assert!(
            checked
                .facts
                .flow
                .terminal_partial_affine_unit_cleanups
                .structural_types
                .iter()
                .any(|structural| structural.identity == result.type_identity),
            "the root survives catalog selection even with no entry parameter"
        );
    }
}
