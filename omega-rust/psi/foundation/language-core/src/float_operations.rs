//! Core owns the public numeric declaration roster and each declaration's
//! actual source form. Targets use the name view; receiving checks the form
//! as well as the source, signature and complete contract.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatDeclarationForm {
    Machine,
    Operator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloatDeclaration {
    pub name: &'static str,
    pub form: FloatDeclarationForm,
}

macro_rules! declarations {
    ($($form:ident $name:literal),+ $(,)?) => {
        pub const CANONICAL_FLOAT_DECLARATIONS: [FloatDeclaration; 104] = [
            $(FloatDeclaration { name: $name, form: FloatDeclarationForm::$form }),+
        ];
        /// The original target-table view of the same declaration roster.
        pub const CANONICAL_FLOAT_OPERATIONS: [&str; 104] = [$($name),+];
    };
}

declarations! {
    Machine "Float::add",
    Machine "Float::subtract",
    Machine "Float::multiply",
    Machine "Float::divide",
    Machine "Float::equal",
    Machine "Float::not_equal",
    Machine "Float::less",
    Machine "Float::less_or_equal",
    Machine "Float::greater",
    Machine "Float::greater_or_equal",
    Machine "F32::negate",
    Machine "F64::negate",
    Machine "F32::multiply_then_add",
    Machine "F64::multiply_then_add",
    Machine "F32::fused_multiply_add",
    Machine "F64::fused_multiply_add",
    Machine "F32::square_root",
    Machine "F64::square_root",
    Machine "F32::minimum",
    Machine "F64::minimum",
    Machine "F32::maximum",
    Machine "F64::maximum",
    Machine "F32::from_f64",
    Machine "F64::from_f32",
    Machine "F32::from_i8",
    Machine "F32::from_i16",
    Machine "F32::from_i32",
    Machine "F32::from_i64",
    Machine "F32::from_u8",
    Machine "F32::from_u16",
    Machine "F32::from_u32",
    Machine "F32::from_u64",
    Machine "F64::from_i8",
    Machine "F64::from_i16",
    Machine "F64::from_i32",
    Machine "F64::from_i64",
    Machine "F64::from_u8",
    Machine "F64::from_u16",
    Machine "F64::from_u32",
    Machine "F64::from_u64",
    Machine "I8::from_f32",
    Machine "I16::from_f32",
    Machine "I32::from_f32",
    Machine "I64::from_f32",
    Machine "U8::from_f32",
    Machine "U16::from_f32",
    Machine "U32::from_f32",
    Machine "U64::from_f32",
    Machine "I8::from_f64",
    Machine "I16::from_f64",
    Machine "I32::from_f64",
    Machine "I64::from_f64",
    Machine "U8::from_f64",
    Machine "U16::from_f64",
    Machine "U32::from_f64",
    Machine "U64::from_f64",
    Machine "F32::classify",
    Machine "F64::classify",
    Machine "F32::is_nan",
    Machine "F64::is_nan",
    Machine "F32::is_infinite",
    Machine "F64::is_infinite",
    Machine "F32::is_finite",
    Machine "F64::is_finite",
    Machine "F32::is_normal",
    Machine "F64::is_normal",
    Machine "F32::is_subnormal",
    Machine "F64::is_subnormal",
    Machine "F32::add_toward_zero",
    Machine "F64::add_toward_zero",
    Machine "F32::add_toward_positive",
    Machine "F64::add_toward_positive",
    Machine "F32::add_toward_negative",
    Machine "F64::add_toward_negative",
    Machine "F32::subtract_toward_zero",
    Machine "F64::subtract_toward_zero",
    Machine "F32::subtract_toward_positive",
    Machine "F64::subtract_toward_positive",
    Machine "F32::subtract_toward_negative",
    Machine "F64::subtract_toward_negative",
    Machine "F32::multiply_toward_zero",
    Machine "F64::multiply_toward_zero",
    Machine "F32::multiply_toward_positive",
    Machine "F64::multiply_toward_positive",
    Machine "F32::multiply_toward_negative",
    Machine "F64::multiply_toward_negative",
    Machine "F32::divide_toward_zero",
    Machine "F64::divide_toward_zero",
    Machine "F32::divide_toward_positive",
    Machine "F64::divide_toward_positive",
    Machine "F32::divide_toward_negative",
    Machine "F64::divide_toward_negative",
    Machine "F32::square_root_toward_zero",
    Machine "F64::square_root_toward_zero",
    Machine "F32::square_root_toward_positive",
    Machine "F64::square_root_toward_positive",
    Machine "F32::square_root_toward_negative",
    Machine "F64::square_root_toward_negative",
    Machine "F32::fused_multiply_add_toward_zero",
    Machine "F64::fused_multiply_add_toward_zero",
    Machine "F32::fused_multiply_add_toward_positive",
    Machine "F64::fused_multiply_add_toward_positive",
    Machine "F32::fused_multiply_add_toward_negative",
    Machine "F64::fused_multiply_add_toward_negative",
}

pub fn declaration_form(name: &str) -> Option<FloatDeclarationForm> {
    CANONICAL_FLOAT_DECLARATIONS
        .iter()
        .find(|row| row.name == name)
        .map(|row| row.form)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_integer_declarations_keep_their_actual_source_form() {
        assert_eq!(
            CANONICAL_FLOAT_OPERATIONS.len(),
            CANONICAL_FLOAT_DECLARATIONS.len()
        );
        for (index, declaration) in CANONICAL_FLOAT_DECLARATIONS.iter().enumerate() {
            assert_eq!(CANONICAL_FLOAT_OPERATIONS[index], declaration.name);
            assert!(!CANONICAL_FLOAT_OPERATIONS[..index].contains(&declaration.name));
        }
        assert_eq!(
            declaration_form("I32::from_f32"),
            Some(FloatDeclarationForm::Machine)
        );
        assert_eq!(
            declaration_form("U32::from_f64"),
            Some(FloatDeclarationForm::Machine)
        );
        assert_eq!(
            declaration_form("I8::from_f32"),
            Some(FloatDeclarationForm::Machine)
        );
        assert_eq!(
            declaration_form("U64::from_f64"),
            Some(FloatDeclarationForm::Machine)
        );
        assert_eq!(
            declaration_form("I16::from_f32"),
            Some(FloatDeclarationForm::Machine)
        );
        assert_eq!(
            declaration_form("U16::from_f64"),
            Some(FloatDeclarationForm::Machine)
        );
        assert_eq!(declaration_form("I7::from_f32"), None);
    }
}
