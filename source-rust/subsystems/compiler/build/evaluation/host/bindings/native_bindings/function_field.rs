//! Read the two reflected Product descriptions that the Build passed to bind_native.

use super::{
    NativeLocatorValue, Path, TerminalBuildValueShape, TerminalScalarValue,
    TerminalStorageContents, ValueRef,
};
use semantic_vocabulary::{
    FieldInfoDescription, FunctionFieldBinding, NativeBindingLocator, ParameterDescription,
    ScalarType,
};

impl<'a> ValueRef<'a> {
    fn member(&self, name: &str) -> Result<Self, String> {
        let TerminalBuildValueShape::Record { fields } = self.shape else {
            return Err("function table binding expected its checked record".into());
        };
        let mut matching = fields.iter().filter(|field| field.name == name);
        let (Some(field), None) = (matching.next(), matching.next()) else {
            return Err(format!(
                "function table binding has no unique `{name}` member"
            ));
        };
        let mut path = self.path.clone();
        path.push(Path::Field(field.id));
        Ok(Self {
            shape: &field.shape,
            values: self.values,
            path,
        })
    }

    fn description_text(&self, name: &str) -> Result<Vec<u8>, String> {
        let member = self.member(name)?;
        match (member.shape, member.observation()?) {
            (TerminalBuildValueShape::Bytes, TerminalStorageContents::Bytes(bytes)) => {
                Ok(bytes.to_vec())
            }
            _ => Err(format!(
                "function table description `{name}` is not its checked byte view"
            )),
        }
    }

    fn description_word(&self, name: &str) -> Result<u64, String> {
        u64::try_from(self.member(name)?.integer()?)
            .map_err(|_| format!("function table description `{name}` exceeds u64"))
    }

    fn description_bool(&self, name: &str) -> Result<bool, String> {
        let member = self.member(name)?;
        match (member.shape, member.observation()?) {
            (
                TerminalBuildValueShape::Scalar(ScalarType::Boolean),
                TerminalStorageContents::Scalar {
                    value: TerminalScalarValue::Boolean(value),
                    stored_scalar: ScalarType::Boolean,
                },
            ) => Ok(value),
            _ => Err(format!(
                "function table description `{name}` is not a checked bool"
            )),
        }
    }
}

pub(super) fn decode(value: &ValueRef<'_>) -> Result<NativeBindingLocator, String> {
    let table = value.member("table")?;
    let field = value.member("field")?;
    Ok(NativeBindingLocator::FunctionField(Box::new(
        FunctionFieldBinding {
            table: ParameterDescription {
                requirement_identity: table.description_text("requirement_identity")?,
                requirement_application: table.description_text("requirement_application")?,
                parameter_identity: table.description_text("parameter_identity")?,
                qualified_type: table.description_text("qualified_type")?,
                declaration_position: table.description_word("declaration_position")?,
                source_revision: table.description_text("source_revision")?,
            },
            field: FieldInfoDescription {
                member_identity: field.description_text("member_identity")?,
                case_identity: field.description_text("case_identity")?,
                owner_identity: field.description_text("owner_identity")?,
                owner_application: field.description_text("owner_application")?,
                name: field.description_text("name")?,
                qualified_type: field.description_text("qualified_type")?,
                declaration_position: field.description_word("declaration_position")?,
                has_stable_number: field.description_bool("has_stable_number")?,
                is_erased: field.description_bool("is_erased")?,
                stable_number: field.description_word("stable_number")?,
                source_revision: field.description_text("source_revision")?,
            },
        },
    )))
}
