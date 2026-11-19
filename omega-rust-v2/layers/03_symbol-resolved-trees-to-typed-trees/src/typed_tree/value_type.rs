use crate::{Access, CallableType, DomainApplication, Opening, RequirementSlot, Site};
use num_bigint::{BigInt, Sign};
use num_rational::BigRational;
use omega_lexer::SourceSpan;
use omega_resolver::DeclarationId;
use std::fmt;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum ValueType {
    Bool,
    LevelKind,
    Squash(Box<ValueType>),
    LevelOf(Box<ValueType>),
    Subject {
        declaration: Site,
        version: Option<Site>,
    },
    BoundSubject(usize),
    MathematicalDefinition(Box<DeclarationId>),
    MathematicalValue {
        definition: Box<DeclarationId>,
        subjects: Vec<ValueType>,
    },
    FamilyApplication {
        family: Box<ValueType>,
        subjects: Vec<ValueType>,
    },
    Level(num_bigint::BigUint),
    Universe {
        strict: bool,
        level: Box<ValueType>,
    },
    Integer {
        signed: bool,
        width: Option<u64>,
    },
    IndexedInteger {
        signed: bool,
        width: Box<ValueType>,
    },
    Address,
    Float32,
    Float64,
    Tuple(Vec<ValueType>),
    Function(Box<ValueType>, Box<ValueType>),
    Nominal(Box<DeclarationId>),
    Exact {
        value: BigRational,
        fractional_origin: Option<SourceSpan>,
    },
    Reference(Access, Box<ValueType>),
    Array(Box<ValueType>, u64),
    IndexedArray(Box<ValueType>, Box<ValueType>),
    Slice(Box<ValueType>),
    AtLifetime(Box<ValueType>, Box<ValueType>),
    /// Region chosen at a local construction site for an omitted lifetime argument.
    InferredLifetime {
        site: Box<Site>,
        index: usize,
    },
    Witness {
        owner: Box<DeclarationId>,
        index: usize,
        opening: Option<Box<Opening>>,
    },
    Callable(Box<CallableType>),
    MachineSelection(Box<DeclarationId>, Vec<ValueType>),
    LogicalRequirement(Site),
    ProofCitation(Box<DeclarationId>),
    ProofTerm(Site),
    Evidence(Box<RequirementSlot>),
    Implementation(Box<DeclarationId>),
    StaticFormula {
        term: crate::IntegerIndex,
        carrier: Box<ValueType>,
    },
    StaticInteger {
        carrier: Box<ValueType>,
        value: BigInt,
    },
    Parameter {
        owner: Box<DeclarationId>,
        index: usize,
    },
    Applied(Box<DeclarationId>, Vec<ValueType>),
    Qualified(Box<ValueType>, Vec<DomainApplication>),
    NoResult,
}

impl fmt::Display for ValueType {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            Self::IndexedInteger { signed, width } => write!(f, "{}<{width}>", if *signed { "i" } else { "u" }),
            Self::Address => write!(f, "addr"),
            Self::Bool => write!(f, "bool"),
            Self::Subject { declaration, version } => write!(
                f,
                "subject {}@{}{}",
                declaration.file,
                declaration.span.get_start_offset(),
                version
                    .as_ref()
                    .map_or(String::new(), |s| format!("/{}", s.span.get_start_offset()))
            ),
            Self::BoundSubject(index) => write!(f, "bound[{index}]"),
            Self::MathematicalValue { definition, subjects } => write!(
                f,
                "{}::{}({})",
                definition.package,
                definition.path,
                subjects
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::MathematicalDefinition(id) => write!(f, "{}::{}", id.package, id.path),
            Self::FamilyApplication { family, subjects } => write!(
                f,
                "{family}({})",
                subjects
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Squash(ty) => write!(f, "core::Squash<{ty}>"),
            Self::LevelOf(ty) => write!(f, "level({ty})"),
            Self::LevelKind => write!(f, "core::Level"),
            Self::Level(level) => write!(f, "{level}"),
            Self::Universe { strict, level } => write!(f, "core::{}<{level}>", if *strict { "Strict" } else { "Type" }),
            Self::Float32 => write!(f, "f32"),
            Self::Tuple(items) => write!(
                f,
                "({})",
                items
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Function(input, out) => write!(f, "({input} -> {out})"),
            Self::Float64 => write!(f, "f64"),
            Self::Integer { signed, width } => {
                f.write_str(if *signed { "i" } else { "u" })?;
                if let Some(width) = width {
                    write!(f, "{width}")?;
                }
                Ok(())
            }
            Self::Nominal(id) => write!(f, "{}::{}", id.package, id.path),
            Self::Exact { value, .. } => write!(f, "{value}"),
            Self::Reference(access, inner) => write!(f, "{}{inner}", access.prefix()),
            Self::IndexedArray(inner, size) => write!(f, "[{inner}; {size}]"),
            Self::Array(inner, size) => write!(f, "[{inner}; {size}]"),
            Self::InferredLifetime { site, index } => write!(f, "lifetime {}@{}[{index}]", site.file, site.span.get_start_offset()),
            Self::AtLifetime(inner, region) => write!(f, "{inner} at {region}"),
            Self::Slice(inner) => write!(f, "[{inner}]"),
            Self::Witness { owner, index, .. } => write!(f, "{}::{}<some {}>", owner.package, owner.path, index),
            Self::Callable(c) => write!(
                f,
                "machine({}) -> {}",
                c.parameters
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", "),
                c.result
            ),
            Self::Evidence(slot) => write!(f, "evidence for {}::{}", slot.declaration.package, slot.declaration.path),
            Self::Implementation(id) => write!(f, "{}::{}", id.package, id.path),
            Self::LogicalRequirement(site) => write!(f, "hypothesis at {}:{}", site.file, site.span.get_start_offset()),
            Self::ProofTerm(site) => write!(f, "evidence at {}:{}", site.file, site.span.get_start_offset()),
            Self::ProofCitation(id) => write!(f, "proof {}::{}", id.package, id.path),
            Self::MachineSelection(id, arguments) => {
                write!(f, "{}::{}", id.package, id.path)?;
                if !arguments.is_empty() {
                    write!(f, "::<")?;
                    for (index, argument) in arguments.iter().enumerate() {
                        if index != 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{argument}")?;
                    }
                    write!(f, ">")?;
                }
                Ok(())
            }
            Self::StaticFormula { term, .. } => write!(f, "{term}"),
            Self::StaticInteger { value, .. } => write!(f, "{value}"),
            Self::Parameter { owner, index } => write!(f, "{}::{}<parameter {}>", owner.package, owner.path, index),
            Self::Applied(id, args) => write!(
                f,
                "{}::{}<{}>",
                id.package,
                id.path,
                args.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Qualified(carrier, domains) => write!(
                f,
                "{carrier} in {}",
                domains
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" & ")
            ),
            Self::NoResult => f.write_str("no result"),
        }
    }
}

impl ValueType {
    pub(crate) fn builtin_scalar(name: &str) -> Option<Self> {
        match name {
            "bool" => Some(Self::Bool),
            "addr" => Some(Self::Address),
            "f32" => Some(Self::Float32),
            "f64" => Some(Self::Float64),
            _ => Self::integer(name),
        }
    }
    pub(crate) fn integer(name: &str) -> Option<Self> {
        let digits = name.strip_prefix(['u', 'i'])?;
        let width = if digits.is_empty() {
            None
        } else {
            Some(digits.parse::<u64>().ok().filter(|&w| w > 0)?)
        };
        Some(Self::Integer {
            signed: name.starts_with('i'),
            width,
        })
    }
    pub(crate) fn contains_integer(
        &self,
        value: &BigInt,
    ) -> bool {
        let ValueType::Integer { signed, width } = self else { return false };
        if !signed && value.sign() == Sign::Minus {
            return false;
        }
        width.is_none_or(|w| {
            if *signed && value.sign() == Sign::Minus {
                (value + 1u8).bits() < w
            } else if *signed {
                value.bits() < w
            } else {
                value.bits() <= w
            }
        })
    }
    pub(crate) fn fits(
        &self,
        expected: &Self,
    ) -> bool {
        if let Self::Exact { value, .. } = self {
            match expected {
                Self::Float32 | Self::Float64 => return true,
                Self::IndexedInteger { signed, .. } => return value.is_integer() && (*signed || value.numer().sign() != Sign::Minus),
                Self::Integer { .. } => return value.is_integer() && expected.contains_integer(value.numer()),
                _ => {}
            }
        }
        if let (Self::Callable(actual), Self::Callable(destination)) = (self, expected) {
            return destination.admits_contract(actual);
        }
        self == expected
    }
    /// An unsuffixed number has no carrier until a destination lands it, and a
    /// tuple literal leaves its components unlanded. A binding or constant
    /// whose type still holds one would let a later use choose that type.
    pub(crate) fn holds_anonymous_number(&self) -> bool {
        match self {
            Self::Exact { .. } => true,
            Self::Tuple(items) => items.iter().any(Self::holds_anonymous_number),
            _ => false,
        }
    }
    pub(crate) fn reference_parts(&self) -> Option<(&crate::Access, &Self)> {
        let reference = match self {
            Self::AtLifetime(inner, _) => inner.as_ref(),
            other => other,
        };
        match reference {
            Self::Reference(access, inner) => Some((access, inner)),
            _ => None,
        }
    }
    pub(crate) fn numeric(&self) -> bool {
        matches!(self, Self::Integer { .. } | Self::IndexedInteger { .. } | Self::Float32 | Self::Float64)
    }
    pub(crate) fn admits(
        &self,
        operand: &Self,
    ) -> bool {
        if let (Self::Callable(expected), Self::Callable(actual)) = (self, operand) {
            return expected.admits_contract(actual);
        }
        self == operand || (self.numeric() && matches!(operand, Self::Exact { .. }))
    }
}
