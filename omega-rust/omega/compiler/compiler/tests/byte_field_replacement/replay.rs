//! Hostile projection changes are checked against one immutable verified artifact.
use super::{AdmissionProfile, NativeTarget};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
#[path = "indexed_replay.rs"]
mod indexed;
use legalized_operations::LegalizedScalarInstructionKind as Legalized;
use selected_instructions::{
    SelectedInstructionKind as Selected, SelectedMemoryAccessRole as Role,
};
use target_operations::TargetUnitOperation as Target;
use target_operations_to_selected_instructions::{
    legalize_target_operations, select_instructions, selection_constraints,
    validate_legalized_operations, validate_selected_instructions,
};

#[test]
fn byte_replacement_replay_binds_equal_capacity_siblings_and_dynamic_copy() {
    // Distinct literal backings have identical live counts. The two destination
    // fields also share capacity and access, so geometry alone cannot pick one.
    let source = r#"
        domain [u8; 3]::Utf8 requires valid_utf8(self);
        data Payload { text: [u8; 3] in Utf8; sibling: [u8; 3] in Utf8; }
        data Record { before: u64; payload: Payload; after: u64; }
        machine Record::replace(&mut self) {
            self.payload.text = "ABC";
            self.payload.sibling = "XYZ";
        }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .unwrap();
    let terminal = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Record::replace"),
    )
    .unwrap();
    let input = terminal_psi_to_abstract_operations::lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &terminal_codec::encode_module(&terminal.semantic_module).unwrap(),
            proof_bytes: &terminal_codec::encode_proof_section(
                &terminal.semantic_module,
                &terminal.proof_bundle,
            )
            .unwrap(),
            obligation_ledger_bytes: None,
        },
        &AdmissionProfile::default(),
    )
    .map(|artifact| {
        artifact
            .into_optimization_artifact()
            .into_optimization_input()
    })
    .unwrap();
    let verified = terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap();
    let source = verified.input().plan();
    let unit = verified.unit();
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            source,
            abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
        )
        .unwrap();
        let legal = legalize_target_operations(&target, source, unit).unwrap();
        validate_legalized_operations(&target, source, unit, legal.plan().clone()).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = selection_constraints(&legal, &environment);
        let selected = select_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        validate_selected_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();

        let target_stores = target
            .functions
            .iter()
            .flat_map(|function| &function.graph.blocks)
            .flat_map(|block| &block.operations)
            .filter(|operation| {
                matches!(operation, Target::StructuralByteSequenceFieldStore { .. })
            })
            .cloned()
            .collect::<Vec<_>>();
        let [
            Target::StructuralByteSequenceFieldStore {
                field: sibling,
                source: other_source,
                length: other_length,
                obligation: other_obligation,
                ..
            },
            _,
        ] = target_stores.as_slice()
        else {
            panic!("two authored stores");
        };
        for mutation in 0..7 {
            let mut changed = target.clone();
            let store = changed
                .functions
                .iter_mut()
                .flat_map(|function| &mut function.graph.blocks)
                .flat_map(|block| &mut block.operations)
                .rfind(|operation| {
                    matches!(operation, Target::StructuralByteSequenceFieldStore { .. })
                })
                .unwrap();
            let Target::StructuralByteSequenceFieldStore {
                destination,
                field,
                source: input,
                length,
                obligation,
                ..
            } = store
            else {
                unreachable!()
            };
            match mutation {
                0 => *field = *sibling,
                1 => destination.path.clear(),
                2 => destination.access = terminal_psi::StructuralAccess::SharedBorrow,
                3 => *input = *other_source,
                4 => *length = *other_length,
                5 => *obligation = *other_obligation,
                _ => destination.place = *other_source,
            }
            assert!(
                legalize_target_operations(&changed, source, unit).is_err(),
                "target mutation {mutation}: {native:?}"
            );
        }
        let legal_stores = legal
            .plan()
            .scalar_functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .filter(|instruction| {
                matches!(
                    instruction.kind,
                    Legalized::StructuralByteSequenceFieldStore { .. }
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        let Legalized::StructuralByteSequenceFieldStore {
            source: other_source,
            length: other_length,
            obligation: other_obligation,
            accepted_fact: other_fact,
            ..
        } = legal_stores[0].kind
        else {
            unreachable!()
        };
        for mutation in 0..9 {
            let mut changed = legal.plan().clone();
            let store = changed
                .scalar_functions
                .iter_mut()
                .flat_map(|function| &mut function.blocks)
                .flat_map(|block| &mut block.instructions)
                .rfind(|instruction| {
                    matches!(
                        instruction.kind,
                        Legalized::StructuralByteSequenceFieldStore { .. }
                    )
                })
                .unwrap();
            let Legalized::StructuralByteSequenceFieldStore {
                destination,
                field,
                source: input,
                length,
                obligation,
                accepted_fact,
            } = &mut store.kind
            else {
                unreachable!()
            };
            match mutation {
                0 => *field = *sibling,
                1 => destination.path.clear(),
                2 => destination.access = terminal_psi::StructuralAccess::SharedBorrow,
                3 => *input = other_source,
                4 => *length = other_length,
                5 => *obligation = other_obligation,
                6 => *accepted_fact = other_fact,
                7 => store.fuel.clear(),
                _ => destination.place = other_source,
            }
            assert!(
                validate_legalized_operations(&target, source, unit, changed).is_err(),
                "legalized mutation {mutation}: {native:?}"
            );
        }
        for mutation in 0..13 {
            let mut changed = selected.plan().clone();
            let function = changed
                .functions
                .iter_mut()
                .rfind(|function| {
                    function.blocks.iter().any(|block| {
                        block
                            .instructions
                            .iter()
                            .any(|instruction| instruction.kind == Selected::CopyBytes)
                    })
                })
                .unwrap();
            let block = function
                .blocks
                .iter_mut()
                .rfind(|block| {
                    block
                        .instructions
                        .iter()
                        .any(|instruction| instruction.kind == Selected::CopyBytes)
                })
                .unwrap();
            let copy = block
                .instructions
                .iter()
                .rposition(|instruction| instruction.kind == Selected::CopyBytes)
                .unwrap();
            let copy_identity = block.instructions[copy].id;
            let read = function
                .memory_accesses
                .iter()
                .position(|access| {
                    access.instruction == copy_identity
                        && matches!(access.role, Role::ReadByteSpan { .. })
                })
                .unwrap();
            let write = function
                .memory_accesses
                .iter()
                .position(|access| {
                    access.instruction == copy_identity
                        && matches!(access.role, Role::WriteByteSpan { .. })
                })
                .unwrap();
            match mutation {
                0 => block.instructions[copy].operands.swap(0, 1),
                1 => block.instructions[copy].operands[2] = block.instructions[copy].operands[0],
                2 => function.memory_accesses[read].byte_count = 3,
                3 => function.memory_accesses[write].byte_count = 3,
                4 => function.memory_accesses[read].place = other_source,
                5 => function.memory_accesses[write].byte_offset += 16,
                6 => block.instructions.swap(copy, copy + 1),
                7 => {
                    let Selected::Store { byte_offset, .. } =
                        &mut block.instructions[copy + 1].kind
                    else {
                        panic!("length publication follows copy");
                    };
                    *byte_offset += 8;
                }
                8 => {
                    function.memory_accesses[read].role = Role::ReadByteSpan {
                        length: other_length,
                        obligation: other_obligation,
                        accepted_fact: other_fact,
                    }
                }
                9 => {
                    function.memory_accesses[write].role = Role::WriteByteSpan {
                        length: other_length,
                        obligation: other_obligation,
                        accepted_fact: other_fact,
                    }
                }
                10 => block.instructions[copy].clobbers.clear(),
                11 => function.memory_accesses[read].role = Role::ReadPlace,
                _ => function.memory_accesses[write].role = Role::WritePlace,
            }
            assert!(
                validate_selected_instructions(
                    &legal,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                    changed
                )
                .is_err(),
                "selected mutation {mutation}: {native:?}"
            );
        }
    }
}
