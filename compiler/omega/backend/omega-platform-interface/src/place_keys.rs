use psi_checked_trees::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable, NamePath, TableNamePath,
};
use psi_checked_trees::name::Identifier;
use psi_symbols::SymbolHandle;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceKey {
    head_symbol: SymbolHandle,
    symbol: SymbolHandle,
    members: Arc<[Identifier]>,
}

impl Default for PlaceKey {
    fn default() -> Self {
        Self {
            head_symbol: SymbolHandle::invalid(),
            symbol: SymbolHandle::invalid(),
            members: Arc::from([]),
        }
    }
}

impl PlaceKey {
    pub fn from_expression(expression: &Expression) -> Option<Self> {
        match expression {
            Expression::Borrow(inner_expression) => Self::from_expression(&inner_expression.target),
            Expression::Name(path) => Some(Self::from_name_path(path)),
            Expression::Indexed(indexed) => {
                let mut key = Self::from_expression(&indexed.collection)?;
                key = key.append_generated_member(format!("[{}]", indexed.index.display_name()));
                key.symbol = SymbolHandle::invalid();
                Some(key)
            }
            Expression::Member(member) => {
                let mut key = Self::from_expression(&member.receiver)?;
                key = key.append_member(member.member.clone());
                key.symbol = member.member_symbol;
                Some(key)
            }
            _ => None,
        }
    }

    pub fn from_expression_handle(
        table: &ExpressionTable,
        expression: ExpressionHandle,
    ) -> Option<Self> {
        match table.expression(expression) {
            ExpressionNode::Borrow(inner_expression) => {
                Self::from_expression_handle(table, inner_expression.target)
            }
            ExpressionNode::Name(path) => Some(Self::from_table_name_path(table, path)),
            ExpressionNode::Indexed(indexed) => {
                let mut key = Self::from_expression_handle(table, indexed.collection)?;
                key =
                    key.append_generated_member(format!("[{}]", table.display_name(indexed.index)));
                key.symbol = SymbolHandle::invalid();
                Some(key)
            }
            ExpressionNode::Member(member) => {
                let mut key = Self::from_expression_handle(table, member.receiver)?;
                key = key.append_member(member.member.clone());
                key.symbol = member.member_symbol;
                Some(key)
            }
            _ => None,
        }
    }

    pub fn append_member(&self, member: Identifier) -> Self {
        let mut members = Vec::with_capacity(self.members.len() + 1);
        members.extend(self.members.iter().cloned());
        members.push(member);

        Self {
            head_symbol: self.head_symbol,
            symbol: SymbolHandle::invalid(),
            members: Arc::from(members.into_boxed_slice()),
        }
    }

    pub fn from_symbol_name(symbol: SymbolHandle, name: Identifier) -> Self {
        Self {
            head_symbol: symbol,
            symbol,
            members: Arc::from(vec![name].into_boxed_slice()),
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

        let mut members = Vec::with_capacity(
            target.members.len() + self.members.len().saturating_sub(prefix.members.len()),
        );
        members.extend(target.members.iter().cloned());
        members.extend(self.members.iter().skip(prefix.members.len()).cloned());

        Self {
            head_symbol: target.head_symbol,
            symbol: if members.len() == target.members.len() {
                target.symbol
            } else {
                SymbolHandle::invalid()
            },
            members: Arc::from(members.into_boxed_slice()),
        }
    }

    fn from_name_path(path: &NamePath) -> Self {
        Self {
            head_symbol: path.head_symbol(),
            symbol: path.symbol(),
            members: Arc::from(path.members()),
        }
    }

    fn from_table_name_path(table: &ExpressionTable, path: &TableNamePath) -> Self {
        Self {
            head_symbol: path.head_symbol,
            symbol: path.symbol,
            members: Arc::from(table.name_path_members(path.members)),
        }
    }

    fn append_generated_member(&self, member: String) -> Self {
        self.append_member(Identifier::generated(member))
    }
}
