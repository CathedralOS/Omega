//! Check entry readback with an actual source-produced callback and image.
//!
//! Candidate bytes and plain byte inventories are observations, not accepted
//! images. Keep the retained object, ABI, body and image identity unchanged.

use std::path::Path;

pub(super) fn inspect(
    report: &compilation_products::CompileReport,
    fixture: &Path,
) -> Result<(), String> {
    inspect_body_cpu_state(report, fixture)?;
    inspect_instruction_sources(report, fixture)?;
    let oracle = fixture.join("expected_native_entry_controls.txt");
    if !oracle.exists() {
        return Ok(());
    }
    let expectation = std::fs::read_to_string(oracle).map_err(|error| error.to_string())?;
    let generated_vector_control = match expectation.trim() {
        "required" => false,
        "generated-vector" => true,
        _ => {
            return Err("native entry control oracle must say required or generated-vector".into());
        }
    };
    let artifact = report
        .retained_native_artifact()
        .ok_or("native entry controls require a retained direct native image")?;
    let object = artifact.object();
    let retained = artifact.image();
    let output = retained.output();
    image_emission::validate_direct_executable_image_output(object, retained, output)
        .map_err(|error| format!("unchanged native entry readback: {error}"))?;
    let entry = object
        .private_functions()
        .iter()
        .find(|entry| entry.entry_bridge.is_some())
        .ok_or("native entry controls require an actually emitted callback adapter")?;
    let bridge = entry
        .entry_bridge
        .as_ref()
        .ok_or("selected native entry lost its adapter")?;
    let start = entry.function.text_offset;
    let end = start
        .checked_add(entry.function.byte_count)
        .ok_or("native entry control span overflows")?;
    let field = start
        .checked_add(bridge.call_field_offset())
        .ok_or("native entry control call field overflows")?;
    let original = output
        .final_text_bytes
        .get(start..end)
        .ok_or("native entry control span is outside text")?;
    let mut changed = output.clone();
    let bytes: [u8; 4] = changed
        .final_text_bytes
        .get(field..field + 4)
        .ok_or("native entry control call field is outside text")?
        .try_into()
        .map_err(|_| "native entry control call field has the wrong width")?;
    let word = u32::from_le_bytes(bytes);
    let replacement = match object.target().architecture {
        target::Architecture::Aarch64 => {
            (word & !0x03ff_ffff) | ((word & 0x03ff_ffff).wrapping_add(1) & 0x03ff_ffff)
        }
        target::Architecture::X86_64 => word.wrapping_add(1),
    };
    changed.final_text_bytes[field..field + 4].copy_from_slice(&replacement.to_le_bytes());
    let call = Some(
        u32::try_from(bridge.call_field_offset()).map_err(|_| "native call field exceeds u32")?,
    );
    let decode = |bytes: &[u8]| match object.target().architecture {
        target::Architecture::Aarch64 => isa_aarch64::decode_aarch64_generated_entry(bytes, call),
        target::Architecture::X86_64 => isa_x86_64::decode_x86_64_generated_entry(bytes, call),
    };
    if decode(original)? != decode(&changed.final_text_bytes[start..end])? {
        return Err("native call-target control changed more than the relocation value".into());
    }
    // Observe the unchanged text extent in the actual writer payload. The
    // complete reader checks the loader mapping before the call destination;
    // an ambiguous observation or a wrong file extent cannot pass this control.
    if output.final_text_bytes.is_empty() {
        return Err("native entry control needs a nonempty text extent".into());
    }
    let positions = output
        .bytes
        .windows(output.final_text_bytes.len())
        .enumerate()
        .filter_map(|(offset, bytes)| (bytes == output.final_text_bytes).then_some(offset))
        .collect::<Vec<_>>();
    let [file_offset] = positions.as_slice() else {
        return Err("native entry control needs one exact mapped text observation".into());
    };
    let file_field = file_offset
        .checked_add(field)
        .ok_or("native entry file field overflows")?;
    changed.bytes[file_field..file_field + 4].copy_from_slice(&replacement.to_le_bytes());
    let regions = output
        .executable_regions
        .regions
        .iter()
        .map(|region| image::FinalExecutableRegion {
            origin: region.origin,
            section_offset: region.section_offset,
            byte_count: region.byte_count,
            symbol: region.symbol.clone(),
            footprint: region.footprint.clone(),
            entry_effects: region.entry_effects.clone(),
        })
        .collect();
    changed.executable_regions = image::place_executable_extent(
        &changed.final_text_bytes,
        regions,
        output.final_image_layout.text_address,
    )
    .map_err(|error| format!("native entry candidate inventory: {error}"))?;
    match image_emission::validate_direct_executable_image_output(object, retained, &changed) {
        Err(error)
            if error.message == "relative field does not resolve to its exact selected target" => {}
        Err(error) => {
            return Err(format!(
                "native call-target control rejected for an unrelated reason: {error}"
            ));
        }
        Ok(()) => return Err("native entry readback accepted a changed body-call target".into()),
    }
    // Keep the actual scalar and vector transports distinct. Scalar bodies
    // no longer spill untouched vector registers; a vector control must use
    // real generated transport rather than require those unnecessary spills.
    use calling_conventions::{MachineState as State, MachineStateSet};
    let vector = MachineStateSet::new([State::VectorRegisters]);
    let generated_vector = bridge
        .generated()
        .effects
        .machine_state()
        .contains_all(vector);
    let body_vector = bridge
        .request()
        .body_footprint
        .as_ref()
        .ok_or("native callback lost its body effects")?
        .machine_state()
        .contains_all(vector);
    if generated_vector_control {
        if !generated_vector {
            return Err(format!(
                "generated-vector control needs actual generated vector transport: generated={generated_vector} body={body_vector}"
            ));
        }
    } else if generated_vector || body_vector {
        return Err(format!(
            "scalar entry control must not use or save vector state: generated={generated_vector} body={body_vector}"
        ));
    }
    let mut missing = bridge.request().clone();
    missing.inbound.state.permitted_transitive_use = missing
        .inbound
        .state
        .permitted_transitive_use
        .intersection(MachineStateSet::new([
            State::GeneralRegisters,
            State::Flags,
            State::InstructionPointer,
            State::StackPointer,
            State::ControlState,
            State::SegmentState,
            State::DebugState,
            State::ExtendedState,
        ]));
    let removed = bridge
        .request()
        .inbound
        .state
        .permitted_transitive_use
        .bits()
        ^ missing.inbound.state.permitted_transitive_use.bits();
    if removed != vector.bits() {
        return Err("native vector control must remove exactly vector permission".into());
    }
    if !generated_vector_control {
        let scalar = machine_emission::native_entry_bridge::emit_native_entry_bridge(missing)
            .map_err(|error| {
                format!("scalar entry unexpectedly needs vector permission: {error:?}")
            })?;
        if scalar.bytes() != bridge.bytes()
            || scalar.complete_effects() != bridge.complete_effects()
        {
            return Err("unused vector permission changed scalar callback transport".into());
        }
        return Ok(());
    }
    // This check isolates generated instructions from the callback body's
    // effects. The complete-emission refusal below must survive as well.
    bridge
        .generated()
        .effects
        .validate(&bridge.request().inbound.state)
        .map_err(|reason| format!("unchanged generated vector evidence: {reason}"))?;
    match bridge.generated().effects.validate(&missing.inbound.state) {
        Err("complete entry code uses machine state outside its declared ceiling") => {}
        Err(reason) => {
            return Err(format!(
                "generated vector control rejected for an unrelated reason: {reason}"
            ));
        }
        Ok(()) => return Err("generated entry accepted undeclared vector use".into()),
    }
    match machine_emission::native_entry_bridge::emit_native_entry_bridge(missing) {
        Err(
            machine_emission::native_entry_bridge::NativeEntryBridgeError::GeneratedEntryEffects(
                reason,
            ),
        ) if reason.starts_with(
            "complete entry code uses machine state outside its declared ceiling:",
        ) => {}
        Err(error) => {
            return Err(format!(
                "generated vector control rejected for an unrelated reason: {error:?}"
            ));
        }
        Ok(_) => return Err("native entry accepted undeclared generated vector use".into()),
    }
    Ok(())
}

fn inspect_body_cpu_state(
    report: &compilation_products::CompileReport,
    fixture: &Path,
) -> Result<(), String> {
    let oracle = fixture.join("expected_native_cpu_state_controls.txt");
    if !oracle.exists() {
        return Ok(());
    }
    let expectation = std::fs::read_to_string(oracle).map_err(|error| error.to_string())?;
    let object = report
        .retained_native_artifact()
        .ok_or("CPU-state controls require an actual native product")?
        .object();
    let observed = image_emission::observe_native_body_cpu_state(
        object,
        calling_conventions::Preemption::NotApplicable,
    )
    .map_err(|error| format!("actual body CPU-state observation: {error}"))?;
    let vector = calling_conventions::MachineStateSet::new([
        calling_conventions::MachineState::VectorRegisters,
    ]);
    match expectation.trim() {
        "msr reads" => {
            let rows = object
                .native_instruction_source_rows()
                .ok_or("MSR control lacks actual retained instruction rows")?;
            let mut selectors = std::collections::BTreeSet::new();
            let mut first = None;
            let mut isolated = Vec::new();
            for function in &rows.selected.functions {
                let mut local_selectors = std::collections::BTreeSet::new();
                let mut has_call = false;
                for instruction in function.instructions() {
                    match instruction.kind {
                        selected_instructions::SelectedInstructionKind::RegisterAccess {
                            operation: selected_instructions::RegisterOperation::ModelSpecificRead,
                        } => {
                            let selector = image_emission::observe_native_constant_operand(
                                object,
                                function.machine,
                                instruction.id,
                                0,
                            )
                            .map_err(|error| format!("actual MSR selector: {error}"))?
                            .ok_or("actual MSR selector is not constant")?;
                            selectors.insert(selector);
                            local_selectors.insert(selector);
                            first.get_or_insert((function.machine, instruction.id));
                        }
                        selected_instructions::SelectedInstructionKind::RegisterAccess {
                            operation: selected_instructions::RegisterOperation::ModelSpecificWrite,
                        } => return Err("read-only MSR control contains a write".into()),
                        selected_instructions::SelectedInstructionKind::CallScalar { .. }
                        | selected_instructions::SelectedInstructionKind::CallAggregate {
                            ..
                        }
                        | selected_instructions::SelectedInstructionKind::InvokeNoNormal {
                            ..
                        }
                        | selected_instructions::SelectedInstructionKind::NormalizedForeignCall {
                            ..
                        } => has_call = true,
                        _ => {}
                    }
                }
                if !has_call
                    && local_selectors == std::collections::BTreeSet::from([0x277, 0xc000_0080])
                {
                    isolated.push(function.machine);
                }
            }
            if selectors != std::collections::BTreeSet::from([0x277, 0xc000_0080]) {
                return Err(format!(
                    "MSR control has different actual selectors: {selectors:?}"
                ));
            }
            let control = calling_conventions::MachineStateSet::new([
                calling_conventions::MachineState::ControlState,
            ]);
            let [root] = isolated.as_slice() else {
                return Err("MSR control requires one retained two-read scalar helper".into());
            };
            let isolated = image_emission::observe_native_function_cpu_state(
                object,
                *root,
                calling_conventions::Preemption::NotApplicable,
            )
            .map_err(|error| format!("isolated MSR helper observation: {error}"))?;
            if !isolated.effects().machine_state().contains_all(control)
                || !isolated
                    .effects()
                    .clobbered_state()
                    .intersection(control)
                    .is_empty()
            {
                return Err("MSR reads must use ControlState without clobbering it".into());
            }
            if image_emission::observe_native_function_cpu_state(
                object,
                semantic_vocabulary::MachineId::new(u64::MAX).unwrap(),
                calling_conventions::Preemption::NotApplicable,
            )
            .is_ok()
            {
                return Err("function observation accepted an absent retained root".into());
            }
            let (machine, instruction) =
                first.ok_or("MSR control has no actual read occurrence")?;
            for (owner, selected, operand) in [
                (
                    semantic_vocabulary::MachineId::new(u64::MAX).unwrap(),
                    instruction,
                    0,
                ),
                (
                    machine,
                    selected_instructions::SelectedInstructionId(u32::MAX),
                    0,
                ),
                (machine, instruction, 1),
            ] {
                if image_emission::observe_native_constant_operand(object, owner, selected, operand)
                    .is_ok()
                {
                    return Err(
                        "constant operand accepted a wrong owner, occurrence or result operand"
                            .into(),
                    );
                }
            }
        }
        "scalar calls" => {
            let rows = object
                .native_instruction_source_rows()
                .ok_or("scalar CPU-state control lacks retained instruction rows")?;
            let source = rows.frame_application.source().source();
            let model = source.register_environment().physical().model();
            let actual_abi_vector_clobber = source
                .machine()
                .machine()
                .plan()
                .functions
                .iter()
                .flat_map(|function| function.instructions())
                .any(|instruction| {
                    instruction.alternative.encoded.control
                        == selected_instructions::MachineEncodedControlEffect::DirectRelativeCallV1
                        && instruction.unit_clobbers.iter().any(|unit| {
                            model.unit(*unit).is_some_and(|unit| {
                                matches!(
                                    unit.kind,
                                    register_model::RegisterUnitKind::VectorLane
                                        | register_model::RegisterUnitKind::FloatingControl
                                )
                            })
                        })
                });
            if !actual_abi_vector_clobber {
                return Err(
                    "scalar CPU-state control needs an actual ABI vector-clobber call".into(),
                );
            }
            if observed.effects().machine_state().contains_all(vector)
                || observed
                    .generated_effects()
                    .machine_state()
                    .contains_all(vector)
                || observed.floating_control()
            {
                return Err("scalar body unexpectedly executes floating/vector state".into());
            }
            observed
                .require_without_floating_state()
                .map_err(|error| format!("scalar call CPU-state control: {error}"))?;
        }
        "floating control" => {
            if !observed.floating_control() {
                return Err(
                    "floating-control witness lacks actual emitted floating-control use".into(),
                );
            }
            require_instruction_refusal(
                observed.require_without_floating_state(),
                "installed body executes floating control without a CPU-state premise",
            )?;
        }
        "vector body" => {
            if !observed.effects().machine_state().contains_all(vector) {
                return Err("vector CPU-state control needs actual emitted vector use".into());
            }
            // IEEE arithmetic can read rounding control as well as vectors.
            // Require the exact observed-state refusal, never an opaque ceiling.
            let expected = if observed.floating_control() {
                "installed body executes floating control without a CPU-state premise"
            } else {
                "installed body executes vector state without a CPU-state premise"
            };
            require_instruction_refusal(observed.require_without_floating_state(), expected)?;
        }
        _ => {
            return Err(
                "CPU-state control oracle must say msr reads, scalar calls, vector body or floating control"
                    .into(),
            );
        }
    }
    Ok(())
}

/// The source's ordinary native object supplies both sides of this relation.
/// Mutated copies remain plain rows; none becomes an admitted object or image.
fn inspect_instruction_sources(
    report: &compilation_products::CompileReport,
    fixture: &Path,
) -> Result<(), String> {
    let oracle = fixture.join("expected_native_instruction_controls.txt");
    if !oracle.exists() {
        return Ok(());
    }
    if std::fs::read_to_string(oracle)
        .map_err(|error| error.to_string())?
        .trim()
        != "required"
    {
        return Err("native instruction control oracle must say required".into());
    }
    let object = report
        .retained_native_artifact()
        .ok_or("native instruction controls require an actual native product")?
        .object();
    let rows = object
        .native_instruction_source_rows()
        .ok_or("native instruction controls require retained source rows")?;
    inspect_relative_branches(rows.frame_application)?;
    use image_emission::correlate_native_instruction_sources as correlate;
    let original = correlate(rows.selected, rows.physical.iter().copied())
        .map_err(|error| format!("unchanged native instruction correspondence: {error:?}"))?;
    if original.len() != rows.physical.len()
        || !rows.physical.iter().any(|&(machine, instruction)| {
            rows.physical
                .iter()
                .any(|&(other_machine, other_instruction)| {
                    machine != other_machine && instruction == other_instruction
                })
        })
    {
        return Err(
            "native instruction control needs repeated numbers in distinct functions".into(),
        );
    }

    let mut duplicate_selected = rows.selected.clone();
    let (function_index, block_index) = rows
        .selected
        .functions
        .iter()
        .enumerate()
        .find_map(|(function_index, function)| {
            function
                .blocks
                .iter()
                .position(|block| !block.instructions.is_empty())
                .map(|block_index| (function_index, block_index))
        })
        .ok_or("native instruction control needs a body instruction and terminator")?;
    let block = &mut duplicate_selected
        .functions
        .get_mut(function_index)
        .ok_or("source function disappeared")?
        .blocks[block_index];
    let machine = rows
        .selected
        .functions
        .get(function_index)
        .ok_or("source function disappeared")?
        .machine;
    let duplicate_id = rows
        .physical
        .iter()
        .find_map(|&(owner, id)| (owner == machine && id != block.instructions[0].id).then_some(id))
        .ok_or("native instruction control needs two distinct IDs in one function")?;
    block.instructions[0].id = duplicate_id;
    require_instruction_refusal(
        correlate(&duplicate_selected, rows.physical.iter().copied()),
        "installed fault analysis has duplicate selected instructions",
    )?;

    let mut duplicate_physical = rows.physical.clone();
    duplicate_physical.push(*rows.physical.first().ok_or("physical rows are empty")?);
    require_instruction_refusal(
        correlate(rows.selected, duplicate_physical),
        "installed physical instruction is duplicated",
    )?;
    let mut omitted = rows.physical.clone();
    omitted.pop().ok_or("physical rows are empty")?;
    require_instruction_refusal(
        correlate(rows.selected, omitted),
        "installed physical closure omits a selected instruction",
    )?;

    let owners = rows
        .physical
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let (position, replacement) = rows
        .physical
        .iter()
        .enumerate()
        .find_map(|(position, &(machine, _))| {
            rows.physical
                .iter()
                .find_map(|&(other_machine, instruction)| {
                    (other_machine != machine && !owners.contains(&(machine, instruction)))
                        .then_some((position, (machine, instruction)))
                })
        })
        .ok_or("native instruction control needs an ID present only in another function")?;
    let mut wrong_owner = rows.physical.clone();
    wrong_owner[position] = replacement;
    require_instruction_refusal(
        correlate(rows.selected, wrong_owner),
        "installed physical instruction has no exact selected source",
    )?;
    Ok(())
}

fn inspect_relative_branches(
    framed: &machine_emission::StagedFunctionFragmentFrameApplication,
) -> Result<(), String> {
    use machine_code::FunctionFragmentBranchEvidence as Branch;
    use machine_emission::FunctionFragmentFrameApplicationError as Error;
    let mut observed = [false; 2];
    for function in &framed.fragments().functions {
        let rows: Vec<_> = function
            .instructions()
            .filter(|row| row.branch.is_some())
            .cloned()
            .collect();
        let validate = |rows: &[machine_code::FunctionFragmentInstructionSpan]| {
            framed.validate_relative_branch_rows(function.machine, rows)
        };
        let checked =
            validate(&rows).map_err(|error| format!("unchanged relative branches: {error:?}"))?;
        if checked.len() != rows.len() {
            return Err("relative branch control omitted a source row".into());
        }
        for (position, row) in rows.iter().enumerate() {
            let variant = match row.branch.as_deref() {
                Some(Branch::Jump(_)) => 0,
                Some(Branch::Conditional(_)) => 1,
                None => return Err("relative branch control lost its evidence".into()),
            };
            if std::mem::replace(&mut observed[variant], true) {
                continue;
            }
            let require = |result: Result<_, Error>, expected: Error| match result {
                Err(actual) if actual == expected => Ok(()),
                actual => Err(format!(
                    "relative branch control expected {expected:?}, got {actual:?}"
                )),
            };
            let mut omitted = rows.clone();
            omitted.pop();
            require(
                validate(&omitted),
                Error::SourceShapeMismatch(function.machine),
            )?;
            let mut duplicate = rows.clone();
            duplicate.push(row.clone());
            require(
                validate(&duplicate),
                Error::SourceShapeMismatch(function.machine),
            )?;
            let mut missing = rows.clone();
            missing[position].branch = None;
            require(validate(&missing), Error::ArtifactMismatch)?;
            let mut outside = rows.clone();
            match outside[position].branch.as_deref_mut() {
                Some(Branch::Jump(branch)) => branch.target_offset = function.byte_count,
                Some(Branch::Conditional(branch)) => branch.when_taken_offset = function.byte_count,
                None => return Err("relative branch control lost its evidence".into()),
            }
            require(validate(&outside), Error::ArtifactMismatch)?;
            let mut wrong_target = rows.clone();
            let other = function
                .blocks
                .iter()
                .find(|block| match row.branch.as_deref() {
                    Some(Branch::Jump(branch)) => block.block != branch.target_block,
                    Some(Branch::Conditional(branch)) => {
                        block.block != branch.when_fallthrough_block
                    }
                    None => false,
                })
                .ok_or("relative branch control needs two actual blocks")?;
            match wrong_target[position].branch.as_deref_mut() {
                Some(Branch::Jump(branch)) => branch.target_block = other.block,
                Some(Branch::Conditional(branch)) => branch.when_fallthrough_block = other.block,
                None => return Err("relative branch control lost its evidence".into()),
            }
            require(validate(&wrong_target), Error::ArtifactMismatch)?;
            let mut bad_bytes = rows.clone();
            bad_bytes[position].bytes.fill(0);
            match validate(&bad_bytes) {
                Err(Error::X86_64Branch(id, _)) | Err(Error::Aarch64Branch(id, _))
                    if id == row.instruction => {}
                actual => return Err(format!("relative branch decoder control: {actual:?}")),
            }
        }
    }
    if observed != [true, true] {
        return Err("relative branch control requires actual jump and conditional rows".into());
    }
    Ok(())
}

fn require_instruction_refusal<T>(
    result: Result<T, image_emission::FunctionFragmentObjectArtifactError>,
    expected: &str,
) -> Result<(), String> {
    match result {
        Err(image_emission::FunctionFragmentObjectArtifactError::Mismatch(reason))
            if reason == expected =>
        {
            Ok(())
        }
        Err(error) => Err(format!(
            "native instruction control refused for another reason: {error:?}"
        )),
        Ok(_) => Err(format!(
            "native instruction correspondence accepted: {expected}"
        )),
    }
}
