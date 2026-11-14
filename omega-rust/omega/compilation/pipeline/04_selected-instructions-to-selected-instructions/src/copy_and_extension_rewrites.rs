/*
We implement direct-call copied-operand folding (`copied_call_operand`),
same-block register copy removal (`copy_removal`), and redundant carrier
extension elimination (`redundant_extension`) where the producer instruction
already normalizes the upper bits.
*/

#[allow(unused_imports)]
pub(crate) use crate::rewrites::*;

pub(crate) mod copied_call_operand {
    pub(crate) mod admission {
        use crate::rewrites::operand_probes::{effect_declaration, last_definition_before};
        use optimization_core::OptimizationWorkBudget;
        use register_environment::ValidatedTargetRegisterEnvironment;
        use register_model::RegisterOperandAccess;
        use selected_instructions::{
            MachineSemanticKind, PreparedMachineEffectCatalog, SelectedFunction,
            SelectedInstruction, SelectedInstructionId, SelectedInstructionKind, VirtualRegisterId,
        };

        use super::CopiedCallOperandError;
        use super::pair::{copied_call_operand_for, declared_consumer};
        use crate::ValidatedSelectedAnalysis;
        use crate::analyses::machine_effects::machine_semantic_kind;
        use crate::rewrites::block_edges::{
            block_instructions, successor_mentions, terminator_successors,
        };

        pub(crate) struct AdmittedPair<'source> {
            pub(crate) function: &'source SelectedFunction,
            pub(crate) block_index: usize,
            pub(crate) position: usize,
            pub(crate) source: VirtualRegisterId,
            pub(crate) destination: VirtualRegisterId,
        }

        pub(crate) fn admit<'source>(
            source: &'source impl ValidatedSelectedAnalysis,
            function_index: usize,
            producer: SelectedInstructionId,
            call: SelectedInstructionId,
            environment: &'source ValidatedTargetRegisterEnvironment,
            effect_catalog: &PreparedMachineEffectCatalog,
            budget: OptimizationWorkBudget,
        ) -> Result<AdmittedPair<'source>, CopiedCallOperandError> {
            let plan = source.selected_plan();
            if plan.target != environment.target() {
                return Err(CopiedCallOperandError::SourceMismatch);
            }
            let catalog = effect_catalog.catalog();
            if catalog.target != plan.target
                || catalog.register_constraints != environment.constraints().identity()
                || catalog.selected_keys != environment.selected_keys()
            {
                return Err(CopiedCallOperandError::EffectSurfaceMismatch);
            }
            let function =
                function_at(plan, function_index, CopiedCallOperandError::SourceMismatch)?;
            let (block_index, position, consumer) = function
                .blocks
                .iter()
                .enumerate()
                .find_map(|(block_index, block)| {
                    block
                        .instructions
                        .iter()
                        .enumerate()
                        .find(|(_, instruction)| instruction.id == call)
                        .map(|(position, instruction)| (block_index, position, instruction))
                })
                .ok_or(CopiedCallOperandError::UnsupportedConsumer)?;
            if !declared_consumer(consumer.kind) {
                return Err(CopiedCallOperandError::UnsupportedConsumer);
            }
            let consumer_kind = machine_semantic_kind(consumer.kind);
            let rule = copied_call_operand_for(MachineSemanticKind::CopyI64, consumer_kind)
                .ok_or(CopiedCallOperandError::UnsupportedConsumer)?;
            debug_assert_eq!(
                rule.rewritten(),
                consumer_kind,
                "every declared copied-call-operand pair rewrites to the consumer's own semantic"
            );
            if !rule.admits_consumer_operands(&consumer.operands) {
                return Err(CopiedCallOperandError::UnsupportedConsumer);
            }
            let find_register = |register| function.virtual_register(register);
            if consumer
                .operands
                .iter()
                .any(|operand| find_register(operand.virtual_register).is_none())
            {
                return Err(CopiedCallOperandError::UnsupportedUse);
            }
            let row = environment
                .constraint(consumer.constraint)
                .ok_or(CopiedCallOperandError::ConstraintMismatch)?;
            if row.operands.len() != consumer.operands.len()
                || !rule.admits_consumer_row(row)
                || row
                    .operands
                    .iter()
                    .zip(&consumer.operands)
                    .any(|(row_operand, operand)| {
                        row_operand.operand != operand.operand
                            || row_operand.access != operand.access
                            || row_operand.class != operand.class
                            || row_operand.fixed_view != operand.fixed_view
                            || row_operand.tied_to != operand.tied_to
                            || row_operand.early_clobber != operand.early_clobber
                    })
                || row.implicit_uses != consumer.implicit_uses
                || row.implicit_defs != consumer.implicit_defs
                || row.clobbers != consumer.clobbers
                || consumer.operands.iter().any(|operand| {
                    find_register(operand.virtual_register)
                        .is_some_and(|register| register.class != operand.class)
                })
            {
                return Err(CopiedCallOperandError::ConstraintMismatch);
            }
            let producer_index = function.blocks[block_index].instructions[..position]
                .iter()
                .position(|instruction| instruction.id == producer)
                .ok_or(CopiedCallOperandError::UnsupportedProducer)?;
            let producer_instruction = &function.blocks[block_index].instructions[producer_index];
            if !rule.matches_producer(producer_instruction.kind)
                || !rule.admits_producer_record(producer_instruction)
            {
                return Err(CopiedCallOperandError::UnsupportedProducer);
            }
            let SelectedInstructionKind::CopyI64 = producer_instruction.kind else {
                return Err(CopiedCallOperandError::UnsupportedProducer);
            };
            let [source_operand, destination_operand] = producer_instruction.operands.as_slice()
            else {
                return Err(CopiedCallOperandError::UnsupportedProducer);
            };
            use crate::rewrites::block_edges::function_at;
            if source_operand.operand != 0
                || source_operand.access != RegisterOperandAccess::Use
                || destination_operand.operand != 1
                || destination_operand.access != RegisterOperandAccess::Def
            {
                return Err(CopiedCallOperandError::UnsupportedProducer);
            }
            let copied_source = source_operand.virtual_register;
            let destination = destination_operand.virtual_register;
            if copied_source == destination {
                return Err(CopiedCallOperandError::UnsupportedProducer);
            }
            if last_definition_before(
                &function.blocks[block_index].instructions,
                position,
                destination,
            ) != Some(producer_index)
            {
                return Err(CopiedCallOperandError::UnsupportedProducer);
            }
            if !consumer.operands.iter().any(|operand| {
                operand.access == RegisterOperandAccess::Use
                    && operand.virtual_register == destination
            }) {
                return Err(CopiedCallOperandError::UnsupportedUse);
            }
            let source_register =
                find_register(copied_source).ok_or(CopiedCallOperandError::UnsupportedUse)?;
            let producer_row = environment
                .constraint(producer_instruction.constraint)
                .ok_or(CopiedCallOperandError::ConstraintMismatch)?;
            if !crate::rewrites::declares_pair(
                producer_row,
                RegisterOperandAccess::Use,
                RegisterOperandAccess::Def,
            ) || producer_row.operands[0].class != source_operand.class
                || producer_row.operands[0].class != source_register.class
                || producer_row.operands[1].class != destination_operand.class
                || producer_row.operands[0].class != producer_row.operands[1].class
                || !rule.admits_producer_row(producer_row)
            {
                return Err(CopiedCallOperandError::ConstraintMismatch);
            }
            if consumer.operands.iter().any(|operand| {
                operand.access == RegisterOperandAccess::Use
                    && operand.virtual_register == destination
                    && operand.class != source_register.class
            }) {
                return Err(CopiedCallOperandError::UnsupportedUse);
            }
            let mut source_views = std::collections::BTreeSet::new();
            for operand in consumer.operands.iter().filter(|operand| {
                operand.access == RegisterOperandAccess::Use
                    && operand.virtual_register == destination
            }) {
                source_views.extend(operand.fixed_view);
            }
            source_views.extend(source_register.entry_fixed_view);
            for block in &function.blocks {
                for instruction in block_instructions(block) {
                    source_views.extend(
                        instruction
                            .operands
                            .iter()
                            .filter(|operand| operand.virtual_register == copied_source)
                            .filter_map(|operand| operand.fixed_view),
                    );
                }
            }
            if source_views.len() > 1 {
                return Err(CopiedCallOperandError::UnsupportedUse);
            }
            for instruction in
                &function.blocks[block_index].instructions[producer_index + 1..position]
            {
                if instruction.operands.iter().any(|operand| {
                    operand.access != RegisterOperandAccess::Use
                        && operand.virtual_register == copied_source
                }) {
                    return Err(CopiedCallOperandError::UnsupportedUse);
                }
            }
            if pinned_view_written_by_call(environment, consumer, destination)
                && mentioned_after_call(function, block_index, position, copied_source)
            {
                return Err(CopiedCallOperandError::UnsupportedUse);
            }
            let producer_declaration = effect_declaration(
                effect_catalog,
                rule.producer(),
                producer_instruction.constraint,
            )
            .ok_or(CopiedCallOperandError::EffectSurfaceMismatch)?;
            let call_declaration =
                effect_declaration(effect_catalog, consumer_kind, consumer.constraint)
                    .ok_or(CopiedCallOperandError::EffectSurfaceMismatch)?;
            if !rule.admits_declarations(producer_declaration, call_declaration, row) {
                return Err(CopiedCallOperandError::EffectSurfaceMismatch);
            }
            let block = &function.blocks[block_index];
            let steps = crate::rewrites::rewrite_scan(plan, function)
                .and_then(|total| {
                    total
                        .checked_add(block.instructions.len())?
                        .checked_add(block.instructions.len())
                })
                .ok_or(CopiedCallOperandError::IdentityOverflow)?;
            crate::rewrites::charge_steps(
                steps,
                budget,
                || CopiedCallOperandError::IdentityOverflow,
                || CopiedCallOperandError::WorkBudgetExceeded,
            )?;
            Ok(AdmittedPair {
                function,
                block_index,
                position,
                source: copied_source,
                destination,
            })
        }

        fn pinned_view_written_by_call(
            environment: &ValidatedTargetRegisterEnvironment,
            call: &SelectedInstruction,
            destination: VirtualRegisterId,
        ) -> bool {
            let physical = environment.physical().model();
            let view_units = |view| {
                physical
                    .view(view)
                    .map(|view| view.units.iter().chain(&view.write_units))
                    .into_iter()
                    .flatten()
                    .copied()
            };
            let written: std::collections::BTreeSet<_> = call
                .clobbers
                .iter()
                .chain(&call.implicit_defs)
                .copied()
                .chain(
                    call.operands
                        .iter()
                        .filter(|operand| operand.access != RegisterOperandAccess::Use)
                        .filter_map(|operand| operand.fixed_view)
                        .flat_map(view_units),
                )
                .collect();
            call.operands
                .iter()
                .filter(|operand| {
                    operand.access == RegisterOperandAccess::Use
                        && operand.virtual_register == destination
                })
                .filter_map(|operand| operand.fixed_view)
                .flat_map(view_units)
                .any(|unit| written.contains(&unit))
        }

        fn mentioned_after_call(
            function: &SelectedFunction,
            block_index: usize,
            position: usize,
            register: VirtualRegisterId,
        ) -> bool {
            let operand_mentions = |instruction: &SelectedInstruction| {
                instruction
                    .operands
                    .iter()
                    .any(|operand| operand.virtual_register == register)
            };
            function.blocks.iter().enumerate().any(|(index, block)| {
                let skipped = if index == block_index {
                    position + 1
                } else {
                    0
                };
                block_instructions(block)
                    .skip(skipped)
                    .any(operand_mentions)
                    || terminator_successors(&block.terminator)
                        .into_iter()
                        .any(|successor| successor_mentions(successor, register))
            })
        }

        pub(crate) fn rewritten(admitted: &AdmittedPair<'_>) -> SelectedInstruction {
            let mut instruction = admitted.function.blocks[admitted.block_index].instructions
                [admitted.position]
                .clone();
            for operand in &mut instruction.operands {
                if operand.access == RegisterOperandAccess::Use
                    && operand.virtual_register == admitted.destination
                {
                    operand.virtual_register = admitted.source;
                }
            }
            instruction
        }
    }
    pub(crate) mod pair {
        use crate::analyses::machine_effects::machine_alternative_is_transparent;
        use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
        use selected_instructions::{
            MachineAlternative, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
            MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
            MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineMemoryEffect,
            MachineSemanticKind, MachineTrapBehavior, SelectedInstruction, SelectedInstructionKind,
            SelectedOperand,
        };

        use crate::analyses::machine_effects::machine_semantic_kind;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum CopiedCallOperandShape {
            CopiedUseOperand { results: CallResultRoster },
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum CallResultRoster {
            Empty,
            Scalar,
            Aggregate,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum CopiedCallOperandUnitFlow {
            CallSurfaceRetained,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum CopiedCallOperandEffects {
            CallLifecycleRetained,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct CopiedCallOperandRule {
            producer: MachineSemanticKind,
            consumer: MachineSemanticKind,
            operand_shape: CopiedCallOperandShape,
            unit_flow: CopiedCallOperandUnitFlow,
            effects: CopiedCallOperandEffects,
        }

        declared_kind_matchers!(CopiedCallOperandRule);

        impl CopiedCallOperandRule {
            const COPIED_CALL: Self = Self {
                producer: MachineSemanticKind::CopyI64,
                consumer: MachineSemanticKind::CallUnit,
                operand_shape: CopiedCallOperandShape::CopiedUseOperand {
                    results: CallResultRoster::Empty,
                },
                unit_flow: CopiedCallOperandUnitFlow::CallSurfaceRetained,
                effects: CopiedCallOperandEffects::CallLifecycleRetained,
            };

            pub(crate) const COPY_CALL_UNIT: Self = Self::COPIED_CALL;
            pub(crate) const COPY_CALL_SCALAR: Self = Self {
                consumer: MachineSemanticKind::CallScalar,
                operand_shape: CopiedCallOperandShape::CopiedUseOperand {
                    results: CallResultRoster::Scalar,
                },
                ..Self::COPIED_CALL
            };
            pub(crate) const COPY_CALL_AGGREGATE: Self = Self {
                consumer: MachineSemanticKind::CallAggregate,
                operand_shape: CopiedCallOperandShape::CopiedUseOperand {
                    results: CallResultRoster::Aggregate,
                },
                ..Self::COPIED_CALL
            };

            pub(crate) const fn producer(self) -> MachineSemanticKind {
                self.producer
            }
            pub(crate) const fn consumer(self) -> MachineSemanticKind {
                self.consumer
            }
            pub(crate) const fn rewritten(self) -> MachineSemanticKind {
                self.consumer
            }

            fn admits_roster_shape(
                &self,
                operands: impl Iterator<Item = (u16, RegisterOperandAccess, Option<u16>, bool)>,
            ) -> bool {
                let CopiedCallOperandShape::CopiedUseOperand { results } = self.operand_shape;
                let fields: Vec<(u16, RegisterOperandAccess, Option<u16>, bool)> =
                    operands.collect();
                let arity = fields
                    .iter()
                    .take_while(|(_, access, _, _)| *access == RegisterOperandAccess::Use)
                    .count();
                let (arguments, results_operands) = fields.split_at(arity);
                let roster = match results {
                    CallResultRoster::Empty => results_operands.is_empty(),
                    CallResultRoster::Scalar => results_operands.len() == 1,
                    CallResultRoster::Aggregate => {
                        !results_operands.is_empty()
                            && results_operands.len() <= 2
                            && !fields.is_empty()
                    }
                };
                roster
                    && results_operands
                        .iter()
                        .all(|(_, access, _, _)| *access == RegisterOperandAccess::Def)
                    && fields.iter().enumerate().all(|(position, field)| {
                        field.0 as usize == position && field.2.is_none() && !field.3
                    })
                    && !arguments
                        .iter()
                        .any(|(_, access, _, _)| *access != RegisterOperandAccess::Use)
            }

            pub(crate) fn admits_consumer_row(&self, row: &RegisterInstructionConstraint) -> bool {
                self.admits_roster_shape(row.operands.iter().map(|operand| {
                    (
                        operand.operand,
                        operand.access,
                        operand.tied_to,
                        operand.early_clobber,
                    )
                })) && row
                    .operands
                    .iter()
                    .all(|operand| operand.fixed_view.is_some())
            }

            pub(crate) fn admits_consumer_operands(&self, operands: &[SelectedOperand]) -> bool {
                self.admits_roster_shape(operands.iter().map(|operand| {
                    (
                        operand.operand,
                        operand.access,
                        operand.tied_to,
                        operand.early_clobber,
                    )
                }))
            }

            pub(crate) fn admits_producer_record(&self, instruction: &SelectedInstruction) -> bool {
                match self.unit_flow {
                    CopiedCallOperandUnitFlow::CallSurfaceRetained => {
                        instruction.implicit_uses.is_empty()
                            && instruction.implicit_defs.is_empty()
                            && instruction.clobbers.is_empty()
                            && instruction.operands.iter().all(|operand| {
                                operand.fixed_view.is_none()
                                    && operand.tied_to.is_none()
                                    && !operand.early_clobber
                            })
                    }
                }
            }

            pub(crate) fn admits_producer_row(&self, row: &RegisterInstructionConstraint) -> bool {
                match self.unit_flow {
                    CopiedCallOperandUnitFlow::CallSurfaceRetained => {
                        row.implicit_uses.is_empty()
                            && row.implicit_defs.is_empty()
                            && row.clobbers.is_empty()
                    }
                }
            }

            pub(crate) fn admits_declarations(
                &self,
                producer: &MachineEffectDeclaration,
                call: &MachineEffectDeclaration,
                row: &RegisterInstructionConstraint,
            ) -> bool {
                match self.effects {
                    CopiedCallOperandEffects::CallLifecycleRetained => {
                        isolated_copy(producer) && retained_call(call, row)
                    }
                }
            }
        }

        pub(crate) const COPIED_CALL_OPERAND_RULES: &[CopiedCallOperandRule] = &[
            CopiedCallOperandRule::COPY_CALL_UNIT,
            CopiedCallOperandRule::COPY_CALL_SCALAR,
            CopiedCallOperandRule::COPY_CALL_AGGREGATE,
        ];

        declared_pair_lookup!(
            copied_call_operand_for,
            COPIED_CALL_OPERAND_RULES,
            CopiedCallOperandRule
        );

        pub(crate) fn declared_consumer(kind: SelectedInstructionKind) -> bool {
            COPIED_CALL_OPERAND_RULES
                .iter()
                .any(|rule| rule.matches_consumer(kind))
        }

        fn isolated_copy(declaration: &MachineEffectDeclaration) -> bool {
            declaration.memory == MachineMemoryEffect::NoneV1
                && declaration.trap == MachineTrapBehavior::NeverV1
                && declaration.barrier == MachineBarrier::None
                && declaration.call == MachineCallEffect::NoneV1
                && declaration.cleanup == MachineCleanupEffect::NoneV1
                && declaration
                    .alternatives
                    .iter()
                    .all(machine_alternative_is_transparent)
        }

        fn retained_call(
            declaration: &MachineEffectDeclaration,
            row: &RegisterInstructionConstraint,
        ) -> bool {
            let reads: Vec<u16> = row
                .operands
                .iter()
                .filter(|operand| operand.access == RegisterOperandAccess::Use)
                .map(|operand| operand.operand)
                .collect();
            let writes: Vec<u16> = row
                .operands
                .iter()
                .filter(|operand| operand.access == RegisterOperandAccess::Def)
                .map(|operand| operand.operand)
                .collect();
            declaration.memory == MachineMemoryEffect::NoneV1
                && declaration.trap == MachineTrapBehavior::NeverV1
                && declaration.barrier == MachineBarrier::Call
                && matches!(
                    declaration.call,
                    MachineCallEffect::InternalNormalReturnV1 { .. }
                )
                && declaration.cleanup == MachineCleanupEffect::NoneV1
                && declaration
                    .alternatives
                    .iter()
                    .all(|alternative| call_alternative(alternative, &reads, &writes, row))
        }

        fn call_alternative(
            alternative: &MachineAlternative,
            reads: &[u16],
            writes: &[u16],
            row: &RegisterInstructionConstraint,
        ) -> bool {
            let encoded = &alternative.encoded;
            let lifecycle = match (encoded.memory, encoded.stack) {
                (
                    MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                        stack_pointer: memory_pointer,
                        byte_count,
                    },
                    MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                        stack_pointer,
                        return_address_byte_count,
                    },
                ) => {
                    memory_pointer == stack_pointer
                        && byte_count == return_address_byte_count
                        && return_address_byte_count != 0
                }
                (MachineEncodedMemoryEffect::NoneV1, MachineEncodedStackEffect::UnchangedV1) => {
                    true
                }
                _ => false,
            };
            lifecycle
                && encoded.external_operand_reads == reads
                && encoded.external_operand_writes == writes
                && encoded.implicit_unit_uses == row.implicit_uses
                && encoded.implicit_unit_defs == row.implicit_defs
                && encoded.implicit_unit_clobbers == row.clobbers
                && encoded.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                && encoded.control == MachineEncodedControlEffect::DirectRelativeCallV1
        }
    }
    pub(crate) mod rewrite {
        use optimization_core::OptimizationWorkBudget;
        use register_environment::ValidatedTargetRegisterEnvironment;
        use selected_instructions::{PreparedMachineEffectCatalog, SelectedInstructionId};

        use super::{CopiedCallOperandError, ValidatedCopiedCallOperand, admission};
        use crate::ValidatedSelectedAnalysis;

        pub(crate) fn fold_selected_copied_call_operand(
            source: &impl ValidatedSelectedAnalysis,
            function_index: usize,
            producer: SelectedInstructionId,
            call: SelectedInstructionId,
            environment: &ValidatedTargetRegisterEnvironment,
            effect_catalog: &PreparedMachineEffectCatalog,
            budget: OptimizationWorkBudget,
        ) -> Result<ValidatedCopiedCallOperand, CopiedCallOperandError> {
            let admitted = admission::admit(
                source,
                function_index,
                producer,
                call,
                environment,
                effect_catalog,
                budget,
            )?;
            let mut transformed = source.selected_plan().clone();
            transformed.functions[function_index].blocks[admitted.block_index].instructions
                [admitted.position] = admission::rewritten(&admitted);
            super::validation::seal_copied_call_operand_fold(
                source,
                function_index,
                producer,
                call,
                transformed,
            )
        }
    }
    pub(crate) mod validation {
        use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};

        use super::{CopiedCallOperandError, CopiedCallOperandReceipt, ValidatedCopiedCallOperand};
        use crate::ValidatedSelectedAnalysis;
        use crate::rewrites::sealed;

        pub(crate) fn copied_call_operand_measured_steps(
            plan: &SelectedInstructionPlan,
            function: &selected_instructions::SelectedFunction,
            block: &selected_instructions::SelectedBlock,
        ) -> Result<u64, CopiedCallOperandError> {
            let steps = crate::rewrites::rewrite_scan(plan, function)
                .and_then(|total| {
                    total
                        .checked_add(block.instructions.len())?
                        .checked_add(block.instructions.len())
                })
                .ok_or(CopiedCallOperandError::IdentityOverflow)?;
            u64::try_from(steps).map_err(|_| CopiedCallOperandError::IdentityOverflow)
        }

        pub(crate) fn seal_copied_call_operand_fold(
            source: &impl ValidatedSelectedAnalysis,
            function_index: usize,
            producer: SelectedInstructionId,
            call: SelectedInstructionId,
            proposed: SelectedInstructionPlan,
        ) -> Result<ValidatedCopiedCallOperand, CopiedCallOperandError> {
            Ok(sealed!(
                ValidatedCopiedCallOperand,
                CopiedCallOperandReceipt,
                source,
                proposed,
                function_index: function_index,
                producer: producer,
                call: call
            ))
        }
    }

    use std::sync::Arc;

    use optimization_core::OptimizationUnitIdentity;
    use selected_instructions::{
        CopiedCallOperandIdentity, SelectedInstructionId, SelectedInstructionPlan,
        SelectedInstructionPlanIdentity,
    };
    use semantic_vocabulary::WorkScheduleIdentity;

    pub(crate) use rewrite::fold_selected_copied_call_operand;
    pub(crate) use validation::copied_call_operand_measured_steps;

    rewrite_receipt_identity!(
        copied_call_operand_identity,
        CopiedCallOperandIdentity,
        CopiedCallOperandReceipt,
        b"omega.terminal-copied-call-operand.v1\0",
        "copied-call-operand",
        producer,
        call,
    );

    validated_transformed_receipt!(ValidatedCopiedCallOperand, CopiedCallOperandReceipt);

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CopiedCallOperandReceipt {
        source_selected: SelectedInstructionPlanIdentity,
        transformed_selected: SelectedInstructionPlanIdentity,
        function_index: usize,
        producer: SelectedInstructionId,
        call: SelectedInstructionId,
        optimization_unit: OptimizationUnitIdentity,
        work_schedule: WorkScheduleIdentity,
    }

    impl CopiedCallOperandReceipt {
        receipt_accessor_rows!(ref CopiedCallOperandReceipt {
            function_index: usize,
            producer: SelectedInstructionId,
            call: SelectedInstructionId,
        });
    }

    receipt_selected_identities!(CopiedCallOperandReceipt);

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum CopiedCallOperandError {
        SourceMismatch,
        UnsupportedConsumer,
        UnsupportedProducer,
        UnsupportedUse,
        ConstraintMismatch,
        EffectSurfaceMismatch,
        WorkBudgetExceeded,
        IdentityOverflow,
        ReplayMismatch,
    }

    failed_display!(CopiedCallOperandError, "invalid copied call operand fold");
}

pub(crate) mod copy_removal {
    pub(crate) mod admission {
        use optimization_core::OptimizationWorkBudget;
        use register_environment::ValidatedTargetRegisterEnvironment;
        use register_model::RegisterOperandAccess;
        use selected_instructions::{
            FrameStorageSlotId, LocalStorageSlotId, SelectedBlock, SelectedBlockId,
            SelectedCasePayloadTransport, SelectedFunction, SelectedInstruction,
            SelectedInstructionId, SelectedInstructionKind, SelectedMemoryAccessRole,
            SelectedStructuralTransport, SelectedSuccessor, SelectedTerminator,
            SelectedValueTransport, VirtualRegisterId, VirtualRegisterOrigin,
            terminator_instruction_mut,
        };

        use super::CopyRemovalError;
        use crate::ValidatedSelectedAnalysis;
        use crate::rewrites::block_edges::{
            block_instructions, local_slot_mentions, member_position, successor_mentions,
            terminator_successors,
        };

        pub(crate) struct Admission<'source> {
            pub function: &'source SelectedFunction,
            pub block_index: usize,
            pub block: SelectedBlockId,
            pub copy_index: usize,
            pub input: VirtualRegisterId,
            pub register_index: usize,
            pub uses: Vec<CopyUse>,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct CopyUse {
            pub position: usize,
            pub operand: usize,
        }

        fn block_instruction_mut(
            block: &mut SelectedBlock,
            position: usize,
        ) -> Option<&mut SelectedInstruction> {
            if position == block.instructions.len() {
                Some(terminator_instruction_mut(&mut block.terminator))
            } else {
                block.instructions.get_mut(position)
            }
        }

        fn frame_slot_mentions(slot: FrameStorageSlotId, register: VirtualRegisterId) -> bool {
            match slot {
                FrameStorageSlotId::Local(slot) => local_slot_mentions(slot, register),
                _ => false,
            }
        }

        fn instruction_slot_mentions(
            instruction: &SelectedInstruction,
            register: VirtualRegisterId,
        ) -> bool {
            match instruction.kind {
                SelectedInstructionKind::Store64 { slot, .. }
                | SelectedInstructionKind::FrameAddress { slot, .. } => {
                    frame_slot_mentions(slot, register)
                }
                selected_instructions::hosted_slot_quad_pattern!(slot, ..) => {
                    local_slot_mentions(slot, register)
                }
                _ => false,
            }
        }

        fn origin_mentions(
            origin: VirtualRegisterOrigin,
            instruction: SelectedInstructionId,
            register: VirtualRegisterId,
        ) -> bool {
            use VirtualRegisterOrigin::*;
            match origin {
                InstructionScratch {
                    instruction: id, ..
                }
                | StructuralObservation {
                    instruction: id, ..
                }
                | ScalarAbiAddress {
                    instruction: id, ..
                }
                | AbiTransport {
                    instruction: id, ..
                }
                | InstructionResult {
                    instruction: id, ..
                } => id == instruction,
                SpillAddress {
                    instruction: id,
                    register: subject,
                } => id == instruction || subject == register,
                StructuralParameter { .. } | EntryParameter { .. } | BlockParameter { .. } => false,
            }
        }

        pub(crate) fn admit<'source>(
            source: &'source impl ValidatedSelectedAnalysis,
            function_index: usize,
            copy: SelectedInstructionId,
            environment: &'source ValidatedTargetRegisterEnvironment,
            budget: OptimizationWorkBudget,
        ) -> Result<Admission<'source>, CopyRemovalError> {
            let (plan, function) =
                crate::rewrites::plan_function(source, function_index, environment, || {
                    CopyRemovalError::SourceMismatch
                })?;
            let (block_index, copy_index) =
                member_position(function, copy).ok_or(CopyRemovalError::SourceMismatch)?;
            let block = &function.blocks[block_index];
            let copy_instruction = &block.instructions[copy_index];
            if copy_instruction.kind != SelectedInstructionKind::CopyI64 {
                return Err(CopyRemovalError::UnsupportedInstruction);
            }
            let Some((input_operand, output_operand)) =
                crate::rewrites::carrier_pair(copy_instruction, RegisterOperandAccess::Def)
            else {
                return Err(CopyRemovalError::UnsupportedInstruction);
            };
            let input = input_operand.virtual_register;
            let output = output_operand.virtual_register;
            if input == output {
                return Err(CopyRemovalError::UnsupportedRegister);
            }
            let register_index = function
                .virtual_registers
                .iter()
                .position(|register| register.id == output)
                .ok_or(CopyRemovalError::UnsupportedRegister)?;
            let output_register = &function.virtual_registers[register_index];
            let input_register = function
                .virtual_register(input)
                .ok_or(CopyRemovalError::UnsupportedRegister)?;
            let copy_defined = match output_register.origin {
                VirtualRegisterOrigin::InstructionResult { instruction, .. } => instruction == copy,
                VirtualRegisterOrigin::InstructionScratch {
                    instruction,
                    operand,
                } => instruction == copy && operand == 1,
                _ => false,
            };
            if !copy_defined
                || output_register.entry_fixed_view.is_some()
                || output_register.scalar_type != input_register.scalar_type
                || output_register.class != input_register.class
            {
                return Err(CopyRemovalError::UnsupportedRegister);
            }
            let row = environment
                .constraint(copy_instruction.constraint)
                .ok_or(CopyRemovalError::ConstraintMismatch)?;
            if !crate::rewrites::declares_pair(
                row,
                RegisterOperandAccess::Use,
                RegisterOperandAccess::Def,
            ) || row.operands[0].class != input_register.class
                || row.operands[1].class != output_register.class
            {
                return Err(CopyRemovalError::ConstraintMismatch);
            }
            if function.call(copy).is_some() || function.memory_access(copy).is_some() {
                return Err(CopyRemovalError::UnsupportedInstruction);
            }
            if function
                .local_storage_slots
                .iter()
                .any(|slot| local_slot_mentions(slot.id, output))
                || function
                    .memory_accesses
                    .iter()
                    .any(|access| match access.role {
                        SelectedMemoryAccessRole::ReadLocal { slot }
                        | SelectedMemoryAccessRole::WriteLocal { slot }
                        | SelectedMemoryAccessRole::AddressLocal { slot } => {
                            local_slot_mentions(slot, output)
                        }
                        _ => false,
                    })
                || function
                    .virtual_registers
                    .iter()
                    .enumerate()
                    .any(|(index, register)| {
                        index != register_index && origin_mentions(register.origin, copy, output)
                    })
            {
                return Err(CopyRemovalError::UnsupportedUse);
            }
            let mut uses = Vec::new();
            for (current_index, current) in function.blocks.iter().enumerate() {
                for (position, instruction) in block_instructions(current).enumerate() {
                    if instruction_slot_mentions(instruction, output) {
                        return Err(CopyRemovalError::UnsupportedUse);
                    }
                    for (operand_index, operand) in instruction.operands.iter().enumerate() {
                        if operand.virtual_register != output {
                            continue;
                        }
                        if instruction.id == copy {
                            continue;
                        }
                        if current_index != block_index
                            || position <= copy_index
                            || operand.access != RegisterOperandAccess::Use
                            || operand.fixed_view.is_some()
                            || operand.tied_to.is_some()
                            || operand.early_clobber
                            || operand.class != input_register.class
                        {
                            return Err(CopyRemovalError::UnsupportedUse);
                        }
                        uses.push(CopyUse {
                            position,
                            operand: operand_index,
                        });
                    }
                }
                if terminator_successors(&current.terminator)
                    .into_iter()
                    .any(|successor| successor_mentions(successor, output))
                {
                    return Err(CopyRemovalError::UnsupportedUse);
                }
            }
            if uses.is_empty() {
                return Err(CopyRemovalError::UnsupportedUse);
            }
            let last = uses.last().map(|site| site.position).unwrap_or(0);
            for (position, instruction) in block_instructions(block).enumerate() {
                if position <= copy_index {
                    continue;
                }
                if position >= last {
                    break;
                }
                if instruction.operands.iter().any(|operand| {
                    operand.access != RegisterOperandAccess::Use
                        && operand.virtual_register == input
                }) {
                    return Err(CopyRemovalError::UnsupportedUse);
                }
            }
            let steps =
                super::copy_scan(plan, function).ok_or(CopyRemovalError::IdentityOverflow)?;
            crate::rewrites::charge_steps(
                steps,
                budget,
                || CopyRemovalError::IdentityOverflow,
                || CopyRemovalError::WorkBudgetExceeded,
            )?;
            Ok(Admission {
                function,
                block_index,
                block: block.id,
                copy_index,
                input,
                register_index,
                uses,
            })
        }

        pub(crate) fn apply(
            admitted: &Admission<'_>,
            function: &mut SelectedFunction,
        ) -> Result<(), CopyRemovalError> {
            let block = function
                .blocks
                .get_mut(admitted.block_index)
                .ok_or(CopyRemovalError::SourceMismatch)?;
            if block.id != admitted.block {
                return Err(CopyRemovalError::SourceMismatch);
            }
            let removed_register = function
                .virtual_registers
                .get(admitted.register_index)
                .map(|register| register.id)
                .ok_or(CopyRemovalError::SourceMismatch)?;
            for site in &admitted.uses {
                block_instruction_mut(block, site.position)
                    .and_then(|instruction| instruction.operands.get_mut(site.operand))
                    .ok_or(CopyRemovalError::SourceMismatch)?
                    .virtual_register = admitted.input;
            }
            function.boundary_settlements = super::super::shifted_boundary_settlements(
                admitted.function,
                admitted.block,
                admitted.copy_index,
                || CopyRemovalError::SourceMismatch,
                || CopyRemovalError::IdentityOverflow,
            )?;
            block.instructions.remove(admitted.copy_index);
            function.virtual_registers.remove(admitted.register_index);
            redensify(function, removed_register)
        }

        fn redensify(
            function: &mut SelectedFunction,
            removed_register: VirtualRegisterId,
        ) -> Result<(), CopyRemovalError> {
            for access in &mut function.memory_accesses {
                match &mut access.role {
                    SelectedMemoryAccessRole::ReadLocal { slot }
                    | SelectedMemoryAccessRole::WriteLocal { slot }
                    | SelectedMemoryAccessRole::AddressLocal { slot } => {
                        lower_local_slot(slot, removed_register)?;
                    }
                    _ => {}
                }
            }
            for slot in &mut function.local_storage_slots {
                lower_local_slot(&mut slot.id, removed_register)?;
            }
            for register in &mut function.virtual_registers {
                register.id = lower_register(register.id, removed_register)?;
                if let VirtualRegisterOrigin::SpillAddress {
                    register: subject, ..
                } = &mut register.origin
                {
                    *subject = lower_register(*subject, removed_register)?;
                }
            }
            for block in &mut function.blocks {
                for instruction in block_instructions_mut(block) {
                    lower_selected_instruction(instruction, removed_register)?;
                }
                for successor in terminator_successors_mut(&mut block.terminator) {
                    lower_successor(successor, removed_register)?;
                }
            }
            Ok(())
        }

        fn block_instructions_mut(
            block: &mut SelectedBlock,
        ) -> impl Iterator<Item = &mut SelectedInstruction> {
            block
                .instructions
                .iter_mut()
                .chain(std::iter::once(terminator_instruction_mut(
                    &mut block.terminator,
                )))
        }

        fn terminator_successors_mut(
            terminator: &mut SelectedTerminator,
        ) -> Vec<&mut SelectedSuccessor> {
            match terminator {
                SelectedTerminator::Jump { successor, .. } => vec![successor],
                SelectedTerminator::ConditionalBranch {
                    when_nonzero,
                    when_zero,
                    ..
                } => vec![when_nonzero, when_zero],
                selected_instructions::conditional_branch_less_pair_pattern!(
                    when_less,
                    when_not_less,
                    ..
                ) => vec![when_less, when_not_less],
                selected_instructions::terminator_no_successor_pattern!(..) => Vec::new(),
            }
        }

        fn lower_selected_instruction(
            instruction: &mut SelectedInstruction,
            removed_register: VirtualRegisterId,
        ) -> Result<(), CopyRemovalError> {
            for operand in &mut instruction.operands {
                operand.virtual_register =
                    lower_register(operand.virtual_register, removed_register)?;
            }
            match &mut instruction.kind {
                SelectedInstructionKind::Store64 { slot, .. }
                | SelectedInstructionKind::FrameAddress { slot, .. } => {
                    lower_frame_slot(slot, removed_register)
                }
                selected_instructions::hosted_slot_quad_pattern!(slot, ..) => {
                    lower_local_slot(slot, removed_register)
                }
                _ => Ok(()),
            }
        }

        fn lower_successor(
            successor: &mut SelectedSuccessor,
            removed_register: VirtualRegisterId,
        ) -> Result<(), CopyRemovalError> {
            for binding in &mut successor.bindings {
                if let SelectedValueTransport::Registers {
                    argument,
                    parameter,
                } = &mut binding.transport
                {
                    *argument = lower_register(*argument, removed_register)?;
                    *parameter = lower_register(*parameter, removed_register)?;
                }
            }
            for binding in &mut successor.structural_bindings {
                match &mut binding.transport {
                    SelectedStructuralTransport::WholeValue {
                        argument,
                        destination,
                        ..
                    } => {
                        *argument = lower_register(*argument, removed_register)?;
                        lower_local_slot(destination, removed_register)?;
                    }
                    SelectedStructuralTransport::ScalarReference {
                        base, destination, ..
                    }
                    | SelectedStructuralTransport::Address {
                        base, destination, ..
                    } => {
                        match base {
                            selected_instructions::SelectedAddressBase::Register(argument) => {
                                *argument = lower_register(*argument, removed_register)?;
                            }
                            selected_instructions::SelectedAddressBase::Local(slot) => {
                                lower_local_slot(slot, removed_register)?;
                            }
                        }
                        lower_local_slot(destination, removed_register)?;
                    }
                    SelectedStructuralTransport::Unused => {}
                }
            }
            if let Some(case) = &mut successor.structural_case {
                match &mut case.source {
                    selected_instructions::SelectedCaseDispatchSource::Local { slot } => {
                        lower_local_slot(slot, removed_register)?;
                    }
                    selected_instructions::SelectedCaseDispatchSource::Borrowed { .. } => {}
                }
                for payload in &mut case.payloads {
                    match &mut payload.transport {
                        SelectedCasePayloadTransport::Unmaterialized { parameter } => {
                            *parameter = lower_register(*parameter, removed_register)?;
                        }
                        SelectedCasePayloadTransport::Registers {
                            argument,
                            parameter,
                        } => {
                            *argument = lower_register(*argument, removed_register)?;
                            *parameter = lower_register(*parameter, removed_register)?;
                        }
                        SelectedCasePayloadTransport::Unused => {}
                    }
                }
            }
            Ok(())
        }

        fn lower_local_slot(
            slot: &mut LocalStorageSlotId,
            removed_register: VirtualRegisterId,
        ) -> Result<(), CopyRemovalError> {
            if let LocalStorageSlotId::Spill { register } = slot {
                *register = lower_register(*register, removed_register)?;
            }
            Ok(())
        }

        fn lower_frame_slot(
            slot: &mut FrameStorageSlotId,
            removed_register: VirtualRegisterId,
        ) -> Result<(), CopyRemovalError> {
            if let FrameStorageSlotId::Local(slot) = slot {
                lower_local_slot(slot, removed_register)?;
            }
            Ok(())
        }

        fn lower_register(
            id: VirtualRegisterId,
            removed: VirtualRegisterId,
        ) -> Result<VirtualRegisterId, CopyRemovalError> {
            if id == removed {
                return Err(CopyRemovalError::SourceMismatch);
            }
            Ok(VirtualRegisterId(if id.0 > removed.0 {
                id.0.checked_sub(1)
                    .ok_or(CopyRemovalError::IdentityOverflow)?
            } else {
                id.0
            }))
        }
    }
    pub(crate) mod rewrite {
        use optimization_core::OptimizationWorkBudget;
        use register_environment::ValidatedTargetRegisterEnvironment;
        use selected_instructions::SelectedInstructionId;

        use super::{CopyRemovalError, ValidatedCopyRemoval, admission};
        use crate::ValidatedSelectedAnalysis;

        pub(crate) fn remove_selected_copy(
            source: &impl ValidatedSelectedAnalysis,
            function_index: usize,
            copy: SelectedInstructionId,
            environment: &ValidatedTargetRegisterEnvironment,
            budget: OptimizationWorkBudget,
        ) -> Result<ValidatedCopyRemoval, CopyRemovalError> {
            let admitted = admission::admit(source, function_index, copy, environment, budget)?;
            let mut transformed = source.selected_plan().clone();
            admission::apply(&admitted, &mut transformed.functions[function_index])?;
            super::validation::seal_copy_removal(source, function_index, copy, transformed)
        }
    }
    pub(crate) mod validation {
        use selected_instructions::{
            SelectedFunction, SelectedInstructionId, SelectedInstructionPlan,
        };

        use super::{CopyRemovalError, CopyRemovalReceipt, ValidatedCopyRemoval};
        use crate::ValidatedSelectedAnalysis;
        use crate::rewrites::sealed;

        pub(crate) fn measured_steps(
            plan: &SelectedInstructionPlan,
            function: &SelectedFunction,
        ) -> Result<u64, CopyRemovalError> {
            let steps =
                super::copy_scan(plan, function).ok_or(CopyRemovalError::IdentityOverflow)?;
            u64::try_from(steps).map_err(|_| CopyRemovalError::IdentityOverflow)
        }

        pub(crate) fn seal_copy_removal(
            source: &impl ValidatedSelectedAnalysis,
            function_index: usize,
            copy: SelectedInstructionId,
            proposed: SelectedInstructionPlan,
        ) -> Result<ValidatedCopyRemoval, CopyRemovalError> {
            Ok(
                sealed!(ValidatedCopyRemoval, CopyRemovalReceipt, source, proposed, function_index: function_index, copy: copy),
            )
        }
    }

    use std::sync::Arc;

    use optimization_core::OptimizationUnitIdentity;
    use selected_instructions::{
        CopyRemovalIdentity, SelectedFunction, SelectedInstructionId, SelectedInstructionPlan,
        SelectedInstructionPlanIdentity,
    };
    use semantic_vocabulary::WorkScheduleIdentity;

    pub(crate) use rewrite::remove_selected_copy;
    pub(crate) use validation::measured_steps;

    validated_transformed_receipt!(ValidatedCopyRemoval, CopyRemovalReceipt);

    rewrite_receipt!(CopyRemovalReceipt {
        function_index: usize,
        copy: SelectedInstructionId,
    });

    rewrite_receipt_identity!(
        copy_removal_identity,
        CopyRemovalIdentity,
        CopyRemovalReceipt,
        b"omega.terminal-copy-removal.v1\0",
        "copy-removal",
        copy,
    );

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum CopyRemovalError {
        SourceMismatch,
        UnsupportedInstruction,
        UnsupportedRegister,
        UnsupportedUse,
        ConstraintMismatch,
        WorkBudgetExceeded,
        IdentityOverflow,
        ReplayMismatch,
    }

    failed_display!(CopyRemovalError, "invalid copy removal");

    pub(crate) fn copy_scan(
        plan: &SelectedInstructionPlan,
        function: &SelectedFunction,
    ) -> Option<usize> {
        crate::rewrites::rewrite_scan(plan, function)
            .and_then(|total| {
                function.blocks.iter().try_fold(total, |total, block| {
                    total
                        .checked_add(block.instructions.len())?
                        .checked_add(1)?
                        .checked_add(
                            crate::rewrites::block_edges::terminator_successors(&block.terminator)
                                .into_iter()
                                .map(|successor| {
                                    successor.bindings.len()
                                        + successor.structural_bindings.len()
                                        + successor
                                            .structural_case
                                            .as_ref()
                                            .map_or(0, |case| case.payloads.len() + 1)
                                })
                                .sum::<usize>(),
                        )
                })
            })
            .and_then(|total| {
                total
                    .checked_add(function.virtual_registers.len())?
                    .checked_add(function.local_storage_slots.len())?
                    .checked_add(function.memory_accesses.len())?
                    .checked_add(function.calls.len())
            })
    }
}

pub(crate) mod redundant_extension {
    pub(crate) mod admission {
        use optimization_core::OptimizationWorkBudget;
        use register_environment::ValidatedTargetRegisterEnvironment;
        use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
        use selected_instructions::{
            SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
            SelectedInstructionProvenance, VirtualRegisterId, VirtualRegisterOrigin,
        };
        use semantic_vocabulary::IntegerValue;

        use super::RedundantExtensionError;
        use crate::ValidatedSelectedAnalysis;

        pub(crate) struct Admission<'source> {
            pub block_index: usize,
            pub extension_index: usize,
            pub extension_id: SelectedInstructionId,
            pub provenance: SelectedInstructionProvenance,
            pub value: VirtualRegisterId,
            pub output: VirtualRegisterId,
            pub copy: &'source RegisterInstructionConstraint,
        }

        use crate::rewrites::block_edges::member_position;
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum Extension {
            Zero { width: u8 },
            Sign { width: u8 },
        }

        fn extension(kind: SelectedInstructionKind) -> Option<Extension> {
            Some(match kind {
                SelectedInstructionKind::ZeroExtendU8 => Extension::Zero { width: 8 },
                SelectedInstructionKind::ZeroExtendU16 => Extension::Zero { width: 16 },
                SelectedInstructionKind::ZeroExtendU32 => Extension::Zero { width: 32 },
                SelectedInstructionKind::SignExtendI8 => Extension::Sign { width: 8 },
                SelectedInstructionKind::SignExtendI16 => Extension::Sign { width: 16 },
                SelectedInstructionKind::SignExtendI32 => Extension::Sign { width: 32 },
                _ => return None,
            })
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum Normalization {
            ZeroExtended { width: u8 },
            SignExtended { width: u8 },
            MeaningfulLow { width: u8 },
            Pattern(u64),
        }

        fn producer_normalization(
            kind: SelectedInstructionKind,
            defined_operand: u16,
        ) -> Option<Normalization> {
            Some(match kind {
                SelectedInstructionKind::Load8 { .. } | SelectedInstructionKind::Load8Indexed => {
                    Normalization::ZeroExtended { width: 8 }
                }
                SelectedInstructionKind::Load16 { .. } => Normalization::ZeroExtended { width: 16 },
                SelectedInstructionKind::Load32 { .. } => Normalization::ZeroExtended { width: 32 },
                SelectedInstructionKind::LoadPacked { width, .. } if defined_operand == 1 => {
                    Normalization::MeaningfulLow {
                        width: width.byte_size() * 8,
                    }
                }
                SelectedInstructionKind::ZeroExtendU8 => Normalization::ZeroExtended { width: 8 },
                SelectedInstructionKind::ZeroExtendU16 => Normalization::ZeroExtended { width: 16 },
                SelectedInstructionKind::ZeroExtendU32 => {
                    Normalization::MeaningfulLow { width: 32 }
                }
                SelectedInstructionKind::SignExtendI8 => Normalization::SignExtended { width: 8 },
                SelectedInstructionKind::SignExtendI16 => Normalization::SignExtended { width: 16 },
                SelectedInstructionKind::SignExtendI32 => Normalization::SignExtended { width: 32 },
                SelectedInstructionKind::Float32ToBits => {
                    Normalization::MeaningfulLow { width: 32 }
                }
                SelectedInstructionKind::MaterializeBooleanEqual
                | SelectedInstructionKind::MaterializeBooleanU64LessThan
                | SelectedInstructionKind::MaterializeBooleanI64LessThan
                | SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
                | SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => {
                    Normalization::ZeroExtended { width: 1 }
                }
                SelectedInstructionKind::MaterializeI64 { value } => {
                    let pattern = match value {
                        IntegerValue::Signed(value) => value as i64 as u64,
                        IntegerValue::Unsigned(value) => value as u64,
                    };
                    Normalization::Pattern(pattern)
                }
                _ => return None,
            })
        }

        fn identity(extension: Extension, normalization: Normalization) -> bool {
            match (extension, normalization) {
                (Extension::Zero { width }, Normalization::ZeroExtended { width: n }) => n <= width,
                (Extension::Zero { width: 32 }, Normalization::MeaningfulLow { width: n }) => {
                    n <= 32
                }
                (Extension::Zero { .. }, Normalization::MeaningfulLow { .. }) => false,
                (Extension::Zero { width }, Normalization::Pattern(pattern)) => {
                    pattern < (1u64 << width)
                }
                (Extension::Zero { .. }, Normalization::SignExtended { .. }) => false,
                (Extension::Sign { width }, Normalization::SignExtended { width: n }) => n <= width,
                (Extension::Sign { width }, Normalization::ZeroExtended { width: n }) => n < width,
                (Extension::Sign { width }, Normalization::Pattern(pattern)) => {
                    let shift = 64 - u32::from(width);
                    (((pattern << shift) as i64) >> shift) as u64 == pattern
                }
                (Extension::Sign { .. }, Normalization::MeaningfulLow { .. }) => false,
            }
        }

        pub(crate) fn admit<'source>(
            source: &'source impl ValidatedSelectedAnalysis,
            function_index: usize,
            extension_id: SelectedInstructionId,
            environment: &'source ValidatedTargetRegisterEnvironment,
            budget: OptimizationWorkBudget,
        ) -> Result<Admission<'source>, RedundantExtensionError> {
            let (plan, function) =
                crate::rewrites::plan_function(source, function_index, environment, || {
                    RedundantExtensionError::SourceMismatch
                })?;
            let (block_index, extension_index) = member_position(function, extension_id)
                .ok_or(RedundantExtensionError::SourceMismatch)?;
            let extension_instruction = &function.blocks[block_index].instructions[extension_index];
            let form = extension(extension_instruction.kind)
                .ok_or(RedundantExtensionError::UnsupportedInstruction)?;
            let Some((use_operand, def_operand)) =
                crate::rewrites::carrier_pair(extension_instruction, RegisterOperandAccess::Def)
            else {
                return Err(RedundantExtensionError::UnsupportedInstruction);
            };
            let value = use_operand.virtual_register;
            let output = def_operand.virtual_register;
            if value == output {
                return Err(RedundantExtensionError::UnsupportedUse);
            }
            let find_register = |register| function.virtual_register(register);
            let value_register =
                find_register(value).ok_or(RedundantExtensionError::UnsupportedProducer)?;
            let output_register =
                find_register(output).ok_or(RedundantExtensionError::UnsupportedUse)?;
            let defines_here = |instruction: SelectedInstructionId, operand: u16| {
                matches!(
                    output_register.origin,
                    VirtualRegisterOrigin::InstructionResult {
                        instruction: origin,
                        ..
                    } if origin == instruction
                ) || matches!(
                    output_register.origin,
                    VirtualRegisterOrigin::InstructionScratch {
                        instruction: origin,
                        operand: origin_operand,
                    } if origin == instruction && origin_operand == operand
                )
            };
            if !defines_here(extension_id, def_operand.operand)
                || output_register.entry_fixed_view.is_some()
            {
                return Err(RedundantExtensionError::UnsupportedUse);
            }
            let row = environment
                .constraint(extension_instruction.constraint)
                .ok_or(RedundantExtensionError::ConstraintMismatch)?;
            if !crate::rewrites::declares_pair(
                row,
                RegisterOperandAccess::Use,
                RegisterOperandAccess::Def,
            ) || row.operands[0].class != value_register.class
                || row.operands[1].class != output_register.class
            {
                return Err(RedundantExtensionError::ConstraintMismatch);
            }
            let mut producers = function.blocks.iter().flat_map(|block| {
                block.instructions.iter().filter(|instruction| {
                    instruction.operands.iter().any(|operand| {
                        operand.access != RegisterOperandAccess::Use
                            && operand.virtual_register == value
                    })
                })
            });
            let producer = producers
                .next()
                .ok_or(RedundantExtensionError::UnsupportedProducer)?;
            if producers.next().is_some() {
                return Err(RedundantExtensionError::UnsupportedProducer);
            }
            let mut defined = producer.operands.iter().filter(|operand| {
                operand.access != RegisterOperandAccess::Use && operand.virtual_register == value
            });
            let Some(defined_operand) = defined.next() else {
                return Err(RedundantExtensionError::UnsupportedProducer);
            };
            if defined.next().is_some() {
                return Err(RedundantExtensionError::UnsupportedProducer);
            }
            let normalization = producer_normalization(producer.kind, defined_operand.operand)
                .ok_or(RedundantExtensionError::UnsupportedProducer)?;
            if !identity(form, normalization) {
                return Err(RedundantExtensionError::UnsupportedProducer);
            }
            let copy = crate::rewrites::integer_copy_row(
                environment,
                value_register.class,
                output_register.class,
                || RedundantExtensionError::ConstraintMismatch,
            )?;
            let steps = crate::rewrites::rewrite_scan(plan, function)
                .and_then(|total| {
                    function.blocks.iter().try_fold(total, |total, block| {
                        total.checked_add(block.instructions.len())
                    })
                })
                .ok_or(RedundantExtensionError::IdentityOverflow)?;
            crate::rewrites::charge_steps(
                steps,
                budget,
                || RedundantExtensionError::IdentityOverflow,
                || RedundantExtensionError::WorkBudgetExceeded,
            )?;
            Ok(Admission {
                block_index,
                extension_index,
                extension_id,
                provenance: extension_instruction.provenance.clone(),
                value,
                output,
                copy,
            })
        }

        pub(crate) fn rewritten(admitted: &Admission<'_>) -> SelectedInstruction {
            crate::rewrites::selected_operands::copy_shaped(
                admitted.extension_id,
                SelectedInstructionKind::CopyI64,
                admitted.copy,
                admitted.value,
                admitted.output,
                admitted.provenance.clone(),
            )
        }
    }
    pub(crate) mod rewrite {
        use optimization_core::OptimizationWorkBudget;
        use register_environment::ValidatedTargetRegisterEnvironment;
        use selected_instructions::SelectedInstructionId;

        use super::{RedundantExtensionError, ValidatedRedundantExtension, admission};
        use crate::ValidatedSelectedAnalysis;

        pub(crate) fn remove_selected_redundant_extension(
            source: &impl ValidatedSelectedAnalysis,
            function_index: usize,
            extension: SelectedInstructionId,
            environment: &ValidatedTargetRegisterEnvironment,
            budget: OptimizationWorkBudget,
        ) -> Result<ValidatedRedundantExtension, RedundantExtensionError> {
            let admitted =
                admission::admit(source, function_index, extension, environment, budget)?;
            let mut transformed = source.selected_plan().clone();
            let function = &mut transformed.functions[function_index];
            function.blocks[admitted.block_index].instructions[admitted.extension_index] =
                admission::rewritten(&admitted);
            super::validation::seal_redundant_extension_removal(
                source,
                function_index,
                extension,
                transformed,
            )
        }
    }
    pub(crate) mod validation {
        use selected_instructions::{
            SelectedFunction, SelectedInstructionId, SelectedInstructionPlan,
        };

        use super::{
            RedundantExtensionError, RedundantExtensionReceipt, ValidatedRedundantExtension,
        };
        use crate::ValidatedSelectedAnalysis;
        use crate::rewrites::sealed;

        pub(crate) fn measured_steps(
            plan: &SelectedInstructionPlan,
            function: &SelectedFunction,
        ) -> Result<u64, RedundantExtensionError> {
            let steps = crate::rewrites::rewrite_scan(plan, function)
                .and_then(|total| {
                    function.blocks.iter().try_fold(total, |total, block| {
                        total.checked_add(block.instructions.len())
                    })
                })
                .ok_or(RedundantExtensionError::IdentityOverflow)?;
            u64::try_from(steps).map_err(|_| RedundantExtensionError::IdentityOverflow)
        }

        pub(crate) fn seal_redundant_extension_removal(
            source: &impl ValidatedSelectedAnalysis,
            function_index: usize,
            extension: SelectedInstructionId,
            proposed: SelectedInstructionPlan,
        ) -> Result<ValidatedRedundantExtension, RedundantExtensionError> {
            Ok(
                sealed!(ValidatedRedundantExtension, RedundantExtensionReceipt, source, proposed, function_index: function_index, extension: extension),
            )
        }
    }

    use std::sync::Arc;

    use optimization_core::OptimizationUnitIdentity;
    use selected_instructions::{
        RedundantExtensionIdentity, SelectedInstructionId, SelectedInstructionPlan,
        SelectedInstructionPlanIdentity,
    };
    use semantic_vocabulary::WorkScheduleIdentity;

    pub(crate) use rewrite::remove_selected_redundant_extension;
    pub(crate) use validation::measured_steps;

    validated_transformed_receipt!(ValidatedRedundantExtension, RedundantExtensionReceipt);

    rewrite_receipt!(RedundantExtensionReceipt {
        function_index: usize,
        extension: SelectedInstructionId,
    });

    rewrite_receipt_identity!(
        redundant_extension_identity,
        RedundantExtensionIdentity,
        RedundantExtensionReceipt,
        b"omega.terminal-redundant-extension-removal.v1\0",
        "redundant-extension",
        extension,
    );

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RedundantExtensionError {
        SourceMismatch,
        UnsupportedInstruction,
        UnsupportedProducer,
        UnsupportedUse,
        ConstraintMismatch,
        WorkBudgetExceeded,
        IdentityOverflow,
        ReplayMismatch,
    }

    failed_display!(
        RedundantExtensionError,
        "invalid redundant-extension removal"
    );
}
