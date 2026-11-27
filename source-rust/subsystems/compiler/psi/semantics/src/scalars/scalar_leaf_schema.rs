#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarLeafResultShape {
    DeclaredInteger,
    DeclaredFloat,
    Boolean,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarLeafOperandShape {
    IntegerLiteral,
    FloatLiteral,
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
    IntegerConstant,
    FloatConstant,
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
pub enum ScalarLeafWorkPolicy {
    ConsumeOne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarLeafFrontierPolicy {
    PreserveLocal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScalarLeafSchema {
    pub(crate) result: ScalarLeafResultShape,
    pub(crate) operands: ScalarLeafOperandShape,
    pub(crate) denotation: ScalarLeafDenotation,
    pub(crate) goal: ScalarLeafGoalShape,
    pub(crate) fact: ScalarLeafFactShape,
    pub(crate) crash: ScalarLeafCrashPolicy,
    pub(crate) work: ScalarLeafWorkPolicy,
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

    pub const fn work(self) -> ScalarLeafWorkPolicy {
        self.work
    }

    pub const fn frontier(self) -> ScalarLeafFrontierPolicy {
        self.frontier
    }
}

pub(crate) type GoalFreeScalarLeafSchema = ScalarLeafSchema;

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
            (_, ScalarLeafDenotation::BooleanConstant) => ScalarLeafFactShape::ResultEquation,
            (ScalarLeafResultShape::Boolean, _) => {
                ScalarLeafFactShape::BooleanResultEquationAndPolarityImplications
            }
            (
                ScalarLeafResultShape::DeclaredInteger | ScalarLeafResultShape::DeclaredFloat,
                _,
            ) => ScalarLeafFactShape::ResultEquation,
        },
        crash: ScalarLeafCrashPolicy::Never,
        work: ScalarLeafWorkPolicy::ConsumeOne,
        frontier: ScalarLeafFrontierPolicy::PreserveLocal,
    })
}
