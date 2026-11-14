/*
We represent the source-free D29 boundary-operator application coverage,
selected comparison occurrence proposals, and by-value opaque representation
custody retained beside one canonical Terminal artifact. Our root carrier,
`TerminalBoundaryApplicationCoverage`, pairs sorted `demands` with one-to-one
`realization` companions, derives reconstructible `coverage` references, and
attaches canonical `opaque_applications` custody while `TerminalIeeeFloatComparisonOccurrenceProposal`
and `TerminalIntegerComparisonOccurrenceProposal` replay selected comparison
plans against the Terminal operation roster.
*/
/*
We separate portable boundary-application evidence from source-arena state and
native realization authority so producers and consumers can verify exact D29
coverage and comparison plan custody without carrying AST handles, display
spellings, or mutable lowering tables across artifact boundaries. Keeping full
demand and realization rows alongside their derived references lets an artifact
distinguish an verified empty demand set from unrecorded coverage.
*/
/*
Checked Psi establishes typed operator applications, selected provider plans,
and opaque representation commitments; Terminal Psi (`terminal_psi`) supplies
the canonical operation and identity tables that our demands, realizations, and
comparison occurrence proposals validate against before `native-artifact` binds
them into physical execution evidence.
*/
#![forbid(unsafe_code)]

use effects::{CompilerNumericType, CompilerPrimitiveIntegerComparisonOperation};
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};

pub(crate) use optimization_core::digest_encoder;

pub mod coverage;
pub mod demands;
pub mod opaque_applications;
pub mod realization;
mod source_guards;

pub use coverage::OperatorApplicationCoverageRef;
pub use demands::{
    BoundaryApplication, BoundaryApplicationArgument, BoundaryOperatorRequirement,
    BoundaryTypeIdentity, NominalDeclarationIdentity, TerminalBoundaryApplicationDemand,
    TerminalBoundaryApplicationDemands,
};
pub use opaque_applications::{
    BoundaryOpaqueRepresentationApplication, BoundaryOpaqueRepresentationApplications,
};
pub use realization::{
    BoundaryApplicationRealization, BoundaryApplicationRealizationCompanion,
    TerminalBoundaryApplicationRealizations,
};

pub mod boundary_applications {
    pub use crate::*;
}

pub mod ieee_float_comparisons {
    pub use crate::TerminalIeeeFloatComparisonOccurrenceProposal;
}

pub mod integer_comparisons {
    pub use crate::TerminalIntegerComparisonOccurrenceProposal;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalBoundaryApplicationCoverage {
    demands: TerminalBoundaryApplicationDemands,
    realizations: TerminalBoundaryApplicationRealizations,
    references: Vec<OperatorApplicationCoverageRef>,
    opaque_applications: opaque_applications::BoundaryOpaqueRepresentationApplications,
}

impl TerminalBoundaryApplicationCoverage {
    pub fn new(
        demands: TerminalBoundaryApplicationDemands,
        realizations: TerminalBoundaryApplicationRealizations,
    ) -> Result<Self, &'static str> {
        let references = realizations.coverage_references(&demands)?;
        Ok(Self {
            demands,
            realizations,
            references,
            opaque_applications:
                opaque_applications::BoundaryOpaqueRepresentationApplications::EMPTY,
        })
    }

    pub fn with_opaque_applications(
        mut self,
        applications: opaque_applications::BoundaryOpaqueRepresentationApplications,
    ) -> Self {
        self.opaque_applications = applications;
        self
    }

    pub fn validate_for_terminal(
        &self,
        terminal: terminal_psi::TerminalPsiIdentity,
    ) -> Result<(), &'static str> {
        self.demands.validate_for_terminal(terminal)?;
        self.realizations.validate_for_demands(&self.demands)?;
        if self.references != self.realizations.coverage_references(&self.demands)? {
            return Err("boundary application coverage references do not replay");
        }
        Ok(())
    }

    pub const fn demands(&self) -> &TerminalBoundaryApplicationDemands {
        &self.demands
    }

    pub const fn realizations(&self) -> &TerminalBoundaryApplicationRealizations {
        &self.realizations
    }

    pub fn references(&self) -> &[OperatorApplicationCoverageRef] {
        &self.references
    }

    pub const fn opaque_applications(
        &self,
    ) -> &opaque_applications::BoundaryOpaqueRepresentationApplications {
        &self.opaque_applications
    }

    pub fn into_parts(
        self,
    ) -> (
        TerminalBoundaryApplicationDemands,
        TerminalBoundaryApplicationRealizations,
        opaque_applications::BoundaryOpaqueRepresentationApplications,
    ) {
        (self.demands, self.realizations, self.opaque_applications)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalIeeeFloatComparisonOccurrenceProposal {
    pub terminal_machine: semantic_vocabulary::MachineId,
    pub terminal_operation: semantic_vocabulary::OperationId,
    pub provider_plan_index: usize,
    pub provider_plan_commitment: effects::provider_plan::ProviderPlanDigest,
    pub comparison: semantic_vocabulary::IeeeFloatComparisonOperation,
    pub format: semantic_vocabulary::IeeeFloatFormat,
}

impl TerminalIeeeFloatComparisonOccurrenceProposal {
    pub fn execution_identity(&self) -> effects::CompilerIntrinsicExecutionIdentity {
        use effects::CompilerPrimitiveFloatBinaryOperation as Execution;
        use semantic_vocabulary::IeeeFloatComparisonOperation as Comparison;
        effects::CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary {
            operation: match self.comparison {
                Comparison::Equal => Execution::Equal,
                Comparison::NotEqual => Execution::NotEqual,
                Comparison::Less => Execution::Less,
                Comparison::LessOrEqual => Execution::LessOrEqual,
                Comparison::Greater => Execution::Greater,
                Comparison::GreaterOrEqual => Execution::GreaterOrEqual,
            },
            format: match self.format {
                semantic_vocabulary::IeeeFloatFormat::Binary32 => {
                    numerics::literals::FloatFormat::F32
                }
                semantic_vocabulary::IeeeFloatFormat::Binary64 => {
                    numerics::literals::FloatFormat::F64
                }
            },
        }
    }

    pub fn validate_roster(
        module: &terminal_psi::TerminalModule,
        plans: &[effects::provider_plan::ProviderPlan],
        occurrences: &[Self],
        execution_machines: &[semantic_vocabulary::MachineId],
    ) -> Result<(), &'static str> {
        validate_comparison_scope(module, execution_machines)?;
        let comparisons = module
            .machines
            .iter()
            .filter(|machine| execution_machines.binary_search(&machine.id).is_ok())
            .flat_map(|machine| {
                machine.blocks.iter().flat_map(move |block| {
                    block.operations.iter().filter_map(move |operation| {
                        matches!(
                            operation.kind,
                            terminal_psi::OperationKind::IeeeFloatCompare { .. }
                        )
                        .then_some((machine, operation))
                    })
                })
            })
            .collect::<Vec<_>>();
        if comparisons.len() != occurrences.len() {
            return Err(
                "Terminal proposal must retain every IEEE comparison occurrence exactly once",
            );
        }
        let mut seen = std::collections::BTreeSet::new();
        for occurrence in occurrences {
            if execution_machines
                .binary_search(&occurrence.terminal_machine)
                .is_err()
            {
                return Err("comparison occurrence belongs to another execution scope");
            }
            if !seen.insert((occurrence.terminal_machine, occurrence.terminal_operation)) {
                return Err("Terminal proposal repeats an IEEE comparison occurrence");
            }
            plans
                .get(occurrence.provider_plan_index)
                .ok_or("Terminal IEEE comparison names an absent selected plan")?;
            let matching = comparisons
                .iter()
                .filter(|(machine, operation)| {
                    machine.id == occurrence.terminal_machine
                        && operation.id == occurrence.terminal_operation
                })
                .collect::<Vec<_>>();
            let [(machine, operation)] = matching.as_slice() else {
                return Err("Terminal IEEE comparison does not name one exact operation");
            };
            let terminal_psi::OperationKind::IeeeFloatCompare {
                comparison,
                left,
                right,
            } = operation.kind
            else {
                return Err("Terminal IEEE comparison changed operation kind");
            };
            if comparison != occurrence.comparison
                || operation
                    .result
                    .scalar()
                    .is_none_or(|result| result.scalar_type != ScalarType::Boolean)
            {
                return Err("Terminal IEEE comparison changed operation or result type");
            }
            for operand in [left, right] {
                let mut declarations = machine
                    .parameters
                    .iter()
                    .copied()
                    .chain(machine.block_parameters().copied())
                    .chain(
                        machine
                            .operations()
                            .filter_map(|operation| operation.result.scalar()),
                    )
                    .filter(|declaration| declaration.id == operand);
                if declarations.next().is_none_or(|declaration| {
                    declaration.scalar_type != ScalarType::IeeeFloat(occurrence.format)
                }) || declarations.next().is_some()
                {
                    return Err("Terminal IEEE comparison changed operand format or identity");
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalIntegerComparisonOccurrenceProposal {
    pub terminal_machine: semantic_vocabulary::MachineId,
    pub terminal_operation: semantic_vocabulary::OperationId,
    pub provider_plan_index: usize,
    pub provider_plan_commitment: effects::provider_plan::ProviderPlanDigest,
    pub comparison: CompilerPrimitiveIntegerComparisonOperation,
    pub integer_type: CompilerNumericType,
}

impl TerminalIntegerComparisonOccurrenceProposal {
    pub fn execution_identity(&self) -> Option<effects::CompilerIntrinsicExecutionIdentity> {
        self.scalar_type()?;
        Some(
            effects::CompilerIntrinsicExecutionIdentity::PrimitiveIntegerComparison {
                operation: self.comparison,
                integer_type: self.integer_type,
            },
        )
    }

    fn scalar_type(&self) -> Option<ScalarType> {
        use CompilerNumericType::*;
        let (sign, bits) = match self.integer_type {
            I8 => (IntegerSign::Signed, 8),
            I16 => (IntegerSign::Signed, 16),
            I32 => (IntegerSign::Signed, 32),
            I64 => (IntegerSign::Signed, 64),
            U8 => (IntegerSign::Unsigned, 8),
            U16 => (IntegerSign::Unsigned, 16),
            U32 => (IntegerSign::Unsigned, 32),
            U64 => (IntegerSign::Unsigned, 64),
            F32 | F64 => return None,
        };
        IntegerType::new(sign, bits).ok().map(ScalarType::Integer)
    }

    pub fn validate_roster(
        module: &terminal_psi::TerminalModule,
        plans: &[effects::provider_plan::ProviderPlan],
        occurrences: &[Self],
        execution_machines: &[semantic_vocabulary::MachineId],
    ) -> Result<(), &'static str> {
        validate_comparison_scope(module, execution_machines)?;
        let mut seen = std::collections::BTreeSet::new();
        for occurrence in occurrences {
            if execution_machines
                .binary_search(&occurrence.terminal_machine)
                .is_err()
            {
                return Err("comparison occurrence belongs to another execution scope");
            }
            if !seen.insert((occurrence.terminal_machine, occurrence.terminal_operation)) {
                return Err("Terminal proposal repeats an integer comparison occurrence");
            }
            let plan = plans
                .get(occurrence.provider_plan_index)
                .ok_or("Terminal integer comparison names an absent selected plan")?;
            if plan.identity_digest() != occurrence.provider_plan_commitment {
                return Err("Terminal integer comparison changed its selected plan");
            }
            let mut matching = module
                .machines
                .iter()
                .flat_map(|machine| {
                    machine.blocks.iter().flat_map(move |block| {
                        block
                            .operations
                            .iter()
                            .map(move |operation| (machine, operation))
                    })
                })
                .filter(|(machine, operation)| {
                    machine.id == occurrence.terminal_machine
                        && operation.id == occurrence.terminal_operation
                });
            let Some((machine, operation)) = matching.next() else {
                return Err("Terminal integer comparison does not name one exact operation");
            };
            if matching.next().is_some() {
                return Err("Terminal integer comparison does not name one exact operation");
            }
            use CompilerPrimitiveIntegerComparisonOperation as Comparison;
            use terminal_psi::OperationKind;
            let (left, right) = match (occurrence.comparison, &operation.kind) {
                (
                    Comparison::Equal | Comparison::NotEqual,
                    OperationKind::IntegerEqual { left, right },
                )
                | (
                    Comparison::Less | Comparison::Greater,
                    OperationKind::IntegerLessThan { left, right },
                )
                | (
                    Comparison::LessOrEqual | Comparison::GreaterOrEqual,
                    OperationKind::IntegerLessOrEqual { left, right },
                ) => (*left, *right),
                _ => return Err("Terminal integer comparison changed operation kind"),
            };
            let Some(result) = operation.result.scalar() else {
                return Err("Terminal integer comparison has no scalar result");
            };
            if result.scalar_type != ScalarType::Boolean {
                return Err("Terminal integer comparison changed its result type");
            }
            if occurrence.comparison == Comparison::NotEqual {
                let negations = machine
                    .operations()
                    .filter(|candidate| {
                        matches!(
                            candidate.kind,
                            OperationKind::BooleanNot { operand } if operand == result.id
                        )
                    })
                    .count();
                if negations != 1 {
                    return Err("Terminal integer comparison changed its authored negation");
                }
            }
            let scalar_type = occurrence
                .scalar_type()
                .ok_or("Terminal integer comparison has a noninteger selected type")?;
            for operand in [left, right] {
                let mut declarations = machine
                    .parameters
                    .iter()
                    .copied()
                    .chain(machine.block_parameters().copied())
                    .chain(
                        machine
                            .operations()
                            .filter_map(|operation| operation.result.scalar()),
                    )
                    .filter(|declaration| declaration.id == operand);
                if declarations
                    .next()
                    .is_none_or(|declaration| declaration.scalar_type != scalar_type)
                    || declarations.next().is_some()
                {
                    return Err("Terminal integer comparison changed operand type or identity");
                }
            }
        }
        Ok(())
    }
}

// Module declarations need not follow scope order. Track each selected ID
// once so repeated module IDs cannot hide an unknown selected machine.
fn validate_comparison_scope(
    module: &terminal_psi::TerminalModule,
    execution_machines: &[semantic_vocabulary::MachineId],
) -> Result<(), &'static str> {
    let invalid_scope = "comparison scope has repeated, unordered or unknown machines";
    if execution_machines.windows(2).any(|rows| rows[0] >= rows[1]) {
        return Err(invalid_scope);
    }
    let mut remaining = execution_machines.len();
    if remaining == 0 {
        return Ok(());
    }
    let mut matched = vec![false; remaining];
    for machine in &module.machines {
        if let Ok(position) = execution_machines.binary_search(&machine.id)
            && !matched[position]
        {
            matched[position] = true;
            remaining -= 1;
            if remaining == 0 {
                return Ok(());
            }
        }
    }
    Err(invalid_scope)
}

#[cfg(test)]
mod comparison_tests {
    use super::{
        TerminalIeeeFloatComparisonOccurrenceProposal as FloatOccurrence,
        TerminalIntegerComparisonOccurrenceProposal as IntegerOccurrence,
    };
    use effects::CompilerPrimitiveIntegerComparisonOperation as Authored;
    use effects::provider_plan::ProviderPlan;
    use effects::{CompilerIntrinsicExecutionIdentity, CompilerNumericType};
    use semantic_vocabulary::{
        BlockId, ContractId, EdgeId, IntegerSign, IntegerType, MachineId, OperationId, ScalarType,
        ValueId,
    };
    use terminal_psi::{
        Block, Operation, OperationKind, OperationResult, TerminalMachine, TerminalMachineResult,
        TerminalModule, Terminator, ValueDeclaration,
    };

    fn comparison_module(operand_type: ScalarType, kind: OperationKind) -> TerminalModule {
        let machine = MachineId::new(1).unwrap();
        let block = BlockId::new(1).unwrap();
        TerminalModule {
            machines: vec![TerminalMachine {
                assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
                id: machine,
                attachment: None,
                nominal_cleanup_subject: None,
                ranked_scc: None,
                structural_places: Vec::new(),
                block_entry_claims: Vec::new(),
                loan_completions: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block,
                blocks: vec![Block {
                    id: block,
                    parameters: Vec::new(),
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    operations: vec![Operation {
                        static_reach_binding: None,
            required_contract: None,
                        suspension_crossing: None,
                        id: OperationId::new(1).unwrap(),
                        result: OperationResult::Scalar(value(3, ScalarType::Boolean)),
                        kind,
                    }],
                    terminator: Terminator::ReturnUnit {
                        edge: EdgeId::new(1).unwrap(),
                        cleanup_actions: Vec::new(),
                    },
                }],
                contract: terminal_psi::MachineContract {
                    selected_application: None,
                    declaration: None,
                    parameters: vec![value(1, operand_type), value(2, operand_type)],
                    parameter_order: vec![terminal_psi::BoundaryParameterKind::Scalar; 2],
                    structural_parameters: Vec::new(),
                    result: TerminalMachineResult::Unit,
                    entry_claims: Vec::new(),
                    declared_service_reach: None,
                    closed_reach_application: None,
                    published_synchronous_entries: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    content_entry_claims: Vec::new(),
                    ..terminal_psi::MachineContract::empty(ContractId::new(1).unwrap())
                },
            }],
            ..TerminalModule::for_entry(machine)
        }
    }

    fn value(identity: u64, scalar_type: ScalarType) -> ValueDeclaration {
        ValueDeclaration {
            id: ValueId::new(identity).unwrap(),
            scalar_type,
            qualifications: Default::default(),
        }
    }

    #[test]
    fn comparison_scopes_reject_bad_ids_without_requiring_module_order() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 64).unwrap();
        let mut module = comparison_module(
            ScalarType::Integer(integer_type),
            OperationKind::IntegerEqual {
                left: ValueId::new(1).unwrap(),
                right: ValueId::new(2).unwrap(),
            },
        );
        module.machines[0].blocks[0].operations.clear();
        let first = MachineId::new(1).unwrap();
        let second = MachineId::new(2).unwrap();
        let unknown = MachineId::new(3).unwrap();
        let mut other = module.machines[0].clone();
        other.id = second;
        module.machines.insert(0, other);
        let invalid = Err("comparison scope has repeated, unordered or unknown machines");
        for (scope, expected) in [
            (vec![], Ok(())),
            (vec![first, second], Ok(())),
            (vec![first, first], invalid),
            (vec![second, first], invalid),
            (vec![first, unknown], invalid),
        ] {
            assert_eq!(
                FloatOccurrence::validate_roster(&module, &[], &[], &scope),
                expected
            );
            assert_eq!(
                IntegerOccurrence::validate_roster(&module, &[], &[], &scope),
                expected
            );
        }
        // Repeated declarations cannot count twice toward a missing ID.
        module.machines[0].id = first;
        let scope = [first, second];
        assert_eq!(
            FloatOccurrence::validate_roster(&module, &[], &[], &scope),
            invalid
        );
        assert_eq!(
            IntegerOccurrence::validate_roster(&module, &[], &[], &scope),
            invalid
        );
    }

    #[test]
    fn integer_rows_preserve_builtin_omission_and_exact_selected_negation() {
        let plans = [ProviderPlan::default()];
        let integer_type = IntegerType::new(IntegerSign::Signed, 64).unwrap();
        let mut module = comparison_module(
            ScalarType::Integer(integer_type),
            OperationKind::IntegerEqual {
                left: ValueId::new(1).unwrap(),
                right: ValueId::new(2).unwrap(),
            },
        );
        let occurrence = IntegerOccurrence {
            terminal_machine: match module.entry {
                Some(terminal_psi::TargetMachineCallable::Machine(machine)) => machine,
                _ => unreachable!("comparison fixture enters a machine"),
            },
            terminal_operation: OperationId::new(1).unwrap(),
            provider_plan_index: 0,
            provider_plan_commitment: plans[0].identity_digest(),
            comparison: Authored::NotEqual,
            integer_type: CompilerNumericType::I64,
        };
        let execution_machines = [module.machines[0].id];
        assert_eq!(
            IntegerOccurrence::validate_roster(&module, &plans, &[], &execution_machines),
            Ok(())
        );
        assert_eq!(
            IntegerOccurrence::validate_roster(&module, &plans, &[occurrence], &execution_machines),
            Err("Terminal integer comparison changed its authored negation")
        );
        module.machines[0].blocks[0].operations.push(Operation {
            static_reach_binding: None,
            required_contract: None,
            suspension_crossing: None,
            id: OperationId::new(2).unwrap(),
            result: OperationResult::Scalar(value(4, ScalarType::Boolean)),
            kind: OperationKind::BooleanNot {
                operand: ValueId::new(3).unwrap(),
            },
        });
        assert_eq!(
            IntegerOccurrence::validate_roster(&module, &plans, &[occurrence], &execution_machines),
            Ok(())
        );
        assert_eq!(
            IntegerOccurrence::validate_roster(
                &module,
                &plans,
                &[occurrence, occurrence],
                &execution_machines
            ),
            Err("Terminal proposal repeats an integer comparison occurrence")
        );
        let mut duplicate_declaration = module.clone();
        duplicate_declaration.machines[0].blocks[0]
            .parameters
            .push(value(1, ScalarType::Integer(integer_type)));
        assert_eq!(
            IntegerOccurrence::validate_roster(
                &duplicate_declaration,
                &plans,
                &[occurrence],
                &execution_machines
            ),
            Err("Terminal integer comparison changed operand type or identity")
        );
        let wrong_type = IntegerOccurrence {
            integer_type: CompilerNumericType::U64,
            ..occurrence
        };
        assert_eq!(
            IntegerOccurrence::validate_roster(&module, &plans, &[wrong_type], &execution_machines),
            Err("Terminal integer comparison changed operand type or identity")
        );
    }

    #[test]
    fn integer_execution_identity_preserves_selected_meaning() {
        for comparison in Authored::ALL {
            let occurrence = IntegerOccurrence {
                terminal_machine: MachineId::new(1).unwrap(),
                terminal_operation: OperationId::new(1).unwrap(),
                provider_plan_index: 0,
                provider_plan_commitment: ProviderPlan::default().identity_digest(),
                comparison,
                integer_type: CompilerNumericType::I64,
            };
            assert_eq!(
                occurrence.execution_identity(),
                Some(
                    CompilerIntrinsicExecutionIdentity::PrimitiveIntegerComparison {
                        operation: comparison,
                        integer_type: CompilerNumericType::I64,
                    }
                )
            );
            for integer_type in [CompilerNumericType::F32, CompilerNumericType::F64] {
                assert_eq!(
                    IntegerOccurrence {
                        integer_type,
                        ..occurrence
                    }
                    .execution_identity(),
                    None
                );
            }
        }
    }
}
