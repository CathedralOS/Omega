#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericLiteralKind {
    Integer(IntegerLiteralKind),
    Float(FloatLiteralKind),
}

impl Default for NumericLiteralKind {
    fn default() -> Self {
        Self::Integer(IntegerLiteralKind::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IntegerLiteralKind {
    pub base: NumericBase,
    pub empty_digits: bool,
    pub has_suffix: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FloatLiteralKind {
    pub has_exponent: bool,
    pub empty_exponent: bool,
    pub has_suffix: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NumericBase {
    Binary,
    Octal,
    #[default]
    Decimal,
    Hexadecimal,
}
