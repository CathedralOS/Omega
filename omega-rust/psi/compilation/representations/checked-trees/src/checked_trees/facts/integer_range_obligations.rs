//! Target-dependent bounds of evaluated integer values, not source replay.
//!
//! Checking captures a value before its storage changes and composes its
//! mathematical bounds with the selected builtin operation. The temporary
//! postfix builder travels with ordinary flow facts. Publication moves those
//! steps into an arena; closure reads only admitted observations and these
//! captured bounds. It never looks up a mutable variable's initializer.
//! Every potentially overflowing child has its own obligation, so a later
//! cancellation cannot erase an earlier formation failure.

use arena::{Arena, HandleSpan};
use diagnostics::Diagnostic;
use numerics::bignum::BigInt;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum RangeStep {
    #[default]
    Missing,
    Bounds {
        low: BigInt,
        high: BigInt,
    },
    Observation {
        expression: ExpressionHandle,
        owner: SymbolHandle,
    },
    ConstantRecipe(typed_trees::constant_recipes::ConstantRecipeHandle),
    TargetChoice {
        family: SymbolHandle,
        arms: u32,
    },
    TargetArm {
        body: SymbolHandle,
        steps: u32,
    },
    Add,
    Subtract,
    Multiply,
    Union,
}

/// Invocation-owned construction storage. Equal snapshots compare by value,
/// not allocation identity, so flow fixed points do not manufacture changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegerRangeRecipe {
    steps: Vec<RangeStep>,
    target_dependent: bool,
}

impl IntegerRangeRecipe {
    pub fn bounds(low: BigInt, high: BigInt) -> Self {
        Self {
            steps: vec![RangeStep::Bounds { low, high }],
            target_dependent: false,
        }
    }

    /// The producer has admitted this observation's semantic identity. This
    /// is not permission to defer arbitrary calls or runtime place reads.
    pub fn observation(expression: ExpressionHandle, owner: SymbolHandle) -> Self {
        Self {
            steps: vec![RangeStep::Observation { expression, owner }],
            target_dependent: true,
        }
    }

    /// An established type position supplies this exact immutable extent.
    /// The selected-target closure must still evaluate its admitted recipe.
    pub fn constant_recipe(recipe: typed_trees::constant_recipes::ConstantRecipeHandle) -> Self {
        Self {
            steps: vec![RangeStep::ConstantRecipe(recipe)],
            target_dependent: true,
        }
    }

    /// A declaration family chooses one implementation in a target context.
    /// This is not a union of runtime paths: every arm retains its exact body.
    pub fn target_choice(family: SymbolHandle, arms: Vec<(SymbolHandle, Self)>) -> Option<Self> {
        if arms.is_empty() {
            return None;
        }
        let mut steps = vec![RangeStep::TargetChoice {
            family,
            arms: u32::try_from(arms.len()).ok()?,
        }];
        for (body, value) in arms {
            let count = u32::try_from(value.steps.len()).ok()?;
            if count == 0 {
                return None;
            }
            steps.push(RangeStep::TargetArm { body, steps: count });
            steps.extend(value.steps);
        }
        Some(Self {
            steps,
            target_dependent: true,
        })
    }

    pub fn observation_dependencies(
        &self,
    ) -> impl Iterator<Item = (ExpressionHandle, SymbolHandle)> + '_ {
        self.steps.iter().filter_map(|step| match step {
            RangeStep::Observation { expression, owner } => Some((*expression, *owner)),
            _ => None,
        })
    }

    pub(super) fn retain(self) -> RetainedIntegerRange {
        let mut steps = Arena::new();
        let recipe = steps.insert_many(self.steps);
        RetainedIntegerRange { steps, recipe }
    }

    pub fn is_target_dependent(&self) -> bool {
        self.target_dependent
    }

    pub fn binary(&self, operator: BinaryOperator, right: &Self) -> Option<Self> {
        let step = match operator {
            BinaryOperator::Add => RangeStep::Add,
            BinaryOperator::Subtract => RangeStep::Subtract,
            BinaryOperator::Multiply => RangeStep::Multiply,
            _ => return None,
        };
        Some(self.compose(right, step))
    }

    pub fn union(&self, right: &Self) -> Self {
        if self == right {
            return self.clone();
        }
        self.compose(right, RangeStep::Union)
    }

    fn compose(&self, right: &Self, step: RangeStep) -> Self {
        let mut steps = Vec::with_capacity(self.steps.len() + right.steps.len() + 1);
        steps.extend_from_slice(&self.steps);
        steps.extend_from_slice(&right.steps);
        steps.push(step);
        Self {
            steps,
            target_dependent: self.target_dependent || right.target_dependent,
        }
    }
}

/// Published value bounds share one arena owner instead of keeping a scratch
/// builder beside every proof consumer. Its leaves retain their exact inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RetainedIntegerRange {
    steps: Arena<RangeStep>,
    recipe: HandleSpan<RangeStep>,
}

impl RetainedIntegerRange {
    pub(super) fn observation_dependencies(
        &self,
    ) -> impl Iterator<Item = (ExpressionHandle, SymbolHandle)> + '_ {
        self.steps
            .span_or_empty(self.recipe)
            .iter()
            .filter_map(|step| match step {
                RangeStep::Observation { expression, owner } => Some((*expression, *owner)),
                _ => None,
            })
    }

    pub(super) fn dependencies_for_target(
        &self,
        program: &TypedTrees,
        target: &str,
    ) -> Result<IntegerRangeDependencies, String> {
        let steps = self
            .steps
            .span(self.recipe)
            .ok_or_else(|| "retained integer range has no recipe storage".to_owned())?;
        let mut dependencies = IntegerRangeDependencies::default();
        for step in SelectedSteps::new(steps, program, target) {
            match step? {
                RangeStep::Observation { expression, owner } => {
                    dependencies.observations.push((*expression, *owner))
                }
                RangeStep::ConstantRecipe(recipe) => dependencies.recipes.push(*recipe),
                _ => {}
            }
        }
        Ok(dependencies)
    }

    pub(super) fn closed_bounds(
        &self,
        program: &TypedTrees,
        target: &str,
    ) -> Result<(BigInt, BigInt), String> {
        self.steps
            .span(self.recipe)
            .ok_or_else(|| "retained integer range has no recipe storage".to_owned())
            .and_then(|steps| evaluate(steps, program, target))
    }
}

/// The endpoint meaning owned by the formation being retained. A selected
/// address model is not a fixed integer ceiling known during shared checking.
#[derive(Debug, Clone, PartialEq, Eq)]
enum RangeRequirement {
    Fixed { minimum: BigInt, maximum: BigInt },
    AddressModel,
}

/// Exact model dependency consumed while closing one address formation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegerRangeModel {
    pub carrier: semantic_vocabulary::IntegerType,
    pub semantics_version: u32,
    pub commitment: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegerRangeObligation {
    expression: ExpressionHandle,
    owner: SymbolHandle,
    recipe: RetainedIntegerRange,
    requirement: RangeRequirement,
    description: String,
    discharged_address_model: Option<IntegerRangeModel>,
    discharged_target: Option<typed_trees::name::Identifier>,
}

impl IntegerRangeObligation {
    pub fn retain(
        expression: ExpressionHandle,
        owner: SymbolHandle,
        recipe: IntegerRangeRecipe,
        minimum: BigInt,
        maximum: BigInt,
        description: String,
    ) -> Self {
        Self::with_requirement(
            expression,
            owner,
            recipe,
            RangeRequirement::Fixed { minimum, maximum },
            description,
        )
    }

    fn with_requirement(
        expression: ExpressionHandle,
        owner: SymbolHandle,
        recipe: IntegerRangeRecipe,
        requirement: RangeRequirement,
        description: String,
    ) -> Self {
        Self {
            expression,
            owner,
            recipe: recipe.retain(),
            requirement,
            description,
            discharged_address_model: None,
            discharged_target: None,
        }
    }

    /// Capture a closed anonymous value at its original formation occurrence.
    /// The producer has checked integrality, selected meaning and every normal
    /// path. This row is conditional; it grants no carrier membership before
    /// the selected target closes it.
    pub fn retain_address_fit(
        expression: ExpressionHandle,
        owner: SymbolHandle,
        low: BigInt,
        high: BigInt,
        description: String,
    ) -> Self {
        Self::with_requirement(
            expression,
            owner,
            IntegerRangeRecipe::bounds(low, high),
            RangeRequirement::AddressModel,
            description,
        )
    }

    pub fn needs_address_model(&self) -> bool {
        matches!(self.requirement, RangeRequirement::AddressModel)
    }

    pub fn discharged_address_model(&self) -> Option<&IntegerRangeModel> {
        self.discharged_address_model.as_ref()
    }

    pub fn owner(&self) -> SymbolHandle {
        self.owner
    }
    pub fn expression(&self) -> ExpressionHandle {
        self.expression
    }
    pub fn is_pending(&self) -> bool {
        self.discharged_target.is_none()
    }

    /// Exact observation leaves and meaning owners captured by the recipe,
    /// independent of the enclosing arithmetic expression or current storage.
    pub fn observation_dependencies(
        &self,
    ) -> impl Iterator<Item = (ExpressionHandle, SymbolHandle)> + '_ {
        self.recipe.observation_dependencies()
    }

    pub fn dependencies_for_target(
        &self,
        program: &TypedTrees,
        target: &str,
    ) -> Result<IntegerRangeDependencies, String> {
        self.recipe.dependencies_for_target(program, target)
    }

    pub fn discharge(
        &mut self,
        program: &TypedTrees,
        target: &str,
        address_model: Option<&IntegerRangeModel>,
    ) -> Result<(), Diagnostic> {
        let limits = match &self.requirement {
            RangeRequirement::Fixed { minimum, maximum } => {
                Some((minimum.clone(), maximum.clone()))
            }
            RangeRequirement::AddressModel => address_model
                .filter(|model| model.carrier.is_address())
                .map(|model| {
                    (
                        BigInt::zero(),
                        BigInt::from_u64(1)
                            .shl_bits(usize::from(model.carrier.bits()))
                            .sub(&BigInt::from_u64(1)),
                    )
                }),
        };
        let result = self
            .recipe
            .closed_bounds(program, target)
            .and_then(|(low, high)| {
                let (minimum, maximum) = limits
                    .as_ref()
                    .ok_or_else(|| "selected address model is not closed".to_owned())?;
                if minimum <= &low && &high <= maximum {
                    Ok(())
                } else {
                    Err(format!(
                        "bounds [{low}, {high}] lie outside [{minimum}, {maximum}]"
                    ))
                }
            });
        match result {
            Ok(()) => {
                self.discharged_target = Some(typed_trees::name::Identifier::generated(target));
                if self.needs_address_model() {
                    self.discharged_address_model = address_model.cloned();
                }
                Ok(())
            }
            Err(reason) => Err(Diagnostic::error(format!(
                "{}: exact arithmetic may overflow under target `{target}`: {reason}",
                self.description,
            ))
            .with_source_span(program.expression_table.source_span(self.expression))),
        }
    }
}

fn evaluate(
    steps: &[RangeStep],
    program: &TypedTrees,
    target: &str,
) -> Result<(BigInt, BigInt), String> {
    let mut values: Vec<(BigInt, BigInt)> = Vec::new();
    for step in SelectedSteps::new(steps, program, target) {
        match step? {
            RangeStep::Missing => return Err("missing integer bound step".to_owned()),
            RangeStep::Bounds { low, high } => {
                if low > high {
                    return Err("inverted captured integer bounds".to_owned());
                }
                values.push((low.clone(), high.clone()));
            }
            RangeStep::Observation { expression, owner } => {
                let value = program
                    .closed_integer_expression_value_in(*expression, *owner)
                    .ok_or_else(|| "target observation has no closed integer value".to_owned())?;
                values.push((value.clone(), value));
            }
            RangeStep::ConstantRecipe(recipe) => {
                let value = program
                    .closed_constant_recipe_value(*recipe)
                    .ok_or_else(|| {
                        "captured constant recipe has no closed integer value".to_owned()
                    })?;
                values.push((value.clone(), value.clone()));
            }
            operation => {
                let (right_low, right_high) = values
                    .pop()
                    .ok_or_else(|| "missing right integer bounds".to_owned())?;
                let (left_low, left_high) = values
                    .pop()
                    .ok_or_else(|| "missing left integer bounds".to_owned())?;
                values.push(match operation {
                    RangeStep::Add => (left_low.add(&right_low), left_high.add(&right_high)),
                    RangeStep::Subtract => (left_low.sub(&right_high), left_high.sub(&right_low)),
                    RangeStep::Multiply => {
                        let corners = [
                            left_low.mul(&right_low),
                            left_low.mul(&right_high),
                            left_high.mul(&right_low),
                            left_high.mul(&right_high),
                        ];
                        let low = corners
                            .iter()
                            .min()
                            .cloned()
                            .ok_or_else(|| "missing product corners".to_owned())?;
                        let high = corners
                            .iter()
                            .max()
                            .cloned()
                            .ok_or_else(|| "missing product corners".to_owned())?;
                        (low, high)
                    }
                    RangeStep::Union => (left_low.min(right_low), left_high.max(right_high)),
                    _ => return Err("invalid integer range operation".to_owned()),
                });
            }
        }
    }
    if values.len() != 1 {
        return Err("integer range recipe does not produce one value".to_owned());
    }
    values
        .pop()
        .ok_or_else(|| "integer range recipe has no result".to_owned())
}

/// Exact inputs of the chosen value branch. Source retention may inventory
/// all arms; selected execution and receiving conditions use this same walk.
#[derive(Default)]
pub struct IntegerRangeDependencies {
    pub observations: Vec<(ExpressionHandle, SymbolHandle)>,
    pub recipes: Vec<typed_trees::constant_recipes::ConstantRecipeHandle>,
}

struct SelectedSteps<'a> {
    program: &'a TypedTrees,
    target: &'a str,
    pending: Vec<&'a [RangeStep]>,
}

impl<'a> SelectedSteps<'a> {
    fn new(steps: &'a [RangeStep], program: &'a TypedTrees, target: &'a str) -> Self {
        Self {
            program,
            target,
            pending: vec![steps],
        }
    }
}

impl<'a> Iterator for SelectedSteps<'a> {
    type Item = Result<&'a RangeStep, String>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let frame = self.pending.pop()?;
            let Some((step, rest)) = frame.split_first() else {
                continue;
            };
            match step {
                RangeStep::TargetChoice { family, arms } => {
                    match selected_arm(self.program, self.target, *family, *arms, rest) {
                        Ok((selected, remaining)) => {
                            self.pending.push(remaining);
                            self.pending.push(selected);
                        }
                        Err(reason) => {
                            self.pending.clear();
                            return Some(Err(reason));
                        }
                    }
                }
                RangeStep::TargetArm { .. } => {
                    self.pending.clear();
                    return Some(Err("target-body arm has no owning family choice".to_owned()));
                }
                _ => {
                    self.pending.push(rest);
                    return Some(Ok(step));
                }
            }
        }
    }
}

fn selected_arm<'a>(
    program: &TypedTrees,
    target: &str,
    family: SymbolHandle,
    arm_count: u32,
    mut remaining: &'a [RangeStep],
) -> Result<(&'a [RangeStep], &'a [RangeStep]), String> {
    let refuse =
        || "retained target-body choice lost its complete exact declaration roster".to_owned();
    let mut families = program
        .target_machine_families
        .iter()
        .filter(|row| row.canonical == family);
    let row = families.next().ok_or_else(refuse)?;
    if families.next().is_some()
        || row.members.len() != arm_count as usize
        || arm_count == 0
        || !program
            .machine_by_symbol(family)
            .is_some_and(|machine| machine.target_family_interface)
    {
        return Err(refuse());
    }
    let mut seen = Vec::new();
    let mut selected = None;
    for _ in 0..arm_count {
        let Some((RangeStep::TargetArm { body, steps }, rest)) = remaining.split_first() else {
            return Err(refuse());
        };
        let mut members = row.members.iter().filter(|member| member.machine == *body);
        let member = members.next().ok_or_else(refuse)?;
        let machine = program.machine_by_symbol(*body).ok_or_else(refuse)?;
        if members.next().is_some()
            || row
                .members
                .iter()
                .any(|peer| peer.machine != member.machine && peer.target == member.target)
            || seen.contains(body)
            || machine.target_family_interface
            || machine.authored_target.as_ref() != Some(&member.target)
            || machine.body_status != typed_trees::machine::MachineBodyStatus::Complete
            || *steps == 0
            || *steps as usize > rest.len()
        {
            return Err(refuse());
        }
        seen.push(*body);
        let (child, tail) = rest.split_at(*steps as usize);
        if member.target.as_str() == target {
            if selected.replace(child).is_some() {
                return Err(refuse());
            }
        }
        remaining = tail;
    }
    let selected = selected
        .ok_or_else(|| format!("retained target-body choice has no body for `{target}`"))?;
    Ok((selected, remaining))
}
