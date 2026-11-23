#[path = "support/source.rs"]
mod sources;

use omega_lowering::{
    BlockTerminator, CallTarget, LoweredMachine, LoweredPackage, OperationKind, OperationResult, Parameter, PlaceProjection, ProjectedPlace,
    StructuralStoreDisposition,
};

fn lower(source: &str) -> LoweredPackage {
    sources::lower(&[("case.omg", source)]).unwrap()
}

fn machine<'a>(
    package: &'a LoweredPackage,
    name: &str,
) -> &'a LoweredMachine {
    package
        .machines
        .iter()
        .find(|machine| machine.declaration.path == name)
        .unwrap()
}

fn field(
    projection: &PlaceProjection,
    path: &str,
) -> bool {
    matches!(projection, PlaceProjection::Field(field) if field.path == path)
}

#[test]
fn dynamic_owned_store_uses_the_new_mutable_home_and_exact_rhs() {
    let package = lower("machine store(i:u64,value:u8) requires i<2 {let mutable local:[u8;2]=[1,2];local[i]=value;}");
    let store = machine(&package, "store");
    let [Parameter::Scalar(i), Parameter::Scalar(value)] = store.body.parameters[..] else {
        panic!("scalar formals")
    };
    let [home] = store.body.mutable_homes[..] else {
        panic!("the fixed array has a writable home")
    };
    let destination = store
        .body
        .operations
        .iter()
        .find_map(|operation| match &operation.kind {
            OperationKind::ProjectedScalarStore { destination, value: input } if *input == value => Some(destination),
            _ => None,
        })
        .unwrap();
    spec_corpus::observe_handoff!(psi "psi-output/owned-fixed-index-store", assert!(
        destination.root == home && matches!(destination.path.as_slice(), [PlaceProjection::RuntimeIndex(index)] if index.selector == i)
    ));
    let [PlaceProjection::RuntimeIndex(index)] = destination.path[..] else {
        unreachable!()
    };
    let bound = &store.body.diagnostics.bounds[index.obligation.get() - 1];
    assert_eq!(
        bound.span.get_start_offset(),
        "machine store(i:u64,value:u8) requires i<2 {let mutable local:[u8;2]=[1,2];local[".len()
    );
}

#[test]
fn composing_indices_and_fields_keeps_one_obligation_per_fixed_dimension() {
    let package = lower(
        "
        data Cell [copy] { mark:u8; }
        data Grid { cells:[[Cell;3];2]; }
        machine set(x:@mutable Grid, i:u64, j:u64, value:u8) requires i<2 && j<3 {
            x.cells[i][j].mark=value;
        }",
    );
    let set = machine(&package, "set");
    let [
        Parameter::Structural(root),
        Parameter::Scalar(i),
        Parameter::Scalar(j),
        Parameter::Scalar(value),
    ] = set.body.parameters[..]
    else {
        panic!("the original root and both index formals")
    };
    let [store] = set.body.operations.as_slice() else {
        panic!("the place is not loaded")
    };
    let OperationKind::StructuralScalarFieldStore {
        destination,
        field: selected,
        value: input,
    } = &store.kind
    else {
        panic!("one projected scalar store")
    };
    let [
        PlaceProjection::Referent,
        cells,
        PlaceProjection::RuntimeIndex(row),
        PlaceProjection::RuntimeIndex(column),
    ] = destination.path.as_slice()
    else {
        panic!("the fixed inner extent followed its own projection")
    };
    spec_corpus::observe_handoff!(psi "psi-output/borrowed-fixed-index-field-store", assert!(
        destination.root == root && field(cells, "Grid::cells") && selected.path == "Cell::mark"
            && (*input, row.selector, column.selector) == (value, i, j)
            && row.obligation != column.obligation
    ));
    assert_eq!(set.body.diagnostics.bounds.len(), 2);
    let bounds = &set.body.diagnostics.bounds;
    assert!(bounds[row.obligation.get() - 1].span.get_start_offset() < bounds[column.obligation.get() - 1].span.get_start_offset());
}

#[test]
fn the_stored_index_is_the_chosen_snapshot_before_the_rhs_read() {
    for (selected, unused) in [("first", "second"), ("second", "first")] {
        let source = format!(
            "
            data Grid {{ cells:[u8;2]; }}
            data Cursor {{ first:u64; second:u64; rhs:u8; }}
            machine store(g:@mutable Grid, c:@Cursor) requires c.{selected}<2 {{
                g.cells[c.{selected}]=c.rhs;
            }}"
        );
        let package = lower(&source);
        let store = machine(&package, "store");
        let [Parameter::Structural(grid), Parameter::Structural(cursor)] = store.body.parameters[..] else {
            panic!("distinct formal homes")
        };
        let [selector, rhs, write] = &store.body.operations[..] else {
            panic!("one selector read, one value read and one write")
        };
        let OperationKind::ProjectedScalarRead { source: selected_path } = &selector.kind else {
            panic!("selector observation")
        };
        let OperationKind::ProjectedScalarRead { source: rhs_path } = &rhs.kind else {
            panic!("replacement observation")
        };
        let OperationKind::ProjectedScalarStore { destination, value } = &write.kind else {
            panic!("one fixed-array leaf update")
        };
        let [
            PlaceProjection::Referent,
            cells,
            PlaceProjection::RuntimeIndex(index),
        ] = destination.path.as_slice()
        else {
            panic!("exact root and element")
        };
        spec_corpus::observe_handoff!(psi "psi-output/fixed-index-store-snapshot", assert!(
            selected_path.root == cursor && rhs_path.root == cursor && destination.root == grid && field(cells, "Grid::cells")
                && field(&selected_path.path[1], &format!("Cursor::{selected}")) && !field(&selected_path.path[1], &format!("Cursor::{unused}"))
                && field(&rhs_path.path[1], "Cursor::rhs") && selector.result == OperationResult::Scalar(index.selector)
                && rhs.result == OperationResult::Scalar(*value)
        ));
    }
}

#[test]
fn a_fixed_literal_bound_keeps_rhs_effects_and_branching_in_source_order() {
    let package = lower(
        "
        data Cursor { index:u64; }
        machine replace(c:@mutable Cursor)->u8 { c.index=2; 70 }
        machine store(c:@mutable Cursor, choose:bool) {
            let mutable cells:[u8;2];
            cells[1]=match choose{true->replace(c),false->71u8};
        }",
    );
    let store = machine(&package, "store");
    let replacement = machine(&package, "replace").id;
    let write = store
        .body
        .operations
        .iter()
        .find(|op| matches!(op.kind, OperationKind::ProjectedScalarStore { .. }))
        .unwrap();
    let OperationKind::ProjectedScalarStore {
        destination: ProjectedPlace { path, .. },
        value,
    } = &write.kind
    else {
        unreachable!()
    };
    let [PlaceProjection::RuntimeIndex(index)] = path.as_slice() else {
        panic!("literal destination keeps its bounds identity")
    };
    let [literal] = store
        .body
        .operations
        .iter()
        .filter(|op| op.result == OperationResult::Scalar(index.selector))
        .collect::<Vec<_>>()[..]
    else {
        panic!("one literal selector capture")
    };
    let selected_call = store
        .body
        .operations
        .iter()
        .find(|op| matches!(op.kind, OperationKind::Call { callee: CallTarget::Machine(id), .. } if id == replacement))
        .unwrap();
    assert!(literal.id.index() < selected_call.id.index() && selected_call.id.index() < write.id.index());
    let join = store
        .body
        .blocks
        .iter()
        .find(|block| block.scalar_parameters == [*value])
        .unwrap();
    assert!(matches!(join.terminator, BlockTerminator::Return { .. }) && join.operations.contains(&write.id.index()));
}

#[test]
fn a_borrowed_row_replacement_installs_the_complete_scalar_array() {
    let package = lower(
        "
        data Grid { cells:[[u8;3];2]; }
        machine set(x:@mutable Grid,i:u64,row:[u8;3]) requires i<2 { x.cells[i]=row; }",
    );
    let set = machine(&package, "set");
    let [
        Parameter::Structural(root),
        Parameter::Scalar(i),
        Parameter::Structural(row),
    ] = set.body.parameters[..]
    else {
        panic!("row and containing homes")
    };
    assert!(matches!(set.body.operations.as_slice(), [store] if
        matches!(&store.kind, OperationKind::StructuralStore {destination, value, disposition: StructuralStoreDisposition::ReplaceUnrestricted}
            if destination.root == root && *value == row && matches!(destination.path.as_slice(),
                [PlaceProjection::Referent, cells, PlaceProjection::RuntimeIndex(index)] if field(cells, "Grid::cells") && index.selector == i))));
}

#[test]
fn a_later_rhs_guarantee_does_not_establish_the_dynamic_capture_bound() {
    let source = "machine certify(i:u64)->u8 ensures i<2 {70}
        machine store(i:u64){let mutable cells:[u8;2]; cells[i]=certify(i);}";
    let error = sources::lower(&[("case.omg", source)]).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("dynamic indexed store needs a pre-RHS bounds proof")
    );
    assert_eq!(error.site.span.get_start_offset(), source.find("cells[i]").unwrap() + "cells[".len());
    lower(&source.replace("cells[i]", "cells[1]"));
}
#[test]
fn a_later_selector_cannot_supply_an_earlier_dimensions_bound() {
    let source = "machine certify(i:u64)->u64 ensures i<2 {1}
        machine store(i:u64,value:u8){let mutable cells:[[u8;2];2]; cells[i][certify(i)]=value;}";
    let error = sources::lower(&[("case.omg", source)]).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("dynamic indexed store needs a pre-RHS bounds proof")
    );
    assert_eq!(error.site.span.get_start_offset(), source.find("cells[i]").unwrap() + "cells[".len());
    lower(&source.replace("cells[i]", "cells[1]"));
}

#[test]
fn a_statically_selected_owned_child_retains_its_displaced_custody() {
    let source = "
        data Cell dropped by release { mark:u8; }
        machine release(cell:Cell){let Cell{mark as _}=cell;}
        data Grid { cells:[Cell;2]; }
        machine set(x:@mutable Grid, replacement:Cell) { x.cells[1]=move replacement; }";
    let package = lower(source);
    let set = machine(&package, "set");
    let [Parameter::Structural(root), Parameter::Structural(replacement)] = set.body.parameters[..] else {
        panic!("loan and transferred replacement")
    };
    let [moved, displaced, installed] = set.body.operations.as_slice() else {
        panic!("one replacement transfer, displacement and store")
    };
    assert!(matches!(&moved.kind, OperationKind::MoveStructural { source } if source.root == replacement && source.path.is_empty()));
    let OperationKind::BorrowedMove { source: old } = &displaced.kind else {
        panic!("displace one owned cell")
    };
    assert!(
        old.root == root && matches!(old.path.as_slice(), [PlaceProjection::Referent, cells, PlaceProjection::FixedIndex(1)] if field(cells, "Grid::cells"))
    );
    let OperationResult::Structural(previous) = displaced.result else {
        panic!("the old cell has its own custody")
    };
    let BlockTerminator::Jump(edge) = &set.body.blocks[0].terminator else {
        panic!("cleanup precedes installation")
    };
    assert!(matches!(edge.cleanup.as_slice(), [omega_lowering::CleanupAction::Invoke { place, target, .. }]
        if place.root == previous && place.path.is_empty() && *target == machine(&package,"release").id));
    assert!(
        matches!(&installed.kind, OperationKind::StructuralStore { destination, value, disposition: StructuralStoreDisposition::FillVacancy }
        if destination == old && moved.result == OperationResult::Structural(*value))
    );
}
#[test]
fn every_assignment_dimension_uses_the_exact_core_mutable_element_handoff() {
    use omega_preparation::{PreparedTrees, prepare_source_package};
    use omega_toolchain::CollectionPrimitive;
    use omega_typed::{Access, ObligationKind, TypedTrees};

    let source = "
        data Grid{cells:[[u8;3];2];}
        machine set(x:@mutable Grid,i:u64,j:u64,value:u8) requires i<2 && j<3 {
            x.cells[i][j]=value;
        }";
    let files = [("case.omg", source)];
    let package = sources::resolve(&files);
    let prepared = prepare_source_package(&package).unwrap();
    let inner_start = source.find("x.cells[i][j]").unwrap();
    let inner_end = inner_start + "x.cells[i]".len();
    let core = omega_toolchain::core_interface();
    let mutable = core.collection(CollectionPrimitive::ElementMut);
    let read = core.collection(CollectionPrimitive::Read);
    for substitute_copy in [true, false] {
        let typed = prepare_source_package(&package).unwrap().into_typed();
        let resolved = typed.resolved().clone();
        let mut facts = typed.into_facts();
        assert_eq!(facts.collection_accesses.len(), 2);
        for access in &facts.collection_accesses {
            assert_eq!((&access.machine, access.primitive), (mutable, CollectionPrimitive::ElementMut));
            assert!(
                facts
                    .obligations
                    .iter()
                    .any(|obligation| obligation.node == access.node && matches!(&obligation.kind, ObligationKind::Borrow { access: Access::Mut, .. }))
            );
        }
        let inner = facts
            .collection_accesses
            .iter_mut()
            .find(|access| access.site.span.get_start_offset() == inner_start && access.site.span.get_end_offset() == inner_end)
            .expect("the inner destination has an independent access record");
        if substitute_copy {
            inner.primitive = CollectionPrimitive::Read;
            inner.machine = read.clone();
            inner.result = inner.element.clone();
        } else {
            std::sync::Arc::make_mut(&mut inner.machine.name).package = "app".into();
        }
        for obligation in facts
            .obligations
            .iter_mut()
            .filter(|obligation| obligation.node == inner.node)
        {
            match &mut obligation.kind {
                ObligationKind::CallPrecondition(machine) => *machine = inner.machine.clone(),
                ObligationKind::Borrow { access, .. } if substitute_copy => *access = Access::Shared,
                _ => {}
            }
        }
        let bad = PreparedTrees::from_preparation(
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
        let refusal = sources::lower_prepared(&bad, &files).unwrap_err();
        assert!(
            refusal
                .to_string()
                .contains("indexed store lacks its exact core mutable element selection")
        );
        assert_eq!(
            (refusal.site.span.get_start_offset(), refusal.site.span.get_end_offset()),
            (inner_start, inner_end)
        );
    }
    sources::lower_prepared(&prepared, &files).unwrap();
}
