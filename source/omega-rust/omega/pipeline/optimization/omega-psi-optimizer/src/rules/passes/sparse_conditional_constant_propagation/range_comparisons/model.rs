#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IntegerRangeComparisonKind {
    RangeEqualConstant,
    ConstantEqualRange,
    RangeLessThanConstant,
    ConstantLessThanRange,
    RangeLessOrEqualConstant,
    ConstantLessOrEqualRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IntegerRangePairComparisonKind {
    Equal,
    LessThan,
    LessOrEqual,
}
