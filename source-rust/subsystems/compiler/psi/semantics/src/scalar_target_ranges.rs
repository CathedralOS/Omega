//! Ground arithmetic that can be checked once the selected scalar values exist.

mod calls;
pub use calls::{
    array_extent_literal_conditions, array_extent_literal_conditions_in, layout_call_conditions,
    layout_call_conditions_in,
};
mod guards;
mod storage;

use bignum::BigInt;
use numerics::integer_bounds::IntegerBoundStep;
use semantic_vocabulary::{
    BlockId, IntegerValue, ObligationId, Proposition, ScalarConditionRole, ScalarTerm, ScalarType,
    ValueId,
};
use std::cell::OnceCell;

use crate::descriptors::row_source::DescriptorRowSource;
use terminal_psi::{
    Block, Operation, OperationKind, ProofDeclarationId, ScalarConditionSubject, ScalarConditions,
    ScalarRangeCondition, ScalarRangeObservation, ScalarRangeRequirement, TargetMachineCallable,
    TerminalMachine, TerminalModule,
};

use crate::{
    CanonicalScalarGoal, MachineScalarIndex, ValueTypes, proof_bearing_scalar_leaf_semantics,
};

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

pub fn target_scalar_ranges(
    module: &TerminalModule,
    machine: &TerminalMachine,
    types: &dyn ValueTypes,
) -> Result<Vec<TargetScalarRange>, &'static str> {
    target_scalar_ranges_from(&DescriptorRowSource::new(module), machine, types)
}

/// `target_scalar_ranges`, reading the machine's borrowed-storage rows from
/// the module's shared analyses, so ranging every machine adapts each body
/// once.
pub fn target_scalar_ranges_in(
    analyses: &crate::ModuleAnalyses<'_>,
    machine: &TerminalMachine,
    types: &dyn ValueTypes,
) -> Result<Vec<TargetScalarRange>, &'static str> {
    target_scalar_ranges_from(&DescriptorRowSource::of(analyses), machine, types)
}

fn target_scalar_ranges_from(
    rows: &DescriptorRowSource<'_, '_>,
    machine: &TerminalMachine,
    types: &dyn ValueTypes,
) -> Result<Vec<TargetScalarRange>, &'static str> {
    let module = rows.module();
    if module.scalars.scalar_applications.declarations().is_empty() {
        return Ok(Vec::new());
    }
    let observes = |predicate: &Proposition| {
        let mut found = false;
        predicate
            .visit_scalar_terms(|term| found |= matches!(term, ScalarTerm::Application { .. }));
        found
    };
    // An expression can only acquire its first target input from a read, a
    // checked entry clause or a callee's normal-result clause.
    if !machine.requires.iter().any(&observes)
        && !machine.operations().any(|operation| {
            matches!(operation.kind, OperationKind::ReadScalarApplication { .. })
                || operation
                    .kind
                    .direct_callee()
                    .and_then(|id| module.machine(id))
                    .is_some_and(|callee| callee.ensures.iter().any(|c| observes(&c.proposition)))
        })
    {
        return Ok(Vec::new());
    }
    let owner = module
        .scalars
        .scalar_applications
        .proof_scope
        .callable_declaration(TargetMachineCallable::Machine(machine.id));
    let mut ranges = Vec::new();
    let index = MachineScalarIndex::new(machine);
    let storage = OnceCell::new();
    let dominators = OnceCell::new();
    let branches = OnceCell::new();
    for block in &machine.blocks {
        let guard_steps = OnceCell::new();
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
                rows,
                index: &index,
                storage: &storage,
                dominators: &dominators,
                active: Default::default(),
                steps: Vec::new(),
                remaining: 4096,
            };
            if ground.term(expression, (block.id, position), 0).is_none()
                || !ground.steps.iter().any(|step| {
                    matches!(
                        step,
                        Step::Observation(ScalarRangeObservation::Application(_))
                    )
                })
            {
                continue;
            }
            // An integer carrier includes zero. Multiplication by each necessary
            // branch indicator leaves the operation's range on reachable paths.
            let guards = guard_steps.get_or_init(|| {
                ground
                    .guard_indicators(block.id, branches.get_or_init(|| guards::branches(machine)))
            });
            ground.steps.extend_from_slice(guards);
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

type Point = (BlockId, usize);

struct GroundRange<'a, 'machine, 'm> {
    module: &'machine TerminalModule,
    /// Where the machine's borrowed-storage rows come from when a read
    /// needs them.
    rows: &'a DescriptorRowSource<'a, 'm>,
    index: &'a MachineScalarIndex<'machine>,
    storage: &'a OnceCell<Option<storage::StorageValues<'a>>>,
    dominators: &'a OnceCell<Option<crate::control_graph::DominatorTree>>,
    active: Vec<ValueId>,
    steps: Vec<Step>,
    remaining: usize,
}

impl<'machine> GroundRange<'_, 'machine, '_> {
    fn visit(&mut self, depth: usize) -> Option<()> {
        self.remaining = self.remaining.checked_sub(1)?;
        (depth < 64).then_some(())
    }

    fn term(&mut self, term: &ScalarTerm, before: Point, depth: usize) -> Option<()> {
        self.visit(depth)?;
        let step = match term {
            ScalarTerm::Value { id, scalar_type } => {
                return self.value(*id, *scalar_type, before, depth + 1);
            }
            ScalarTerm::Application {
                application,
                scalar_type,
            } => {
                if self
                    .module
                    .scalars
                    .scalar_applications
                    .declaration(*application)?
                    .scalar_type()
                    != Some(*scalar_type)
                {
                    return None;
                }
                Step::Observation(ScalarRangeObservation::Application(*application))
            }
            ScalarTerm::IntegerWiden { operand, .. }
            | ScalarTerm::IntegerExactCast { operand, .. } => {
                self.term(operand, before, depth + 1)?;
                return Some(());
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
            _ => Step::Observation(ScalarRangeObservation::Literal(mathematical_integer(
                term.integer_value()?.1,
            ))),
        };
        self.steps.push(step);
        Some(())
    }

    fn value(&mut self, id: ValueId, ty: ScalarType, before: Point, depth: usize) -> Option<()> {
        self.visit(depth)?;
        if self.active.contains(&id) {
            return None;
        }
        self.active.push(id);
        let result = self.value_origin(id, ty, before, depth);
        self.active.pop();
        result
    }

    fn definition(&self, id: ValueId, before: Point) -> Option<(&'machine Block, usize)> {
        let mut sites = self.index.definition_sites(id);
        let (block, position) = sites.next()?;
        if sites.next().is_some() {
            return None;
        }
        let (owner, limit) = before;
        let dominates = if owner == block.id {
            position < limit
        } else {
            self.tree()?.dominates(block.id, owner)
        };
        dominates.then_some((block, position))
    }

    fn tree(&self) -> Option<&crate::control_graph::DominatorTree> {
        self.dominators
            .get_or_init(|| {
                let machine = self.index.machine();
                crate::control_graph::validate_control_graph(machine).ok()?;
                Some(crate::control_graph::dominators(machine))
            })
            .as_ref()
    }

    fn value_origin(
        &mut self,
        id: ValueId,
        ty: ScalarType,
        before: Point,
        depth: usize,
    ) -> Option<()> {
        if self.index.value_type(id)? != ty {
            return None;
        }
        if self.index.definition_sites(id).next().is_some() {
            let (block, position) = self.definition(id, before)?;
            return self.operation(&block.operations[position], block, position, ty, depth);
        }
        if let Some(incoming) = self.index.incoming_values(id) {
            if incoming.is_empty() {
                return None;
            }
            for (ordinal, (source, value)) in incoming.into_iter().enumerate() {
                self.value(value, ty, (source.id, source.operations.len()), depth + 1)?;
                if ordinal != 0 {
                    self.steps.push(Step::Union);
                }
            }
            return Some(());
        }
        let machine = self.index.machine();
        let formal = machine
            .parameters
            .iter()
            .find(|parameter| parameter.id == id)?;
        let name = ScalarTerm::value(formal.id, formal.scalar_type);
        machine.requires.iter().find_map(|predicate| {
            self.equated(
                predicate,
                &name,
                &|term| Some(term.clone()),
                before,
                depth + 1,
            )
        })
    }

    fn operation(
        &mut self,
        operation: &Operation,
        block: &Block,
        position: usize,
        ty: ScalarType,
        depth: usize,
    ) -> Option<()> {
        let before = (block.id, position);
        match &operation.kind {
            OperationKind::IntegerConstant { value } => {
                self.steps
                    .push(Step::Observation(ScalarRangeObservation::Literal(
                        mathematical_integer(*value),
                    )));
                Some(())
            }
            OperationKind::ReadScalarApplication { application } => self.term(
                &ScalarTerm::Application {
                    application: *application,
                    scalar_type: ty,
                },
                before,
                depth + 1,
            ),
            OperationKind::PrimitiveScalarRead { .. }
            | OperationKind::ScalarStructuralField { .. } => {
                let machine = self.index.machine();
                let storage = self
                    .storage
                    .get_or_init(|| storage::StorageValues::new(self.rows, machine))
                    .as_ref()?;
                let at = (block.id, u32::try_from(position).ok()?);
                let sources = storage.sources(
                    self.module,
                    machine.id,
                    at,
                    operation.result.scalar_ref()?.id,
                    ty,
                )?;
                for (ordinal, ((block, position), term)) in sources.into_iter().enumerate() {
                    self.term(&term, (block, position as usize), depth + 1)?;
                    if ordinal != 0 {
                        self.steps.push(Step::Union);
                    }
                }
                Some(())
            }
            _ if operation.module_invocation().is_some() => {
                let invocation = operation.module_invocation()?;
                if invocation.required_contract.is_some()
                    || operation.static_reach_binding.is_some()
                {
                    return None;
                }
                let callee = self.module.machine(invocation.callee)?;
                let result = callee.result.scalar()?;
                if result.scalar_type != ty || callee.parameters.len() != invocation.arguments.len()
                {
                    return None;
                }
                let rename = |expression: &ScalarTerm| {
                    let mut complete = true;
                    let mut expression = expression.clone();
                    expression.remap_value_ids(|id| {
                        let actual = callee
                            .parameters
                            .iter()
                            .position(|formal| formal.id == id)
                            .and_then(|index| invocation.arguments.get(index));
                        if actual.is_none() {
                            complete = false;
                        }
                        actual.copied().unwrap_or(id)
                    });
                    complete.then_some(expression)
                };
                let name = ScalarTerm::value(result.id, ty);
                callee.ensures.iter().find_map(|clause| {
                    self.equated(&clause.proposition, &name, &rename, before, depth + 1)
                })
            }
            _ => {
                let semantics =
                    proof_bearing_scalar_leaf_semantics(operation, self.index).ok()??;
                self.term(semantics.denotation(), before, depth + 1)
            }
        }
    }

    // A checked input or normal-result clause can name a ground expression.
    // A disjunction gives a bound only when every alternative supplies one.
    fn equated(
        &mut self,
        predicate: &Proposition,
        name: &ScalarTerm,
        rename: &impl Fn(&ScalarTerm) -> Option<ScalarTerm>,
        before: Point,
        depth: usize,
    ) -> Option<()> {
        self.visit(depth)?;
        let start = self.steps.len();
        let found = match predicate {
            Proposition::Equal(lhs, rhs) if lhs == name && lhs != rhs => {
                self.term(&rename(rhs)?, before, depth + 1)
            }
            Proposition::Equal(lhs, rhs) if rhs == name && lhs != rhs => {
                self.term(&rename(lhs)?, before, depth + 1)
            }
            Proposition::Conjunction(parts) => parts
                .iter()
                .find_map(|part| self.equated(part, name, rename, before, depth + 1)),
            Proposition::Disjunction(parts) if !parts.is_empty() => {
                parts.iter().enumerate().try_for_each(|(ordinal, part)| {
                    self.equated(part, name, rename, before, depth + 1)?;
                    if ordinal != 0 {
                        self.steps.push(Step::Union);
                    }
                    Some(())
                })
            }
            _ => None,
        };
        if found.is_none() {
            self.steps.truncate(start);
        }
        found
    }
}

fn mathematical_integer(value: IntegerValue) -> BigInt {
    match value {
        IntegerValue::Unsigned(value) => BigInt::from_u128(value),
        IntegerValue::Signed(value) => BigInt::from_i128(value),
    }
}
