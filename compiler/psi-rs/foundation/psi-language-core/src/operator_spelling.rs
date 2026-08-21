//! Fixed operator-token identities.
//!
//! The token set and operand-directed semantics are settled. The source head
//! writes the literal token immediately after `operator`.

/// The legal fixed operator tokens. A named operator may be associated with
/// one token; receiver/operand machinery then picks the unique candidate. The
/// canonical source form places that token in the operator declaration head.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperatorSpelling {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Index,
    Range,
}

impl OperatorSpelling {
    /// Every legal spelling, in declaration order.
    pub const ALL: [Self; 13] = [
        Self::Add,
        Self::Subtract,
        Self::Multiply,
        Self::Divide,
        Self::Modulo,
        Self::Equal,
        Self::NotEqual,
        Self::Less,
        Self::LessEqual,
        Self::Greater,
        Self::GreaterEqual,
        Self::Index,
        Self::Range,
    ];

    /// Parse a spelling from its surface symbol, e.g. `+` or `[..]`.
    pub fn from_symbol(symbol: &str) -> Option<Self> {
        Some(match symbol {
            "+" => Self::Add,
            "-" => Self::Subtract,
            "*" => Self::Multiply,
            "/" => Self::Divide,
            "%" => Self::Modulo,
            "==" => Self::Equal,
            "!=" => Self::NotEqual,
            "<" => Self::Less,
            "<=" => Self::LessEqual,
            ">" => Self::Greater,
            ">=" => Self::GreaterEqual,
            "[]" => Self::Index,
            "[..]" => Self::Range,
            _ => return None,
        })
    }

    /// The canonical surface symbol for this spelling.
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Subtract => "-",
            Self::Multiply => "*",
            Self::Divide => "/",
            Self::Modulo => "%",
            Self::Equal => "==",
            Self::NotEqual => "!=",
            Self::Less => "<",
            Self::LessEqual => "<=",
            Self::Greater => ">",
            Self::GreaterEqual => ">=",
            Self::Index => "[]",
            Self::Range => "[..]",
        }
    }
}
