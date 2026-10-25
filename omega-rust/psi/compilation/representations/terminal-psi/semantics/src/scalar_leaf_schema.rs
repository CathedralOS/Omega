//! The goal-free scalar-leaf schema: result and operand shapes, the
//! denotation of each scalar leaf, and its crash, fuel and frontier policies.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarLeafResultShape {
    ByteCount,
    DeclaredInteger,
    Boolean,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarLeafOperandShape {
    LayoutObservation,
    IntegerLiteral,
    BooleanLiteral,
    UnaryBoolean,
    BinaryBoolean,
    UnaryInteger,
    BinaryInteger,
    WideningInteger,
    ExactCastInteger,
    IntegerShift,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarLeafDenotation {
    LayoutObservation,
    IntegerConstant,
    BooleanConstant,
    BooleanNot,
    BooleanEqual,
    IntegerEqual,
    IntegerLessThan,
    IntegerLessOrEqual,
    IntegerBitwiseNot,
    IntegerWiden,
    IntegerExactCast,
    IntegerBitwiseAnd,
    IntegerBitwiseOr,
    IntegerBitwiseXor,
    WrappingIntegerShiftLeft,
    WrappingIntegerShiftRight,
    ExactIntegerShiftLeft,
    ExactIntegerShiftRight,
    ExactIntegerAdd,
    ExactIntegerSubtract,
    ExactIntegerMultiply,
    ExactIntegerDivide,
    ExactIntegerRemainder,
    WrappingIntegerDivide,
    WrappingIntegerRemainder,
    SaturatingIntegerDivide,
    SaturatingIntegerRemainder,
    WrappingIntegerAdd,
    SaturatingIntegerAdd,
    WrappingIntegerSubtract,
    SaturatingIntegerSubtract,
    WrappingIntegerMultiply,
    SaturatingIntegerMultiply,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarLeafGoalShape {
    None,
    ExactCastRepresentable,
    ExactShiftCount,
    ExactShiftLeftRepresentable,
    ExactArithmeticRepresentable,
    ExactDivisionDefined,
    NonzeroDivisor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarLeafFactShape {
    ResultEquation,
    BooleanResultEquationAndPolarityImplications,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarLeafCrashPolicy {
    Never,
    CrashesUnlessGoal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarLeafFuelPolicy {
    ConsumeOne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarLeafFrontierPolicy {
    PreserveLocal,
}

/// One total scalar leaf row whose direct result equation needs no proof
/// reduction. Every semantic axis remains explicit even where this first Rust
/// cohort has only one admitted policy value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScalarLeafSchema {
    pub(crate) result: ScalarLeafResultShape,
    pub(crate) operands: ScalarLeafOperandShape,
    pub(crate) denotation: ScalarLeafDenotation,
    pub(crate) goal: ScalarLeafGoalShape,
    pub(crate) fact: ScalarLeafFactShape,
    pub(crate) crash: ScalarLeafCrashPolicy,
    pub(crate) fuel: ScalarLeafFuelPolicy,
    pub(crate) frontier: ScalarLeafFrontierPolicy,
}

impl ScalarLeafSchema {
    pub const fn result(self) -> ScalarLeafResultShape {
        self.result
    }

    pub const fn operands(self) -> ScalarLeafOperandShape {
        self.operands
    }

    pub const fn denotation(self) -> ScalarLeafDenotation {
        self.denotation
    }

    pub const fn goal(self) -> ScalarLeafGoalShape {
        self.goal
    }

    pub const fn fact(self) -> ScalarLeafFactShape {
        self.fact
    }

    pub const fn crash(self) -> ScalarLeafCrashPolicy {
        self.crash
    }

    pub const fn fuel(self) -> ScalarLeafFuelPolicy {
        self.fuel
    }

    pub const fn frontier(self) -> ScalarLeafFrontierPolicy {
        self.frontier
    }
}

/// The goal-free cohort of [`ScalarLeafSchema`] rows.
pub type GoalFreeScalarLeafSchema = ScalarLeafSchema;

pub(crate) const fn goal_free_scalar_leaf(
    result: ScalarLeafResultShape,
    operands: ScalarLeafOperandShape,
    denotation: ScalarLeafDenotation,
) -> Option<ScalarLeafSchema> {
    Some(ScalarLeafSchema {
        result,
        operands,
        denotation,
        goal: ScalarLeafGoalShape::None,
        fact: match (result, denotation) {
            // A Boolean literal already exposes its exact polarity directly.
            (_, ScalarLeafDenotation::BooleanConstant) => ScalarLeafFactShape::ResultEquation,
            (ScalarLeafResultShape::Boolean, _) => {
                ScalarLeafFactShape::BooleanResultEquationAndPolarityImplications
            }
            (ScalarLeafResultShape::DeclaredInteger | ScalarLeafResultShape::ByteCount, _) => {
                ScalarLeafFactShape::ResultEquation
            }
        },
        crash: ScalarLeafCrashPolicy::Never,
        fuel: ScalarLeafFuelPolicy::ConsumeOne,
        frontier: ScalarLeafFrontierPolicy::PreserveLocal,
    })
}
