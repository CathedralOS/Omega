use super::{SymbolKind, SymbolNameRef};

pub const BUILTIN_TYPE_COUNT: usize = 22;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinType {
    UInt,
    Int,
    Real,
}

impl BuiltinType {
    pub fn name(self) -> &'static str {
        match self {
            Self::UInt => "UInt",
            Self::Int => "Int",
            Self::Real => "Real",
        }
    }

    pub fn ordinal(self) -> usize {
        match self {
            Self::UInt => 19,
            Self::Int => 20,
            Self::Real => 21,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinFunction {
    Max,
    Min,
}

impl BuiltinFunction {
    pub const COUNT: usize = 2;

    pub fn name(self) -> &'static str {
        match self {
            Self::Max => "max",
            Self::Min => "min",
        }
    }

    pub fn ordinal(self) -> usize {
        match self {
            Self::Max => 0,
            Self::Min => 1,
        }
    }
}

pub fn builtin_type_symbols() -> [(SymbolKind, SymbolNameRef<'static>); BUILTIN_TYPE_COUNT] {
    [
        (SymbolKind::BuiltinType, SymbolNameRef::Static("bool")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("i8")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("i16")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("i32")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("i64")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("isize")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("u8")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("u16")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("u32")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("u64")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("usize")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("f32")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("f64")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("String")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("Slice")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("Result")),
        (
            SymbolKind::BuiltinType,
            SymbolNameRef::Static("SyscallResult"),
        ),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("Terminal")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("Never")),
        (
            SymbolKind::BuiltinType,
            SymbolNameRef::Static(BuiltinType::UInt.name()),
        ),
        (
            SymbolKind::BuiltinType,
            SymbolNameRef::Static(BuiltinType::Int.name()),
        ),
        (
            SymbolKind::BuiltinType,
            SymbolNameRef::Static(BuiltinType::Real.name()),
        ),
    ]
}

pub fn builtin_function_symbols() -> [(SymbolKind, SymbolNameRef<'static>); BuiltinFunction::COUNT]
{
    [
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::Max.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::Min.name()),
        ),
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinTypeMember {
    RealFrom,
}

impl BuiltinTypeMember {
    pub fn owner(self) -> BuiltinType {
        match self {
            Self::RealFrom => BuiltinType::Real,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::RealFrom => "from",
        }
    }
}

pub fn builtin_type_member_symbols(
    builtin_type: BuiltinType,
) -> impl Iterator<Item = (SymbolKind, SymbolNameRef<'static>)> {
    [BuiltinTypeMember::RealFrom]
        .into_iter()
        .filter(move |member| member.owner() == builtin_type)
        .map(|member| {
            (
                SymbolKind::BuiltinFunction,
                SymbolNameRef::Static(member.name()),
            )
        })
}
