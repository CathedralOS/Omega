/*
We provide target-neutral source-language vocabulary shared across Psi frontend
stages. We define source-semantics tables that resolution and typing consult
(`source_semantics`, with `receiver_binding`, `place_root`, and
`contract_result`), memory orderings and compare-exchange outcome schemas
(`atomic`), value and borrow-recast cast forms (`cast_form`), inline-assembly
catalog contracts and scratch-register tables (`inline_assembly`), operator
spellings (`operator_spelling`), compiler-owned layout query names
(`data_layout_queries`), evaluated constant values (`constant_value`), and core
surface operation descriptors (`activation_storage`, `descriptor_tables`,
`layout_storage`, `static_interrupts`), and the sealed intrinsic catalog of
bundled declarations that receive compiler custody (`sealed_intrinsics`).
*/

/*
Several frontend and checking stages must agree on the exact spellings,
multiplicity axes, carry policies, atomic ordering plans, and inline-assembly
contracts without coupling to any single AST or typed-tree representation crate.
By keeping this shared vocabulary in a zero-representation foundation crate, we
let parsing, name resolution, type checking, and verification share canonical
rules and tables without circular dependencies.
*/

/*
We neither parse nor judge a program, and we depend on `compiler_storage` and
`numerics` for record derives and float literal formats.
*/

#![forbid(unsafe_code)]

pub mod atomic;
pub mod inline_assembly;
pub mod sealed_intrinsics;
mod source_semantics;

pub use atomic::{AtomicOrderingPlan, MemoryOrdering};
pub use cast_form::CastForm;
pub use operator_spelling::OperatorSpelling;
pub use source_semantics::contract_result::{
    self, RESERVED_RESULT, is_reserved_result, signature_reserves_result,
};
pub use source_semantics::place_root::{
    self, PLACE_MEMBER_SEPARATOR, is_rooted_at, place_field, place_label,
};
pub use source_semantics::receiver_binding::{self, SELF_RECEIVER, is_self_receiver};
pub use source_semantics::{
    BindingRelevance, CallOperationalAcknowledgement, CallOperationalAcknowledgementOrigin,
    CarryAddress, CarryCpu, CarryHostThread, CarryPermission, CarryPolicy, CarrySuspension,
    DataSupplyMode, DomainClassification, DomainPredicatePresence, LocalBindingRole, Multiplicity,
    PatternComponent, ReferenceAccess,
};

pub mod activation_storage {
    pub const ACTIVATION_CORE_SOURCE: &str = "activation.omg";

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum ActivationStorageOperation {
        Establish,
        BorrowBytes,
        Release,
        Split, // Divide one established claim into two children that cover it.
        Merge, // Merge the two children of one split back into that claim.
    }
}

pub mod cast_form {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, compiler_storage::Record)]
    pub enum CastForm {
        #[default]
        Value, // Value conversion (`x as T`).
        Recast(crate::ReferenceAccess), // Re-view the same backing with the stated reference access.
        /*
            One `=D`, `-D` or `+D` entry of a domain cast (domains.md, "Domain
            casts"). The parser writes one node per entry because each entry
            names one domain; typing accounts for the whole run at once and
            then leaves ordinary `Value` casts behind, so no later stage sees
            this form.
        */
        Entry(CastEntry),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum CastEntryKind {
        Keep,   // `=D`: the operand carries `D` and so does the result.
        Remove, // `-D`: the operand carries `D` and the result does not.
        Add,    // `+D`: the operand lacks `D` and the result carries it.
    }

    impl CastEntryKind {
        pub const fn sign(self) -> &'static str {
            match self {
                Self::Keep => "=",
                Self::Remove => "-",
                Self::Add => "+",
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct CastEntry {
        pub kind: CastEntryKind,
        pub first: bool, // The innermost entry: its operand is the value the whole cast accounts for.
        pub after_carrier: bool, // The run follows `as T`, which may only add.
    }

    impl CastForm {
        pub fn is_recast(self) -> bool {
            matches!(self, Self::Recast(_))
        }

        pub const fn recast_access(self) -> Option<crate::ReferenceAccess> {
            match self {
                Self::Value | Self::Entry(_) => None,
                Self::Recast(access) => Some(access),
            }
        }

        pub const fn entry(self) -> Option<CastEntry> {
            match self {
                Self::Entry(entry) => Some(entry),
                Self::Value | Self::Recast(_) => None,
            }
        }
    }
}

pub mod constant_value {
    use numerics::literals::FloatFormat;

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum EvaluatedConstant {
        Encoded(String), // An existing declaration encoding; index consumers still validate eligibility.
        UndeterminedNaN(FloatFormat), // Format and NaN meaning are known, but no observable payload bits are fixed.
    }

    impl EvaluatedConstant {
        pub fn encoding(&self) -> Option<&str> {
            match self {
                Self::Encoded(encoding) => Some(encoding),
                Self::UndeterminedNaN(_) => None,
            }
        }
    }

    impl Default for EvaluatedConstant {
        fn default() -> Self {
            Self::Encoded(String::new())
        }
    }

    impl From<String> for EvaluatedConstant {
        fn from(encoding: String) -> Self {
            Self::Encoded(encoding)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{EvaluatedConstant, FloatFormat};

        #[test]
        fn semantic_nan_never_supplies_an_encoding() {
            for format in [FloatFormat::F32, FloatFormat::F64] {
                assert_eq!(EvaluatedConstant::UndeterminedNaN(format).encoding(), None);
            }
            let encoded = EvaluatedConstant::from("boolean4:true".to_owned());
            assert_eq!(encoded.encoding(), Some("boolean4:true"));
        }
    }
}

pub mod data_layout_queries {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum DataLayoutQueryKind {
        Size,      // `size_of<T>()`: the fixed complete extent in bytes, including padding.
        Alignment, // `align_of<T>()`: the validated alignment in bytes.
    }

    impl DataLayoutQueryKind {
        pub fn from_target_name(name: &str) -> Option<Self> {
            match name {
                "size_of" => Some(Self::Size),
                "align_of" => Some(Self::Alignment),
                _ => None,
            }
        }

        pub fn target_name(self) -> &'static str {
            match self {
                Self::Size => "size_of",
                Self::Alignment => "align_of",
            }
        }
    }

    pub fn is_data_layout_query_spelling(
        target: &str,
        target_symbol_valid: bool,
        receiver_valid: bool,
    ) -> bool {
        !target_symbol_valid
            && !receiver_valid
            && DataLayoutQueryKind::from_target_name(target).is_some()
    }
}

pub mod descriptor_tables {
    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub enum DescriptorTableOperation {
        ActivateTask {
            entries: u16, // Checked table entry count before changing TR.
        },
        ReadTask, // Observe the selector currently retained by TR.
        ReloadSegments {
            entries: u16,
            kind: SegmentReload, // Kernel code or data descriptor from the current borrowed GDT.
        },
        ReadSegment {
            register: SegmentRegister, // Exact segment selector to observe.
        },
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub enum SegmentReload {
        Code,
        Data,
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub enum SegmentRegister {
        Code,
        Data,
        Extra,
        Stack,
        Fs,
        Gs,
    }

    impl SegmentReload {
        pub const fn code(self) -> u8 {
            match self {
                Self::Code => 0,
                Self::Data => 1,
            }
        }
        pub const fn from_code(code: u8) -> Option<Self> {
            match code {
                0 => Some(Self::Code),
                1 => Some(Self::Data),
                _ => None,
            }
        }
    }

    impl SegmentRegister {
        pub const fn code(self) -> u8 {
            match self {
                Self::Code => 0,
                Self::Data => 1,
                Self::Extra => 2,
                Self::Stack => 3,
                Self::Fs => 4,
                Self::Gs => 5,
            }
        }
        pub const fn from_code(code: u8) -> Option<Self> {
            match code {
                0 => Some(Self::Code),
                1 => Some(Self::Data),
                2 => Some(Self::Extra),
                3 => Some(Self::Stack),
                4 => Some(Self::Fs),
                5 => Some(Self::Gs),
                _ => None,
            }
        }
    }
}

pub mod layout_storage {
    pub const CONSTRUCTOR_NAME: &str = "LayoutStorage";
    pub const SOURCE_FILE_NAME: &str = "layout.omg";
    pub const SOURCE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../source/library/core/layout.omg"
    ));
}

pub mod operator_spelling {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compiler_storage::Record)]
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
}

pub mod static_interrupts {
    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub enum StaticInterruptOperation {
        Install,
        Context,
        BeginCompletion,
        FinishCompletion,
    }

    pub const CORE_DECLARATIONS: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../source/library/core/static_interrupt.omg"
    ));

    impl StaticInterruptOperation {
        pub const ALL: [Self; 4] = [
            Self::Install,
            Self::Context,
            Self::BeginCompletion,
            Self::FinishCompletion,
        ];

        pub const fn required_service_identity(self) -> Option<&'static str> {
            match self {
                Self::Install => Some("toolchain::MachineControl"),
                Self::Context | Self::BeginCompletion | Self::FinishCompletion => None,
            }
        }

        pub const fn code(self) -> u8 {
            match self {
                Self::Install => 0,
                Self::Context => 1,
                Self::BeginCompletion => 2,
                Self::FinishCompletion => 3,
            }
        }
    }
}
