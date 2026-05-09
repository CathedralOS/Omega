use omega_core::symbols::SymbolHandle;
use omega_typed_program::expression::{Expression, NamePath};
use omega_typed_program::name::ProgramName;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlaceKey {
    head_symbol: SymbolHandle,
    symbol: SymbolHandle,
    members: Vec<ProgramName>,
}

impl PlaceKey {
    pub fn from_expression(expression: &Expression) -> Option<Self> {
        match expression {
            Expression::Mutable(inner_expression) => Self::from_expression(inner_expression),
            Expression::Name(path) => Some(Self::from_name_path(path)),
            Expression::Indexed(indexed) => {
                let mut key = Self::from_expression(&indexed.collection)?;
                key.members.push(ProgramName::generated(format!(
                    "[{}]",
                    indexed.index.display_name()
                )));
                key.symbol = SymbolHandle::invalid();
                Some(key)
            }
            _ => None,
        }
    }

    pub fn append_member(&self, member: ProgramName) -> Self {
        let mut key = self.clone();
        key.members.push(member);
        key.symbol = SymbolHandle::invalid();
        key
    }

    pub fn from_symbol_name(symbol: SymbolHandle, name: ProgramName) -> Self {
        Self {
            head_symbol: symbol,
            symbol,
            members: vec![name],
        }
    }

    pub fn starts_with(&self, prefix: &Self) -> bool {
        if prefix.members.len() > self.members.len() {
            return false;
        }

        if self.head_symbol.is_valid()
            && prefix.head_symbol.is_valid()
            && self.head_symbol != prefix.head_symbol
        {
            return false;
        }

        self.members
            .iter()
            .zip(prefix.members.iter())
            .all(|(member, prefix_member)| member == prefix_member)
    }

    pub fn replace_prefix(&self, prefix: &Self, target: &Self) -> Self {
        if !self.starts_with(prefix) {
            return self.clone();
        }

        let mut members = target.members.clone();
        members.extend(self.members.iter().skip(prefix.members.len()).cloned());

        Self {
            head_symbol: target.head_symbol,
            symbol: if members.len() == target.members.len() {
                target.symbol
            } else {
                SymbolHandle::invalid()
            },
            members,
        }
    }

    fn from_name_path(path: &NamePath) -> Self {
        Self {
            head_symbol: path.head_symbol(),
            symbol: path.symbol(),
            members: path.members().to_vec(),
        }
    }
}
