use crate::ValueType;
use omega_resolver::DeclarationId;
use std::fmt;

#[derive(Clone, Hash, Eq, PartialEq, Debug)]
pub struct DomainApplication {
    pub declaration: DeclarationId,
    pub arguments: Vec<ValueType>,
}
impl DomainApplication {
    pub(crate) fn substitute(
        &self,
        id: &DeclarationId,
        args: &[ValueType],
    ) -> Self {
        Self {
            declaration: self.declaration.clone(),
            arguments: self.arguments.iter().map(|t| t.substitute(id, args)).collect(),
        }
    }
}
impl fmt::Display for DomainApplication {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write!(f, "{}::{}", self.declaration.package, self.declaration.path)?;
        if !self.arguments.is_empty() {
            write!(
                f,
                "<{}>",
                self.arguments
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )?;
        }
        Ok(())
    }
}
