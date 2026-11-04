//! Same-producer record recovery preserves pending and completed contracts.
use super::*;
use crate::expression::{StaticMachineArgument, StaticSymbolApplication};
use crate::name::Identifier;
use crate::static_applications::{StaticMachineBinding, StaticMachineBindingValue};
use arena::HandleSpan;
use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};

#[test]
fn selected_contract_records_keep_caller_bindings_and_completion_state() {
    let mut symbols = SymbolTableBuilder::default();
    let caller = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("caller"));
    let helper = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("helper"));
    let binder = symbols.insert_root(
        SymbolKind::MachineParameter,
        SymbolNameRef::Static("Required"),
    );
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    let original_member = program
        .expression_table
        .insert_static_member(StaticMachineArgument {
            symbol: helper,
            path: Box::from([Identifier::from("member")]),
            ..Default::default()
        });
    let supplied = StaticMachineArgument {
        symbol: helper,
        path: Box::from([Identifier::from("helper")]),
        application: Some(Box::new(StaticSymbolApplication {
            member: original_member,
            lifetime_arguments: Box::from([Identifier::from("borrow")]),
            arguments: Box::from([StaticMachineArgument {
                const_literal: Some(
                    numerics::literals::IntegerLiteral::from_parts(
                        false,
                        numerics::literals::IntegerRadix::Decimal,
                        "37",
                    )
                    .unwrap(),
                ),
                ..Default::default()
            }]),
        })),
        ..Default::default()
    };
    let invocation = program.expression_table.retain_static_machine_application(
        caller,
        Identifier::from("caller"),
        std::slice::from_ref(&supplied),
    );
    let binding = StaticMachineBinding {
        parameter: binder,
        value: StaticMachineBindingValue::Machine(supplied),
    };
    program
        .expression_table
        .set_static_machine_application_bindings(invocation, std::slice::from_ref(&binding));
    let snapshot = program
        .expression_table
        .snapshot_static_machine_bindings(invocation);
    let snapshot_rows = program
        .expression_table
        .static_machine_binding_snapshot(snapshot)
        .to_vec();
    let StaticMachineBindingValue::Machine(snapshot_argument) = &snapshot_rows[0].value else {
        unreachable!()
    };
    let copied_member = snapshot_argument.application.as_ref().unwrap().member;
    assert_ne!(copied_member, original_member);
    assert_eq!(
        program.expression_table.static_member(copied_member),
        program.expression_table.static_member(original_member)
    );

    let signature = signature::StateSignature {
        symbol: helper,
        name: Identifier::from("required helper"),
        ..Default::default()
    };
    let mut records = Vec::new();
    for required in [None, Some(signature.clone())] {
        let record = AppliedCallableSignature {
            owner: AppliedCallableOwner::MachineSelection {
                invocation,
                ordinal: 1,
                demand: binder,
                selected: helper,
                provider: invocation,
                required,
                source_required: signature.clone(),
                caller_operands: HandleSpan::empty(),
                caller_bindings: snapshot,
                caller_lifetimes: HandleSpan::empty(),
                caller_lifetime_declaration: caller,
                caller_lifetime_owner: caller,
                provider_lifetime_declaration: helper,
                provider_lifetime_owner: caller,
                provider_lifetimes: HandleSpan::empty(),
            },
            signature: signature.clone(),
        };
        let handle = program.applied_callable_signatures.insert(record.clone());
        records.push((handle, record));
    }
    let bytes =
        compiler_storage::encode(&program, compiler_storage::RecordLimits::default()).unwrap();
    let restored: TypedTrees =
        compiler_storage::decode(&bytes, compiler_storage::RecordLimits::default()).unwrap();
    for (handle, original) in records {
        assert_eq!(restored.applied_callable_signatures.get(handle), &original);
        let AppliedCallableOwner::MachineSelection {
            caller_bindings, ..
        } = original.owner
        else {
            unreachable!()
        };
        assert_eq!(
            restored
                .expression_table
                .static_machine_binding_snapshot(caller_bindings),
            snapshot_rows.as_slice()
        );
    }
    assert_eq!(
        restored.static_machine_application(invocation),
        program.static_machine_application(invocation)
    );
}
