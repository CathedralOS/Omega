use omega_core::symbols::SymbolHandle;
use omega_typed_program::expression::{Expression, NamePath};
use omega_typed_program::name::ProgramName;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PlaceKey {
    head_symbol: SymbolHandle,
    symbol: SymbolHandle,
    members: Vec<ProgramName>,
}

impl PlaceKey {
    pub(in crate::state_schedule) fn from_expression(expression: &Expression) -> Option<Self> {
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

    pub(in crate::state_schedule) fn append_member(&self, member: ProgramName) -> Self {
        let mut key = self.clone();
        key.members.push(member);
        key.symbol = SymbolHandle::invalid();
        key
    }

    pub(in crate::state_schedule) fn from_symbol_name(
        symbol: SymbolHandle,
        name: ProgramName,
    ) -> Self {
        Self {
            head_symbol: symbol,
            symbol,
            members: vec![name],
        }
    }

    fn from_name_path(path: &NamePath) -> Self {
        Self {
            head_symbol: path.head_symbol(),
            symbol: path.symbol(),
            members: path.members().to_vec(),
        }
    }

    pub(in crate::state_schedule) fn starts_with(&self, prefix: &Self) -> bool {
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

    pub(in crate::state_schedule) fn replace_prefix(&self, prefix: &Self, target: &Self) -> Self {
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
}

pub(in crate::state_schedule) fn argument_binding_place_key(
    expression: &Expression,
    aliases: &[(PlaceKey, PlaceKey)],
) -> Option<PlaceKey> {
    match expression {
        Expression::Mutable(inner_expression) => {
            shallow_canonical_place_key(inner_expression, aliases)
        }
        _ => canonical_place_key(expression, aliases),
    }
}

pub(in crate::state_schedule) fn canonical_place_key(
    expression: &Expression,
    aliases: &[(PlaceKey, PlaceKey)],
) -> Option<PlaceKey> {
    let key = match expression {
        Expression::Mutable(inner_expression) => {
            return canonical_place_key(inner_expression, aliases);
        }
        Expression::Name(_) | Expression::Indexed(_) => PlaceKey::from_expression(expression)?,
        _ => return None,
    };

    Some(resolve_alias(&key, aliases))
}

pub(in crate::state_schedule) fn shallow_canonical_place_key(
    expression: &Expression,
    aliases: &[(PlaceKey, PlaceKey)],
) -> Option<PlaceKey> {
    let key = match expression {
        Expression::Mutable(inner_expression) => {
            return shallow_canonical_place_key(inner_expression, aliases);
        }
        Expression::Name(_) | Expression::Indexed(_) => PlaceKey::from_expression(expression)?,
        _ => return None,
    };

    Some(resolve_alias_once(&key, aliases))
}

fn resolve_alias(key: &PlaceKey, aliases: &[(PlaceKey, PlaceKey)]) -> PlaceKey {
    let mut resolved = key.clone();

    for _ in 0..aliases.len() {
        let Some((alias, target)) = aliases
            .iter()
            .rev()
            .find(|(alias, _)| resolved.starts_with(alias))
        else {
            return resolved;
        };

        resolved = resolved.replace_prefix(alias, target);
    }

    resolved
}

fn resolve_alias_once(key: &PlaceKey, aliases: &[(PlaceKey, PlaceKey)]) -> PlaceKey {
    aliases
        .iter()
        .rev()
        .find(|(alias, _)| key.starts_with(alias))
        .map_or_else(
            || key.clone(),
            |(alias, target)| key.replace_prefix(alias, target),
        )
}
