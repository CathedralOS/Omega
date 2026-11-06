//! Evaluate arguments in source parameter order before entering another machine.
//! A copied reference still keeps the data it points to alive.

use super::{ComposedCompletion, Query, Transfers};
use crate::execution::terminal_unit::storage_transfers::{Destination, Transfer};
use checked_trees::{
    CheckedScalarExpressionRole, CheckedStructuralAccess, CheckedStructuralControlSuccessorPlan,
    CheckedStructuralControlTransferPlan, CheckedStructuralControlTransferSourcePlan as Source,
    CheckedStructuralScalarArgumentSourcePlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
};
use language_semantics::Multiplicity;

enum Operand<'a> {
    Structural(
        &'a CheckedStructuralControlTransferPlan,
        &'a CheckedUnitStructuralParameterPlan,
    ),
    Scalar(&'a checked_trees::CheckedStructuralScalarArgumentPlan),
}

impl Query<'_> {
    pub(super) fn successor_transfers(
        &self,
        graph: ComposedCompletion<'_>,
        successor: &CheckedStructuralControlSuccessorPlan,
    ) -> Result<Transfers, &'static str> {
        let mut signatures = graph
            .signatures
            .iter()
            .filter(|row| row.state == successor.target_state);
        let signature = signatures
            .next()
            .ok_or("completion cleanup: the successor has no prepared signature")?;
        if signatures.next().is_some() {
            return Err("completion cleanup: the successor signature is ambiguous");
        }
        let mut operands =
            Vec::with_capacity(successor.transfers.len() + successor.scalar_arguments.len());
        for transfer in &successor.transfers {
            let parameter = signature
                .structural
                .get(transfer.target_parameter_index as usize)
                .ok_or("completion cleanup: a structural argument has no target parameter")?;
            operands.push((parameter.position, Operand::Structural(transfer, parameter)));
        }
        for argument in &successor.scalar_arguments {
            let parameter = signature
                .scalar
                .get(argument.target_scalar_parameter_index as usize)
                .ok_or("completion cleanup: a scalar argument has no target parameter")?;
            if argument.argument_ordinal != parameter.source_position {
                return Err(
                    "completion cleanup: a scalar argument has a different authored position",
                );
            }
            operands.push((parameter.source_position, Operand::Scalar(argument)));
        }
        operands.sort_by_key(|(position, _)| *position);
        if operands.windows(2).any(|rows| rows[0].0 == rows[1].0) {
            return Err("completion cleanup: two successor operands share a position");
        }
        let mut output = Transfers::default();
        for (_, operand) in operands {
            match operand {
                Operand::Scalar(argument) => {
                    if argument.source == CheckedStructuralScalarArgumentSourcePlan::Expression {
                        let role = CheckedScalarExpressionRole::TransitionArgument {
                            argument_ordinal: argument.argument_ordinal,
                        };
                        output.append(self.scalar_role(
                            graph.state,
                            successor.statement_ordinal,
                            role,
                        )?);
                    }
                }
                Operand::Structural(transfer, parameter) => {
                    output.append(self.successor_argument(transfer, parameter)?);
                    self.retain_successor_parents(transfer, &mut output);
                }
            }
        }
        Ok(output)
    }

    fn successor_argument(
        &self,
        transfer: &CheckedStructuralControlTransferPlan,
        parameter: &CheckedUnitStructuralParameterPlan,
    ) -> Result<Transfers, &'static str> {
        let argument = |source| CheckedUnitStructuralArgumentPlan {
            source,
            path: Vec::new(),
            projection_evaluations: Default::default(),
            type_identity: parameter.type_identity,
            access: parameter.access,
        };
        match &transfer.source {
            Source::Parameter { index } => self.successor_value(
                &argument(CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: *index,
                }),
                parameter,
            ),
            Source::StructuralResult { binding_ordinal } => self.successor_value(
                &argument(CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal: *binding_ordinal,
                }),
                parameter,
            ),
            Source::ProjectedValue { subject } | Source::ViewSubslice(subject) => {
                self.successor_value(subject, parameter)
            }
            Source::ProjectedLoan { subject, .. } => self
                .argument_computations(subject)
                .ok_or("completion cleanup: a successor loan has incomplete address operands"),
            // Copying this selected payload never transfers its enclosing owner.
            Source::CasePayload { subject, .. } => self
                .argument_computations(subject)
                .ok_or("completion cleanup: a case payload has incomplete address operands"),
            Source::ByteSequenceLiteral { .. } => Ok(Transfers::default()),
            Source::EstablishedValue {
                operands,
                establishment,
                presentation,
            } => {
                let mut output = self.operation_sequence(operands.iter())?;
                output.append(if presentation.access == CheckedStructuralAccess::Owned {
                    self.outgoing_operation(establishment)?
                } else {
                    self.operation_sequence(std::iter::once(establishment.as_ref()))?
                });
                output.append(self.successor_value(presentation, parameter)?);
                Ok(output)
            }
            Source::EvaluateNoNormal { .. } => {
                Err("completion cleanup: a stopped operand cannot arrive at a successor")
            }
        }
    }

    fn successor_value(
        &self,
        subject: &CheckedUnitStructuralArgumentPlan,
        parameter: &CheckedUnitStructuralParameterPlan,
    ) -> Result<Transfers, &'static str> {
        let mut output = self
            .argument_computations(subject)
            .ok_or("completion cleanup: a successor value has incomplete address operands")?;
        if parameter.access == CheckedStructuralAccess::Owned {
            output.append(self.outgoing_argument(
                subject,
                parameter.multiplicity == Multiplicity::Unrestricted,
            )?);
        }
        Ok(output)
    }

    fn retain_successor_parents(
        &self,
        transfer: &CheckedStructuralControlTransferPlan,
        output: &mut Transfers,
    ) {
        if let Some(binding_ordinal) = transfer.retained_loan_backing {
            self.retain_source(
                &CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal },
                output,
            );
        }
        for (dependent, parent) in &transfer.retained_loan_parents {
            self.retain_source(dependent, output);
            self.retain_source(parent, output);
        }
        for parent in &transfer.retained_operand_parents {
            self.retain_source(parent, output);
        }
    }

    fn retain_source(
        &self,
        source: &CheckedUnitStructuralArgumentSourcePlan,
        output: &mut Transfers,
    ) {
        // Parent retention concerns this registered home in either direction;
        // custody matching remains unchanged and cannot gain move authority.
        let same_home = self.same_source(source)
            || match (&self.source, source) {
                (
                    CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
                    CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal },
                ) => self.results.iter().any(|(result, root)| {
                    result.binding_ordinal == *binding_ordinal
                        && *root == facts::PlaceRoot::Symbol(*symbol)
                }),
                _ => false,
            };
        if same_home {
            output.possible.push(Transfer {
                source: Vec::new(),
                destination: Destination::RetainedBySuccessor,
            });
        }
    }
}
