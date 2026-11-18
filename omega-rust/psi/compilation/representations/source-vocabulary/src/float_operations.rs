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

/// Finds the canonical `Owner::operation` spelling inside a requirement
/// identity. Identities arrive as `operator::F32::negate(named(name(f32)))..`
/// for named operations and as escaped `named-callable(path(nominal\\(..\\,
/// path\\(Float::less\\)..` for spelled operator families, so the name is
/// matched as a whole path token: the longest canonical name whose
/// neighbouring characters are not identifier characters.
pub fn canonical_float_operation_name(requirement: &str) -> Option<&'static str> {
    let identifier = |c: char| c.is_ascii_alphanumeric() || c == '_';
    CANONICAL_FLOAT_OPERATIONS
        .iter()
        .copied()
        .filter(|canonical| {
            requirement.match_indices(canonical).any(|(start, _)| {
                let before = requirement[..start].chars().next_back();
                let after = requirement[start + canonical.len()..].chars().next();
                before.is_none_or(|c| !identifier(c)) && after.is_none_or(|c| !identifier(c))
            })
        })
        .max_by_key(|canonical| canonical.len())
}

/// A target owner's declared support for numeric language operations.
/// Source settlement observes support; native planning separately chooses ISA
/// realizations and proves their calling and control-state obligations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericTargetCapabilities {
    pub target: &'static str,
    pub operations: Vec<(&'static str, Result<(), &'static str>)>,
}

impl NumericTargetCapabilities {
    pub fn support_for(&self, requirement: &str) -> Option<Result<(), &'static str>> {
        let name = canonical_float_operation_name(requirement)?;
        self.operations
            .iter()
            .find(|(operation, _)| *operation == name)
            .map(|(_, support)| *support)
    }
}
