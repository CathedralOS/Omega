//! Shared address joins lend their referent's address to borrowed calls.
use super::structural_borrows::source_plan;
use abstract_operations::{AbstractOperation, AbstractOperationPlan};
use calling_conventions::ValueShape;
use semantic_vocabulary::PlaceId;
use target::NativeTarget;
use target_operations::{TargetStructuralArgumentSource, TargetUnitOperation};
use terminal_psi::{StructuralAccess, StructuralPathSegment};

/// `borrowed_results::PRIMITIVE_CALL_SOURCE` behind an entry that stores it.
const PRIMITIVE_CALL_SOURCE: &str = "data Payload { left: u64; right: u64; }
    data Main { out: u64; }
    machine read(value: &u64) -> u64 { value }
    machine choose(other: bool) -> u64 {
        let a: Payload = Payload { left: 1, right: 2 };
        let b: Payload = Payload { left: 3, right: 4 };
        let view: &u64 = match other { true -> &a.left, false -> &b.right };
        read(view)
    }
    machine Main::run(&mut self) { self.out = choose(true); }";

/// `borrowed_results::RECORD_CALL_SOURCE` behind the same entry.
const RECORD_CALL_SOURCE: &str = "data Payload { left: u64; right: u64; }
    data Main { out: u64; }
    machine read(value: &Payload) -> u64 { value.left ^ value.right }
    machine choose(other: bool) -> u64 {
        let a: Payload = Payload { left: 1, right: 2 };
        let b: Payload = Payload { left: 3, right: 4 };
        let view: &Payload = match other { true -> &a, false -> &b };
        read(view)
    }
    machine Main::run(&mut self) { self.out = choose(true); }";

/// The owning function index and the join's declaration place.
fn join(plan: &AbstractOperationPlan) -> (usize, PlaceId) {
    plan.functions
        .iter()
        .enumerate()
        .find_map(|(index, function)| {
            function
                .block_entries
                .iter()
                .flat_map(|entry| &entry.structural_parameters)
                .find(|parameter| parameter.access == StructuralAccess::SharedBorrow)
                .map(|parameter| (index, parameter.place))
        })
        .expect("customer keeps its shared join")
}

fn edit_join_bindings(
    plan: &mut AbstractOperationPlan,
    edit: impl Fn(&mut abstract_operations::AbstractStructuralBinding),
) {
    let (function, place) = join(plan);
    for operation in &mut plan.functions[function].operations {
        if let AbstractOperation::Jump {
            structural_bindings,
            ..
        } = operation
        {
            for binding in structural_bindings
                .iter_mut()
                .filter(|binding| binding.parameter == place)
            {
                edit(binding);
            }
        }
    }
}

#[test]
fn projected_and_whole_joins_lend_the_block_carrier_to_borrowed_calls() {
    for (source, referent) in [
        (PRIMITIVE_CALL_SOURCE, ValueShape::borrowed_reference(8, 8)),
        (RECORD_CALL_SOURCE, ValueShape::borrowed_reference(16, 8)),
    ] {
        let plan = source_plan(source);
        let (function, place) = join(&plan);
        let machine = plan.functions[function].machine;
        for native in [NativeTarget::macos_arm64(), NativeTarget::windows_x64()] {
            let target =
                crate::lower_to_target_operations(&plan, crate::TargetLoweringRequest::new(native))
                    .expect("address joins lower");
            crate::validate_abstract_to_target_translation(&plan, native, &target).unwrap();
            let lowered = target
                .functions
                .iter()
                .find(|function| function.machine == machine)
                .unwrap();
            let argument = lowered
                .graph
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find_map(|operation| match operation {
                    TargetUnitOperation::Call { arguments, .. } => arguments.first(),
                    _ => None,
                })
                .expect("the join feeds its call");
            let owner = lowered
                .graph
                .blocks
                .iter()
                .find(|block| {
                    block
                        .structural_parameters
                        .iter()
                        .any(|parameter| parameter.place == place)
                })
                .unwrap()
                .block;
            assert_eq!(argument.place, place);
            assert_eq!(argument.access, StructuralAccess::SharedBorrow);
            assert!(argument.path.is_empty());
            assert_eq!(argument.source_byte_offset, 0);
            assert_eq!(argument.shape, referent);
            assert_eq!(
                argument.source,
                TargetStructuralArgumentSource::BlockParameter {
                    block: owner,
                    place
                }
            );
        }
    }
}

#[test]
fn address_join_edges_reject_substituted_referents_widened_access_and_forged_roots() {
    let edits: [(
        &str,
        fn(&mut abstract_operations::AbstractStructuralBinding),
    ); 4] = [
        ("whole record for a leaf", |binding| {
            binding.argument.path.clear()
        }),
        ("exclusive", |binding| {
            binding.argument.access = StructuralAccess::MutableBorrow
        }),
        ("forged root", |binding| {
            binding.argument.place = PlaceId::new(9_999).unwrap()
        }),
        ("byte window", |binding| {
            binding.argument.path = vec![StructuralPathSegment::FixedByteRange { start: 0, end: 8 }]
        }),
    ];
    for (label, edit) in edits {
        let mut plan = source_plan(PRIMITIVE_CALL_SOURCE);
        edit_join_bindings(&mut plan, edit);
        assert!(
            crate::lower_to_target_operations(
                &plan,
                crate::TargetLoweringRequest::new(NativeTarget::macos_arm64())
            )
            .is_err(),
            "{label} must not lower as an address join"
        );
    }
}

#[test]
fn owned_scalar_fields_can_back_mutable_address_joins() {
    // Both roots are owned locals, so exclusive loans are valid when the edge
    // and declaration agree. Address transport is identical; Terminal admission
    // independently checks whether the producer had permission to take a loan.
    let mut plan = source_plan(PRIMITIVE_CALL_SOURCE);
    let (function, place) = join(&plan);
    edit_join_bindings(&mut plan, |binding| {
        binding.argument.access = StructuralAccess::MutableBorrow
    });
    for parameter in plan.functions[function]
        .block_entries
        .iter_mut()
        .flat_map(|entry| &mut entry.structural_parameters)
        .filter(|parameter| parameter.place == place)
    {
        parameter.access = StructuralAccess::MutableBorrow;
    }
    let native = NativeTarget::macos_arm64();
    let target =
        crate::lower_to_target_operations(&plan, crate::TargetLoweringRequest::new(native))
            .expect("owned scalar fields can back mutable address joins");
    crate::validate_abstract_to_target_translation(&plan, native, &target).unwrap();
}

#[test]
fn loop_carried_borrows_project_call_arguments_from_the_joined_address() {
    let source = "data Cell { value: u64; }
        machine Cell::copy_from(&mut self, source: &Cell) { self.value = source.value; }
        data Holder { prefix: u64; cells: [Cell; 4]; }
        machine Holder::copy_all(&mut self, source: &Holder) {
            transition { _ -> copy(source, 0) }
            state copy(&mut self, source: &Holder, position: u64) {
                transition position < 4 { true -> step(source, position) false -> done() }
            }
            state step(&mut self, source: &Holder, position: u64) {
                self.cells[position].copy_from(&source.cells[position]);
                transition { _ -> copy(source, position + 1) }
            }
            state done(&mut self) {}
        }
        data Main { source: Holder; target: Holder; }
        machine Main::run(&mut self) { self.target.copy_all(&self.source); }";
    let plan = source_plan(source);
    for native in [NativeTarget::macos_arm64(), NativeTarget::windows_x64()] {
        let target =
            crate::lower_to_target_operations(&plan, crate::TargetLoweringRequest::new(native))
                .expect("loop-carried borrows retain projected arguments");
        crate::validate_abstract_to_target_translation(&plan, native, &target).unwrap();
        let projected = target
            .functions
            .iter()
            .flat_map(|function| &function.graph.blocks)
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match operation {
                TargetUnitOperation::Call { arguments, .. } => Some(arguments),
                _ => None,
            })
            .flatten()
            .filter(|argument| {
                matches!(
                    argument.source,
                    TargetStructuralArgumentSource::BlockParameter { .. }
                ) && !argument.indices.is_empty()
            })
            .collect::<Vec<_>>();
        assert!(
            !projected.is_empty(),
            "a loop parameter supplies the projected loan"
        );
        for argument in projected {
            assert_eq!(argument.source_byte_offset, 8);
            assert_eq!(argument.indices.len(), 1);
            assert_eq!(argument.indices[0].stride, 8);
            assert_eq!(argument.shape, ValueShape::borrowed_reference(8, 8));
        }
        for corrupt_offset in [true, false] {
            let mut forged = target.clone();
            let argument = forged
                .functions
                .iter_mut()
                .flat_map(|function| &mut function.graph.blocks)
                .flat_map(|block| &mut block.operations)
                .find_map(|operation| match operation {
                    TargetUnitOperation::Call { arguments, .. } => {
                        arguments.iter_mut().find(|argument| {
                            matches!(
                                argument.source,
                                TargetStructuralArgumentSource::BlockParameter { .. }
                            ) && !argument.indices.is_empty()
                        })
                    }
                    _ => None,
                })
                .unwrap();
            if corrupt_offset {
                argument.source_byte_offset += 8;
            } else {
                argument.indices[0].stride += 8;
            }
            assert!(
                crate::validate_abstract_to_target_translation(&plan, native, &forged).is_err()
            );
        }
    }
}
