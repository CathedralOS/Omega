#[path = "support/source.rs"]
mod sources;

use omega_lowering::{BlockTerminator, CallTarget, CleanupAction, OperationKind, OperationResult, Parameter, PlaceProjection, ReturnEdge};
use omega_toolchain::CollectionPrimitive;
use omega_typed::{Access, ObligationKind};

#[test]
fn affine_fixed_array_projection_reads_only_the_selected_scalar_leaf() {
    for (parameter, collection, borrowed) in [
        ("cells:[Cell;4]", "cells", false),
        ("bag:@Bag", "bag.cells", true),
    ] {
        for (index, selected_field) in [("first", "left"), ("second", "right")] {
            let program = format!(
                "
                data Cell{{left:u8;right:u8;}}
                data Bag{{cells:[Cell;4];}}
                machine choose(i:u64)->u64{{i}}
                machine read({parameter},first:u64,second:u64)->u8 {{({collection}[choose({index})]).{selected_field}}}
            "
            );
            let files = [("case.omg", program.as_str())];
            let source = sources::resolve(&files);
            let prepared = omega_preparation::prepare_source_package(&source).unwrap();
            let [access] = prepared.typed().collection_accesses.as_slice() else {
                panic!("one planned collection use")
            };
            assert_eq!(access.primitive, CollectionPrimitive::Element);
            assert_eq!(&access.machine, omega_toolchain::core_interface().collection(CollectionPrimitive::Element));
            assert!(
                prepared
                    .typed()
                    .obligations
                    .iter()
                    .any(|o| o.node == access.node && matches!(&o.kind, ObligationKind::Borrow { access: Access::Shared, .. }))
            );
            let package = sources::lower_prepared(&prepared, &files).unwrap();
            let read = package
                .machines
                .iter()
                .find(|machine| machine.declaration.path == "read")
                .unwrap();
            let selector = package
                .machines
                .iter()
                .find(|machine| machine.declaration.path == "choose")
                .unwrap()
                .id;
            let [
                Parameter::Structural(root),
                Parameter::Scalar(first),
                Parameter::Scalar(second),
            ] = read.body.parameters[..]
            else {
                panic!("exact parameters")
            };
            let [call, observe] = &read.body.operations[..] else {
                panic!("selector runs once and the array element never materializes")
            };
            assert!(matches!(&call.kind, OperationKind::Call {callee:CallTarget::Machine(id), arguments, ..}
                if *id == selector && arguments == &[omega_lowering::CallArgument::Scalar(if index == "first" {first} else {second})]));
            let OperationKind::ProjectedScalarRead { source } = &observe.kind else {
                panic!("projected leaf observation")
            };
            assert_eq!(root, source.root);
            let tail = if borrowed {
                let [
                    PlaceProjection::Referent,
                    PlaceProjection::Field(cells),
                    rest @ ..,
                ] = source.path.as_slice()
                else {
                    panic!("original collection backing")
                };
                assert_eq!(cells.path, "Bag::cells");
                rest
            } else {
                &source.path[..]
            };
            let [
                PlaceProjection::RuntimeIndex(runtime),
                PlaceProjection::Field(field),
            ] = tail
            else {
                panic!("fixed-array index followed by field")
            };
            assert_eq!(call.result, OperationResult::Scalar(runtime.selector));
            assert_eq!(field.path, format!("Cell::{selected_field}"));
            assert_eq!(
                read.body.diagnostics.bounds[runtime.obligation.get() - 1]
                    .span
                    .get_start_offset(),
                program.find(&format!("choose({index})")).unwrap()
            );
            let BlockTerminator::Return {
                result: ReturnEdge::Scalar(out),
                cleanup,
                ..
            } = &read.body.blocks[0].terminator
            else {
                panic!("read returns the leaf")
            };
            assert_eq!(observe.result, OperationResult::Scalar(*out));
            assert_eq!(cleanup, &if borrowed { vec![] } else { vec![CleanupAction::DiscardRoot(root)] });
        }
    }
}

#[test]
fn a_shared_affine_slice_keeps_the_checked_element_request_but_has_no_portable_loan_yet() {
    let program = "data Cell{value:u8;} machine read(x:@[Cell],i:u64)->u8 requires i<x.length { x[i].value }";
    let files = [("case.omg", program)];
    let source = sources::resolve(&files);
    let prepared = omega_preparation::prepare_source_package(&source).unwrap();
    assert_eq!(prepared.typed().collection_accesses[0].primitive, CollectionPrimitive::Element);
    let error = sources::lower_prepared(&prepared, &files).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("needs a portable shared element loan")
    );
}

#[test]
fn requesting_a_whole_affine_element_is_still_refused_before_lowering() {
    for array in ["[Cell;4]", "@[Cell]"] {
        let program = format!("data Cell{{value:u8;}} machine read(x:{array},i:u64)->Cell {{x[i]}}");
        let files = [("case.omg", program.as_str())];
        let source = sources::resolve(&files);
        let error = omega_preparation::prepare_source_package(&source).unwrap_err();
        assert!(error.message.contains("indexed read requires a copy element"));
    }
}
#[test]
fn a_local_affine_fixed_array_keeps_its_cleanup_after_the_read() {
    let source = "data Cell { value:u8; }
        machine create()->[Cell;2] { [Cell{value:7}, Cell{value:8}] }
        machine read(i:u64)->u8 requires i<2 {
            let cells:[Cell;2]=create();
            cells[i].value
        }";
    let package = sources::lower(&[("case.omg", source)]).unwrap();
    let create = package
        .machines
        .iter()
        .find(|m| m.declaration.path == "create")
        .unwrap()
        .id;
    let m = package
        .machines
        .iter()
        .find(|m| m.declaration.path == "read")
        .unwrap();
    let array = m
        .body
        .operations
        .iter()
        .find(|op| matches!(op.kind, OperationKind::Call {callee: CallTarget::Machine(id), ..} if id == create))
        .unwrap();
    let OperationResult::Structural(home) = array.result else {
        panic!("completed array home")
    };
    let last = m.body.operations.last().unwrap();
    assert!(matches!(&last.kind, OperationKind::ProjectedScalarRead { source } if source.root == home));
    assert!(matches!(&m.body.blocks[0].terminator, BlockTerminator::Return { cleanup,.. } if cleanup == &[CleanupAction::DiscardRoot(home)]));
}

#[test]
fn the_affine_leaf_read_requires_the_exact_bound_and_shared_loan() {
    use omega_typed::TypedTrees;
    let program = "data Cell{value:u8;} machine read(x:[Cell;4],i:u64)->u8 requires i<4 { x[i].value }";
    let files = [("case.omg", program)];
    let source = sources::resolve(&files);
    for (remove_bound, change_access) in [(true, false), (false, false), (false, true)] {
        let prepared = omega_preparation::prepare_source_package(&source).unwrap();
        let selected = prepared.typed().collection_accesses[0].node;
        let typed = prepared.into_typed();
        let resolved = typed.resolved().clone();
        let mut facts = typed.into_facts();
        facts.obligations.retain(|obligation| {
            obligation.node != selected
                || match obligation.kind {
                    ObligationKind::CallPrecondition(_) => !remove_bound,
                    ObligationKind::Borrow { .. } => remove_bound || change_access,
                    _ => true,
                }
        });
        if change_access {
            let obligation = facts
                .obligations
                .iter_mut()
                .find(|obligation| obligation.node == selected && matches!(obligation.kind, ObligationKind::Borrow { .. }))
                .unwrap();
            let ObligationKind::Borrow { access, .. } = &mut obligation.kind else {
                unreachable!();
            };
            assert_eq!(*access, Access::Shared);
            *access = Access::Mut;
        }
        let prepared = omega_preparation::PreparedTrees::from_preparation(
            TypedTrees::new(resolved, facts),
            Default::default(),
            Default::default(),
            Default::default(),
            vec![],
            vec![],
            None,
            Default::default(),
            Default::default(),
            vec![],
        );
        let refusal = sources::lower_prepared(&prepared, &files).unwrap_err();
        assert!(
            refusal
                .to_string()
                .contains("lacks its exact core shared element selection, loan or bound")
        );
        assert_eq!(refusal.site.span.get_start_offset(), program.find("x[i]").unwrap());
    }
}

#[test]
fn a_local_literal_arrays_scalar_read_preserves_the_original_home_and_cleanup() {
    for selected in [0, 1] {
        let source = format!(
            "data Cell{{value:u8;}}
            machine read(a:Cell,b:Cell)->u8 {{
                let cells:[Cell;2]=[a,b];
                cells[{selected}].value
            }}"
        );
        let package = sources::lower(&[("case.omg", &source)]).unwrap();
        let read = &package.machines[0];
        let array = read
            .body
            .operations
            .iter()
            .find(|op| matches!(op.kind, OperationKind::EstablishStructuralArray { .. }))
            .unwrap();
        let OperationResult::Structural(home) = array.result else {
            panic!("the affine literal establishes a home")
        };
        let [selector, observation] = &read.body.operations[array.id.index() + 1..] else {
            panic!("only evaluate the selector and read the leaf")
        };
        assert!(matches!(&selector.kind, OperationKind::Constant(omega_lowering::ScalarLiteral::Integer(index)) if index == &selected.into()));
        let OperationKind::ProjectedScalarRead { source: place } = &observation.kind else {
            panic!("original storage observation")
        };
        assert_eq!(place.root, home);
        let [
            PlaceProjection::RuntimeIndex(index),
            PlaceProjection::Field(field),
        ] = place.path.as_slice()
        else {
            panic!("exact indexed field")
        };
        assert_eq!(field.path, "Cell::value");
        assert_eq!(selector.result, OperationResult::Scalar(index.selector));
        assert_eq!(
            read.body.diagnostics.bounds[index.obligation.get() - 1]
                .span
                .get_start_offset(),
            source.find(&format!("cells[{selected}]")).unwrap() + "cells[".len()
        );
        let BlockTerminator::Return {
            result: ReturnEdge::Scalar(returned),
            cleanup,
            ..
        } = &read.body.blocks[0].terminator
        else {
            panic!("normal leaf result")
        };
        assert_eq!(observation.result, OperationResult::Scalar(*returned));
        assert_eq!(cleanup, &[CleanupAction::DiscardRoot(home)]);
    }
}
