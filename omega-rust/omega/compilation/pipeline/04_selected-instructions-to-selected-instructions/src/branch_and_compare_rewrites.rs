/*
We implement condition-state rewrites over the selected CFG: folding
`MaterializeBoolean*` instructions whose reached compare is compile-time
constant (`constant_boolean`), folding conditional-branch terminators whose
reached compare is compile-time constant (`constant_branch`), and removing dead,
redundant, or equivalent flag-publishing compares (`dead_compare`).
*/

#[allow(unused_imports)]
pub(crate) use crate::rewrites::*;

pub(crate) mod constant_boolean {
    pub(crate) mod admission {
        use crate::rewrites::operand_probes::effect_declaration;
        use std::collections::BTreeSet;

        use optimization_core::OptimizationWorkBudget;
        use register_environment::ValidatedTargetRegisterEnvironment;
        use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
        use selected_instructions::{
            MachineSemanticKind, PreparedMachineEffectCatalog, SelectedInstruction,
            SelectedInstructionId, SelectedInstructionKind, SelectedInstructionProvenance,
            SelectedOperand, VirtualRegisterId,
        };
        use semantic_vocabulary::IntegerValue;

        use super::ConstantBooleanError;
        use super::pair::{
            CONDITION_MATERIALIZATION_RULES, condition_materialization_for, declared_consumer,
            declared_producers,
        };
        use crate::ValidatedSelectedAnalysis;
        use crate::analyses::machine_effects::machine_semantic_kind;
        use crate::rewrites::block_edges::{function_at, member_position};
        use crate::rewrites::condition_state::{
            ConditionStateError, adjacency, backward_cone, entry_index, instruction_at,
            reaching_event, resolved_operands,
        };

        impl From<ConditionStateError> for ConstantBooleanError {
            fn from(error: ConditionStateError) -> Self {
                match error {
                    ConditionStateError::Use => ConstantBooleanError::UnsupportedUse,
                    ConditionStateError::Producer => ConstantBooleanError::UnsupportedProducer,
                    ConditionStateError::Literal => ConstantBooleanError::UnsupportedLiteral,
                }
            }
        }

        pub(crate) struct Admission<'source> {
            pub block_index: usize,
            pub materialization_index: usize,
            pub materialization_id: SelectedInstructionId,
            pub provenance: SelectedInstructionProvenance,
            pub operand: SelectedOperand,
            pub value: IntegerValue,
            pub row: &'source RegisterInstructionConstraint,
        }

        pub(crate) fn admit<'source>(
            source: &'source impl ValidatedSelectedAnalysis,
            function_index: usize,
            materialization: SelectedInstructionId,
            environment: &'source ValidatedTargetRegisterEnvironment,
            effect_catalog: &PreparedMachineEffectCatalog,
            budget: OptimizationWorkBudget,
        ) -> Result<Admission<'source>, ConstantBooleanError> {
            let plan = source.selected_plan();
            if plan.target != environment.target() {
                return Err(ConstantBooleanError::SourceMismatch);
            }
            let catalog = effect_catalog.catalog();
            if catalog.target != plan.target
                || catalog.register_constraints != environment.constraints().identity()
                || catalog.selected_keys != environment.selected_keys()
            {
                return Err(ConstantBooleanError::EffectSurfaceMismatch);
            }
            let function = function_at(plan, function_index, ConstantBooleanError::SourceMismatch)?;
            let (block_index, materialization_index) =
                member_position(function, materialization)
                    .ok_or(ConstantBooleanError::SourceMismatch)?;
            let materialization_instruction =
                &function.blocks[block_index].instructions[materialization_index];
            let consumer_kind = machine_semantic_kind(materialization_instruction.kind);
            if !declared_consumer(materialization_instruction.kind)
                || materialization_instruction.operands.len() != 1
                || materialization_instruction.implicit_uses.is_empty()
                || !materialization_instruction.implicit_defs.is_empty()
                || !materialization_instruction.clobbers.is_empty()
            {
                return Err(ConstantBooleanError::UnsupportedInstruction);
            }
            let operand = &materialization_instruction.operands[0];
            if operand.operand != 0
                || operand.access != RegisterOperandAccess::Def
                || operand.fixed_view.is_some()
                || operand.tied_to.is_some()
                || operand.early_clobber
            {
                return Err(ConstantBooleanError::UnsupportedInstruction);
            }
            let result: VirtualRegisterId = operand.virtual_register;
            let result_register = function
                .virtual_register(result)
                .ok_or(ConstantBooleanError::UnsupportedUse)?;
            let keys = environment.selected_keys();
            let consumer_row = environment
                .constraint(materialization_instruction.constraint)
                .ok_or(ConstantBooleanError::ConstraintMismatch)?;
            if !(consumer_row.operands.len() == 1
                && consumer_row.operands[0].operand == 0
                && consumer_row.operands[0].access == RegisterOperandAccess::Def)
            {
                return Err(ConstantBooleanError::ConstraintMismatch);
            }
            let rewritten_key = keys
                .for_semantic(MachineSemanticKind::MaterializeI64)
                .ok_or(ConstantBooleanError::ConstraintMismatch)?;
            let rewritten_row = environment
                .constraint(rewritten_key)
                .ok_or(ConstantBooleanError::ConstraintMismatch)?;
            if rewritten_row.operands.len() != 1
                || rewritten_row.operands[0].operand != 0
                || rewritten_row.operands[0].access != RegisterOperandAccess::Def
                || rewritten_row.operands[0].class != result_register.class
                || rewritten_row.operands[0].fixed_view.is_some()
                || rewritten_row.operands[0].tied_to.is_some()
                || rewritten_row.operands[0].early_clobber
                || !rewritten_row.implicit_uses.is_empty()
                || !rewritten_row.implicit_defs.is_empty()
                || !rewritten_row.clobbers.is_empty()
            {
                return Err(ConstantBooleanError::ConstraintMismatch);
            }
            let mut flag_universe = BTreeSet::new();
            for producer_kind in declared_producers() {
                let key = keys
                    .for_semantic(producer_kind)
                    .ok_or(ConstantBooleanError::ConstraintMismatch)?;
                flag_universe.extend(
                    environment
                        .constraint(key)
                        .ok_or(ConstantBooleanError::ConstraintMismatch)?
                        .implicit_defs
                        .iter()
                        .copied(),
                );
            }
            let (successors, predecessors) = adjacency(function);
            let entry = entry_index(function).ok_or(ConstantBooleanError::SourceMismatch)?;
            let cone = backward_cone(&predecessors, block_index);
            let mut flag_uses = Vec::new();
            let mut plain_uses = Vec::new();
            let mut compare_site = None;
            for unit in &materialization_instruction.implicit_uses {
                if !flag_universe.contains(unit) {
                    plain_uses.push(*unit);
                    continue;
                }
                flag_uses.push(*unit);
                let event = reaching_event(
                    function,
                    entry,
                    &successors,
                    &cone,
                    block_index,
                    materialization_index,
                    *unit,
                )?;
                if compare_site.is_some_and(|site| site != event) {
                    return Err(ConstantBooleanError::UnsupportedUse);
                }
                compare_site = Some(event);
            }
            if flag_uses.is_empty() {
                return Err(ConstantBooleanError::UnsupportedUse);
            }
            let compare_site = compare_site.ok_or(ConstantBooleanError::UnsupportedUse)?;
            let compare_instruction = instruction_at(function, compare_site);
            if flag_uses
                .iter()
                .any(|unit| !compare_instruction.implicit_defs.contains(unit))
            {
                return Err(ConstantBooleanError::UnsupportedUse);
            }
            let rule = condition_materialization_for(
                machine_semantic_kind(compare_instruction.kind),
                consumer_kind,
            )
            .ok_or(ConstantBooleanError::UnsupportedProducer)?;
            if !rule.matches_producer(compare_instruction.kind) {
                return Err(ConstantBooleanError::UnsupportedProducer);
            }
            debug_assert!(
                CONDITION_MATERIALIZATION_RULES
                    .iter()
                    .all(|declared| declared.rewritten() == rule.rewritten()),
                "every declared materialization pair rewrites to MaterializeI64"
            );
            if !rule.admits_unit_flow(
                &flag_uses,
                &plain_uses,
                materialization_instruction,
                rewritten_row,
            ) {
                return Err(ConstantBooleanError::UnsupportedUse);
            }
            let (left, right) =
                resolved_operands(function, compare_instruction, rule.operand_resolution())?;
            let value = rule.decided_value(left, right);
            let consumer_declaration = effect_declaration(
                effect_catalog,
                consumer_kind,
                materialization_instruction.constraint,
            )
            .ok_or(ConstantBooleanError::EffectSurfaceMismatch)?;
            let rewritten_declaration =
                effect_declaration(effect_catalog, rule.rewritten(), rewritten_row.key)
                    .ok_or(ConstantBooleanError::EffectSurfaceMismatch)?;
            if !rule.admits_declarations(
                &flag_universe,
                consumer_declaration,
                rewritten_declaration,
            ) {
                return Err(ConstantBooleanError::EffectSurfaceMismatch);
            }
            let materialize_surface = rewritten_row
                .implicit_uses
                .iter()
                .chain(rewritten_row.implicit_defs.iter())
                .chain(rewritten_row.clobbers.iter())
                .collect::<BTreeSet<_>>()
                .len();
            let steps = crate::rewrites::constant_fold_steps(
                plan,
                function,
                &successors,
                materialization_instruction.implicit_uses.len(),
                flag_universe.len(),
                materialize_surface,
                compare_instruction.implicit_defs.len(),
                || ConstantBooleanError::IdentityOverflow,
            )?;
            crate::rewrites::charge_steps(
                steps,
                budget,
                || ConstantBooleanError::IdentityOverflow,
                || ConstantBooleanError::WorkBudgetExceeded,
            )?;
            Ok(Admission {
                block_index,
                materialization_index,
                materialization_id: materialization,
                provenance: materialization_instruction.provenance.clone(),
                operand: *operand,
                value,
                row: rewritten_row,
            })
        }

        pub(crate) fn rewritten(admitted: &Admission<'_>) -> SelectedInstruction {
            SelectedInstruction {
                id: admitted.materialization_id,
                kind: SelectedInstructionKind::MaterializeI64 {
                    value: admitted.value,
                },
                constraint: admitted.row.key,
                operands: vec![admitted.operand],
                implicit_uses: admitted.row.implicit_uses.clone(),
                implicit_defs: admitted.row.implicit_defs.clone(),
                clobbers: admitted.row.clobbers.clone(),
                provenance: admitted.provenance.clone(),
            }
        }
    }
    pub(crate) mod pair {
        use std::collections::BTreeSet;

        use register_model::{RegisterInstructionConstraint, RegisterUnitId};
        use selected_instructions::{
            MachineAlternative, MachineEffectDeclaration, MachineEncodedControlEffect,
            MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
            MachineSemanticKind, SelectedInstruction, SelectedInstructionKind,
        };
        use semantic_vocabulary::IntegerValue;

        use crate::analyses::machine_effects::{
            machine_alternative_is_transparent, machine_semantic_kind,
        };
        use crate::rewrites::condition_state::{ConditionOperandResolution, resolved_unit_surface};
        use crate::rewrites::operand_probes::isolated_declaration;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum ConditionMaterializationUnitFlow {
            ConditionStateResolved,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum ConditionMaterializationEffects {
            Isolated,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct ConditionMaterializationRule {
            producer: MachineSemanticKind,
            consumer: MachineSemanticKind,
            operand_resolution: ConditionOperandResolution,
            unit_flow: ConditionMaterializationUnitFlow,
            effects: ConditionMaterializationEffects,
        }

        declared_kind_matchers!(ConditionMaterializationRule);

        impl ConditionMaterializationRule {
            const DECIDED_MATERIALIZATION: Self = Self {
                producer: MachineSemanticKind::CompareI64,
                consumer: MachineSemanticKind::MaterializeBooleanEqual,
                operand_resolution: ConditionOperandResolution::TwoRegisterOperands,
                unit_flow: ConditionMaterializationUnitFlow::ConditionStateResolved,
                effects: ConditionMaterializationEffects::Isolated,
            };

            pub(crate) const COMPARE_EQUAL: Self = Self::DECIDED_MATERIALIZATION;
            pub(crate) const COMPARE_U64_LESS_THAN: Self = Self {
                consumer: MachineSemanticKind::MaterializeBooleanU64LessThan,
                ..Self::DECIDED_MATERIALIZATION
            };
            pub(crate) const COMPARE_I64_LESS_THAN: Self = Self {
                consumer: MachineSemanticKind::MaterializeBooleanI64LessThan,
                ..Self::DECIDED_MATERIALIZATION
            };
            pub(crate) const COMPARE_U64_LESS_OR_EQUAL: Self = Self {
                consumer: MachineSemanticKind::MaterializeBooleanU64LessOrEqual,
                ..Self::DECIDED_MATERIALIZATION
            };
            pub(crate) const COMPARE_I64_LESS_OR_EQUAL: Self = Self {
                consumer: MachineSemanticKind::MaterializeBooleanI64LessOrEqual,
                ..Self::DECIDED_MATERIALIZATION
            };
            pub(crate) const COMPARE_IMMEDIATE_EQUAL: Self = Self {
                producer: MachineSemanticKind::CompareI64Immediate,
                operand_resolution: ConditionOperandResolution::RegisterAndKindImmediate,
                ..Self::DECIDED_MATERIALIZATION
            };
            pub(crate) const COMPARE_IMMEDIATE_U64_LESS_THAN: Self = Self {
                consumer: MachineSemanticKind::MaterializeBooleanU64LessThan,
                ..Self::COMPARE_IMMEDIATE_EQUAL
            };
            pub(crate) const COMPARE_IMMEDIATE_I64_LESS_THAN: Self = Self {
                consumer: MachineSemanticKind::MaterializeBooleanI64LessThan,
                ..Self::COMPARE_IMMEDIATE_EQUAL
            };
            pub(crate) const COMPARE_IMMEDIATE_U64_LESS_OR_EQUAL: Self = Self {
                consumer: MachineSemanticKind::MaterializeBooleanU64LessOrEqual,
                ..Self::COMPARE_IMMEDIATE_EQUAL
            };
            pub(crate) const COMPARE_IMMEDIATE_I64_LESS_OR_EQUAL: Self = Self {
                consumer: MachineSemanticKind::MaterializeBooleanI64LessOrEqual,
                ..Self::COMPARE_IMMEDIATE_EQUAL
            };
            pub(crate) const COMPARE_ZERO_EQUAL: Self = Self {
                producer: MachineSemanticKind::CompareI64Zero,
                operand_resolution: ConditionOperandResolution::RegisterAndZeroBound,
                ..Self::DECIDED_MATERIALIZATION
            };
            pub(crate) const COMPARE_ZERO_U64_LESS_THAN: Self = Self {
                consumer: MachineSemanticKind::MaterializeBooleanU64LessThan,
                ..Self::COMPARE_ZERO_EQUAL
            };
            pub(crate) const COMPARE_ZERO_I64_LESS_THAN: Self = Self {
                consumer: MachineSemanticKind::MaterializeBooleanI64LessThan,
                ..Self::COMPARE_ZERO_EQUAL
            };
            pub(crate) const COMPARE_ZERO_U64_LESS_OR_EQUAL: Self = Self {
                consumer: MachineSemanticKind::MaterializeBooleanU64LessOrEqual,
                ..Self::COMPARE_ZERO_EQUAL
            };
            pub(crate) const COMPARE_ZERO_I64_LESS_OR_EQUAL: Self = Self {
                consumer: MachineSemanticKind::MaterializeBooleanI64LessOrEqual,
                ..Self::COMPARE_ZERO_EQUAL
            };

            pub(crate) const fn producer(self) -> MachineSemanticKind {
                self.producer
            }
            pub(crate) const fn consumer(self) -> MachineSemanticKind {
                self.consumer
            }
            pub(crate) const fn rewritten(self) -> MachineSemanticKind {
                MachineSemanticKind::MaterializeI64
            }
            pub(crate) const fn operand_resolution(self) -> ConditionOperandResolution {
                self.operand_resolution
            }

            pub(crate) fn decided_value(&self, left: u64, right: u64) -> IntegerValue {
                let holds = match self.consumer {
                    MachineSemanticKind::MaterializeBooleanEqual => left == right,
                    MachineSemanticKind::MaterializeBooleanU64LessThan => left < right,
                    MachineSemanticKind::MaterializeBooleanI64LessThan => {
                        (left as i64) < (right as i64)
                    }
                    MachineSemanticKind::MaterializeBooleanU64LessOrEqual => left <= right,
                    MachineSemanticKind::MaterializeBooleanI64LessOrEqual => {
                        (left as i64) <= (right as i64)
                    }
                    _ => unreachable!("declared consumers are the materialization kinds"),
                };
                IntegerValue::Unsigned(u128::from(holds))
            }

            pub(crate) fn admits_unit_flow(
                &self,
                flag_uses: &[RegisterUnitId],
                plain_uses: &[RegisterUnitId],
                consumer: &SelectedInstruction,
                rewritten: &RegisterInstructionConstraint,
            ) -> bool {
                match self.unit_flow {
                    ConditionMaterializationUnitFlow::ConditionStateResolved => {
                        resolved_unit_surface(flag_uses, plain_uses, consumer, rewritten)
                    }
                }
            }

            pub(crate) fn admits_declarations(
                &self,
                flag_universe: &BTreeSet<RegisterUnitId>,
                consumer: &MachineEffectDeclaration,
                rewritten: &MachineEffectDeclaration,
            ) -> bool {
                match self.effects {
                    ConditionMaterializationEffects::Isolated => {
                        isolated_declaration(consumer)
                            && isolated_declaration(rewritten)
                            && consumer.alternatives.iter().all(|alternative| {
                                materialization_alternative(alternative, flag_universe)
                            })
                            && rewritten
                                .alternatives
                                .iter()
                                .all(machine_alternative_is_transparent)
                    }
                }
            }
        }

        pub(crate) const CONDITION_MATERIALIZATION_RULES: &[ConditionMaterializationRule] = &[
            ConditionMaterializationRule::COMPARE_EQUAL,
            ConditionMaterializationRule::COMPARE_U64_LESS_THAN,
            ConditionMaterializationRule::COMPARE_I64_LESS_THAN,
            ConditionMaterializationRule::COMPARE_U64_LESS_OR_EQUAL,
            ConditionMaterializationRule::COMPARE_I64_LESS_OR_EQUAL,
            ConditionMaterializationRule::COMPARE_IMMEDIATE_EQUAL,
            ConditionMaterializationRule::COMPARE_IMMEDIATE_U64_LESS_THAN,
            ConditionMaterializationRule::COMPARE_IMMEDIATE_I64_LESS_THAN,
            ConditionMaterializationRule::COMPARE_IMMEDIATE_U64_LESS_OR_EQUAL,
            ConditionMaterializationRule::COMPARE_IMMEDIATE_I64_LESS_OR_EQUAL,
            ConditionMaterializationRule::COMPARE_ZERO_EQUAL,
            ConditionMaterializationRule::COMPARE_ZERO_U64_LESS_THAN,
            ConditionMaterializationRule::COMPARE_ZERO_I64_LESS_THAN,
            ConditionMaterializationRule::COMPARE_ZERO_U64_LESS_OR_EQUAL,
            ConditionMaterializationRule::COMPARE_ZERO_I64_LESS_OR_EQUAL,
        ];

        declared_pair_lookup!(
            condition_materialization_for,
            CONDITION_MATERIALIZATION_RULES,
            ConditionMaterializationRule
        );

        pub(crate) fn declared_consumer(kind: SelectedInstructionKind) -> bool {
            CONDITION_MATERIALIZATION_RULES
                .iter()
                .any(|rule| rule.matches_consumer(kind))
        }

        pub(crate) fn declared_producers() -> impl Iterator<Item = MachineSemanticKind> {
            [
                MachineSemanticKind::CompareI64,
                MachineSemanticKind::CompareI64Immediate,
                MachineSemanticKind::CompareI64Zero,
            ]
            .into_iter()
        }

        fn materialization_alternative(
            alternative: &MachineAlternative,
            flag_universe: &BTreeSet<RegisterUnitId>,
        ) -> bool {
            let encoded = &alternative.encoded;
            encoded.memory == MachineEncodedMemoryEffect::NoneV1
                && encoded.stack == MachineEncodedStackEffect::UnchangedV1
                && encoded.trap == MachineEncodedTrapBehavior::NeverV1
                && encoded.control == MachineEncodedControlEffect::FallThroughV1
                && !encoded.implicit_unit_uses.is_empty()
                && encoded
                    .implicit_unit_uses
                    .iter()
                    .all(|unit| flag_universe.contains(unit))
                && encoded.implicit_unit_defs.is_empty()
                && encoded.implicit_unit_clobbers.is_empty()
        }
    }
    pub(crate) mod rewrite {
        use optimization_core::OptimizationWorkBudget;
        use register_environment::ValidatedTargetRegisterEnvironment;
        use selected_instructions::{PreparedMachineEffectCatalog, SelectedInstructionId};

        use super::{ConstantBooleanError, ValidatedConstantBoolean, admission};
        use crate::ValidatedSelectedAnalysis;

        pub(crate) fn fold_selected_constant_boolean(
            source: &impl ValidatedSelectedAnalysis,
            function_index: usize,
            materialization: SelectedInstructionId,
            environment: &ValidatedTargetRegisterEnvironment,
            effect_catalog: &PreparedMachineEffectCatalog,
            budget: OptimizationWorkBudget,
        ) -> Result<ValidatedConstantBoolean, ConstantBooleanError> {
            let admitted = admission::admit(
                source,
                function_index,
                materialization,
                environment,
                effect_catalog,
                budget,
            )?;
            let mut transformed = source.selected_plan().clone();
            let function = &mut transformed.functions[function_index];
            function.blocks[admitted.block_index].instructions[admitted.materialization_index] =
                admission::rewritten(&admitted);
            super::validation::seal_constant_boolean_fold(
                source,
                function_index,
                materialization,
                transformed,
            )
        }
    }
    pub(crate) mod validation {
        use std::collections::BTreeSet;

        use register_environment::ValidatedTargetRegisterEnvironment;
        use register_model::RegisterUnitId;
        use selected_instructions::{
            MachineSemanticKind, SelectedFunction, SelectedInstructionId, SelectedInstructionPlan,
        };

        use super::{ConstantBooleanError, ConstantBooleanReceipt, ValidatedConstantBoolean};
        use crate::ValidatedSelectedAnalysis;
        use crate::rewrites::sealed;

        pub(crate) fn surface_sizes(
            environment: &ValidatedTargetRegisterEnvironment,
        ) -> Result<(usize, usize), ConstantBooleanError> {
            let keys = environment.selected_keys();
            let mut flag_universe = BTreeSet::new();
            for key in [
                keys.compare_i64,
                keys.compare_i64_immediate,
                keys.compare_i64_zero,
            ] {
                flag_universe.extend(
                    environment
                        .constraint(key)
                        .ok_or(ConstantBooleanError::ConstraintMismatch)?
                        .implicit_defs
                        .iter()
                        .copied(),
                );
            }
            let materialize_row = environment
                .constraint(
                    keys.for_semantic(MachineSemanticKind::MaterializeI64)
                        .ok_or(ConstantBooleanError::ConstraintMismatch)?,
                )
                .ok_or(ConstantBooleanError::ConstraintMismatch)?;
            let materialize_surface: BTreeSet<RegisterUnitId> = materialize_row
                .implicit_uses
                .iter()
                .chain(materialize_row.implicit_defs.iter())
                .chain(materialize_row.clobbers.iter())
                .copied()
                .collect();
            Ok((flag_universe.len(), materialize_surface.len()))
        }

        pub(crate) fn measured_steps(
            plan: &SelectedInstructionPlan,
            function: &SelectedFunction,
            successors: &[Vec<usize>],
            implicit_uses: usize,
            flag_universe: usize,
            materialize_surface: usize,
            producer_defs: usize,
        ) -> Result<u64, ConstantBooleanError> {
            let steps = crate::rewrites::constant_fold_steps(
                plan,
                function,
                successors,
                implicit_uses,
                flag_universe,
                materialize_surface,
                producer_defs,
                || ConstantBooleanError::IdentityOverflow,
            )?;
            u64::try_from(steps).map_err(|_| ConstantBooleanError::IdentityOverflow)
        }

        pub(crate) fn seal_constant_boolean_fold(
            source: &impl ValidatedSelectedAnalysis,
            function_index: usize,
            materialization: SelectedInstructionId,
            proposed: SelectedInstructionPlan,
        ) -> Result<ValidatedConstantBoolean, ConstantBooleanError> {
            Ok(
                sealed!(ValidatedConstantBoolean, ConstantBooleanReceipt, source, proposed, function_index: function_index, materialization: materialization),
            )
        }
    }

    use std::sync::Arc;

    use optimization_core::OptimizationUnitIdentity;
    use selected_instructions::{
        ConstantBooleanIdentity, SelectedInstructionId, SelectedInstructionPlan,
        SelectedInstructionPlanIdentity,
    };
    use semantic_vocabulary::WorkScheduleIdentity;

    pub(crate) use rewrite::fold_selected_constant_boolean;
    pub(crate) use validation::{measured_steps, surface_sizes};

    validated_transformed_receipt!(ValidatedConstantBoolean, ConstantBooleanReceipt);

    rewrite_receipt!(ConstantBooleanReceipt {
        function_index: usize,
        materialization: SelectedInstructionId,
    });

    rewrite_receipt_identity!(
        constant_boolean_identity,
        ConstantBooleanIdentity,
        ConstantBooleanReceipt,
        b"omega.terminal-constant-boolean-fold.v1\0",
        "constant-boolean",
        materialization,
    );

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ConstantBooleanError {
        SourceMismatch,
        UnsupportedInstruction,
        UnsupportedUse,
        UnsupportedProducer,
        UnsupportedLiteral,
        ConstraintMismatch,
        EffectSurfaceMismatch,
        WorkBudgetExceeded,
        IdentityOverflow,
        ReplayMismatch,
    }

    failed_display!(ConstantBooleanError, "invalid constant boolean fold");
}

pub(crate) mod constant_branch {
    pub(crate) mod admission {
        use crate::rewrites::operand_probes::effect_declaration;
        use std::collections::BTreeSet;

        use optimization_core::OptimizationWorkBudget;
        use register_environment::ValidatedTargetRegisterEnvironment;
        use register_model::{RegisterInstructionConstraint, RegisterUnitId};
        use selected_instructions::{
            PreparedMachineEffectCatalog, SelectedInstruction, SelectedInstructionId,
            SelectedInstructionKind, SelectedInstructionProvenance, SelectedSuccessor,
            SelectedTerminator, terminator_instruction,
        };

        use super::ConstantBranchError;
        use super::pair::{
            TERMINATOR_PAIR_RULES, TerminatorPairRule, declared_consumer, declared_producers,
            terminator_pair_for,
        };
        use crate::ValidatedSelectedAnalysis;
        use crate::analyses::machine_effects::machine_semantic_kind;
        use crate::rewrites::block_edges::function_at;
        use crate::rewrites::condition_state::{
            ConditionStateError, adjacency, backward_cone, entry_index, instruction_at,
            reaching_event, resolved_operands,
        };

        impl From<ConditionStateError> for ConstantBranchError {
            fn from(error: ConditionStateError) -> Self {
                match error {
                    ConditionStateError::Use => ConstantBranchError::UnsupportedUse,
                    ConditionStateError::Producer => ConstantBranchError::UnsupportedProducer,
                    ConditionStateError::Literal => ConstantBranchError::UnsupportedLiteral,
                }
            }
        }

        pub(crate) struct Admission<'source> {
            pub block_index: usize,
            pub branch_id: SelectedInstructionId,
            pub provenance: SelectedInstructionProvenance,
            pub successor: SelectedSuccessor,
            pub row: &'source RegisterInstructionConstraint,
        }

        pub(crate) fn admit<'source>(
            source: &'source impl ValidatedSelectedAnalysis,
            function_index: usize,
            branch: SelectedInstructionId,
            environment: &'source ValidatedTargetRegisterEnvironment,
            effect_catalog: &PreparedMachineEffectCatalog,
            budget: OptimizationWorkBudget,
        ) -> Result<Admission<'source>, ConstantBranchError> {
            let plan = source.selected_plan();
            if plan.target != environment.target() {
                return Err(ConstantBranchError::SourceMismatch);
            }
            let catalog = effect_catalog.catalog();
            if catalog.target != plan.target
                || catalog.register_constraints != environment.constraints().identity()
                || catalog.selected_keys != environment.selected_keys()
            {
                return Err(ConstantBranchError::EffectSurfaceMismatch);
            }
            let function = function_at(plan, function_index, ConstantBranchError::SourceMismatch)?;
            let block_index = function
                .blocks
                .iter()
                .position(|block| terminator_instruction(&block.terminator).id == branch)
                .ok_or(ConstantBranchError::SourceMismatch)?;
            let block = &function.blocks[block_index];
            let branch_instruction = terminator_instruction(&block.terminator);
            let consumer_kind = machine_semantic_kind(branch_instruction.kind);
            let candidate_rules: Vec<&TerminatorPairRule> = TERMINATOR_PAIR_RULES
                .iter()
                .filter(|rule| rule.consumer() == consumer_kind)
                .collect();
            if !declared_consumer(branch_instruction.kind)
                || !candidate_rules
                    .iter()
                    .any(|rule| rule.matches_terminator(&block.terminator))
                || !branch_instruction.operands.is_empty()
                || branch_instruction.implicit_uses.is_empty()
            {
                return Err(ConstantBranchError::UnsupportedInstruction);
            }
            let keys = environment.selected_keys();
            let mut flag_universe = BTreeSet::new();
            for producer_kind in declared_producers() {
                let key = keys
                    .for_semantic(producer_kind)
                    .ok_or(ConstantBranchError::ConstraintMismatch)?;
                flag_universe.extend(
                    environment
                        .constraint(key)
                        .ok_or(ConstantBranchError::ConstraintMismatch)?
                        .implicit_defs
                        .iter()
                        .copied(),
                );
            }
            let jump_key = keys
                .for_semantic(candidate_rules[0].rewritten())
                .ok_or(ConstantBranchError::ConstraintMismatch)?;
            let jump_row = environment
                .constraint(jump_key)
                .ok_or(ConstantBranchError::ConstraintMismatch)?;
            let jump_surface: BTreeSet<RegisterUnitId> = jump_row
                .implicit_uses
                .iter()
                .chain(jump_row.implicit_defs.iter())
                .chain(jump_row.clobbers.iter())
                .copied()
                .collect();
            if !jump_row.operands.is_empty()
                || BTreeSet::from_iter(jump_row.implicit_defs.iter().copied())
                    != BTreeSet::from_iter(branch_instruction.implicit_defs.iter().copied())
                || BTreeSet::from_iter(jump_row.clobbers.iter().copied())
                    != BTreeSet::from_iter(branch_instruction.clobbers.iter().copied())
            {
                return Err(ConstantBranchError::ConstraintMismatch);
            }
            let (successors, predecessors) = adjacency(function);
            let entry = entry_index(function).ok_or(ConstantBranchError::SourceMismatch)?;
            let cone = backward_cone(&predecessors, block_index);
            let read_position = block.instructions.len();
            let mut flag_uses = Vec::new();
            let mut plain_uses = Vec::new();
            let mut compare_site = None;
            for unit in &branch_instruction.implicit_uses {
                if !flag_universe.contains(unit) {
                    if !jump_surface.contains(unit) {
                        return Err(ConstantBranchError::UnsupportedUse);
                    }
                    plain_uses.push(*unit);
                    continue;
                }
                flag_uses.push(*unit);
                let event = reaching_event(
                    function,
                    entry,
                    &successors,
                    &cone,
                    block_index,
                    read_position,
                    *unit,
                )?;
                if compare_site.is_some_and(|site| site != event) {
                    return Err(ConstantBranchError::UnsupportedUse);
                }
                compare_site = Some(event);
            }
            if flag_uses.is_empty() {
                return Err(ConstantBranchError::UnsupportedInstruction);
            }
            let compare_site = compare_site.ok_or(ConstantBranchError::UnsupportedUse)?;
            let compare_instruction = instruction_at(function, compare_site);
            if flag_uses
                .iter()
                .any(|unit| !compare_instruction.implicit_defs.contains(unit))
            {
                return Err(ConstantBranchError::UnsupportedUse);
            }
            let rule = terminator_pair_for(
                machine_semantic_kind(compare_instruction.kind),
                consumer_kind,
            )
            .ok_or(ConstantBranchError::UnsupportedProducer)?;
            if !rule.matches_producer(compare_instruction.kind) {
                return Err(ConstantBranchError::UnsupportedProducer);
            }
            if !rule.admits_unit_flow(&flag_uses, &plain_uses, branch_instruction, jump_row) {
                return Err(ConstantBranchError::UnsupportedUse);
            }
            let (left, right) =
                resolved_operands(function, compare_instruction, rule.operand_resolution())?;
            let successor = rule
                .decided_successor(&block.terminator, left, right)
                .ok_or(ConstantBranchError::UnsupportedInstruction)?
                .clone();
            let consumer_declaration =
                effect_declaration(effect_catalog, consumer_kind, branch_instruction.constraint)
                    .ok_or(ConstantBranchError::EffectSurfaceMismatch)?;
            let rewritten_declaration =
                effect_declaration(effect_catalog, rule.rewritten(), jump_row.key)
                    .ok_or(ConstantBranchError::EffectSurfaceMismatch)?;
            if !rule.admits_declarations(consumer_declaration, rewritten_declaration) {
                return Err(ConstantBranchError::EffectSurfaceMismatch);
            }
            let steps = crate::rewrites::constant_fold_steps(
                plan,
                function,
                &successors,
                branch_instruction.implicit_uses.len(),
                flag_universe.len(),
                jump_surface.len(),
                compare_instruction.implicit_defs.len(),
                || ConstantBranchError::IdentityOverflow,
            )?;
            crate::rewrites::charge_steps(
                steps,
                budget,
                || ConstantBranchError::IdentityOverflow,
                || ConstantBranchError::WorkBudgetExceeded,
            )?;
            Ok(Admission {
                block_index,
                branch_id: branch,
                provenance: branch_instruction.provenance.clone(),
                successor,
                row: jump_row,
            })
        }

        pub(crate) fn rewritten(admitted: &Admission<'_>) -> SelectedTerminator {
            SelectedTerminator::Jump {
                instruction: SelectedInstruction {
                    id: admitted.branch_id,
                    kind: SelectedInstructionKind::Jump,
                    constraint: admitted.row.key,
                    operands: Vec::new(),
                    implicit_uses: admitted.row.implicit_uses.clone(),
                    implicit_defs: admitted.row.implicit_defs.clone(),
                    clobbers: admitted.row.clobbers.clone(),
                    provenance: admitted.provenance.clone(),
                },
                successor: admitted.successor.clone(),
            }
        }
    }
    pub(crate) mod pair {
        use register_model::RegisterInstructionConstraint;
        use selected_instructions::{
            MachineAlternative, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
            MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
            MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineMemoryEffect,
            MachineSemanticKind, MachineTrapBehavior, SelectedInstructionKind, SelectedTerminator,
        };

        use crate::analyses::machine_effects::machine_semantic_kind;
        use crate::rewrites::condition_state::{ConditionOperandResolution, resolved_unit_surface};

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum TerminatorPairUnitFlow {
            ConditionStateResolved,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum TerminatorPairControlFlow {
            ConditionalResolvedToUnconditional,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct TerminatorPairRule {
            producer: MachineSemanticKind,
            consumer: MachineSemanticKind,
            rewritten: MachineSemanticKind,
            operand_resolution: ConditionOperandResolution,
            unit_flow: TerminatorPairUnitFlow,
            control: TerminatorPairControlFlow,
        }

        declared_kind_matchers!(TerminatorPairRule);

        impl TerminatorPairRule {
            const DECIDED_BRANCH: Self = Self {
                producer: MachineSemanticKind::CompareI64,
                consumer: MachineSemanticKind::ConditionalBranchNonZero,
                rewritten: MachineSemanticKind::Jump,
                operand_resolution: ConditionOperandResolution::TwoRegisterOperands,
                unit_flow: TerminatorPairUnitFlow::ConditionStateResolved,
                control: TerminatorPairControlFlow::ConditionalResolvedToUnconditional,
            };

            pub(crate) const COMPARE_BRANCH_NONZERO: Self = Self::DECIDED_BRANCH;
            pub(crate) const COMPARE_BRANCH_U64_LESS_THAN: Self = Self {
                consumer: MachineSemanticKind::ConditionalBranchU64LessThan,
                ..Self::DECIDED_BRANCH
            };
            pub(crate) const COMPARE_BRANCH_I64_LESS_THAN: Self = Self {
                consumer: MachineSemanticKind::ConditionalBranchI64LessThan,
                ..Self::DECIDED_BRANCH
            };
            pub(crate) const COMPARE_IMMEDIATE_BRANCH_NONZERO: Self = Self {
                producer: MachineSemanticKind::CompareI64Immediate,
                operand_resolution: ConditionOperandResolution::RegisterAndKindImmediate,
                ..Self::DECIDED_BRANCH
            };
            pub(crate) const COMPARE_IMMEDIATE_BRANCH_U64_LESS_THAN: Self = Self {
                consumer: MachineSemanticKind::ConditionalBranchU64LessThan,
                ..Self::COMPARE_IMMEDIATE_BRANCH_NONZERO
            };
            pub(crate) const COMPARE_IMMEDIATE_BRANCH_I64_LESS_THAN: Self = Self {
                consumer: MachineSemanticKind::ConditionalBranchI64LessThan,
                ..Self::COMPARE_IMMEDIATE_BRANCH_NONZERO
            };
            pub(crate) const COMPARE_ZERO_BRANCH_NONZERO: Self = Self {
                producer: MachineSemanticKind::CompareI64Zero,
                operand_resolution: ConditionOperandResolution::RegisterAndZeroBound,
                ..Self::DECIDED_BRANCH
            };
            pub(crate) const COMPARE_ZERO_BRANCH_U64_LESS_THAN: Self = Self {
                consumer: MachineSemanticKind::ConditionalBranchU64LessThan,
                ..Self::COMPARE_ZERO_BRANCH_NONZERO
            };
            pub(crate) const COMPARE_ZERO_BRANCH_I64_LESS_THAN: Self = Self {
                consumer: MachineSemanticKind::ConditionalBranchI64LessThan,
                ..Self::COMPARE_ZERO_BRANCH_NONZERO
            };

            pub(crate) const fn producer(self) -> MachineSemanticKind {
                self.producer
            }
            pub(crate) const fn consumer(self) -> MachineSemanticKind {
                self.consumer
            }
            pub(crate) const fn rewritten(self) -> MachineSemanticKind {
                self.rewritten
            }
            pub(crate) const fn operand_resolution(self) -> ConditionOperandResolution {
                self.operand_resolution
            }
            pub(crate) const fn unit_flow(self) -> TerminatorPairUnitFlow {
                self.unit_flow
            }
            pub(crate) const fn control(self) -> TerminatorPairControlFlow {
                self.control
            }

            pub(crate) fn matches_terminator(&self, terminator: &SelectedTerminator) -> bool {
                matches!(
                    (self.consumer, terminator),
                    (
                        MachineSemanticKind::ConditionalBranchNonZero,
                        SelectedTerminator::ConditionalBranch { .. }
                    ) | (
                        MachineSemanticKind::ConditionalBranchU64LessThan,
                        SelectedTerminator::ConditionalBranchU64LessThan { .. }
                    ) | (
                        MachineSemanticKind::ConditionalBranchI64LessThan,
                        SelectedTerminator::ConditionalBranchI64LessThan { .. }
                    )
                )
            }

            pub(crate) fn decided_successor<'terminator>(
                &self,
                terminator: &'terminator SelectedTerminator,
                left: u64,
                right: u64,
            ) -> Option<&'terminator selected_instructions::SelectedSuccessor> {
                match (self.consumer, terminator) {
                    (
                        MachineSemanticKind::ConditionalBranchNonZero,
                        SelectedTerminator::ConditionalBranch {
                            when_nonzero,
                            when_zero,
                            ..
                        },
                    ) => Some(if left != right {
                        when_nonzero
                    } else {
                        when_zero
                    }),
                    (
                        MachineSemanticKind::ConditionalBranchU64LessThan,
                        SelectedTerminator::ConditionalBranchU64LessThan {
                            when_less,
                            when_not_less,
                            ..
                        },
                    ) => Some(if left < right {
                        when_less
                    } else {
                        when_not_less
                    }),
                    (
                        MachineSemanticKind::ConditionalBranchI64LessThan,
                        SelectedTerminator::ConditionalBranchI64LessThan {
                            when_less,
                            when_not_less,
                            ..
                        },
                    ) => Some(if (left as i64) < (right as i64) {
                        when_less
                    } else {
                        when_not_less
                    }),
                    _ => None,
                }
            }

            pub(crate) fn admits_unit_flow(
                &self,
                flag_uses: &[register_model::RegisterUnitId],
                plain_uses: &[register_model::RegisterUnitId],
                consumer: &selected_instructions::SelectedInstruction,
                rewritten: &RegisterInstructionConstraint,
            ) -> bool {
                match self.unit_flow() {
                    TerminatorPairUnitFlow::ConditionStateResolved => {
                        resolved_unit_surface(flag_uses, plain_uses, consumer, rewritten)
                    }
                }
            }

            pub(crate) fn admits_declarations(
                &self,
                consumer: &MachineEffectDeclaration,
                rewritten: &MachineEffectDeclaration,
            ) -> bool {
                match self.control() {
                    TerminatorPairControlFlow::ConditionalResolvedToUnconditional => {
                        isolated_declaration(consumer)
                            && isolated_declaration(rewritten)
                            && consumer
                                .alternatives
                                .iter()
                                .all(|alternative| branch_alternative(alternative, true))
                            && rewritten
                                .alternatives
                                .iter()
                                .all(|alternative| branch_alternative(alternative, false))
                    }
                }
            }
        }

        pub(crate) const TERMINATOR_PAIR_RULES: &[TerminatorPairRule] = &[
            TerminatorPairRule::COMPARE_BRANCH_NONZERO,
            TerminatorPairRule::COMPARE_BRANCH_U64_LESS_THAN,
            TerminatorPairRule::COMPARE_BRANCH_I64_LESS_THAN,
            TerminatorPairRule::COMPARE_IMMEDIATE_BRANCH_NONZERO,
            TerminatorPairRule::COMPARE_IMMEDIATE_BRANCH_U64_LESS_THAN,
            TerminatorPairRule::COMPARE_IMMEDIATE_BRANCH_I64_LESS_THAN,
            TerminatorPairRule::COMPARE_ZERO_BRANCH_NONZERO,
            TerminatorPairRule::COMPARE_ZERO_BRANCH_U64_LESS_THAN,
            TerminatorPairRule::COMPARE_ZERO_BRANCH_I64_LESS_THAN,
        ];

        declared_pair_lookup!(
            terminator_pair_for,
            TERMINATOR_PAIR_RULES,
            TerminatorPairRule
        );

        pub(crate) fn declared_consumer(kind: SelectedInstructionKind) -> bool {
            TERMINATOR_PAIR_RULES
                .iter()
                .any(|rule| rule.matches_consumer(kind))
        }

        pub(crate) fn declared_producers() -> impl Iterator<Item = MachineSemanticKind> {
            [
                MachineSemanticKind::CompareI64,
                MachineSemanticKind::CompareI64Immediate,
                MachineSemanticKind::CompareI64Zero,
            ]
            .into_iter()
        }

        fn isolated_declaration(declaration: &MachineEffectDeclaration) -> bool {
            declaration.memory == MachineMemoryEffect::NoneV1
                && declaration.trap == MachineTrapBehavior::NeverV1
                && declaration.barrier == MachineBarrier::ControlFlow
                && declaration.call == MachineCallEffect::NoneV1
                && declaration.cleanup == MachineCleanupEffect::NoneV1
        }

        fn branch_alternative(alternative: &MachineAlternative, conditional: bool) -> bool {
            let encoded = &alternative.encoded;
            encoded.memory == MachineEncodedMemoryEffect::NoneV1
                && encoded.stack == MachineEncodedStackEffect::UnchangedV1
                && encoded.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                && encoded.control
                    == if conditional {
                        MachineEncodedControlEffect::ConditionalRelativeBranchV1
                    } else {
                        MachineEncodedControlEffect::UnconditionalRelativeBranchV1
                    }
        }
    }
    pub(crate) mod rewrite {
        use optimization_core::OptimizationWorkBudget;
        use register_environment::ValidatedTargetRegisterEnvironment;
        use selected_instructions::{PreparedMachineEffectCatalog, SelectedInstructionId};

        use super::{ConstantBranchError, ValidatedConstantBranch, admission};
        use crate::ValidatedSelectedAnalysis;

        pub(crate) fn fold_selected_constant_branch(
            source: &impl ValidatedSelectedAnalysis,
            function_index: usize,
            branch: SelectedInstructionId,
            environment: &ValidatedTargetRegisterEnvironment,
            effect_catalog: &PreparedMachineEffectCatalog,
            budget: OptimizationWorkBudget,
        ) -> Result<ValidatedConstantBranch, ConstantBranchError> {
            let admitted = admission::admit(
                source,
                function_index,
                branch,
                environment,
                effect_catalog,
                budget,
            )?;
            let mut transformed = source.selected_plan().clone();
            transformed.functions[function_index].blocks[admitted.block_index].terminator =
                admission::rewritten(&admitted);
            super::validation::seal_constant_branch_fold(
                source,
                function_index,
                branch,
                transformed,
            )
        }
    }
    pub(crate) mod validation {
        use std::collections::BTreeSet;

        use register_environment::ValidatedTargetRegisterEnvironment;
        use register_model::RegisterUnitId;
        use selected_instructions::{
            SelectedFunction, SelectedInstructionId, SelectedInstructionPlan,
        };

        use super::{ConstantBranchError, ConstantBranchReceipt, ValidatedConstantBranch};
        use crate::ValidatedSelectedAnalysis;
        use crate::rewrites::sealed;

        pub(crate) fn surface_sizes(
            environment: &ValidatedTargetRegisterEnvironment,
        ) -> Result<(usize, usize), ConstantBranchError> {
            let keys = environment.selected_keys();
            let mut flag_universe = BTreeSet::new();
            for key in [
                keys.compare_i64,
                keys.compare_i64_immediate,
                keys.compare_i64_zero,
            ] {
                flag_universe.extend(
                    environment
                        .constraint(key)
                        .ok_or(ConstantBranchError::ConstraintMismatch)?
                        .implicit_defs
                        .iter()
                        .copied(),
                );
            }
            let jump_row = environment
                .constraint(keys.jump)
                .ok_or(ConstantBranchError::ConstraintMismatch)?;
            let jump_surface: BTreeSet<RegisterUnitId> = jump_row
                .implicit_uses
                .iter()
                .chain(jump_row.implicit_defs.iter())
                .chain(jump_row.clobbers.iter())
                .copied()
                .collect();
            Ok((flag_universe.len(), jump_surface.len()))
        }

        pub(crate) fn measured_steps(
            plan: &SelectedInstructionPlan,
            function: &SelectedFunction,
            successors: &[Vec<usize>],
            implicit_uses: usize,
            flag_universe: usize,
            jump_surface: usize,
            compare_defs: usize,
        ) -> Result<u64, ConstantBranchError> {
            let steps = crate::rewrites::constant_fold_steps(
                plan,
                function,
                successors,
                implicit_uses,
                flag_universe,
                jump_surface,
                compare_defs,
                || ConstantBranchError::IdentityOverflow,
            )?;
            u64::try_from(steps).map_err(|_| ConstantBranchError::IdentityOverflow)
        }

        pub(crate) fn seal_constant_branch_fold(
            source: &impl ValidatedSelectedAnalysis,
            function_index: usize,
            branch: SelectedInstructionId,
            proposed: SelectedInstructionPlan,
        ) -> Result<ValidatedConstantBranch, ConstantBranchError> {
            Ok(
                sealed!(ValidatedConstantBranch, ConstantBranchReceipt, source, proposed, function_index: function_index, branch: branch),
            )
        }
    }

    use std::sync::Arc;

    use optimization_core::OptimizationUnitIdentity;
    use selected_instructions::{
        ConstantBranchIdentity, SelectedInstructionId, SelectedInstructionPlan,
        SelectedInstructionPlanIdentity,
    };
    use semantic_vocabulary::WorkScheduleIdentity;

    pub(crate) use rewrite::fold_selected_constant_branch;
    pub(crate) use validation::{measured_steps, surface_sizes};

    validated_transformed_receipt!(ValidatedConstantBranch, ConstantBranchReceipt);

    rewrite_receipt!(ConstantBranchReceipt {
        function_index: usize,
        branch: SelectedInstructionId,
    });

    rewrite_receipt_identity!(
        constant_branch_identity,
        ConstantBranchIdentity,
        ConstantBranchReceipt,
        b"omega.terminal-constant-branch-fold.v1\0",
        "constant-branch",
        branch,
    );

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ConstantBranchError {
        SourceMismatch,
        UnsupportedInstruction,
        UnsupportedUse,
        UnsupportedProducer,
        UnsupportedLiteral,
        ConstraintMismatch,
        EffectSurfaceMismatch,
        WorkBudgetExceeded,
        IdentityOverflow,
        ReplayMismatch,
    }

    failed_display!(ConstantBranchError, "invalid constant branch fold");
}

pub(crate) mod dead_compare {
    pub(crate) mod admission {
        use std::collections::BTreeSet;

        use optimization_core::OptimizationWorkBudget;
        use register_environment::ValidatedTargetRegisterEnvironment;
        use register_model::{RegisterOperandAccess, RegisterUnitId};
        use selected_instructions::{
            SelectedBlockId, SelectedCasePayloadTransport, SelectedFunction, SelectedInstruction,
            SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan,
            SelectedValueTransport, VirtualRegisterId,
        };

        use super::DeadCompareError;
        use crate::ValidatedSelectedAnalysis;
        use crate::rewrites::block_edges::{
            block_instructions, edge_surface, member_position, terminator_successors,
        };
        use crate::rewrites::condition_state::{
            EventSite, adjacency, backward_cone, entry_index, instruction_at, reaching_events,
        };
        use crate::rewrites::plan_function;

        pub(crate) struct Admission<'source> {
            pub function: &'source SelectedFunction,
            pub block_index: usize,
            pub block: SelectedBlockId,
            pub compare_index: usize,
        }

        pub(crate) fn compare_operand_arity(kind: SelectedInstructionKind) -> Option<usize> {
            match kind {
                SelectedInstructionKind::CompareI64 => Some(2),
                SelectedInstructionKind::CompareI64Immediate { .. }
                | SelectedInstructionKind::CompareI64Zero => Some(1),
                _ => None,
            }
        }

        pub(crate) fn canonical_compare_operands(
            instruction: &SelectedInstruction,
        ) -> Option<Vec<VirtualRegisterId>> {
            if compare_operand_arity(instruction.kind)? != instruction.operands.len()
                || !instruction.implicit_uses.is_empty()
                || instruction
                    .implicit_defs
                    .iter()
                    .chain(instruction.clobbers.iter())
                    .next()
                    .is_none()
                || instruction
                    .operands
                    .iter()
                    .enumerate()
                    .any(|(position, operand)| {
                        operand.operand != position as u16
                            || operand.access != RegisterOperandAccess::Use
                            || operand.fixed_view.is_some()
                            || operand.tied_to.is_some()
                            || operand.early_clobber
                    })
            {
                return None;
            }
            Some(
                instruction
                    .operands
                    .iter()
                    .map(|operand| operand.virtual_register)
                    .collect(),
            )
        }

        pub(crate) fn audit_stable_paths<E: Copy>(
            function: &SelectedFunction,
            predecessors: &[Vec<usize>],
            block_index: usize,
            compare_index: usize,
            unit_sites: &BTreeSet<EventSite>,
            registers: &BTreeSet<VirtualRegisterId>,
            violation: E,
        ) -> Result<(), E> {
            let compare_site = (block_index, compare_index);
            let mut visited = BTreeSet::new();
            let mut frontier = vec![compare_site];
            while let Some(site @ (block, position)) = frontier.pop() {
                if !visited.insert(site) {
                    continue;
                }
                if site != compare_site && unit_sites.contains(&site) {
                    continue;
                }
                let instruction = instruction_at(function, site);
                if instruction.operands.iter().any(|operand| {
                    registers.contains(&operand.virtual_register)
                        && (operand.access != RegisterOperandAccess::Use || operand.early_clobber)
                }) {
                    return Err(violation);
                }
                if position > 0 {
                    frontier.push((block, position - 1));
                    continue;
                }
                for &predecessor in &predecessors[block] {
                    for successor in terminator_successors(&function.blocks[predecessor].terminator)
                    {
                        if successor.block != function.blocks[block].id {
                            continue;
                        }
                        for binding in &successor.bindings {
                            if let SelectedValueTransport::Registers { parameter, .. } =
                                binding.transport
                                && registers.contains(&parameter)
                            {
                                return Err(violation);
                            }
                        }
                        if let Some(case) = &successor.structural_case {
                            for payload in &case.payloads {
                                let parameter = match payload.transport {
                                    SelectedCasePayloadTransport::Unused => continue,
                                    SelectedCasePayloadTransport::Unmaterialized { parameter }
                                    | SelectedCasePayloadTransport::Registers {
                                        parameter, ..
                                    } => parameter,
                                };
                                if registers.contains(&parameter) {
                                    return Err(violation);
                                }
                            }
                        }
                    }
                    frontier.push((predecessor, function.blocks[predecessor].instructions.len()));
                }
            }
            Ok(())
        }

        pub(crate) struct ShadowRefusals<E> {
            pub source: E,
            pub unsupported_instruction: E,
            pub constraint: E,
            pub unsupported_use: E,
            pub overflow: E,
            pub budget: E,
        }

        pub(crate) struct ShadowVictim<V> {
            pub registers: Vec<VirtualRegisterId>,
            pub read: V,
        }

        type ShadowAgreement<V> = fn(
            &SelectedFunction,
            &SelectedInstruction,
            &ShadowVictim<V>,
            &SelectedInstruction,
            &mut BTreeSet<VirtualRegisterId>,
        ) -> bool;

        pub(crate) fn admit_shadowed_compare<'source, E: Copy, V>(
            source: &'source impl ValidatedSelectedAnalysis,
            function_index: usize,
            compare: SelectedInstructionId,
            environment: &'source ValidatedTargetRegisterEnvironment,
            budget: OptimizationWorkBudget,
            refusals: ShadowRefusals<E>,
            victim_read: impl FnOnce(&SelectedInstruction) -> Option<ShadowVictim<V>>,
            agreement: ShadowAgreement<V>,
            per_unit: impl FnOnce(&crate::rewrites::ConePropagation) -> Result<usize, E>,
        ) -> Result<Admission<'source>, E> {
            let (plan, function) =
                plan_function(source, function_index, environment, || refusals.source)?;
            let (block_index, compare_index) =
                member_position(function, compare).ok_or(refusals.source)?;
            let block = &function.blocks[block_index];
            let compare_instruction = &block.instructions[compare_index];
            let victim =
                victim_read(compare_instruction).ok_or(refusals.unsupported_instruction)?;
            let arity = compare_instruction.operands.len();
            agree_compare_removal_row(
                function,
                compare,
                compare_instruction,
                arity,
                environment,
                || refusals.constraint,
                || refusals.unsupported_instruction,
            )?;
            let (successors, predecessors) = adjacency(function);
            let entry = entry_index(function).ok_or(refusals.source)?;
            let cone = backward_cone(&predecessors, block_index);
            let units: BTreeSet<RegisterUnitId> = compare_instruction
                .implicit_defs
                .iter()
                .chain(compare_instruction.clobbers.iter())
                .copied()
                .collect();
            for &unit in &units {
                let defined = compare_instruction.implicit_defs.contains(&unit);
                let sites = reaching_events(
                    function,
                    entry,
                    &successors,
                    &cone,
                    block_index,
                    compare_index,
                    unit,
                )
                .map_err(|_| refusals.unsupported_use)?;
                let mut unit_sites = BTreeSet::new();
                let mut audited = BTreeSet::new();
                for site in sites {
                    let event = instruction_at(function, site);
                    if defined {
                        if !event.implicit_defs.contains(&unit)
                            || event.clobbers.contains(&unit)
                            || !agreement(
                                function,
                                compare_instruction,
                                &victim,
                                event,
                                &mut audited,
                            )
                        {
                            return Err(refusals.unsupported_use);
                        }
                        unit_sites.insert(site);
                    } else if !event.clobbers.contains(&unit) || event.implicit_defs.contains(&unit)
                    {
                        return Err(refusals.unsupported_use);
                    }
                }
                if defined {
                    audit_stable_paths(
                        function,
                        &predecessors,
                        block_index,
                        compare_index,
                        &unit_sites,
                        &audited,
                        refusals.unsupported_use,
                    )?;
                }
            }
            let steps =
                compare_removal_steps(plan, function, &successors, units.len(), per_unit, || {
                    refusals.overflow
                })?;
            crate::rewrites::charge_steps(steps, budget, || refusals.overflow, || refusals.budget)?;
            Ok(Admission {
                function,
                block_index,
                block: block.id,
                compare_index,
            })
        }

        pub(crate) fn compare_removal_steps<E>(
            plan: &SelectedInstructionPlan,
            function: &SelectedFunction,
            successors: &[Vec<usize>],
            units: usize,
            per_unit: impl FnOnce(&crate::rewrites::ConePropagation) -> Result<usize, E>,
            overflow: impl Fn() -> E + Copy,
        ) -> Result<usize, E> {
            let cone = crate::rewrites::cone_propagation(function, successors, overflow)?;
            let function_scan = cone.function_scan;
            let edge_count = cone.edge_count;
            let per_unit = per_unit(&cone)?;
            let walk_setup = cone.walk_setup_base;
            let transport_rows = function
                .blocks
                .iter()
                .try_fold(0usize, |total, block| {
                    terminator_successors(&block.terminator)
                        .iter()
                        .try_fold(total, |total, successor| {
                            total.checked_add(edge_surface(successor))
                        })
                })
                .ok_or_else(overflow)?;
            let audit = units
                .checked_mul(
                    function_scan
                        .checked_add(edge_count)
                        .and_then(|total| total.checked_add(transport_rows))
                        .ok_or_else(overflow)?,
                )
                .ok_or_else(overflow)?;
            crate::rewrites::rewrite_scan(plan, function)
                .and_then(|total| {
                    total
                        .checked_add(function_scan)?
                        .checked_add(function.virtual_registers.len())?
                        .checked_add(function.calls.len())?
                        .checked_add(function.memory_accesses.len())
                })
                .and_then(|total| {
                    total
                        .checked_add(walk_setup)?
                        .checked_add(units.checked_mul(per_unit)?)?
                        .checked_add(audit)
                })
                .ok_or_else(overflow)
        }

        fn audit_dead_unit(
            function: &SelectedFunction,
            block_index: usize,
            compare_index: usize,
            unit: RegisterUnitId,
        ) -> Result<(), DeadCompareError> {
            let mut visited = BTreeSet::new();
            let mut frontier = vec![(block_index, compare_index + 1)];
            while let Some((current, start)) = frontier.pop() {
                if !visited.insert((current, start)) {
                    continue;
                }
                let block = &function.blocks[current];
                let mut killed = false;
                for instruction in block_instructions(block).skip(start) {
                    if instruction.implicit_uses.contains(&unit) {
                        return Err(DeadCompareError::UnsupportedUse);
                    }
                    if instruction.implicit_defs.contains(&unit)
                        || instruction.clobbers.contains(&unit)
                    {
                        killed = true;
                        break;
                    }
                }
                if killed {
                    continue;
                }
                for successor in terminator_successors(&block.terminator) {
                    let target = function
                        .block_index(successor.block)
                        .ok_or(DeadCompareError::UnsupportedUse)?;
                    frontier.push((target, 0));
                }
            }
            Ok(())
        }

        pub(crate) fn admit<'source>(
            source: &'source impl ValidatedSelectedAnalysis,
            function_index: usize,
            compare: SelectedInstructionId,
            environment: &'source ValidatedTargetRegisterEnvironment,
            budget: OptimizationWorkBudget,
        ) -> Result<Admission<'source>, DeadCompareError> {
            let (plan, function) = plan_function(source, function_index, environment, || {
                DeadCompareError::SourceMismatch
            })?;
            let (block_index, compare_index) =
                member_position(function, compare).ok_or(DeadCompareError::SourceMismatch)?;
            let block = &function.blocks[block_index];
            let compare_instruction = &block.instructions[compare_index];
            let arity = canonical_compare_operands(compare_instruction)
                .ok_or(DeadCompareError::UnsupportedInstruction)?
                .len();
            agree_compare_removal_row(
                function,
                compare,
                compare_instruction,
                arity,
                environment,
                || DeadCompareError::ConstraintMismatch,
                || DeadCompareError::UnsupportedInstruction,
            )?;
            let units: BTreeSet<RegisterUnitId> = compare_instruction
                .implicit_defs
                .iter()
                .chain(compare_instruction.clobbers.iter())
                .copied()
                .collect();
            for &unit in &units {
                audit_dead_unit(function, block_index, compare_index, unit)?;
            }
            let function_scan = crate::rewrites::function_scan(function)
                .ok_or(DeadCompareError::IdentityOverflow)?;
            let edge_count = function
                .blocks
                .iter()
                .map(|block| terminator_successors(&block.terminator).len())
                .sum::<usize>();
            let dead_audit = units
                .len()
                .checked_mul(
                    function_scan
                        .checked_mul(2)
                        .and_then(|scan| scan.checked_add(edge_count))
                        .ok_or(DeadCompareError::IdentityOverflow)?,
                )
                .ok_or(DeadCompareError::IdentityOverflow)?;
            let steps = crate::rewrites::rewrite_scan(plan, function)
                .and_then(|total| {
                    total
                        .checked_add(function_scan)?
                        .checked_add(function.virtual_registers.len())?
                        .checked_add(function.calls.len())?
                        .checked_add(function.memory_accesses.len())
                })
                .and_then(|total| total.checked_add(dead_audit))
                .ok_or(DeadCompareError::IdentityOverflow)?;
            crate::rewrites::charge_steps(
                steps,
                budget,
                || DeadCompareError::IdentityOverflow,
                || DeadCompareError::WorkBudgetExceeded,
            )?;
            Ok(Admission {
                function,
                block_index,
                block: block.id,
                compare_index,
            })
        }

        pub(crate) fn apply<E>(
            admitted: &Admission<'_>,
            function: &mut SelectedFunction,
            map_error: impl Fn(DeadCompareError) -> E,
        ) -> Result<(), E> {
            let block = function
                .blocks
                .get_mut(admitted.block_index)
                .ok_or_else(|| map_error(DeadCompareError::SourceMismatch))?;
            if block.id != admitted.block {
                return Err(map_error(DeadCompareError::SourceMismatch));
            }
            function.boundary_settlements = super::super::shifted_boundary_settlements(
                admitted.function,
                admitted.block,
                admitted.compare_index,
                || DeadCompareError::SourceMismatch,
                || DeadCompareError::IdentityOverflow,
            )
            .map_err(map_error)?;
            block.instructions.remove(admitted.compare_index);
            Ok(())
        }

        pub(crate) fn remove_compare<'source, S, E, V>(
            source: &'source S,
            function_index: usize,
            compare: SelectedInstructionId,
            environment: &'source ValidatedTargetRegisterEnvironment,
            budget: OptimizationWorkBudget,
            admit: fn(
                &'source S,
                usize,
                SelectedInstructionId,
                &'source ValidatedTargetRegisterEnvironment,
                OptimizationWorkBudget,
            ) -> Result<Admission<'source>, E>,
            map_error: impl Fn(DeadCompareError) -> E,
            seal: impl Fn(
                &'source S,
                usize,
                SelectedInstructionId,
                SelectedInstructionPlan,
            ) -> Result<V, E>,
        ) -> Result<V, E>
        where
            S: ValidatedSelectedAnalysis + ?Sized,
        {
            let admitted = admit(source, function_index, compare, environment, budget)?;
            let mut transformed = source.selected_plan().clone();
            apply(
                &admitted,
                &mut transformed.functions[function_index],
                map_error,
            )?;
            seal(source, function_index, compare, transformed)
        }

        pub(crate) fn agree_compare_removal_row<E>(
            function: &SelectedFunction,
            compare: SelectedInstructionId,
            compare_instruction: &SelectedInstruction,
            arity: usize,
            environment: &ValidatedTargetRegisterEnvironment,
            constraint_mismatch: impl Fn() -> E,
            unsupported: impl Fn() -> E,
        ) -> Result<(), E> {
            let row = environment
                .constraint(compare_instruction.constraint)
                .ok_or_else(&constraint_mismatch)?;
            if row.operands.len() != arity
                || row.implicit_uses != compare_instruction.implicit_uses
                || row.implicit_defs != compare_instruction.implicit_defs
                || row.clobbers != compare_instruction.clobbers
            {
                return Err(constraint_mismatch());
            }
            for (position, operand) in compare_instruction.operands.iter().enumerate() {
                let register = function
                    .virtual_register(operand.virtual_register)
                    .ok_or_else(&constraint_mismatch)?;
                if row.operands[position].operand != operand.operand
                    || row.operands[position].access != RegisterOperandAccess::Use
                    || row.operands[position].class != register.class
                {
                    return Err(constraint_mismatch());
                }
            }
            if function.call(compare).is_some() || function.memory_access(compare).is_some() {
                return Err(unsupported());
            }
            Ok(())
        }
    }
    pub(crate) mod equivalent {
        pub(crate) mod admission {
            use std::collections::BTreeSet;

            use optimization_core::OptimizationWorkBudget;
            use register_environment::ValidatedTargetRegisterEnvironment;
            use selected_instructions::{
                SelectedFunction, SelectedInstruction, SelectedInstructionId,
                SelectedInstructionKind, VirtualRegisterId,
            };

            use super::super::admission::{
                Admission, ShadowRefusals, ShadowVictim, admit_shadowed_compare,
                canonical_compare_operands,
            };
            use super::EquivalentCompareError;
            use crate::ValidatedSelectedAnalysis;
            use crate::rewrites::condition_state::{immediate_bits, materialized_bits};

            #[derive(Clone, Copy, PartialEq, Eq)]
            enum CompareRight {
                Register(VirtualRegisterId),
                Literal(u64),
            }

            fn compare_sides(
                instruction: &SelectedInstruction,
            ) -> Option<(VirtualRegisterId, CompareRight)> {
                let operands = canonical_compare_operands(instruction)?;
                match instruction.kind {
                    SelectedInstructionKind::CompareI64 => {
                        Some((operands[0], CompareRight::Register(operands[1])))
                    }
                    SelectedInstructionKind::CompareI64Immediate { immediate } => Some((
                        operands[0],
                        CompareRight::Literal(immediate_bits(immediate)?),
                    )),
                    SelectedInstructionKind::CompareI64Zero => {
                        Some((operands[0], CompareRight::Literal(0)))
                    }
                    _ => None,
                }
            }

            fn right_bits(function: &SelectedFunction, side: CompareRight) -> Option<u64> {
                match side {
                    CompareRight::Register(register) => materialized_bits(function, register).ok(),
                    CompareRight::Literal(bits) => Some(bits),
                }
            }

            fn sides_equal(
                function: &SelectedFunction,
                victim: (VirtualRegisterId, CompareRight),
                shadow: (VirtualRegisterId, CompareRight),
                audited: &mut BTreeSet<VirtualRegisterId>,
            ) -> bool {
                let left_equal = if victim.0 == shadow.0 {
                    audited.insert(victim.0);
                    true
                } else {
                    matches!(
                        (
                            materialized_bits(function, victim.0).ok(),
                            materialized_bits(function, shadow.0).ok()
                        ),
                        (Some(victim_bits), Some(shadow_bits)) if victim_bits == shadow_bits
                    )
                };
                if !left_equal {
                    return false;
                }
                match (victim.1, shadow.1) {
                    (
                        CompareRight::Register(victim_register),
                        CompareRight::Register(shadow_register),
                    ) if victim_register == shadow_register => {
                        audited.insert(victim_register);
                        true
                    }
                    (victim_right, shadow_right) => matches!(
                        (
                            right_bits(function, victim_right),
                            right_bits(function, shadow_right)
                        ),
                        (Some(victim_bits), Some(shadow_bits)) if victim_bits == shadow_bits
                    ),
                }
            }

            pub(crate) fn admit<'source>(
                source: &'source impl ValidatedSelectedAnalysis,
                function_index: usize,
                compare: SelectedInstructionId,
                environment: &'source ValidatedTargetRegisterEnvironment,
                budget: OptimizationWorkBudget,
            ) -> Result<Admission<'source>, EquivalentCompareError> {
                admit_shadowed_compare(
                    source,
                    function_index,
                    compare,
                    environment,
                    budget,
                    ShadowRefusals {
                        source: EquivalentCompareError::SourceMismatch,
                        unsupported_instruction: EquivalentCompareError::UnsupportedInstruction,
                        constraint: EquivalentCompareError::ConstraintMismatch,
                        unsupported_use: EquivalentCompareError::UnsupportedUse,
                        overflow: EquivalentCompareError::IdentityOverflow,
                        budget: EquivalentCompareError::WorkBudgetExceeded,
                    },
                    |instruction| {
                        let sides = compare_sides(instruction)?;
                        let registers = canonical_compare_operands(instruction)?;
                        Some(ShadowVictim {
                            registers,
                            read: sides,
                        })
                    },
                    |function, _victim_instruction, victim, event, audited| {
                        let Some(site_sides) = compare_sides(event) else {
                            return false;
                        };
                        sides_equal(function, victim.read, site_sides, audited)
                    },
                    |cone| {
                        cone.per_unit_base
                            .checked_add(
                                cone.elements
                                    .checked_mul(cone.function_scan)
                                    .and_then(|product| product.checked_mul(4))
                                    .ok_or(EquivalentCompareError::IdentityOverflow)?,
                            )
                            .ok_or(EquivalentCompareError::IdentityOverflow)
                    },
                )
            }
        }
        pub(crate) mod rewrite {
            use optimization_core::OptimizationWorkBudget;
            use register_environment::ValidatedTargetRegisterEnvironment;
            use selected_instructions::SelectedInstructionId;

            use super::super::DeadCompareError;
            use super::{EquivalentCompareError, ValidatedEquivalentCompare, admission};
            use crate::ValidatedSelectedAnalysis;

            pub(crate) fn remove_equivalent_compare(
                source: &impl ValidatedSelectedAnalysis,
                function_index: usize,
                compare: SelectedInstructionId,
                environment: &ValidatedTargetRegisterEnvironment,
                budget: OptimizationWorkBudget,
            ) -> Result<ValidatedEquivalentCompare, EquivalentCompareError> {
                super::super::admission::remove_compare(
                    source,
                    function_index,
                    compare,
                    environment,
                    budget,
                    admission::admit,
                    |error| match error {
                        DeadCompareError::IdentityOverflow => {
                            EquivalentCompareError::IdentityOverflow
                        }
                        _ => EquivalentCompareError::SourceMismatch,
                    },
                    super::validation::seal_equivalent_compare,
                )
            }
        }
        pub(crate) mod validation {
            use selected_instructions::{
                SelectedFunction, SelectedInstructionId, SelectedInstructionPlan,
            };

            use super::super::admission::compare_removal_steps;
            use super::{
                EquivalentCompareError, EquivalentCompareReceipt, ValidatedEquivalentCompare,
            };
            use crate::ValidatedSelectedAnalysis;
            use crate::rewrites::sealed;

            pub(crate) fn measured_steps(
                plan: &SelectedInstructionPlan,
                function: &SelectedFunction,
                successors: &[Vec<usize>],
                units: usize,
            ) -> Result<u64, EquivalentCompareError> {
                let steps = compare_removal_steps(
                    plan,
                    function,
                    successors,
                    units,
                    |cone| {
                        cone.per_unit_base
                            .checked_add(
                                cone.elements
                                    .checked_mul(cone.function_scan)
                                    .and_then(|product| product.checked_mul(4))
                                    .ok_or(EquivalentCompareError::IdentityOverflow)?,
                            )
                            .ok_or(EquivalentCompareError::IdentityOverflow)
                    },
                    || EquivalentCompareError::IdentityOverflow,
                )?;
                u64::try_from(steps).map_err(|_| EquivalentCompareError::IdentityOverflow)
            }

            pub(crate) fn seal_equivalent_compare(
                source: &impl ValidatedSelectedAnalysis,
                function_index: usize,
                compare: SelectedInstructionId,
                proposed: SelectedInstructionPlan,
            ) -> Result<ValidatedEquivalentCompare, EquivalentCompareError> {
                Ok(
                    sealed!(ValidatedEquivalentCompare, EquivalentCompareReceipt, source, proposed, function_index: function_index, compare: compare),
                )
            }
        }

        use std::sync::Arc;

        use optimization_core::OptimizationUnitIdentity;
        use selected_instructions::{
            EquivalentCompareIdentity, SelectedInstructionId, SelectedInstructionPlan,
            SelectedInstructionPlanIdentity,
        };
        use semantic_vocabulary::WorkScheduleIdentity;

        pub(crate) use rewrite::remove_equivalent_compare;
        pub(crate) use validation::measured_steps;

        validated_transformed_receipt!(ValidatedEquivalentCompare, EquivalentCompareReceipt);

        rewrite_receipt!(EquivalentCompareReceipt {
            function_index: usize,
            compare: SelectedInstructionId,
        });

        rewrite_receipt_identity!(
            equivalent_compare_identity,
            EquivalentCompareIdentity,
            EquivalentCompareReceipt,
            b"omega.terminal-equivalent-compare-removal.v1\0",
            "equivalent-compare",
            compare,
        );

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum EquivalentCompareError {
            SourceMismatch,
            UnsupportedInstruction,
            UnsupportedUse,
            ConstraintMismatch,
            WorkBudgetExceeded,
            IdentityOverflow,
            ReplayMismatch,
        }

        failed_display!(EquivalentCompareError, "invalid equivalent-compare removal");
    }
    pub(crate) mod redundant {
        pub(crate) mod admission {
            use optimization_core::OptimizationWorkBudget;
            use register_environment::ValidatedTargetRegisterEnvironment;
            use selected_instructions::SelectedInstructionId;

            use super::super::admission::{
                Admission, ShadowRefusals, ShadowVictim, admit_shadowed_compare,
                canonical_compare_operands,
            };
            use super::RedundantCompareError;
            use crate::ValidatedSelectedAnalysis;

            pub(crate) fn admit<'source>(
                source: &'source impl ValidatedSelectedAnalysis,
                function_index: usize,
                compare: SelectedInstructionId,
                environment: &'source ValidatedTargetRegisterEnvironment,
                budget: OptimizationWorkBudget,
            ) -> Result<Admission<'source>, RedundantCompareError> {
                admit_shadowed_compare(
                    source,
                    function_index,
                    compare,
                    environment,
                    budget,
                    ShadowRefusals {
                        source: RedundantCompareError::SourceMismatch,
                        unsupported_instruction: RedundantCompareError::UnsupportedInstruction,
                        constraint: RedundantCompareError::ConstraintMismatch,
                        unsupported_use: RedundantCompareError::UnsupportedUse,
                        overflow: RedundantCompareError::IdentityOverflow,
                        budget: RedundantCompareError::WorkBudgetExceeded,
                    },
                    |instruction| {
                        canonical_compare_operands(instruction).map(|registers| ShadowVictim {
                            registers,
                            read: (),
                        })
                    },
                    |_function, victim_instruction, victim, event, audited| {
                        let Some(site_registers) = canonical_compare_operands(event) else {
                            return false;
                        };
                        if event.kind != victim_instruction.kind
                            || site_registers != victim.registers
                        {
                            return false;
                        }
                        audited.extend(site_registers.iter().copied());
                        true
                    },
                    |cone| Ok(cone.per_unit_base),
                )
            }
        }
        pub(crate) mod rewrite {
            use optimization_core::OptimizationWorkBudget;
            use register_environment::ValidatedTargetRegisterEnvironment;
            use selected_instructions::SelectedInstructionId;

            use super::super::DeadCompareError;
            use super::{RedundantCompareError, ValidatedRedundantCompare, admission};
            use crate::ValidatedSelectedAnalysis;

            pub(crate) fn remove_redundant_compare(
                source: &impl ValidatedSelectedAnalysis,
                function_index: usize,
                compare: SelectedInstructionId,
                environment: &ValidatedTargetRegisterEnvironment,
                budget: OptimizationWorkBudget,
            ) -> Result<ValidatedRedundantCompare, RedundantCompareError> {
                super::super::admission::remove_compare(
                    source,
                    function_index,
                    compare,
                    environment,
                    budget,
                    admission::admit,
                    |error| match error {
                        DeadCompareError::IdentityOverflow => {
                            RedundantCompareError::IdentityOverflow
                        }
                        _ => RedundantCompareError::SourceMismatch,
                    },
                    super::validation::seal_redundant_compare,
                )
            }
        }
        pub(crate) mod validation {
            use selected_instructions::{
                SelectedFunction, SelectedInstructionId, SelectedInstructionPlan,
            };

            use super::super::admission::compare_removal_steps;
            use super::{
                RedundantCompareError, RedundantCompareReceipt, ValidatedRedundantCompare,
            };
            use crate::ValidatedSelectedAnalysis;
            use crate::rewrites::sealed;

            pub(crate) fn measured_steps(
                plan: &SelectedInstructionPlan,
                function: &SelectedFunction,
                successors: &[Vec<usize>],
                units: usize,
            ) -> Result<u64, RedundantCompareError> {
                let steps = compare_removal_steps(
                    plan,
                    function,
                    successors,
                    units,
                    |cone| Ok(cone.per_unit_base),
                    || RedundantCompareError::IdentityOverflow,
                )?;
                u64::try_from(steps).map_err(|_| RedundantCompareError::IdentityOverflow)
            }

            pub(crate) fn seal_redundant_compare(
                source: &impl ValidatedSelectedAnalysis,
                function_index: usize,
                compare: SelectedInstructionId,
                proposed: SelectedInstructionPlan,
            ) -> Result<ValidatedRedundantCompare, RedundantCompareError> {
                Ok(
                    sealed!(ValidatedRedundantCompare, RedundantCompareReceipt, source, proposed, function_index: function_index, compare: compare),
                )
            }
        }

        use std::sync::Arc;

        use optimization_core::OptimizationUnitIdentity;
        use selected_instructions::{
            RedundantCompareIdentity, SelectedInstructionId, SelectedInstructionPlan,
            SelectedInstructionPlanIdentity,
        };
        use semantic_vocabulary::WorkScheduleIdentity;

        pub(crate) use rewrite::remove_redundant_compare;
        pub(crate) use validation::measured_steps;

        validated_transformed_receipt!(ValidatedRedundantCompare, RedundantCompareReceipt);

        rewrite_receipt!(RedundantCompareReceipt {
            function_index: usize,
            compare: SelectedInstructionId,
        });

        rewrite_receipt_identity!(
            redundant_compare_identity,
            RedundantCompareIdentity,
            RedundantCompareReceipt,
            b"omega.terminal-redundant-compare-removal.v1\0",
            "redundant-compare",
            compare,
        );

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum RedundantCompareError {
            SourceMismatch,
            UnsupportedInstruction,
            UnsupportedUse,
            ConstraintMismatch,
            WorkBudgetExceeded,
            IdentityOverflow,
            ReplayMismatch,
        }

        failed_display!(RedundantCompareError, "invalid redundant-compare removal");
    }
    pub(crate) mod rewrite {
        use optimization_core::OptimizationWorkBudget;
        use register_environment::ValidatedTargetRegisterEnvironment;
        use selected_instructions::SelectedInstructionId;

        use super::{DeadCompareError, ValidatedDeadCompare, admission};
        use crate::ValidatedSelectedAnalysis;

        pub(crate) fn remove_dead_compare(
            source: &impl ValidatedSelectedAnalysis,
            function_index: usize,
            compare: SelectedInstructionId,
            environment: &ValidatedTargetRegisterEnvironment,
            budget: OptimizationWorkBudget,
        ) -> Result<ValidatedDeadCompare, DeadCompareError> {
            let admitted = admission::admit(source, function_index, compare, environment, budget)?;
            let mut transformed = source.selected_plan().clone();
            admission::apply(
                &admitted,
                &mut transformed.functions[function_index],
                |error| error,
            )?;
            super::validation::seal_dead_compare(source, function_index, compare, transformed)
        }
    }
    pub(crate) mod validation {
        use selected_instructions::{
            SelectedFunction, SelectedInstructionId, SelectedInstructionPlan,
        };

        use super::{DeadCompareError, DeadCompareReceipt, ValidatedDeadCompare};
        use crate::ValidatedSelectedAnalysis;
        use crate::rewrites::block_edges::terminator_successors;
        use crate::rewrites::sealed;

        pub(crate) fn measured_steps(
            plan: &SelectedInstructionPlan,
            function: &SelectedFunction,
            units: usize,
        ) -> Result<u64, DeadCompareError> {
            let function_scan = crate::rewrites::function_scan(function)
                .ok_or(DeadCompareError::IdentityOverflow)?;
            let edge_count = function
                .blocks
                .iter()
                .map(|block| terminator_successors(&block.terminator).len())
                .sum::<usize>();
            let dead_audit = units
                .checked_mul(
                    function_scan
                        .checked_mul(2)
                        .and_then(|scan| scan.checked_add(edge_count))
                        .ok_or(DeadCompareError::IdentityOverflow)?,
                )
                .ok_or(DeadCompareError::IdentityOverflow)?;
            let steps = crate::rewrites::rewrite_scan(plan, function)
                .and_then(|total| {
                    total
                        .checked_add(function_scan)?
                        .checked_add(function.virtual_registers.len())?
                        .checked_add(function.calls.len())?
                        .checked_add(function.memory_accesses.len())
                })
                .and_then(|total| total.checked_add(dead_audit))
                .ok_or(DeadCompareError::IdentityOverflow)?;
            u64::try_from(steps).map_err(|_| DeadCompareError::IdentityOverflow)
        }

        pub(crate) fn seal_dead_compare(
            source: &impl ValidatedSelectedAnalysis,
            function_index: usize,
            compare: SelectedInstructionId,
            proposed: SelectedInstructionPlan,
        ) -> Result<ValidatedDeadCompare, DeadCompareError> {
            Ok(
                sealed!(ValidatedDeadCompare, DeadCompareReceipt, source, proposed, function_index: function_index, compare: compare),
            )
        }
    }

    use std::sync::Arc;

    use optimization_core::OptimizationUnitIdentity;
    use selected_instructions::{
        DeadCompareIdentity, SelectedInstructionId, SelectedInstructionPlan,
        SelectedInstructionPlanIdentity,
    };
    use semantic_vocabulary::WorkScheduleIdentity;

    pub(crate) use equivalent::{EquivalentCompareError, ValidatedEquivalentCompare};
    pub(crate) use equivalent::{
        measured_steps as equivalent_compare_measured_steps, remove_equivalent_compare,
    };
    pub(crate) use redundant::{RedundantCompareError, ValidatedRedundantCompare};
    pub(crate) use redundant::{
        measured_steps as redundant_compare_measured_steps, remove_redundant_compare,
    };
    pub(crate) use rewrite::remove_dead_compare;
    pub(crate) use validation::measured_steps;

    validated_transformed_receipt!(ValidatedDeadCompare, DeadCompareReceipt);

    rewrite_receipt!(DeadCompareReceipt {
        function_index: usize,
        compare: SelectedInstructionId,
    });

    rewrite_receipt_identity!(
        dead_compare_identity,
        DeadCompareIdentity,
        DeadCompareReceipt,
        b"omega.terminal-dead-compare-removal.v1\0",
        "dead-compare",
        compare,
    );

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum DeadCompareError {
        SourceMismatch,
        UnsupportedInstruction,
        UnsupportedUse,
        ConstraintMismatch,
        WorkBudgetExceeded,
        IdentityOverflow,
        ReplayMismatch,
    }

    failed_display!(DeadCompareError, "invalid dead-compare removal");
}
