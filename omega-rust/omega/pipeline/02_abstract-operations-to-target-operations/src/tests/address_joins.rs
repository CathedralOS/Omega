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
    // Widening the join itself leaves no carrier: an exclusive primitive join
    // is neither an address join nor a descriptor view.
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
    assert!(
        crate::lower_to_target_operations(
            &plan,
            crate::TargetLoweringRequest::new(NativeTarget::macos_arm64())
        )
        .is_err()
    );
}

#[test]
fn address_join_call_rows_reject_forged_carrier_sources() {
    let plan = source_plan(RECORD_CALL_SOURCE);
    let native = NativeTarget::macos_arm64();
    let target =
        crate::lower_to_target_operations(&plan, crate::TargetLoweringRequest::new(native))
            .unwrap();
    for mutation in 0..3 {
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
                        )
                    })
                }
                _ => None,
            })
            .unwrap();
        let TargetStructuralArgumentSource::BlockParameter { block, place } = &mut argument.source
        else {
            unreachable!()
        };
        match mutation {
            0 => *place = PlaceId::new(9_999).unwrap(),
            1 => *block = semantic_vocabulary::BlockId::new(9_999).unwrap(),
            _ => {
                argument.source = TargetStructuralArgumentSource::StructuralHome {
                    psi_operation: semantic_vocabulary::OperationId::new(9_999).unwrap(),
                }
            }
        }
        assert!(
            crate::validate_abstract_to_target_translation(&plan, native, &forged).is_err(),
            "mutation {mutation}"
        );
    }
}
