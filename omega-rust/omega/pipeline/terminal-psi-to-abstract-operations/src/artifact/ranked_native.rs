use abstract_operations::{
    RankedFixedEntryFuel, RankedLiveClaim, RankedMachineStructuralFrontiers,
    RankedNativeAbstractOperationPlan, RankedOwnedStructuralPlace, RankedPartialStructuralCustody,
    RankedStructuralOwnershipFrontier, RankedU32CountdownCustody, RankedU32CountdownGraph,
};
use semantic_vocabulary::IntegerValue;
use terminal_psi::{
    Operation, OperationKind, OperationResult, TerminalMachine, TerminalRankedGuard,
    TerminalRankedSuccessorArgument, Terminator,
};

use super::ArtifactLoweringError;
use crate::lowering::lower_decoded_native_ranked_module;

const EXACT_U32_COUNTDOWN_CEILING_UNITS: u64 = 25_769_803_775;

/// Admit and retain the exact ranked native countdown independently of the
/// ordinary acyclic artifact entrance.
///
/// The semantic and proof sections are decoded once. Native and fixed-fuel
/// authority are nevertheless constructed independently from those decoded
/// values, so neither opaque verifier result can stand in for the other.
pub fn lower_artifact_sections_for_native_ranked_countdown(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
) -> Result<RankedNativeAbstractOperationPlan, ArtifactLoweringError> {
    let module = terminal_codec::decode_module(semantic_bytes)
        .map_err(ArtifactLoweringError::SemanticDecode)?;
    let proof = terminal_codec::decode_proof_bundle(proof_bytes)
        .map_err(ArtifactLoweringError::ProofDecode)?;

    lower_decoded_native_ranked_countdown(&module, &proof, profile)
}

pub(super) fn lower_decoded_native_ranked_countdown(
    module: &terminal_psi::TerminalModule,
    proof: &terminal_verifier::ProofBundle,
    profile: &proof_admission::AdmissionProfile,
) -> Result<RankedNativeAbstractOperationPlan, ArtifactLoweringError> {
    if profile != &proof_admission::AdmissionProfile::default() {
        return Err(ArtifactLoweringError::RankedNativeCustody(
            "ranked native replay does not admit profile-local proof assumptions",
        ));
    }
    let native =
        terminal_verifier::verify_module_for_native_ranked_countdown(module, proof, profile)
            .map_err(ArtifactLoweringError::Verification)?;
    let fixed = terminal_verifier::verify_module_for_fixed_fuel(module, proof, profile)
        .map_err(ArtifactLoweringError::Verification)?;
    let fixed_fuel = terminal_fixed_fuel::derive_ranked_countdown_entry_fuel(&fixed, module.entry)
        .map_err(ArtifactLoweringError::FixedFuel)?;
    terminal_fixed_fuel::validate_ranked_countdown_entry_fuel(&fixed, &fixed_fuel)
        .map_err(ArtifactLoweringError::FixedFuel)?;

    let plan =
        lower_decoded_native_ranked_module(&native).map_err(ArtifactLoweringError::Lowering)?;
    if fixed_fuel.terminal_psi() != plan.psi
        || fixed_fuel.entry() != plan.entry
        || !fixed_fuel.relevant_preconditions().is_empty()
        || fixed_fuel.ceiling_units() != EXACT_U32_COUNTDOWN_CEILING_UNITS
    {
        return Err(ArtifactLoweringError::RankedNativeCustody(
            "ranked countdown fixed-fuel projection is not the settled exact slice",
        ));
    }

    let machine = entry_machine(module)?;
    let ranked_scc =
        machine
            .ranked_scc
            .clone()
            .ok_or(ArtifactLoweringError::RankedNativeCustody(
                "entry machine has no ranked SCC",
            ))?;
    let structural_frontiers = native
        .structural_frontiers()
        .machine(machine.id)
        .cloned()
        .ok_or(ArtifactLoweringError::RankedNativeCustody(
            "entry machine has no verifier structural frontiers",
        ))?;
    let graph = extract_countdown_graph(machine)?;
    let [covered] = ranked_scc.covered_cyclic_edges.as_slice() else {
        return Err(ArtifactLoweringError::RankedNativeCustody(
            "ranked countdown does not have exactly one covered backedge",
        ));
    };
    let ranked_header = ranked_scc.header;
    let ranked_backedge = covered.edge;
    if structural_frontiers.block_entry(ranked_scc.header)
        != structural_frontiers.edge_exit(covered.edge)
    {
        return Err(ArtifactLoweringError::RankedNativeCustody(
            "ranked header and backedge structural frontiers differ",
        ));
    }
    let header_entry = project_frontier(structural_frontiers.block_entry(ranked_header).ok_or(
        ArtifactLoweringError::RankedNativeCustody("ranked header has no structural frontier"),
    )?);
    let backedge_exit = project_frontier(structural_frontiers.edge_exit(ranked_backedge).ok_or(
        ArtifactLoweringError::RankedNativeCustody("ranked backedge has no structural frontier"),
    )?);
    let proof_replay =
        terminal_codec::encode_proof_bundle(proof).map_err(ArtifactLoweringError::ProofEncode)?;

    Ok(RankedNativeAbstractOperationPlan {
        plan,
        countdown: RankedU32CountdownCustody {
            semantic_replay: module.clone(),
            proof_replay,
            ranked_scc,
            fixed_fuel: RankedFixedEntryFuel {
                terminal_psi: fixed_fuel.terminal_psi(),
                schedule: fixed_fuel.schedule(),
                entry: fixed_fuel.entry(),
                relevant_preconditions: fixed_fuel.relevant_preconditions().to_vec(),
                ceiling_units: fixed_fuel.ceiling_units(),
            },
            graph,
            structural_frontiers: RankedMachineStructuralFrontiers {
                machine: machine.id,
                header: ranked_header,
                backedge: ranked_backedge,
                header_entry,
                backedge_exit,
            },
        },
    })
}

fn project_frontier(
    frontier: &terminal_verifier::VerifiedStructuralOwnershipFrontier,
) -> RankedStructuralOwnershipFrontier {
    RankedStructuralOwnershipFrontier {
        claims: frontier
            .claims()
            .iter()
            .map(|claim| RankedLiveClaim {
                claim: claim.claim,
                input: claim.input,
                path: claim.path.clone(),
                multiplicity: claim.multiplicity,
            })
            .collect(),
        owned_places: frontier
            .owned_places()
            .iter()
            .map(|owned| RankedOwnedStructuralPlace {
                place: owned.place,
                multiplicity: owned.multiplicity,
            })
            .collect(),
        partial_custody: frontier
            .partial_custody()
            .iter()
            .map(|partial| RankedPartialStructuralCustody {
                place: partial.place,
                moved_paths: partial.moved_paths.clone(),
            })
            .collect(),
    }
}

fn entry_machine(
    module: &terminal_psi::TerminalModule,
) -> Result<&TerminalMachine, ArtifactLoweringError> {
    module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .ok_or(ArtifactLoweringError::RankedNativeCustody(
            "verified ranked entry machine is absent",
        ))
}

fn scalar_result(
    operation: &Operation,
) -> Result<semantic_vocabulary::ValueId, ArtifactLoweringError> {
    let OperationResult::Scalar(result) = operation.result else {
        return Err(ArtifactLoweringError::RankedNativeCustody(
            "ranked graph operation has no scalar result",
        ));
    };
    Ok(result.id)
}

fn extract_countdown_graph(
    machine: &TerminalMachine,
) -> Result<RankedU32CountdownGraph, ArtifactLoweringError> {
    let ranked = machine
        .ranked_scc
        .as_ref()
        .ok_or(ArtifactLoweringError::RankedNativeCustody(
            "entry machine has no ranked SCC",
        ))?;
    let [covered] = ranked.covered_cyclic_edges.as_slice() else {
        return Err(ArtifactLoweringError::RankedNativeCustody(
            "ranked countdown does not have exactly one covered backedge",
        ));
    };
    let block = |id| {
        machine.blocks.iter().find(|block| block.id == id).ok_or(
            ArtifactLoweringError::RankedNativeCustody("ranked graph names an absent block"),
        )
    };

    let entry = block(machine.entry)?;
    let Terminator::Jump {
        edge: preheader_edge,
        target: preheader_target,
        arguments: preheader_arguments,
        ..
    } = &entry.terminator
    else {
        return Err(ArtifactLoweringError::RankedNativeCustody(
            "ranked entry is not the canonical preheader",
        ));
    };
    if *preheader_target != ranked.header
        || preheader_arguments.len() != header_parameter_count(machine, ranked.header)?
    {
        return Err(ArtifactLoweringError::RankedNativeCustody(
            "ranked preheader does not enter the header",
        ));
    }
    let header = block(ranked.header)?;
    let rank_index = header
        .parameters
        .iter()
        .position(|parameter| parameter.id == ranked.rank_parameter)
        .ok_or(ArtifactLoweringError::RankedNativeCustody(
            "ranked header parameter is absent",
        ))?;
    let initial_value =
        *preheader_arguments
            .get(rank_index)
            .ok_or(ArtifactLoweringError::RankedNativeCustody(
                "ranked preheader argument is absent",
            ))?;

    let [zero, compare] = header.operations.as_slice() else {
        return Err(ArtifactLoweringError::RankedNativeCustody(
            "ranked header operations are not canonical",
        ));
    };
    if !matches!(
        &zero.kind,
        OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(0)
        }
    ) {
        return Err(ArtifactLoweringError::RankedNativeCustody(
            "ranked header has no canonical zero",
        ));
    }
    let zero_value = scalar_result(zero)?;
    if !matches!(
        &compare.kind,
        OperationKind::IntegerLessThan { left, right }
            if *left == zero_value && *right == ranked.rank_parameter
    ) {
        return Err(ArtifactLoweringError::RankedNativeCustody(
            "ranked header has no canonical positive comparison",
        ));
    }
    let condition = scalar_result(compare)?;
    let Terminator::Conditional {
        condition: terminator_condition,
        when_true,
        when_false,
    } = &header.terminator
    else {
        return Err(ArtifactLoweringError::RankedNativeCustody(
            "ranked header is not conditional",
        ));
    };
    let TerminalRankedGuard::UnsignedParameterPositive {
        block: guard_block,
        edge: guard_edge,
        condition: guard_condition,
        parameter: guard_parameter,
    } = covered.guard;
    if *terminator_condition != condition
        || guard_block != ranked.header
        || guard_edge != when_true.edge
        || when_true.target != covered.source
        || guard_condition != condition
        || guard_parameter != ranked.rank_parameter
    {
        return Err(ArtifactLoweringError::RankedNativeCustody(
            "ranked guard identity disagrees with the graph",
        ));
    }

    let decrement = block(covered.source)?;
    let [one, subtract] = decrement.operations.as_slice() else {
        return Err(ArtifactLoweringError::RankedNativeCustody(
            "ranked decrement operations are not canonical",
        ));
    };
    if !matches!(
        &one.kind,
        OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(1)
        }
    ) {
        return Err(ArtifactLoweringError::RankedNativeCustody(
            "ranked decrement has no canonical one",
        ));
    }
    let one_value = scalar_result(one)?;
    let OperationKind::ExactIntegerSubtract {
        left,
        right,
        obligation,
    } = &subtract.kind
    else {
        return Err(ArtifactLoweringError::RankedNativeCustody(
            "ranked decrement has no exact subtraction",
        ));
    };
    if *left != ranked.rank_parameter || *right != one_value {
        return Err(ArtifactLoweringError::RankedNativeCustody(
            "ranked decrement operands disagree",
        ));
    }
    let subtract_value = scalar_result(subtract)?;
    let TerminalRankedSuccessorArgument::UnsignedParameterMinusOne {
        argument,
        source_parameter,
        target_parameter,
        ..
    } = covered.successor_argument;
    if argument != subtract_value
        || source_parameter != ranked.rank_parameter
        || target_parameter != ranked.rank_parameter
    {
        return Err(ArtifactLoweringError::RankedNativeCustody(
            "ranked successor identity disagrees with the subtraction",
        ));
    }
    let Terminator::Jump {
        edge: backedge,
        target: backedge_target,
        arguments: backedge_arguments,
        ..
    } = &decrement.terminator
    else {
        return Err(ArtifactLoweringError::RankedNativeCustody(
            "ranked decrement does not end in the covered backedge",
        ));
    };
    let argument_index = match covered.successor_argument {
        TerminalRankedSuccessorArgument::UnsignedParameterMinusOne { argument_index, .. } => {
            usize::try_from(argument_index).map_err(|_| {
                ArtifactLoweringError::RankedNativeCustody(
                    "ranked successor argument index is not representable",
                )
            })?
        }
    };
    if *backedge != covered.edge
        || *backedge_target != covered.target
        || covered.target != ranked.header
        || backedge_arguments.get(argument_index) != Some(&subtract_value)
    {
        return Err(ArtifactLoweringError::RankedNativeCustody(
            "ranked covered backedge disagrees with the graph",
        ));
    }

    let done = block(when_false.target)?;
    let Terminator::ReturnUnit {
        edge: return_edge, ..
    } = &done.terminator
    else {
        return Err(ArtifactLoweringError::RankedNativeCustody(
            "ranked false successor is not the canonical done block",
        ));
    };
    if !done.operations.is_empty() {
        return Err(ArtifactLoweringError::RankedNativeCustody(
            "ranked done block is not operation-free",
        ));
    }

    Ok(RankedU32CountdownGraph {
        entry: machine.entry,
        preheader_edge: *preheader_edge,
        initial_value,
        zero_operation: zero.id,
        zero_value,
        compare_operation: compare.id,
        false_exit_edge: when_false.edge,
        done_block: done.id,
        one_operation: one.id,
        one_value,
        subtract_operation: subtract.id,
        subtract_obligation: *obligation,
        return_edge: *return_edge,
    })
}

fn header_parameter_count(
    machine: &TerminalMachine,
    header: semantic_vocabulary::BlockId,
) -> Result<usize, ArtifactLoweringError> {
    machine
        .blocks
        .iter()
        .find(|block| block.id == header)
        .map(|block| block.parameters.len())
        .ok_or(ArtifactLoweringError::RankedNativeCustody(
            "ranked graph names an absent header",
        ))
}
