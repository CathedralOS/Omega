//! `ScalarControlFlowEvidence::Acyclic` retains the complete forward graph of
//! a no-call scalar function: each block ends in one real transfer terminator
//! and every transfer interval is owned by an edge attribution row, with the
//! conditional additionally owning a zero-width row at its fallthrough offset.
//! Object construction decodes the terminators back out of the final bytes,
//! reconstructs the block topology from the edge rows alone
//! (`reconstruct_scalar_control_flow`), and replays each region's claimed
//! stack mutations to one convergent incoming depth — the retained `blocks`
//! are a witness the boundary re-derives, never a trusted description. Until
//! now this route was reachable only through the private unit tests inside
//! `object_artifact/replay/scalar/control_flow.rs`; these tests pin the
//! receiver-level custody through `build_object_artifact` and executable
//! image emission on both scalar architectures.

use super::{edge_id, machine_id, operation_id, scalar_acyclic_plan, scalar_mutation};
use calling_conventions::{ValuePlacement, ValueShape};
use image_emission::{
    ObjectError, ObjectScalarStack, build_object_artifact, emit_direct_executable_image,
    validate_direct_executable_image,
};
use machine_code::{
    FunctionFragmentConditionalBranchPredicate as Predicate, InternalCallRelocation,
    MachineCodePlan, ScalarControlBlockEvidence, ScalarControlFlowEvidence,
    ScalarControlTerminatorEvidence, ScalarStackMutationKind, SemanticCodeAttribution,
    SemanticCodeSite, UnitParameterHomeRecord,
};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use target::{Architecture, NativeTarget};
use target_operations::CallSiteOwner;
use terminal_psi::StructuralMultiplicity;

fn acyclic_blocks(plan: &mut MachineCodePlan) -> &mut Vec<ScalarControlBlockEvidence> {
    let ScalarControlFlowEvidence::Acyclic { blocks } = &mut plan.functions[0]
        .scalar_stack
        .as_mut()
        .unwrap()
        .control_flow
    else {
        unreachable!("acyclic fixture")
    };
    blocks
}

#[test]
fn acyclic_scalar_graph_replays_through_object_construction_and_emission() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let plan = scalar_acyclic_plan(target);
        let expected_peak = match target.architecture {
            Architecture::X86_64 => 8,
            Architecture::Aarch64 => 16,
        };
        let artifact = build_object_artifact(&plan).expect("acyclic scalar object");
        assert_eq!(artifact.functions().len(), 1);
        assert_eq!(
            artifact.functions()[0].scalar_stack,
            Some(ObjectScalarStack {
                local_peak_bytes: expected_peak,
                stack_alignment: 16,
            }),
            "{target:?} replays the taken arm's balanced mutation to its peak",
        );
        let image = emit_direct_executable_image(&artifact, 3).expect("acyclic scalar image");
        validate_direct_executable_image(&artifact, &image)
            .expect("emitted image replays the sealed acyclic custody");
    }
}

/// The claimed block list is only a witness: retargeting a jump, dropping the
/// shared return, or substituting the decoded predicate all diverge from the
/// graph reconstructed out of the final bytes and must fail closed.
#[test]
fn acyclic_claimed_blocks_only_witness_the_reconstructed_topology() {
    fn stale_witness() -> Result<image_emission::ObjectArtifact, ObjectError> {
        Err(ObjectError::InvalidScalarConditionalEvidence {
            machine: machine_id(1),
            offset: 0,
        })
    }

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let mut plan = scalar_acyclic_plan(target);
        let ScalarControlTerminatorEvidence::Jump { target_offset, .. } =
            &mut acyclic_blocks(&mut plan)[1].terminator
        else {
            unreachable!()
        };
        *target_offset += 4;
        assert_eq!(build_object_artifact(&plan), stale_witness());

        let mut plan = scalar_acyclic_plan(target);
        acyclic_blocks(&mut plan).pop();
        assert_eq!(build_object_artifact(&plan), stale_witness());

        let mut plan = scalar_acyclic_plan(target);
        let ScalarControlTerminatorEvidence::Conditional(branch) =
            &mut acyclic_blocks(&mut plan)[0].terminator
        else {
            unreachable!()
        };
        branch.predicate = Predicate::U64LessThanV1;
        assert_eq!(build_object_artifact(&plan), stale_witness());
    }
}

/// Every transfer interval and conditional fallthrough is owned by an edge
/// row: dropping the taken-arm join's row leaves bytes without a topology
/// boundary, a padded zero-width row owns no real fallthrough, a misdeclared
/// interval no longer decodes as a transfer, and bytes past the last owned
/// transfer have no owner.
#[test]
fn acyclic_edge_attributions_own_every_transfer_interval() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let mut plan = scalar_acyclic_plan(target);
        plan.functions[0].semantic_code_attribution.pop();
        assert!(matches!(
            build_object_artifact(&plan),
            Err(ObjectError::InvalidScalarConditionalEvidence { machine, .. })
                if machine == machine_id(1)
        ));

        let mut plan = scalar_acyclic_plan(target);
        plan.functions[0].provenance.edges.push(edge_id(6));
        plan.functions[0]
            .semantic_code_attribution
            .push(SemanticCodeAttribution {
                site: SemanticCodeSite::Edge(edge_id(6)),
                operation_ordinal: 7,
                code_offset: 0,
                byte_count: 0,
            });
        let tail = plan.functions[0].bytes.len();
        assert_eq!(
            build_object_artifact(&plan),
            Err(ObjectError::InvalidScalarConditionalEvidence {
                machine: machine_id(1),
                offset: tail,
            })
        );

        let mut plan = scalar_acyclic_plan(target);
        plan.functions[0].semantic_code_attribution[0].byte_count += match target.architecture {
            Architecture::X86_64 => 3,
            Architecture::Aarch64 => 4,
        };
        let offset = plan.functions[0].semantic_code_attribution[0].code_offset;
        assert_eq!(
            build_object_artifact(&plan),
            Err(ObjectError::InvalidScalarConditionalEvidence {
                machine: machine_id(1),
                offset,
            })
        );

        let mut plan = scalar_acyclic_plan(target);
        plan.functions[0].bytes.push(0x90);
        let tail = plan.functions[0].bytes.len() - 1;
        assert_eq!(
            build_object_artifact(&plan),
            Err(ObjectError::InvalidScalarConditionalEvidence {
                machine: machine_id(1),
                offset: tail,
            })
        );

        // Reconstruction sorts the rows itself, but the stored carrier still
        // requires canonical (operation_ordinal, code_offset) order.
        let mut plan = scalar_acyclic_plan(target);
        plan.functions[0].semantic_code_attribution.swap(3, 4);
        assert_eq!(
            build_object_artifact(&plan),
            Err(ObjectError::NonCanonicalSemanticCodeAttributionOrder(
                machine_id(1)
            ))
        );
    }
}

/// Forward-only custody: a join retargeted to an earlier block, or to an
/// offset that is no block boundary at all, rejects during reconstruction.
#[test]
fn acyclic_transfers_must_reach_a_later_block_boundary() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let mut plan = scalar_acyclic_plan(target);
        let backward_target = match target.architecture {
            Architecture::X86_64 => {
                // The taken-arm join at 22 lands on the fallthrough arm at 5.
                plan.functions[0].bytes[23..27].copy_from_slice(&(-22_i32).to_le_bytes());
                5
            }
            Architecture::Aarch64 => {
                // The taken-arm join at 28 lands on the fallthrough arm at 8.
                plan.functions[0].bytes[28..32].copy_from_slice(&0x17ff_fffb_u32.to_le_bytes());
                8
            }
        };
        assert_eq!(
            build_object_artifact(&plan),
            Err(ObjectError::InvalidScalarConditionalEvidence {
                machine: machine_id(1),
                offset: backward_target,
            })
        );

        let mut plan = scalar_acyclic_plan(target);
        let interior_target = match target.architecture {
            Architecture::X86_64 => {
                // Inside the join's own instruction: no block boundary at 26.
                plan.functions[0].bytes[23..27].copy_from_slice(&(-1_i32).to_le_bytes());
                26
            }
            Architecture::Aarch64 => {
                // The middle of the taken arm: no block boundary at 20.
                plan.functions[0].bytes[28..32].copy_from_slice(&0x17ff_fffe_u32.to_le_bytes());
                20
            }
        };
        assert_eq!(
            build_object_artifact(&plan),
            Err(ObjectError::InvalidScalarConditionalEvidence {
                machine: machine_id(1),
                offset: interior_target,
            })
        );
    }
}

/// Region replay consumes each claimed mutation against the decoded
/// instruction and requires every predecessor to deliver the same incoming
/// depth at a join: a phantom claim, a kind-substituted claim, or an arm that
/// changes depth without releasing it all fail closed.
#[test]
fn acyclic_regions_replay_claimed_mutations_to_one_convergent_depth() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let mut plan = scalar_acyclic_plan(target);
        let phantom_offset = match target.architecture {
            // Inside the fallthrough arm's `mov eax, 0` (no mutation exists).
            Architecture::X86_64 => 8,
            // The taken arm's `mov w0, #7` (no stack adjustment exists).
            Architecture::Aarch64 => 20,
        };
        let mutations = &mut plan.functions[0].scalar_stack.as_mut().unwrap().mutations;
        mutations.push(scalar_mutation(
            phantom_offset,
            match target.architecture {
                Architecture::X86_64 => 1,
                Architecture::Aarch64 => 4,
            },
            match target.architecture {
                Architecture::X86_64 => ScalarStackMutationKind::X86Push,
                Architecture::Aarch64 => ScalarStackMutationKind::Allocate { byte_size: 16 },
            },
        ));
        mutations.sort_by_key(|mutation| mutation.offset);
        assert_eq!(
            build_object_artifact(&plan),
            Err(ObjectError::InvalidScalarStackEvidence {
                machine: machine_id(1),
                offset: phantom_offset,
            })
        );

        let mut plan = scalar_acyclic_plan(target);
        let (kind_offset, wrong_kind) = match target.architecture {
            // The release byte is a `pop`, not a `push`.
            Architecture::X86_64 => (21, ScalarStackMutationKind::X86Push),
            // The release word is `add sp, sp, #16`, not an allocation.
            Architecture::Aarch64 => (24, ScalarStackMutationKind::Allocate { byte_size: 16 }),
        };
        plan.functions[0]
            .scalar_stack
            .as_mut()
            .unwrap()
            .mutations
            .iter_mut()
            .find(|mutation| mutation.offset == kind_offset)
            .unwrap()
            .kind = wrong_kind;
        assert_eq!(
            build_object_artifact(&plan),
            Err(ObjectError::InvalidScalarStackEvidence {
                machine: machine_id(1),
                offset: kind_offset,
            })
        );

        let mut plan = scalar_acyclic_plan(target);
        let (converge_offset, unbalanced) = match target.architecture {
            // The fallthrough arm pushes without releasing: `push rax; nop`x4.
            Architecture::X86_64 => {
                plan.functions[0].bytes[5..10].copy_from_slice(&[0x50, 0x90, 0x90, 0x90, 0x90]);
                (27, scalar_mutation(5, 1, ScalarStackMutationKind::X86Push))
            }
            // The fallthrough arm allocates without releasing.
            Architecture::Aarch64 => {
                plan.functions[0].bytes[8..12].copy_from_slice(&0xd100_43ff_u32.to_le_bytes());
                (
                    32,
                    scalar_mutation(8, 4, ScalarStackMutationKind::Allocate { byte_size: 16 }),
                )
            }
        };
        let mutations = &mut plan.functions[0].scalar_stack.as_mut().unwrap().mutations;
        mutations.push(unbalanced);
        mutations.sort_by_key(|mutation| mutation.offset);
        assert_eq!(
            build_object_artifact(&plan),
            Err(ObjectError::InvalidScalarStackEvidence {
                machine: machine_id(1),
                offset: converge_offset,
            })
        );
    }
}

/// The Acyclic lane is the no-call scalar lane: retained internal calls or
/// structural parameter homes reject before reconstruction, and the evidence
/// admits only the 16-byte scalar stack alignment.
#[test]
fn acyclic_evidence_excludes_call_and_parameter_custody() {
    fn excluded() -> Result<image_emission::ObjectArtifact, ObjectError> {
        Err(ObjectError::InvalidScalarConditionalEvidence {
            machine: machine_id(1),
            offset: 0,
        })
    }

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let mut plan = scalar_acyclic_plan(target);
        plan.functions[0].internal_calls = vec![InternalCallRelocation {
            owner: CallSiteOwner::Operation(operation_id(1)),
            target: machine_id(1),
            unit_stack: None,
            scalar_stack: None,
            offset: 0,
        }];
        assert_eq!(build_object_artifact(&plan), excluded());

        let mut plan = scalar_acyclic_plan(target);
        plan.functions[0].scalar_structural_parameter_homes = vec![UnitParameterHomeRecord {
            place: PlaceId::new(1).unwrap(),
            structural_type: StructuralTypeId::new(1).unwrap(),
            multiplicity: StructuralMultiplicity::Affine,
            access: terminal_psi::StructuralAccess::Owned,
            shape: ValueShape::integer(0, 1),
            source: ValuePlacement {
                shape: ValueShape::integer(0, 1),
                locations: Vec::new(),
            },
            location: machine_code::StructuralSourceLocation::Stack { byte_offset: 0 },
            indirect: false,
        }];
        assert_eq!(build_object_artifact(&plan), excluded());

        let mut plan = scalar_acyclic_plan(target);
        plan.functions[0]
            .scalar_stack
            .as_mut()
            .unwrap()
            .stack_alignment = 8;
        assert_eq!(
            build_object_artifact(&plan),
            Err(ObjectError::InvalidScalarStackAlignment {
                machine: machine_id(1),
                alignment: 8,
            })
        );
    }
}
