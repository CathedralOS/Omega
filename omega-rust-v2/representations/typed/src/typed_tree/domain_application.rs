use crate::ValueType;
use omega_resolved::DeclarationId;
use std::fmt;

#[derive(Clone, Hash, Eq, PartialEq, Debug)]
pub struct DomainApplication {
    pub declaration: DeclarationId,
    pub arguments: Vec<ValueType>,
}
impl DomainApplication {
    pub fn substitute(
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
        write!(f, "{}\\{}", self.declaration.package, self.declaration.path)?;
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

pub fn qualified(
    carrier: ValueType,
    mut atoms: Vec<DomainApplication>,
) -> ValueType {
    atoms.sort_by_cached_key(|a| (a.declaration.package.clone(), a.declaration.path.clone(), format!("{:?}", a.arguments)));
    atoms.dedup();
    if atoms.is_empty() {
        carrier
    } else {
        ValueType::Qualified(Box::new(carrier), atoms)
    }
}
impl ValueType {
    pub fn domains(&self) -> &[DomainApplication] {
        match self {
            Self::Qualified(_, atoms) => atoms,
            _ => &[],
        }
    }
    pub fn carrier(&self) -> &Self {
        match self {
            Self::Qualified(base, _) => base,
            _ => self,
        }
    }
}
