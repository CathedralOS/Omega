//! Ground arithmetic that can be checked once the selected scalar values exist.

use numerics::{bignum::BigInt, integer_bounds::IntegerBoundStep};
use semantic_vocabulary::{
    IntegerValue, ObligationId, ScalarConditionRole, ScalarTerm, StructuralTypeId, ValueId,
};
use terminal_psi::{
    ArrayExtent, Operation, OperationKind, ProofDeclarationId, ScalarApplication,
    ScalarConditionSubject, ScalarConditions, ScalarRangeCondition, ScalarRangeObservation,
    ScalarRangeRequirement, StructuralTypeShape, TargetMachineCallable, TerminalMachine,
    TerminalModule,
};

use crate::{CanonicalScalarGoal, ValueTypes, proof_bearing_scalar_leaf_semantics};

type Step = IntegerBoundStep<ScalarRangeObservation>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetScalarRange {
    pub obligation: ObligationId,
    pub owner: ProofDeclarationId,
    requirement: ScalarRangeRequirement,
    steps: Vec<Step>,
}

impl TargetScalarRange {
    pub fn retain(&self, conditions: &mut ScalarConditions) -> Result<(), &'static str> {
        conditions.retain_range(
            ScalarConditionSubject::Declaration(self.owner),
            ScalarConditionRole::Prerequisite,
            self.requirement.clone(),
            self.steps.clone(),
        )
    }

    pub fn matches(&self, conditions: &ScalarConditions, actual: &ScalarRangeCondition) -> bool {
        matches!(actual, ScalarRangeCondition::Range {
            subject: ScalarConditionSubject::Declaration(owner),
            role: ScalarConditionRole::Prerequisite,
            requirement,
            ..
        } if *owner == self.owner && *requirement == self.requirement)
            && conditions.steps(actual) == Ok(self.steps.as_slice())
    }
}

/// Returns the layouts whose by-value storage contains a scalar application
/// for a fixed-array extent. A measurement of any of these needs that value.
pub fn application_sized_storage(
    module: &TerminalModule,
) -> Result<Vec<StructuralTypeId>, &'static str> {
    let roots = module
        .structural_types
        .iter()
        .filter_map(|declaration| {
            matches!(
                declaration.shape,
                StructuralTypeShape::FixedArray {
                    length: ArrayExtent::Application(_),
                    ..
                }
            )
            .then_some(declaration.id)
        })
        .collect::<Vec<_>>();
    if roots.is_empty() {
        return Ok(Vec::new());
    }
    crate::storage::type_graph::StorageTypeGraph::new(module.structural_types.iter())
        .and_then(|graph| graph.storage_dependents(roots))
        .map_err(|_| "scalar application storage has malformed ownership")
}

pub fn target_scalar_ranges(
    module: &TerminalModule,
    machine: &TerminalMachine,
    types: &dyn ValueTypes,
    application_sized_storage: &[StructuralTypeId],
) -> Result<Vec<TargetScalarRange>, &'static str> {
    if module.scalar_applications.declarations().is_empty() {
        return Ok(Vec::new());
    }
    let owner = module
        .scalar_applications
        .proof_scope
        .callable_declaration(TargetMachineCallable::Machine(machine.id));
    let mut ranges = Vec::new();
    for block in &machine.blocks {
        let mut definitions: Vec<_> = block
            .operations
            .iter()
            .enumerate()
            .filter_map(|(position, op)| op.result.scalar_ref().map(|result| (result.id, position)))
            .collect();
        definitions.sort_unstable_by_key(|(id, _)| *id);
        for (position, operation) in block.operations.iter().enumerate() {
            let Some(semantics) = proof_bearing_scalar_leaf_semantics(operation, types)
                .map_err(|_| "target arithmetic lost its canonical scalar operation")?
            else {
                continue;
            };
            let CanonicalScalarGoal::ExactArithmeticRepresentable {
                integer_type,
                expression,
            } = semantics.canonical_goal()
            else {
                continue;
            };
            let (Some(minimum), Some(maximum)) =
                (integer_type.minimum_value(), integer_type.maximum_value())
            else {
                continue;
            };
            let mut ground = GroundRange {
                module,
                operations: &block.operations,
                definitions: &definitions,
                types,
                application_sized_storage,
                steps: Vec::new(),
                observes_evaluation: false,
            };
            if ground.term(expression, position, 0).is_none() || !ground.observes_evaluation {
                continue;
            }
            ranges.push(TargetScalarRange {
                obligation: semantics.obligation(),
                owner: owner.ok_or("target arithmetic has no exact declaration owner")?,
                requirement: ScalarRangeRequirement::Fixed {
                    minimum: mathematical_integer(minimum),
                    maximum: mathematical_integer(maximum),
                },
                steps: ground.steps,
            });
        }
    }
    ranges.sort_unstable_by_key(|range| range.obligation);
    if ranges
        .windows(2)
        .any(|pair| pair[0].obligation == pair[1].obligation)
    {
        return Err("target arithmetic reuses an obligation identity");
    }
    Ok(ranges)
}

struct GroundRange<'a> {
    module: &'a TerminalModule,
    operations: &'a [Operation],
    definitions: &'a [(ValueId, usize)],
    types: &'a dyn ValueTypes,
    application_sized_storage: &'a [StructuralTypeId],
    steps: Vec<Step>,
    observes_evaluation: bool,
}

impl GroundRange<'_> {
    fn term(&mut self, term: &ScalarTerm, before: usize, depth: usize) -> Option<()> {
        // Ground dependencies use exact earlier SSA definitions. Joins and
        // runtime inputs stay with the ordinary proof checker.
        if depth >= 64 || self.steps.len() >= 4096 {
            return None;
        }
        let step = match term {
            ScalarTerm::Value { id, scalar_type } => {
                let (_, position) = self.definitions[self
                    .definitions
                    .binary_search_by_key(id, |(id, _)| *id)
                    .ok()?];
                if position >= before {
                    return None;
                }
                let operation = &self.operations[position];
                if operation.result.scalar_ref()?.scalar_type != *scalar_type {
                    return None;
                }
                match operation.kind {
                    OperationKind::IntegerConstant { value } => Step::Observation(
                        ScalarRangeObservation::Literal(mathematical_integer(value)),
                    ),
                    OperationKind::ReadScalarApplication { application } => {
                        let declaration =
                            self.module.scalar_applications.declaration(application)?;
                        if declaration.scalar_type() != Some(*scalar_type) {
                            return None;
                        }
                        // A direct layout read already participates in ordinary
                        // arithmetic proofs. A measurement of application-sized
                        // storage also needs the exact value of that extent.
                        self.observes_evaluation |= match declaration {
                            ScalarApplication::Evaluate { .. } => true,
                            ScalarApplication::LayoutObservation { subject, .. } => self
                                .application_sized_storage
                                .binary_search(subject)
                                .is_ok(),
                            ScalarApplication::Missing => return None,
                        };
                        Step::Observation(ScalarRangeObservation::Application(application))
                    }
                    _ => {
                        let semantics =
                            proof_bearing_scalar_leaf_semantics(operation, self.types).ok()??;
                        return self.term(semantics.denotation(), position, depth + 1);
                    }
                }
            }
            ScalarTerm::ExactIntegerAdd { left, right, .. }
            | ScalarTerm::ExactIntegerSubtract { left, right, .. }
            | ScalarTerm::ExactIntegerMultiply { left, right, .. } => {
                self.term(left, before, depth + 1)?;
                self.term(right, before, depth + 1)?;
                match term {
                    ScalarTerm::ExactIntegerAdd { .. } => Step::Add,
                    ScalarTerm::ExactIntegerSubtract { .. } => Step::Subtract,
                    _ => Step::Multiply,
                }
            }
            _ => return None,
        };
        self.steps.push(step);
        Some(())
    }
}

fn mathematical_integer(value: IntegerValue) -> BigInt {
    match value {
        IntegerValue::Unsigned(value) => BigInt::from_u128(value),
        IntegerValue::Signed(value) => BigInt::from_i128(value),
    }
}
