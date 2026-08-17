//! Fixed operator-token identities and boundary provider categories.
//!
//! The token set and operand-directed semantics are settled. The source clause
//! binding one of these tokens to a named declaration is not: the parser's
//! current `spelling` clause is temporary bootstrap syntax pending
//! `FIXED-OPERATOR-SURFACE-BINDING`.

/// The legal fixed operator tokens. A named operator may be associated with
/// one token; receiver/operand machinery then picks the unique candidate. The
/// source form for that association is deliberately not defined here.
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

/// Boundary primitive provider categories (frozen Wave 0 decision #4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderCategory {
    SliceIndexing,
    PointerOffset,
    PointerAccess,
    DescriptorConstruction,
    Allocation,
    HostAbiCall,
}

impl ProviderCategory {
    pub const ALL: [Self; 6] = [
        Self::SliceIndexing,
        Self::PointerOffset,
        Self::PointerAccess,
        Self::DescriptorConstruction,
        Self::Allocation,
        Self::HostAbiCall,
    ];

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "SliceIndexing" => Self::SliceIndexing,
            "PointerOffset" => Self::PointerOffset,
            "PointerAccess" => Self::PointerAccess,
            "DescriptorConstruction" => Self::DescriptorConstruction,
            "Allocation" => Self::Allocation,
            "HostAbiCall" => Self::HostAbiCall,
            _ => return None,
        })
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::SliceIndexing => "SliceIndexing",
            Self::PointerOffset => "PointerOffset",
            Self::PointerAccess => "PointerAccess",
            Self::DescriptorConstruction => "DescriptorConstruction",
            Self::Allocation => "Allocation",
            Self::HostAbiCall => "HostAbiCall",
        }
    }
}
