#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::rules::passes) enum IntegerRangeComparisonKind {
    RangeEqualConstant,
    ConstantEqualRange,
    RangeLessThanConstant,
    ConstantLessThanRange,
    RangeLessOrEqualConstant,
    ConstantLessOrEqualRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::rules::passes) enum IntegerRangePairComparisonKind {
    Equal,
    LessThan,
    LessOrEqual,
}
