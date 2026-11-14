/*
We materialize explicit fixed-view copies (`fixed_view_copy`) across
authenticated precolored segment transitions and independently replay the
transformed selected CFG, while re-exporting `pressure_rematerialization` for
the shared allocation-recovery surface.
*/

pub(crate) mod fixed_view_copy {
    use crate::ValidatedAllocationLegality;
    use crate::ValidatedFixedPrecoloredIntervals;
    use crate::ValidatedFixedPrecoloredSegmentHomes;

    use crate::ValidatedLiveRanges;
    use register_model::{
        PreparedRegisterConstraintCatalog, TargetRegisterEnvironmentConstraintKeys,
        TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
        ValidatedRegisterReservationProfile,
    };

    use selected_instructions::SelectedProgram;

    pub(crate) mod compute {
        pub(crate) mod apply {
            use super::{RegisterInstructionConstraint, ScalarType};
            use crate::{FixedViewCopy, FixedViewCopyError};

            pub(crate) fn apply_copy(
                function_index: usize,
                function: &mut selected_instructions::SelectedFunction,
                copy: &FixedViewCopy,
                row: &RegisterInstructionConstraint,
            ) -> Result<(), FixedViewCopyError> {
                super::super::work::apply_fixed_view_copy(
                    function_index,
                    function,
                    copy,
                    row,
                    false,
                )
            }

            pub(crate) fn is_i64(scalar: ScalarType) -> bool {
                matches!(
                    scalar,
                    ScalarType::Integer(integer) if integer.bits() == 64
                )
            }
        }
        pub(crate) mod preflight {
            use super::{
                OptimizationWorkUsage, PreparedRegisterConstraintCatalog,
                RegisterInstructionConstraint, RegisterOperandAccess, SelectedInstructionId,
                SelectedProgram, SelectedTerminator, TargetRegisterEnvironmentConstraintKeys,
                TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
                ValidatedRegisterReservationProfile, VirtualRegisterId,
                target_register_environment_identity,
            };
            use crate::rewrites::block_edges::block_instruction_ids;
            use crate::{
                FixedViewCopyError, FixedViewCopyPolicy, ValidatedAllocationLegality,
                ValidatedLiveRanges,
            };
            use selected_instructions::terminator_instruction;

            pub(crate) fn validate_roots(
                selected: &SelectedProgram,
                ranges: &ValidatedLiveRanges,
                legality: &ValidatedAllocationLegality,
                register_environment: TargetRegisterEnvironmentIdentity,
                physical: &ValidatedPhysicalRegisterModel,
                constraints: &PreparedRegisterConstraintCatalog,
                reservations: &ValidatedRegisterReservationProfile,
                selected_keys: &TargetRegisterEnvironmentConstraintKeys,
            ) -> Result<(), FixedViewCopyError> {
                if ranges.plan().selected != selected.identity()
                    || ranges.plan().optimization_unit != selected.optimization_unit()
                    || ranges.plan().work_schedule != selected.work_schedule()
                    || ranges.plan().target != selected.plan().target
                    || legality.receipt().ranges() != ranges.receipt().identity()
                    || legality.receipt().register_environment() != register_environment
                    || constraints.physical_identity() != physical.identity()
                    || reservations.physical_identity() != physical.identity()
                    || reservations.target() != selected.plan().target
                    || target_register_environment_identity(
                        selected.plan().target,
                        physical,
                        constraints,
                        reservations,
                        selected_keys,
                    ) != register_environment
                    || selected.plan().functions.len() != legality.plan().functions.len()
                {
                    return Err(FixedViewCopyError::RootMismatch);
                }
                Ok(())
            }

            pub(crate) fn copy_row<'a>(
                constraints: &'a PreparedRegisterConstraintCatalog,
                selected_keys: &TargetRegisterEnvironmentConstraintKeys,
            ) -> Result<&'a RegisterInstructionConstraint, FixedViewCopyError> {
                let row = register_model::find_register_constraint(
                    &constraints.catalog().constraints,
                    selected_keys.copy_i64,
                )
                .ok_or(FixedViewCopyError::CopyConstraintMismatch)?;
                if !crate::rewrites::declares_pair(
                    row,
                    RegisterOperandAccess::Use,
                    RegisterOperandAccess::Def,
                ) || row.operands[0].class != row.operands[1].class
                    || row.operands.iter().any(|operand| {
                        operand.fixed_view.is_some()
                            || operand.tied_to.is_some()
                            || operand.early_clobber
                    })
                    || !row.implicit_uses.is_empty()
                    || !row.implicit_defs.is_empty()
                    || !row.clobbers.is_empty()
                {
                    return Err(FixedViewCopyError::CopyConstraintMismatch);
                }
                Ok(row)
            }

            pub(crate) fn work_usage(
                selected: &SelectedProgram,
                boundaries: &[crate::rewrites::allocation_recovery::fixed_view_copy::evidence::AuthenticatedFixedViewBoundary],
                policy: FixedViewCopyPolicy,
            ) -> Result<OptimizationWorkUsage, FixedViewCopyError> {
                let functions = u64::try_from(selected.plan().functions.len())
                    .map_err(|_| FixedViewCopyError::WorkOverflow)?;
                let requirements = u64::try_from(boundaries.len())
                    .map_err(|_| FixedViewCopyError::WorkOverflow)?;
                let commits = match policy {
                    FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1
                    | FixedViewCopyPolicy::ImmediateBeforeFixedUseV1 => requirements,
                    FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1
                    | FixedViewCopyPolicy::SharedSourceExitBeforeFixedUseV1 => {
                        let references = boundaries.iter().collect::<Vec<_>>();
                        u64::try_from(super::super::emission::copy_emissions(&references).len())
                            .map_err(|_| FixedViewCopyError::WorkOverflow)?
                    }
                };
                Ok(OptimizationWorkUsage {
                    rule_evaluations: functions,
                    candidates: requirements,
                    validation_steps: requirements,
                    commits,
                    iterations: 1,
                })
            }

            pub(crate) fn next_instruction_id(
                function_index: usize,
                function: &selected_instructions::SelectedFunction,
            ) -> Result<u32, FixedViewCopyError> {
                let mut ids = function
                    .blocks
                    .iter()
                    .flat_map(block_instruction_ids)
                    .collect::<Vec<_>>();
                ids.sort_unstable();
                if ids
                    != (0..u32::try_from(ids.len()).map_err(|_| {
                        FixedViewCopyError::IdentifierOverflow {
                            function: function_index,
                        }
                    })?)
                        .collect::<Vec<_>>()
                {
                    return Err(FixedViewCopyError::FunctionMismatch {
                        function: function_index,
                    });
                }
                u32::try_from(ids.len()).map_err(|_| FixedViewCopyError::IdentifierOverflow {
                    function: function_index,
                })
            }

            pub(crate) fn next_register_id(
                function_index: usize,
                function: &selected_instructions::SelectedFunction,
            ) -> Result<u32, FixedViewCopyError> {
                if !function.ids_are_positions() {
                    return Err(FixedViewCopyError::FunctionMismatch {
                        function: function_index,
                    });
                }
                u32::try_from(function.virtual_registers.len()).map_err(|_| {
                    FixedViewCopyError::IdentifierOverflow {
                        function: function_index,
                    }
                })
            }

            pub(crate) fn find_leaf_block(
                function_index: usize,
                function: &selected_instructions::SelectedFunction,
                instruction: SelectedInstructionId,
                operand: u16,
                source: VirtualRegisterId,
                to_view: register_model::RegisterViewId,
            ) -> Result<selected_instructions::SelectedBlockId, FixedViewCopyError> {
                for block in &function.blocks {
                    let SelectedTerminator::Return {
                        instruction: destination,
                        ..
                    } = &block.terminator
                    else {
                        continue;
                    };
                    if destination.id != instruction {
                        continue;
                    }
                    if block.id == function.entry_block {
                        return Err(FixedViewCopyError::NonLeafDestination {
                            function: function_index,
                            instruction: instruction.0,
                        });
                    }
                    let Some(destination_operand) = destination.operand(operand) else {
                        return Err(FixedViewCopyError::MissingDestination {
                            function: function_index,
                            instruction: instruction.0,
                        });
                    };
                    if destination_operand.virtual_register != source
                        || destination_operand.access != RegisterOperandAccess::Use
                        || destination_operand.fixed_view != Some(to_view)
                    {
                        return Err(FixedViewCopyError::MissingDestination {
                            function: function_index,
                            instruction: instruction.0,
                        });
                    }
                    return Ok(block.id);
                }
                Err(FixedViewCopyError::MissingDestination {
                    function: function_index,
                    instruction: instruction.0,
                })
            }

            pub(crate) fn find_site_block(
                function_index: usize,
                function: &selected_instructions::SelectedFunction,
                instruction: SelectedInstructionId,
                operand: u16,
                source: VirtualRegisterId,
                to_view: register_model::RegisterViewId,
            ) -> Result<selected_instructions::SelectedBlockId, FixedViewCopyError> {
                for block in &function.blocks {
                    let site = block.instruction(instruction).or_else(|| {
                        let destination = terminator_instruction(&block.terminator);
                        (destination.id == instruction).then_some(destination)
                    });
                    let Some(site) = site else {
                        continue;
                    };
                    let Some(site_operand) = site.operand(operand) else {
                        return Err(FixedViewCopyError::MissingDestination {
                            function: function_index,
                            instruction: instruction.0,
                        });
                    };
                    if site_operand.virtual_register != source
                        || site_operand.access != RegisterOperandAccess::Use
                        || site_operand.fixed_view != Some(to_view)
                    {
                        return Err(FixedViewCopyError::MissingDestination {
                            function: function_index,
                            instruction: instruction.0,
                        });
                    }
                    return Ok(block.id);
                }
                Err(FixedViewCopyError::MissingDestination {
                    function: function_index,
                    instruction: instruction.0,
                })
            }
        }
        pub(crate) mod site {
            use super::apply::{apply_copy, is_i64};
            use super::preflight::{find_leaf_block, find_site_block};
            use super::{
                BTreeSet, FixedViewCopy, FixedViewCopyDestination, FixedViewCopyError,
                RegisterInstructionConstraint, RegisterOperandAccess, SelectedInstructionId,
                VirtualFixedConstraintSite, VirtualRegisterId, VirtualRegisterOrigin,
            };
            use register_model::RegisterConstraintKey;

            pub(crate) fn admitted_source<'a>(
                function_index: usize,
                function: &'a selected_instructions::SelectedFunction,
                boundary: &super::super::evidence::AuthenticatedFixedViewBoundary,
                leaf_local: bool,
            ) -> Result<
                (
                    &'a selected_instructions::VirtualRegister,
                    semantic_vocabulary::ValueId,
                ),
                FixedViewCopyError,
            > {
                let source_register = function
                    .virtual_registers
                    .get(usize::try_from(boundary.virtual_register.0).map_err(|_| {
                        FixedViewCopyError::UnsupportedSourceRegister {
                            function: function_index,
                            register: boundary.virtual_register.0,
                        }
                    })?)
                    .filter(|register| {
                        register.id == boundary.virtual_register
                            && register.class == boundary.class
                            && (!leaf_local
                                || register.entry_fixed_view == Some(boundary.from_view))
                    })
                    .ok_or(FixedViewCopyError::UnsupportedSourceRegister {
                        function: function_index,
                        register: boundary.virtual_register.0,
                    })?;
                let source_value = match (leaf_local, source_register.origin) {
                    (_, VirtualRegisterOrigin::EntryParameter { source_value, .. })
                    | (
                        false,
                        VirtualRegisterOrigin::InstructionResult { source_value, .. }
                        | VirtualRegisterOrigin::BlockParameter { source_value, .. }
                        | VirtualRegisterOrigin::ScalarAbiAddress { source_value, .. },
                    ) => source_value,
                    _ => {
                        return Err(FixedViewCopyError::UnsupportedSourceRegister {
                            function: function_index,
                            register: boundary.virtual_register.0,
                        });
                    }
                };
                if !is_i64(source_register.scalar_type) {
                    return Err(FixedViewCopyError::UnsupportedSourceRegister {
                        function: function_index,
                        register: boundary.virtual_register.0,
                    });
                }
                Ok((source_register, source_value))
            }

            pub(crate) fn destination_block(
                function_index: usize,
                function: &selected_instructions::SelectedFunction,
                boundary: &super::super::evidence::AuthenticatedFixedViewBoundary,
                instruction: SelectedInstructionId,
                operand: u16,
                source: VirtualRegisterId,
                leaf_local: bool,
            ) -> Result<selected_instructions::SelectedBlockId, FixedViewCopyError> {
                let block = if leaf_local {
                    find_leaf_block(
                        function_index,
                        function,
                        instruction,
                        operand,
                        source,
                        boundary.to_view,
                    )?
                } else {
                    find_site_block(
                        function_index,
                        function,
                        instruction,
                        operand,
                        source,
                        boundary.to_view,
                    )?
                };
                if block != boundary.block {
                    return Err(FixedViewCopyError::SegmentEvidenceMismatch);
                }
                Ok(block)
            }

            #[allow(clippy::too_many_arguments)]
            pub(crate) fn build_site_copies(
                function_index: usize,
                source_function: &selected_instructions::SelectedFunction,
                boundaries: &[&super::super::evidence::AuthenticatedFixedViewBoundary],
                transformed: &mut selected_instructions::SelectedFunction,
                copy_row: &RegisterInstructionConstraint,
                copy_key: RegisterConstraintKey,
                leaf_local: bool,
                mut next_instruction: u32,
                mut next_register: u32,
            ) -> Result<Vec<FixedViewCopy>, FixedViewCopyError> {
                let mut copies = Vec::new();
                let mut destinations = BTreeSet::new();
                for boundary in boundaries {
                    let VirtualFixedConstraintSite::Operand {
                        instruction,
                        operand,
                        access: RegisterOperandAccess::Use,
                        ..
                    } = boundary.site
                    else {
                        return Err(FixedViewCopyError::UnsupportedTransitionSite {
                            function: function_index,
                            register: boundary.virtual_register.0,
                        });
                    };
                    if !destinations.insert((instruction, operand)) {
                        return Err(FixedViewCopyError::NonCanonicalCopies);
                    }
                    let (source_register, source_value) =
                        admitted_source(function_index, source_function, boundary, leaf_local)?;
                    if copy_row.operands[0].class != source_register.class
                        || copy_row.operands[1].class != source_register.class
                    {
                        return Err(FixedViewCopyError::UnsupportedSourceRegister {
                            function: function_index,
                            register: boundary.virtual_register.0,
                        });
                    }
                    let function_u32 = u32::try_from(function_index).map_err(|_| {
                        FixedViewCopyError::IdentifierOverflow {
                            function: function_index,
                        }
                    })?;
                    let copy = FixedViewCopy {
                        function: function_u32,
                        machine: source_function.machine,
                        source_virtual_register: source_register.id,
                        source_value,
                        source_definition_site: source_register.definition_site.ok_or(
                            FixedViewCopyError::UnsupportedSourceRegister {
                                function: function_index,
                                register: source_register.id.0,
                            },
                        )?,
                        from_view: boundary.from_view,
                        to_view: boundary.to_view,
                        insertion_block: destination_block(
                            function_index,
                            source_function,
                            boundary,
                            instruction,
                            operand,
                            source_register.id,
                            leaf_local,
                        )?,
                        before_instruction: instruction,
                        destinations: vec![FixedViewCopyDestination {
                            site: boundary.site,
                            block: boundary.block,
                            view: boundary.to_view,
                        }],
                        copy_instruction: SelectedInstructionId(next_instruction),
                        result_virtual_register: VirtualRegisterId(next_register),
                        copy_constraint: copy_key,
                    };
                    apply_copy(function_index, transformed, &copy, copy_row)?;
                    copies.push(copy);
                    super::super::bump_identifier(&mut next_instruction, function_index)?;
                    super::super::bump_identifier(&mut next_register, function_index)?;
                }
                Ok(copies)
            }
        }
        pub(crate) mod source_exit {
            use super::apply::apply_copy;
            use super::site::{admitted_source, destination_block};
            use super::{
                BTreeSet, FixedViewCopy, FixedViewCopyDestination, FixedViewCopyError,
                FixedViewCopyPolicy, RegisterInstructionConstraint, RegisterOperandAccess,
                SelectedInstructionId, VirtualFixedConstraintSite, VirtualRegisterId,
            };
            use register_model::RegisterConstraintKey;

            #[allow(clippy::too_many_arguments)]
            pub(crate) fn build_source_exit_copies(
                function_index: usize,
                source_function: &selected_instructions::SelectedFunction,
                boundaries: &[&super::super::evidence::AuthenticatedFixedViewBoundary],
                transformed: &mut selected_instructions::SelectedFunction,
                copy_row: &RegisterInstructionConstraint,
                copy_key: RegisterConstraintKey,
                policy: FixedViewCopyPolicy,
                mut next_instruction: u32,
                mut next_register: u32,
            ) -> Result<Vec<FixedViewCopy>, FixedViewCopyError> {
                let declared = policy == FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1;
                let mut copies = Vec::new();
                let mut destinations = BTreeSet::new();
                for emission in super::super::emission::copy_emissions(boundaries) {
                    if declared && emission.exit.is_none() {
                        return Err(FixedViewCopyError::UnsupportedSharedTransitionSet {
                            function: function_index,
                        });
                    }
                    let first = emission.members[0];
                    let (source_register, source_value) =
                        admitted_source(function_index, source_function, first, declared)?;
                    if copy_row.operands[0].class != source_register.class
                        || copy_row.operands[1].class != source_register.class
                    {
                        return Err(FixedViewCopyError::UnsupportedSourceRegister {
                            function: function_index,
                            register: first.virtual_register.0,
                        });
                    }
                    let mut members = Vec::with_capacity(emission.members.len());
                    for member in &emission.members {
                        let VirtualFixedConstraintSite::Operand {
                            instruction,
                            operand,
                            access: RegisterOperandAccess::Use,
                            ..
                        } = member.site
                        else {
                            return Err(FixedViewCopyError::UnsupportedTransitionSite {
                                function: function_index,
                                register: member.virtual_register.0,
                            });
                        };
                        if !destinations.insert((instruction, operand)) {
                            return Err(FixedViewCopyError::NonCanonicalCopies);
                        }
                        let block = destination_block(
                            function_index,
                            source_function,
                            member,
                            instruction,
                            operand,
                            source_register.id,
                            false,
                        )?;
                        members.push(FixedViewCopyDestination {
                            site: member.site,
                            block,
                            view: member.to_view,
                        });
                    }
                    let (insertion_block, before_instruction) = match emission.exit {
                        Some(connector) => (connector.source, connector.terminator),
                        None => {
                            let VirtualFixedConstraintSite::Operand { instruction, .. } =
                                members[0].site
                            else {
                                return Err(FixedViewCopyError::UnsupportedTransitionSite {
                                    function: function_index,
                                    register: first.virtual_register.0,
                                });
                            };
                            (first.block, instruction)
                        }
                    };
                    let copy = FixedViewCopy {
                        function: u32::try_from(function_index).map_err(|_| {
                            FixedViewCopyError::IdentifierOverflow {
                                function: function_index,
                            }
                        })?,
                        machine: source_function.machine,
                        source_virtual_register: source_register.id,
                        source_value,
                        source_definition_site: source_register.definition_site.ok_or(
                            FixedViewCopyError::UnsupportedSourceRegister {
                                function: function_index,
                                register: source_register.id.0,
                            },
                        )?,
                        from_view: first.from_view,
                        to_view: first.to_view,
                        insertion_block,
                        before_instruction,
                        destinations: members,
                        copy_instruction: SelectedInstructionId(next_instruction),
                        result_virtual_register: VirtualRegisterId(next_register),
                        copy_constraint: copy_key,
                    };
                    apply_copy(function_index, transformed, &copy, copy_row)?;
                    copies.push(copy);
                    super::super::bump_identifier(&mut next_instruction, function_index)?;
                    super::super::bump_identifier(&mut next_register, function_index)?;
                }
                Ok(copies)
            }
        }

        use std::collections::BTreeSet;

        use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
        use register_model::{
            PreparedRegisterConstraintCatalog, RegisterInstructionConstraint,
            RegisterOperandAccess, TargetRegisterEnvironmentConstraintKeys,
            TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
            ValidatedRegisterReservationProfile, target_register_environment_identity,
        };
        use selected_instructions::SelectedProgram;
        use selected_instructions::{
            SelectedInstructionId, SelectedTerminator, VirtualRegisterId, VirtualRegisterOrigin,
        };
        use semantic_vocabulary::ScalarType;

        use crate::{
            FixedViewCopy, FixedViewCopyDestination, FixedViewCopyError, FixedViewCopyPlan,
            FixedViewCopyPolicy, FixedViewCopySourceEvidence, ValidatedAllocationLegality,
            ValidatedFixedPrecoloredIntervals, ValidatedFixedPrecoloredSegmentHomes,
            ValidatedFixedPrecoloredSplitRequirements, ValidatedLiveRanges,
        };
        use selected_instructions::VirtualFixedConstraintSite;

        use preflight::{
            copy_row, next_instruction_id, next_register_id, validate_roots, work_usage,
        };
        use site::build_site_copies;
        use source_exit::build_source_exit_copies;

        pub(crate) fn compute_terminal_fixed_view_copies(
            selected: &SelectedProgram,
            ranges: &ValidatedLiveRanges,
            legality: &ValidatedAllocationLegality,
            fixed: &ValidatedFixedPrecoloredIntervals,
            requirements: &ValidatedFixedPrecoloredSplitRequirements,
            homes: &ValidatedFixedPrecoloredSegmentHomes,
            register_environment: TargetRegisterEnvironmentIdentity,
            physical: &ValidatedPhysicalRegisterModel,
            constraints: &PreparedRegisterConstraintCatalog,
            reservations: &ValidatedRegisterReservationProfile,
            selected_keys: &TargetRegisterEnvironmentConstraintKeys,
            policy: FixedViewCopyPolicy,
            budget: OptimizationWorkBudget,
        ) -> Result<FixedViewCopyPlan, FixedViewCopyError> {
            validate_roots(
                selected,
                ranges,
                legality,
                register_environment,
                physical,
                constraints,
                reservations,
                selected_keys,
            )?;
            let evidence =
                super::evidence::derive_positionally(ranges, legality, fixed, requirements, homes)?;
            let copy_row = copy_row(constraints, selected_keys)?;
            let usage = super::work::combined_usage(
                evidence.usage,
                work_usage(selected, &evidence.boundaries, policy)?,
            )?;
            crate::work_accounting::admit(usage, budget, |required, budget| {
                FixedViewCopyError::BudgetExceeded { required, budget }
            })?;

            let mut transformed = selected.plan().clone();
            let mut copies = Vec::new();
            for (function_index, source_function) in selected.plan().functions.iter().enumerate() {
                let boundaries = evidence
                    .boundaries
                    .iter()
                    .filter(|boundary| boundary.function == function_index)
                    .collect::<Vec<_>>();
                let next_instruction = next_instruction_id(function_index, source_function)?;
                let next_register = next_register_id(function_index, source_function)?;
                if matches!(
                    policy,
                    FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1
                        | FixedViewCopyPolicy::SharedSourceExitBeforeFixedUseV1
                ) {
                    copies.extend(build_source_exit_copies(
                        function_index,
                        source_function,
                        &boundaries,
                        &mut transformed.functions[function_index],
                        copy_row,
                        selected_keys.copy_i64,
                        policy,
                        next_instruction,
                        next_register,
                    )?);
                    continue;
                }
                let leaf_local = policy == FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1;
                copies.extend(build_site_copies(
                    function_index,
                    source_function,
                    &boundaries,
                    &mut transformed.functions[function_index],
                    copy_row,
                    selected_keys.copy_i64,
                    leaf_local,
                    next_instruction,
                    next_register,
                )?);
            }

            Ok(FixedViewCopyPlan {
                source_selected: selected.identity(),
                source_ranges: ranges.receipt().identity(),
                source_legality: legality.receipt().identity(),
                register_environment,
                allocator_availability: legality.receipt().allocator_availability(),
                source_evidence: FixedViewCopySourceEvidence::FixedPrecoloredSegmentHomesV1 {
                    fixed_intervals: fixed.receipt().identity(),
                    split_requirements: requirements.receipt().identity(),
                    segment_homes: homes.receipt().identity(),
                },
                policy,
                budget,
                usage,
                copies,
                transformed: transformed.into(),
            })
        }
    }
    pub(crate) mod emission {
        use selected_instructions::LiveRangeEdgeConnector;

        use selected_instructions::VirtualFixedConstraintSite;

        use super::evidence::AuthenticatedFixedViewBoundary;

        pub(crate) struct CopyEmission<'a> {
            pub(crate) members: Vec<&'a AuthenticatedFixedViewBoundary>,
            pub(crate) exit: Option<LiveRangeEdgeConnector>,
        }

        pub(crate) fn copy_emissions<'a>(
            boundaries: &[&'a AuthenticatedFixedViewBoundary],
        ) -> Vec<CopyEmission<'a>> {
            let mut groups: Vec<Vec<&'a AuthenticatedFixedViewBoundary>> = Vec::new();
            for boundary in boundaries.iter().copied() {
                if let Some(group) = groups.iter_mut().find(|group| {
                    let first = group[0];
                    first.function == boundary.function
                        && first.virtual_register == boundary.virtual_register
                        && first.source_segment == boundary.source_segment
                        && first.source_domain == boundary.source_domain
                        && first.from_view == boundary.from_view
                        && first.to_view == boundary.to_view
                }) {
                    group.push(boundary);
                } else {
                    groups.push(vec![boundary]);
                }
            }
            let mut emissions = Vec::with_capacity(groups.len());
            for mut members in groups {
                let exit = if members.len() > 1 {
                    members
                        .iter()
                        .map(|member| member.incoming)
                        .collect::<Option<Vec<_>>>()
                        .and_then(|connectors| {
                            let first = connectors[0];
                            connectors
                                .iter()
                                .all(|connector| {
                                    connector.source == first.source
                                        && connector.terminator == first.terminator
                                })
                                .then_some(first)
                        })
                } else {
                    None
                };
                if let Some(exit) = exit {
                    members.sort_by_key(|member| site_key(member));
                    emissions.push(CopyEmission {
                        members,
                        exit: Some(exit),
                    });
                } else {
                    emissions.extend(members.into_iter().map(|member| CopyEmission {
                        members: vec![member],
                        exit: None,
                    }));
                }
            }
            emissions
        }

        fn site_key(boundary: &AuthenticatedFixedViewBoundary) -> (u32, u16) {
            match boundary.site {
                VirtualFixedConstraintSite::Operand {
                    instruction,
                    operand,
                    ..
                } => (instruction.0, operand),
                VirtualFixedConstraintSite::Entry => (u32::MAX, u16::MAX),
            }
        }
    }
    pub(crate) mod evidence {
        pub(crate) mod compute {
            use std::collections::BTreeMap;

            use optimization_core::OptimizationWorkUsage;

            use crate::{
                FixedViewCopyError, ValidatedAllocationLegality, ValidatedFixedPrecoloredIntervals,
                ValidatedFixedPrecoloredSegmentHomes, ValidatedFixedPrecoloredSplitRequirements,
                ValidatedLiveRanges,
            };
            use register_homes::{
                FixedPrecoloredIntervalPolicy, FixedPrecoloredSegmentHomePolicy,
                FixedPrecoloredSourceSegmentHome, FixedPrecoloredSourceSegmentOpening,
                FixedPrecoloredSplitRequirementPolicy, FunctionFixedPrecoloredSegmentHomes,
                FunctionFixedPrecoloredSplitRequirements,
            };

            use super::{AuthenticatedFixedViewBoundary, FixedViewBoundaryEvidence};

            pub(crate) fn derive(
                ranges: &ValidatedLiveRanges,
                legality: &ValidatedAllocationLegality,
                fixed: &ValidatedFixedPrecoloredIntervals,
                requirements: &ValidatedFixedPrecoloredSplitRequirements,
                homes: &ValidatedFixedPrecoloredSegmentHomes,
            ) -> Result<FixedViewBoundaryEvidence, FixedViewCopyError> {
                validate_roots(ranges, legality, fixed, requirements, homes)?;
                let mut work = Work::new();
                let mut boundaries = derive_roster(
                    requirements.plan().functions.as_slice(),
                    homes.plan().functions.as_slice(),
                    &mut work,
                )?;
                boundaries.sort_by_key(boundary_key);
                Ok(FixedViewBoundaryEvidence {
                    boundaries,
                    usage: work.finish()?,
                })
            }

            fn validate_roots(
                ranges: &ValidatedLiveRanges,
                legality: &ValidatedAllocationLegality,
                fixed: &ValidatedFixedPrecoloredIntervals,
                requirements: &ValidatedFixedPrecoloredSplitRequirements,
                homes: &ValidatedFixedPrecoloredSegmentHomes,
            ) -> Result<(), FixedViewCopyError> {
                if fixed.receipt().ranges() != ranges.receipt().identity()
                    || fixed.receipt().legality() != legality.receipt().identity()
                    || fixed.receipt().policy()
                        != FixedPrecoloredIntervalPolicy::FixedConstraintPointIntervalsV1
                    || requirements.receipt().fixed_intervals() != fixed.receipt().identity()
                    || requirements.receipt().ranges() != ranges.receipt().identity()
                    || requirements.receipt().legality() != legality.receipt().identity()
                    || requirements.receipt().policy()
                        != FixedPrecoloredSplitRequirementPolicy::FixedUseBoundaryRequirementsV1
                    || homes.receipt().fixed_intervals() != fixed.receipt().identity()
                    || homes.receipt().split_requirements() != requirements.receipt().identity()
                    || homes.receipt().ranges() != ranges.receipt().identity()
                    || homes.receipt().legality() != legality.receipt().identity()
                    || homes.receipt().register_environment()
                        != legality.receipt().register_environment()
                    || homes.receipt().allocator_availability()
                        != legality.receipt().allocator_availability()
                    || homes.receipt().optimization_unit() != ranges.receipt().optimization_unit()
                    || homes.receipt().work_schedule() != ranges.receipt().work_schedule()
                    || homes.receipt().target() != ranges.plan().target
                    || homes.receipt().policy()
                        != FixedPrecoloredSegmentHomePolicy::MostConstrainedLowestCompatibleViewV1
                {
                    return Err(FixedViewCopyError::SegmentEvidenceMismatch);
                }
                Ok(())
            }

            fn derive_roster(
                requirements: &[FunctionFixedPrecoloredSplitRequirements],
                homes: &[FunctionFixedPrecoloredSegmentHomes],
                work: &mut Work,
            ) -> Result<Vec<AuthenticatedFixedViewBoundary>, FixedViewCopyError> {
                if requirements.len() != homes.len() {
                    return Err(FixedViewCopyError::SegmentEvidenceMismatch);
                }
                let mut boundaries = Vec::new();
                for (function, (requirements, homes)) in requirements.iter().zip(homes).enumerate()
                {
                    bump(&mut work.functions)?;
                    if requirements.machine != homes.machine {
                        return Err(FixedViewCopyError::SegmentEvidenceMismatch);
                    }
                    let mut assignment_index = 0;
                    for register in &requirements.registers {
                        bump(&mut work.registers)?;
                        let mut closing = BTreeMap::<_, FixedPrecoloredSourceSegmentHome>::new();
                        for fragment in &register.fragments {
                            bump(&mut work.fragments)?;
                            let mut previous = None;
                            for segment in &fragment.segments {
                                bump(&mut work.segments)?;
                                let assignment = homes
                                    .assignments
                                    .get(assignment_index)
                                    .ok_or(FixedViewCopyError::SegmentEvidenceMismatch)?;
                                assignment_index += 1;
                                bump(&mut work.homes)?;
                                validate_assignment(
                                    register.virtual_register,
                                    register.class,
                                    segment,
                                    assignment,
                                )?;
                                match segment.opening {
                                    FixedPrecoloredSourceSegmentOpening::SourceRangeStartV1 => {
                                        if previous.is_some() {
                                            return Err(FixedViewCopyError::SegmentEvidenceMismatch);
                                        }
                                    }
                                    FixedPrecoloredSourceSegmentOpening::IncomingSourceEdgeV1 { connector } => {
                                        let source = closing.get(&connector.source).copied().ok_or(
                                            FixedViewCopyError::SegmentEvidenceMismatch,
                                        )?;
                                        if connector.target != fragment.block
                                            || source.allocation_domain != assignment.allocation_domain
                                            || source.view != assignment.view
                                        {
                                            return Err(FixedViewCopyError::SegmentEvidenceMismatch);
                                        }
                                    }
                                    FixedPrecoloredSourceSegmentOpening::IncompatibleFixedUseDomainBoundaryV1 {
                                        incoming,
                                        site,
                                        destination_view,
                                    } => {
                                        let source = match incoming {
                                            Some(connector) => {
                                                if connector.target != fragment.block {
                                                    return Err(FixedViewCopyError::SegmentEvidenceMismatch);
                                                }
                                                closing.get(&connector.source).copied()
                                            }
                                            None => previous,
                                        }
                                        .ok_or(FixedViewCopyError::SegmentEvidenceMismatch)?;
                                        if assignment.view != destination_view
                                            || source.allocation_domain == assignment.allocation_domain
                                        {
                                            return Err(FixedViewCopyError::SegmentEvidenceMismatch);
                                        }
                                        bump(&mut work.boundaries)?;
                                        boundaries.push(AuthenticatedFixedViewBoundary {
                                            function,
                                            machine: requirements.machine,
                                            virtual_register: register.virtual_register,
                                            class: register.class,
                                            source_segment: source.source_segment,
                                            source_domain: source.allocation_domain,
                                            from_view: source.view,
                                            destination_segment: assignment.source_segment,
                                            destination_domain: assignment.allocation_domain,
                                            site,
                                            block: fragment.block,
                                            to_view: assignment.view,
                                            incoming,
                                        });
                                    }
                                }
                                previous = Some(*assignment);
                            }
                            let last =
                                previous.ok_or(FixedViewCopyError::SegmentEvidenceMismatch)?;
                            closing.insert(fragment.block, last);
                        }
                    }
                    if assignment_index != homes.assignments.len() {
                        return Err(FixedViewCopyError::SegmentEvidenceMismatch);
                    }
                }
                Ok(boundaries)
            }

            fn validate_assignment(
                register: selected_instructions::VirtualRegisterId,
                class: register_model::RegisterClassId,
                segment: &register_homes::FixedPrecoloredSourceSegment,
                assignment: &FixedPrecoloredSourceSegmentHome,
            ) -> Result<(), FixedViewCopyError> {
                if assignment.virtual_register != register
                    || assignment.class != class
                    || assignment.source_segment != segment.id
                    || segment.candidates.binary_search(&assignment.view).is_err()
                {
                    return Err(FixedViewCopyError::SegmentEvidenceMismatch);
                }
                Ok(())
            }

            fn boundary_key(boundary: &AuthenticatedFixedViewBoundary) -> (usize, u32, u32, u32) {
                (
                    boundary.function,
                    boundary.virtual_register.0,
                    boundary.block.0,
                    boundary.destination_segment.0,
                )
            }

            struct Work {
                functions: u64,
                registers: u64,
                fragments: u64,
                segments: u64,
                homes: u64,
                boundaries: u64,
            }

            impl Work {
                const fn new() -> Self {
                    Self {
                        functions: 0,
                        registers: 0,
                        fragments: 0,
                        segments: 0,
                        homes: 0,
                        boundaries: 0,
                    }
                }
                fn finish(self) -> Result<OptimizationWorkUsage, FixedViewCopyError> {
                    Ok(OptimizationWorkUsage {
                        rule_evaluations: add(add(1, self.functions)?, self.boundaries)?,
                        candidates: self.boundaries,
                        validation_steps: add(
                            add(
                                add(
                                    add(
                                        add(add(1, self.functions)?, self.registers)?,
                                        self.fragments,
                                    )?,
                                    self.segments,
                                )?,
                                self.homes,
                            )?,
                            self.boundaries,
                        )?,
                        commits: self.boundaries,
                        iterations: add(
                            add(add(add(1, self.functions)?, self.registers)?, self.segments)?,
                            self.boundaries,
                        )?,
                    })
                }
            }

            fn bump(value: &mut u64) -> Result<(), FixedViewCopyError> {
                *value = value
                    .checked_add(1)
                    .ok_or(FixedViewCopyError::WorkOverflow)?;
                Ok(())
            }

            use super::super::work::add;
        }
        pub(crate) use compute::derive as derive_positionally;
        use optimization_core::OptimizationWorkUsage;
        use register_homes::{FixedPrecoloredHomeDomainId, FixedPrecoloredSourceSegmentId};
        use register_model::{RegisterClassId, RegisterViewId};

        use selected_instructions::{
            LiveRangeEdgeConnector, SelectedBlockId, VirtualFixedConstraintSite, VirtualRegisterId,
        };
        use semantic_vocabulary::MachineId;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct AuthenticatedFixedViewBoundary {
            pub(crate) function: usize,
            pub(crate) machine: MachineId,
            pub(crate) virtual_register: VirtualRegisterId,
            pub(crate) class: RegisterClassId,
            pub(crate) source_segment: FixedPrecoloredSourceSegmentId,
            pub(crate) source_domain: FixedPrecoloredHomeDomainId,
            pub(crate) from_view: RegisterViewId,
            pub(crate) destination_segment: FixedPrecoloredSourceSegmentId,
            pub(crate) destination_domain: FixedPrecoloredHomeDomainId,
            pub(crate) site: VirtualFixedConstraintSite,
            pub(crate) block: SelectedBlockId,
            pub(crate) to_view: RegisterViewId,
            pub(crate) incoming: Option<LiveRangeEdgeConnector>,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct FixedViewBoundaryEvidence {
            pub(crate) boundaries: Vec<AuthenticatedFixedViewBoundary>,
            pub(crate) usage: OptimizationWorkUsage,
        }
    }
    pub(crate) mod validate {
        pub(crate) mod seal {
            use selected_instructions::{SelectedProgram, selected_instruction_plan_identity};

            use crate::{
                FixedViewCopyPlan, FixedViewCopyValidationReceipt, ValidatedFixedViewCopies,
                fixed_view_copy_identity,
            };

            pub(crate) fn seal_validation(
                selected: &SelectedProgram,
                plan: FixedViewCopyPlan,
            ) -> ValidatedFixedViewCopies {
                let receipt = FixedViewCopyValidationReceipt {
                    identity: fixed_view_copy_identity(&plan),
                    source_selected: plan.source_selected,
                    source_ranges: plan.source_ranges,
                    source_legality: plan.source_legality,
                    register_environment: plan.register_environment,
                    allocator_availability: plan.allocator_availability,
                    source_evidence: plan.source_evidence,
                    transformed_selected: selected_instruction_plan_identity(&plan.transformed),
                    optimization_unit: selected.optimization_unit(),
                    work_schedule: selected.work_schedule(),
                    policy: plan.policy,
                    usage: plan.usage,
                    function_count: plan.transformed.functions.len(),
                    copy_count: plan.copies.len(),
                };
                ValidatedFixedViewCopies { plan, receipt }
            }
        }
        pub(crate) use seal::seal_validation;
    }
    pub(crate) mod work {
        use optimization_core::OptimizationWorkUsage;

        use crate::FixedViewCopyError;

        use selected_instructions::VirtualFixedConstraintSite;

        use super::FixedViewCopy;
        use crate::rewrites::selected_operands::selected_operand;

        pub(crate) fn combined_usage(
            evidence: OptimizationWorkUsage,
            transformation: OptimizationWorkUsage,
        ) -> Result<OptimizationWorkUsage, FixedViewCopyError> {
            crate::work_accounting::combine(evidence, transformation)
                .map_err(|_| FixedViewCopyError::WorkOverflow)
        }

        pub(crate) fn add(left: u64, right: u64) -> Result<u64, FixedViewCopyError> {
            left.checked_add(right)
                .ok_or(FixedViewCopyError::WorkOverflow)
        }

        pub(crate) fn apply_fixed_view_copy(
            function_index: usize,
            function: &mut selected_instructions::SelectedFunction,
            copy: &FixedViewCopy,
            row: &register_model::RegisterInstructionConstraint,
            require_use_access: bool,
        ) -> Result<(), FixedViewCopyError> {
            let source = function
                .virtual_register(copy.source_virtual_register)
                .cloned()
                .ok_or(FixedViewCopyError::UnsupportedSourceRegister {
                    function: function_index,
                    register: copy.source_virtual_register.0,
                })?;
            for destination in &copy.destinations {
                let VirtualFixedConstraintSite::Operand {
                    instruction,
                    operand,
                    access,
                    ..
                } = destination.site
                else {
                    return Err(FixedViewCopyError::UnsupportedTransitionSite {
                        function: function_index,
                        register: copy.source_virtual_register.0,
                    });
                };
                if require_use_access && access != register_model::RegisterOperandAccess::Use {
                    return Err(FixedViewCopyError::UnsupportedTransitionSite {
                        function: function_index,
                        register: copy.source_virtual_register.0,
                    });
                }
                let destination_block = function
                    .blocks
                    .iter_mut()
                    .find(|block| block.id == destination.block)
                    .ok_or(FixedViewCopyError::MissingDestination {
                        function: function_index,
                        instruction: instruction.0,
                    })?;
                let site_instruction =
                    if selected_instructions::terminator_instruction(&destination_block.terminator)
                        .id
                        == instruction
                    {
                        selected_instructions::terminator_instruction_mut(
                            &mut destination_block.terminator,
                        )
                    } else {
                        destination_block
                            .instructions
                            .iter_mut()
                            .find(|candidate| candidate.id == instruction)
                            .ok_or(FixedViewCopyError::MissingDestination {
                                function: function_index,
                                instruction: instruction.0,
                            })?
                    };
                site_instruction
                    .operands
                    .iter_mut()
                    .find(|candidate| candidate.operand == operand)
                    .ok_or(FixedViewCopyError::MissingDestination {
                        function: function_index,
                        instruction: instruction.0,
                    })?
                    .virtual_register = copy.result_virtual_register;
            }
            let block = function
                .blocks
                .iter_mut()
                .find(|block| block.id == copy.insertion_block)
                .ok_or(FixedViewCopyError::MissingDestination {
                    function: function_index,
                    instruction: copy.before_instruction.0,
                })?;
            let insertion_index =
                if selected_instructions::terminator_instruction(&block.terminator).id
                    == copy.before_instruction
                {
                    block.instructions.len()
                } else {
                    block.instruction_index(copy.before_instruction).ok_or(
                        FixedViewCopyError::InvalidInsertionSite {
                            function: function_index,
                            instruction: copy.before_instruction.0,
                        },
                    )?
                };
            function
                .virtual_registers
                .push(selected_instructions::VirtualRegister {
                    id: copy.result_virtual_register,
                    scalar_type: source.scalar_type,
                    class: source.class,
                    origin: selected_instructions::VirtualRegisterOrigin::InstructionResult {
                        instruction: copy.copy_instruction,
                        source_value: copy.source_value,
                    },
                    definition_site: Some(copy.source_definition_site),
                    entry_fixed_view: None,
                });
            let insertion_index =
                u32::try_from(insertion_index).map_err(|_| FixedViewCopyError::WorkOverflow)?;
            for settlement in &mut function.boundary_settlements {
                if settlement.block == block.id && settlement.instruction_index >= insertion_index {
                    settlement.instruction_index = settlement
                        .instruction_index
                        .checked_add(1)
                        .ok_or(FixedViewCopyError::WorkOverflow)?;
                }
            }
            block.instructions.insert(
                usize::try_from(insertion_index).map_err(|_| FixedViewCopyError::WorkOverflow)?,
                selected_instructions::SelectedInstruction {
                    id: copy.copy_instruction,
                    kind: selected_instructions::SelectedInstructionKind::CopyI64,
                    constraint: copy.copy_constraint,
                    operands: vec![
                        selected_operand(&row.operands[0], copy.source_virtual_register),
                        selected_operand(&row.operands[1], copy.result_virtual_register),
                    ],
                    implicit_uses: row.implicit_uses.clone(),
                    implicit_defs: row.implicit_defs.clone(),
                    clobbers: row.clobbers.clone(),
                    provenance: selected_instructions::SelectedInstructionProvenance {
                        operations: Vec::new(),
                        values: vec![copy.source_value],
                        edges: Vec::new(),
                        obligations: Vec::new(),
                        work: Vec::new(),
                    },
                },
            );
            Ok(())
        }
    }

    use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
    use register_homes::{AllocationLegalityIdentity, AllocatorAvailabilityIdentity};
    pub use register_homes::{
        FixedViewCopy, FixedViewCopyDestination, FixedViewCopyPlan, FixedViewCopyPolicy,
        FixedViewCopySourceEvidence, fixed_view_copy_identity,
    };

    use crate::ValidatedFixedPrecoloredSplitRequirements;
    use selected_instructions::{
        FixedViewCopyIdentity, LiveRangeIdentity, SelectedInstructionPlanIdentity,
    };

    pub(crate) fn materialize_fixed_view_copies(
        selected: &SelectedProgram,
        ranges: &ValidatedLiveRanges,
        legality: &ValidatedAllocationLegality,
        fixed: &ValidatedFixedPrecoloredIntervals,
        requirements: &ValidatedFixedPrecoloredSplitRequirements,
        homes: &ValidatedFixedPrecoloredSegmentHomes,
        register_environment: TargetRegisterEnvironmentIdentity,
        physical: &ValidatedPhysicalRegisterModel,
        constraints: &PreparedRegisterConstraintCatalog,
        reservations: &ValidatedRegisterReservationProfile,
        selected_keys: &TargetRegisterEnvironmentConstraintKeys,
        policy: FixedViewCopyPolicy,
        budget: optimization_core::OptimizationWorkBudget,
    ) -> Result<ValidatedFixedViewCopies, FixedViewCopyError> {
        let plan = compute::compute_terminal_fixed_view_copies(
            selected,
            ranges,
            legality,
            fixed,
            requirements,
            homes,
            register_environment,
            physical,
            constraints,
            reservations,
            selected_keys,
            policy,
            budget,
        )?;
        Ok(validate::seal_validation(selected, plan))
    }

    copy_receipt! {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct FixedViewCopyValidationReceipt {
            pub(crate) identity: FixedViewCopyIdentity,
            pub(crate) source_selected: SelectedInstructionPlanIdentity,
            pub(crate) source_ranges: LiveRangeIdentity,
            pub(crate) source_legality: AllocationLegalityIdentity,
            pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
            pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
            pub(crate) source_evidence: FixedViewCopySourceEvidence,
            pub(crate) transformed_selected: SelectedInstructionPlanIdentity,
            pub(crate) optimization_unit: optimization_core::OptimizationUnitIdentity,
            pub(crate) work_schedule: semantic_vocabulary::WorkScheduleIdentity,
            pub(crate) policy: FixedViewCopyPolicy,
            pub(crate) usage: OptimizationWorkUsage,
            pub(crate) function_count: usize,
            pub(crate) copy_count: usize,
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ValidatedFixedViewCopies {
        pub(crate) plan: FixedViewCopyPlan,
        pub(crate) receipt: FixedViewCopyValidationReceipt,
    }

    impl ValidatedFixedViewCopies {
        pub const fn plan(&self) -> &FixedViewCopyPlan {
            &self.plan
        }
        receipt_accessor_rows!(ref ValidatedFixedViewCopies {
            receipt: FixedViewCopyValidationReceipt,
        });
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum FixedViewCopyError {
        RootMismatch,
        WorkOverflow,
        BudgetExceeded {
            required: OptimizationWorkUsage,
            budget: OptimizationWorkBudget,
        },
        FunctionMismatch {
            function: usize,
        },
        UnsupportedPolicy,
        LegacySourceEvidence,
        SegmentEvidenceMismatch,
        UnsupportedSegmentBoundarySet {
            function: usize,
        },
        UnsupportedTransitionSite {
            function: usize,
            register: u32,
        },
        UnsupportedSourceRegister {
            function: usize,
            register: u32,
        },
        MissingDestination {
            function: usize,
            instruction: u32,
        },
        NonLeafDestination {
            function: usize,
            instruction: u32,
        },
        UnsupportedSharedTransitionSet {
            function: usize,
        },
        InvalidInsertionSite {
            function: usize,
            instruction: u32,
        },
        CopyConstraintMismatch,
        IdentifierOverflow {
            function: usize,
        },
        NonCanonicalCopies,
        CopyMismatch {
            index: usize,
        },
        TransformedPlanMismatch,
        ReceiptMismatch,
    }

    failed_display!(
        FixedViewCopyError,
        "Terminal fixed-view copy materialization failed"
    );

    pub(crate) fn bump_identifier(
        value: &mut u32,
        function: usize,
    ) -> Result<(), super::FixedViewCopyError> {
        *value = value
            .checked_add(1)
            .ok_or(super::FixedViewCopyError::IdentifierOverflow { function })?;
        Ok(())
    }
}
pub(crate) use crate::pressure_rematerialization_recovery as pressure_rematerialization;

pub(crate) use fixed_view_copy::materialize_fixed_view_copies;
pub use fixed_view_copy::{
    FixedViewCopy, FixedViewCopyDestination, FixedViewCopyPlan, FixedViewCopyPolicy,
    FixedViewCopySourceEvidence, ValidatedFixedViewCopies, fixed_view_copy_identity,
};
pub(crate) use fixed_view_copy::{FixedViewCopyError, FixedViewCopyValidationReceipt};
pub(crate) use pressure_rematerialization::{
    FunctionPressureRematerialization, PressureRematerializationAction,
    PressureRematerializationPlan, PressureRematerializationRewrite,
    PressureRematerializationValidationReceipt, pressure_rematerialization_identity,
};
pub use pressure_rematerialization::{
    PressureRematerializationError, PressureRematerializationPolicy,
    ValidatedPressureRematerialization, rematerialize_selected_active_resident,
};
